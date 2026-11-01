#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::Engine;
use frost_client::auth::{self, Account, AccountStore};
use frost_client::config::Settings;
use frost_client::cosmetics::{self, FrostApi, SkinVariant};
use frost_client::downloader::Progress;
use frost_client::ipc::{self, IpcHandle, ToGame};
use frost_client::modloader;
use frost_client::modrinth::{self, SearchQuery};
use frost_client::trainer::{CombatStyle, KbProfile};
use frost_client::{launch, versions};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};

const NO_CLIENT_ID: &str = "NO_CLIENT_ID";

/// Error string for the UI: `NO_CLIENT_ID`, or `BAD_CLIENT_ID:<value>` when one is set but malformed.
fn client_id_error(st: &AppState) -> String {
    let over = st.settings.lock().unwrap().ms_client_id.clone();
    match auth::invalid_client_id(&over) {
        Some(bad) => format!("BAD_CLIENT_ID:{bad}"),
        None => NO_CLIENT_ID.to_owned(),
    }
}

struct AppState {
    http: reqwest::Client,
    settings: Mutex<Settings>,
    accounts: Mutex<AccountStore>,
    ipc: Option<IpcHandle>,
    running: AtomicBool,
}

fn err<E: std::fmt::Display>(e: E) -> String {
    format!("{e:#}")
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn client_id(st: &AppState) -> Option<String> {
    let over = st.settings.lock().unwrap().ms_client_id.clone();
    auth::effective_client_id(&over)
}

/// Active account with a valid Minecraft token (refreshing it when needed).
async fn fresh_account(st: &AppState) -> Result<Account, String> {
    let acc = st.accounts.lock().unwrap().active().cloned().ok_or("Sign in first")?;
    if !acc.is_expired() {
        return Ok(acc);
    }
    let cid = client_id(st).ok_or_else(|| client_id_error(st))?;
    let new = auth::refresh(&st.http, &cid, &acc).await.map_err(err)?;
    let mut store = st.accounts.lock().unwrap();
    store.upsert(new.clone());
    let _ = store.save();
    Ok(new)
}

fn push_settings(st: &AppState) {
    if let Some(ipc) = &st.ipc {
        let s = st.settings.lock().unwrap();
        ipc.send(ToGame::Settings { qol: s.qol.clone(), hud: s.hud.clone() });
    }
}

fn ipc_of(st: &AppState) -> Result<&IpcHandle, String> {
    st.ipc.as_ref().ok_or_else(|| "The in-game bridge is not available".to_owned())
}

fn account_json(acc: &Account) -> Value {
    json!({ "username": acc.username, "uuid": acc.uuid })
}

fn loader_label(mc: &str) -> &'static str {
    match modloader::loader_for(mc) {
        modloader::Loader::Fabric => "Fabric",
        modloader::Loader::Forge => "Forge",
    }
}

// ------------------------------------------------------------------------------------------------
// window controls (custom title bar)
// ------------------------------------------------------------------------------------------------

#[tauri::command]
fn win_minimize(w: WebviewWindow) {
    let _ = w.minimize();
}

#[tauri::command]
fn win_toggle_max(w: WebviewWindow) {
    if w.is_maximized().unwrap_or(false) {
        let _ = w.unmaximize();
    } else {
        let _ = w.maximize();
    }
}

#[tauri::command]
fn win_close(w: WebviewWindow) {
    let _ = w.close();
}

#[tauri::command]
fn win_drag(w: WebviewWindow) {
    let _ = w.start_dragging();
}

// ------------------------------------------------------------------------------------------------
// state / account
// ------------------------------------------------------------------------------------------------

#[tauri::command]
fn get_state(st: State<'_, AppState>) -> Value {
    let settings = st.settings.lock().unwrap().clone();
    let account = st.accounts.lock().unwrap().active().map(account_json);
    let instances: Vec<Value> = versions::SUPPORTED
        .iter()
        .map(|v| {
            json!({
                "id": v,
                "loader": loader_label(v),
                "has_mod": modloader::find_frost_mod(v).is_some(),
                "content": modrinth::list_installed(v).len(),
                "last_played": settings.last_played.get(*v),
            })
        })
        .collect();
    json!({
        "app_version": env!("CARGO_PKG_VERSION"),
        "settings": settings,
        "account": account,
        "client_id_configured": client_id(&st).is_some(),
        "client_id_invalid": auth::invalid_client_id(&st.settings.lock().unwrap().ms_client_id),
        "game_running": st.running.load(Ordering::SeqCst),
        "ipc_connected": st.ipc.as_ref().map(|i| i.connected()).unwrap_or(false),
        "instances": instances,
        "data_dir": frost_client::config::data_dir(),
        "os": format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
    })
}

#[tauri::command]
async fn login(st: State<'_, AppState>) -> Result<Value, String> {
    let cid = client_id(&st).ok_or_else(|| client_id_error(&st))?;
    let tokens = auth::browser_login(&st.http, &cid, |url| {
        let _ = open::that(url);
    })
    .await
    .map_err(err)?;
    let acc = auth::login_with_ms(&st.http, &tokens).await.map_err(err)?;
    let out = account_json(&acc);
    let mut store = st.accounts.lock().unwrap();
    store.upsert(acc);
    store.save().map_err(err)?;
    Ok(out)
}

#[tauri::command]
fn logout(st: State<'_, AppState>) -> Result<(), String> {
    let mut store = st.accounts.lock().unwrap();
    store.remove_active();
    store.save().map_err(err)
}

#[tauri::command]
fn save_settings(st: State<'_, AppState>, settings: Settings) -> Result<(), String> {
    {
        let mut cur = st.settings.lock().unwrap();
        // last_played is owned by the backend
        let last = std::mem::take(&mut cur.last_played);
        *cur = settings;
        cur.last_played = last;
        cur.save().map_err(err)?;
    }
    push_settings(&st);
    Ok(())
}

// ------------------------------------------------------------------------------------------------
// launching
// ------------------------------------------------------------------------------------------------

fn status(app: &AppHandle, phase: &str, message: &str) {
    let _ = app.emit("launch-status", json!({ "phase": phase, "message": message }));
}

#[tauri::command]
async fn launch_game(app: AppHandle, st: State<'_, AppState>, version: String) -> Result<(), String> {
    if !versions::SUPPORTED.contains(&version.as_str()) {
        return Err(format!("Unsupported version {version}"));
    }
    if st.running.swap(true, Ordering::SeqCst) {
        return Err("A game is already running".into());
    }
    let account = match fresh_account(&st).await {
        Ok(a) => a,
        Err(e) => {
            st.running.store(false, Ordering::SeqCst);
            return Err(e);
        }
    };
    let settings = {
        let mut s = st.settings.lock().unwrap();
        s.selected_version = version.clone();
        s.last_played.insert(version.clone(), now());
        let _ = s.save();
        s.clone()
    };
    let http = st.http.clone();
    tauri::async_runtime::spawn(async move {
        run_game(&app, http, settings, account, version).await;
        app.state::<AppState>().running.store(false, Ordering::SeqCst);
    });
    Ok(())
}

async fn run_game(app: &AppHandle, http: reqwest::Client, settings: Settings, account: Account, version: String) {
    status(app, "preparing", "Downloading game files...");
    let progress = Arc::new(Progress::default());
    let (stop_tx, mut stop_rx) = tokio::sync::oneshot::channel::<()>();
    let ticker = {
        let app = app.clone();
        let progress = progress.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut stop_rx => break,
                    _ = tokio::time::sleep(Duration::from_millis(250)) => {
                        let _ = app.emit("launch-progress", json!({ "fraction": progress.fraction() }));
                    }
                }
            }
        })
    };

    let plan = launch::prepare(&http, &settings, &account, &version, progress).await;
    let _ = stop_tx.send(());
    let _ = ticker.await;

    let plan = match plan {
        Ok(p) => p,
        Err(e) => {
            status(app, "error", &err(e));
            return;
        }
    };
    for w in &plan.warnings {
        let _ = app.emit("launch-warning", w);
    }
    status(app, "starting", "Starting Minecraft...");
    let mut child = match launch::spawn(&plan) {
        Ok(c) => c,
        Err(e) => {
            status(app, "error", &err(e));
            return;
        }
    };
    status(app, "running", "Minecraft is running");
    let hide = settings.while_playing == "hide";
    if hide {
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.hide();
        }
    }
    let result = child.wait().await;
    if hide {
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.show();
        }
    }
    match result {
        Ok(code) if code.success() => status(app, "exited", "Game closed"),
        Ok(code) => status(
            app,
            "error",
            &format!("Game exited with {code}. See {}/frost-latest.log", plan.game_dir.display()),
        ),
        Err(e) => status(app, "error", &err(e)),
    }
}

// ------------------------------------------------------------------------------------------------
// cosmetics
// ------------------------------------------------------------------------------------------------

fn decode_png(b64: &str) -> Result<Vec<u8>, String> {
    base64::engine::general_purpose::STANDARD.decode(b64.trim()).map_err(err)
}

#[tauri::command]
async fn upload_skin(st: State<'_, AppState>, png_b64: String, slim: bool) -> Result<(), String> {
    let acc = fresh_account(&st).await?;
    let variant = if slim { SkinVariant::Slim } else { SkinVariant::Classic };
    cosmetics::upload_skin(&st.http, &acc, decode_png(&png_b64)?, variant).await.map_err(err)
}

async fn frost_api(st: &AppState) -> Result<FrostApi, String> {
    let acc = fresh_account(st).await?;
    let base = st.settings.lock().unwrap().frost_api.clone();
    let mut api = FrostApi::new(st.http.clone(), &base);
    api.login(&acc).await.map_err(err)?;
    Ok(api)
}

#[tauri::command]
async fn upload_cape(st: State<'_, AppState>, png_b64: String) -> Result<(), String> {
    let png = decode_png(&png_b64)?;
    cosmetics::validate_cape_png(&png).map_err(err)?;
    frost_api(&st).await?.upload_cape(png).await.map_err(err)
}

#[tauri::command]
async fn remove_cape(st: State<'_, AppState>) -> Result<(), String> {
    frost_api(&st).await?.delete_cape().await.map_err(err)
}

// ------------------------------------------------------------------------------------------------
// in-game bridge: trainer bots, knockback, bedwars
// ------------------------------------------------------------------------------------------------

#[tauri::command]
fn ipc_status(st: State<'_, AppState>) -> Value {
    match &st.ipc {
        Some(i) => json!({ "connected": i.connected(), "stats": i.stats() }),
        None => json!({ "connected": false }),
    }
}

#[tauri::command]
fn spawn_bot(st: State<'_, AppState>, style: CombatStyle, difficulty: f32) -> Result<bool, String> {
    let legacy = st.settings.lock().unwrap().selected_version == "1.8.9";
    Ok(ipc_of(&st)?.spawn_bot("Frost Bot", style, difficulty.clamp(0.1, 1.0), legacy))
}

#[tauri::command]
fn clear_bots(st: State<'_, AppState>) -> Result<bool, String> {
    Ok(ipc_of(&st)?.clear_bots())
}

#[tauri::command]
fn kb_trainer(st: State<'_, AppState>, enabled: bool) -> Result<bool, String> {
    let legacy = st.settings.lock().unwrap().selected_version == "1.8.9";
    let profile = if legacy { KbProfile::legacy_1_8() } else { KbProfile::modern() };
    Ok(ipc_of(&st)?.send(ToGame::KnockbackTrainer { enabled, profile }))
}

#[tauri::command]
fn bedwars_bridge(st: State<'_, AppState>, enabled: bool) -> Result<bool, String> {
    Ok(ipc_of(&st)?.send(ToGame::Bedwars { bridge_trainer: enabled }))
}

// ------------------------------------------------------------------------------------------------
// Modrinth
// ------------------------------------------------------------------------------------------------

#[tauri::command]
async fn mr_search(st: State<'_, AppState>, query: SearchQuery) -> Result<Value, String> {
    let page = modrinth::search(&st.http, &query).await.map_err(err)?;
    serde_json::to_value(page).map_err(err)
}

#[tauri::command]
async fn mr_install(st: State<'_, AppState>, project_id: String, mc: String) -> Result<Value, String> {
    if !versions::SUPPORTED.contains(&mc.as_str()) {
        return Err(format!("Unsupported version {mc}"));
    }
    let added = modrinth::install(&st.http, &project_id, &mc, Arc::new(Progress::default()))
        .await
        .map_err(err)?;
    serde_json::to_value(added).map_err(err)
}

#[tauri::command]
fn mr_installed(mc: String) -> Value {
    serde_json::to_value(modrinth::list_installed(&mc)).unwrap_or(Value::Null)
}

#[tauri::command]
fn mr_remove(mc: String, project_id: String) -> Result<(), String> {
    modrinth::remove(&mc, &project_id).map_err(err)
}

#[tauri::command]
fn open_instance_folder(mc: String) -> Result<(), String> {
    let dir = modrinth::instance_dir(&mc);
    std::fs::create_dir_all(&dir).map_err(err)?;
    open::that(dir).map_err(err)
}

#[tauri::command]
fn open_data_folder() -> Result<(), String> {
    let dir = frost_client::config::data_dir();
    std::fs::create_dir_all(&dir).map_err(err)?;
    open::that(dir).map_err(err)
}

// ------------------------------------------------------------------------------------------------

fn main() {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let http = reqwest::Client::builder()
        .user_agent(concat!("FrostClient/", env!("CARGO_PKG_VERSION"), " (launcher)"))
        .connect_timeout(Duration::from_secs(15))
        .build()
        .expect("http client");
    let settings = Settings::load();
    let port = settings.ipc_port;

    tauri::Builder::default()
        .setup(move |app| {
            if let Ok(dir) = app.path().resource_dir() {
                modloader::set_resource_dir(dir);
            }
            let ipc = match tauri::async_runtime::block_on(ipc::serve(port)) {
                Ok(h) => Some(h),
                Err(e) => {
                    tracing::warn!("in-game bridge unavailable on port {port}: {e:#}");
                    None
                }
            };
            app.manage(AppState {
                http,
                settings: Mutex::new(settings),
                accounts: Mutex::new(AccountStore::load()),
                ipc,
                running: AtomicBool::new(false),
            });

            // Push HUD/QoL settings whenever the game connects and tell the UI about it.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut was = false;
                loop {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    let st = handle.state::<AppState>();
                    let now = st.ipc.as_ref().map(|i| i.connected()).unwrap_or(false);
                    if now && !was {
                        push_settings(&st);
                    }
                    if now != was {
                        let _ = handle.emit("ipc-connected", now);
                    }
                    was = now;
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            win_minimize,
            win_toggle_max,
            win_close,
            win_drag,
            get_state,
            login,
            logout,
            save_settings,
            launch_game,
            upload_skin,
            upload_cape,
            remove_cape,
            ipc_status,
            spawn_bot,
            clear_bots,
            kb_trainer,
            bedwars_bridge,
            mr_search,
            mr_install,
            mr_installed,
            mr_remove,
            open_instance_folder,
            open_data_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Frost Client");
}
