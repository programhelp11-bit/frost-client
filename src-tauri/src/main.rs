#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::Engine;
use std::collections::HashMap;
use frost_client::auth::{self, Account, AccountStore};
use frost_client::config::Settings;
use frost_client::cosmetics::{self, FrostApi, SkinVariant};
use frost_client::downloader::Progress;
use frost_client::ipc::{self, GameRequest, IpcHandle, LibEntry, ToGame};
use frost_client::library;
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
    /// Signed-in Frost server session (re-created when it expires or the account changes).
    api_cache: Mutex<Option<(FrostApi, String, std::time::Instant)>>,
    /// Cape the player currently wears (`frost`, `none` or a community id).
    active_cape: Mutex<String>,
    uploaded_capes: Mutex<HashMap<String, String>>,
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
    // First launch of a version: set up the base Frost modpack (once; removing a mod later is respected).
    if settings.auto_pack && !modrinth::instance_dir(&version).join(".frost-pack").exists() {
        status(app, "preparing", "Installing the Frost modpack...");
        let _ = pack_apply(&http, &version, true).await;
    }
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
    cosmetics::upload_skin(&st.http, &acc, decode_png(&png_b64)?, variant).await.map_err(err)?;
    // let other Frost players know (best effort: the server may not be configured)
    if let Ok(api) = api(&st).await {
        let _ = api.bump_skin().await;
    }
    Ok(())
}

fn frost_base(st: &AppState) -> Result<String, String> {
    let base = st.settings.lock().unwrap().frost_api.trim().to_owned();
    if base.is_empty() {
        Err("This build has no Frost server configured, so other players cannot see your cape.".into())
    } else {
        Ok(base)
    }
}

/// Signed-in Frost API handle; the session is cached for a few hours.
async fn api(st: &AppState) -> Result<FrostApi, String> {
    let base = frost_base(st)?;
    let acc = fresh_account(st).await?;
    if let Some((api, uuid, at)) = st.api_cache.lock().unwrap().as_ref() {
        if *uuid == acc.uuid && at.elapsed() < Duration::from_secs(5 * 3600) {
            return Ok(api.clone());
        }
    }
    let mut api = FrostApi::new(st.http.clone(), &base);
    api.login(&acc).await.map_err(err)?;
    *st.api_cache.lock().unwrap() = Some((api.clone(), acc.uuid.clone(), std::time::Instant::now()));
    Ok(api)
}

fn forget_api(st: &AppState) {
    *st.api_cache.lock().unwrap() = None;
}

/// Tell the game what the radial menus should show.
fn push_library(st: &AppState) {
    let Some(ipc) = &st.ipc else { return };
    let items = library::list();
    let entry = |id: &str, name: &str, color: &str| LibEntry { id: id.to_owned(), name: name.to_owned(), color: color.to_owned() };
    let skins: Vec<LibEntry> = items
        .iter()
        .filter(|i| i.kind == "skin")
        .map(|i| entry(&i.id, &i.name, &i.color))
        .collect();
    let mut capes = vec![entry("frost", "Frost", "5AAFFF")];
    capes.extend(items.iter().filter(|i| i.kind == "cape").map(|i| entry(&i.id, &i.name, &i.color)));
    capes.push(entry("none", "No cape", "6B7A99"));
    let active_cape = st.active_cape.lock().unwrap().clone();
    ipc.send(ToGame::Library { skins, capes, active_cape });
    push_own_cape(st);
}

/// Send the PNG of the worn custom cape so the game can show it without any server.
fn push_own_cape(st: &AppState) {
    let Some(ipc) = &st.ipc else { return };
    let id = st.active_cape.lock().unwrap().clone();
    let png_b64 = if id == "frost" || id == "none" {
        None
    } else {
        library::load_png(&id).ok().map(|b| base64::engine::general_purpose::STANDARD.encode(b))
    };
    ipc.send(ToGame::OwnCape { png_b64 });
}

/// Wear a cape: Frost cape, no cape, or one of your own. Works fully offline; when a Frost
/// server is configured it is also shared so other Frost players can see it.
async fn set_cape(st: &AppState, id: &str) -> String {
    *st.active_cape.lock().unwrap() = id.to_owned();
    push_library(st);
    if let Ok(api) = api(st).await {
        let server_id = if id == "frost" || id == "none" {
            Some(id.to_owned())
        } else {
            let known = st.uploaded_capes.lock().unwrap().get(id).cloned();
            match known {
                Some(sid) => Some(sid),
                None => {
                    let item = library::get(id);
                    let png = library::load_png(id).ok();
                    match (item, png) {
                        (Some(item), Some(png)) => {
                            match api.upload_cape(item.name.trim(), png).await {
                                Ok(sid) => {
                                    st.uploaded_capes.lock().unwrap().insert(id.to_owned(), sid.clone());
                                    Some(sid)
                                }
                                Err(_) => None,
                            }
                        }
                        _ => None,
                    }
                }
            }
        };
        if let Some(sid) = server_id {
            if api.select_cape(&sid).await.is_err() {
                forget_api(st);
            }
        }
    }
    match id {
        "none" => "Cape removed".to_owned(),
        _ => "Cape changed".to_owned(),
    }
}

fn notice(st: &AppState, text: impl Into<String>) {
    if let Some(ipc) = &st.ipc {
        ipc.send(ToGame::Notice { text: text.into() });
    }
}

#[tauri::command]
fn lib_list() -> Value {
    serde_json::to_value(library::list()).unwrap_or(Value::Null)
}

#[tauri::command]
fn lib_save(st: State<'_, AppState>, kind: String, name: String, png_b64: String, slim: bool) -> Result<Value, String> {
    let item = library::save(&kind, &name, &decode_png(&png_b64)?, slim).map_err(err)?;
    push_library(&st);
    serde_json::to_value(item).map_err(err)
}

#[tauri::command]
fn lib_png(id: String) -> Result<String, String> {
    let bytes = library::load_png(&id).map_err(err)?;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

#[tauri::command]
fn lib_delete(st: State<'_, AppState>, id: String) -> Result<(), String> {
    library::remove(&id).map_err(err)?;
    push_library(&st);
    Ok(())
}

/// The skin currently on the signed-in account, to start editing from.
#[tauri::command]
async fn skin_current(st: State<'_, AppState>) -> Result<Value, String> {
    let acc = fresh_account(&st).await?;
    let (png, slim) = cosmetics::fetch_current_skin(&st.http, &acc.uuid).await.map_err(err)?;
    Ok(json!({ "png_b64": base64::engine::general_purpose::STANDARD.encode(png), "slim": slim }))
}

#[tauri::command]
async fn community_list(st: State<'_, AppState>, offset: u32, sort: String) -> Result<Value, String> {
    let base = frost_base(&st)?;
    let api = FrostApi::new(st.http.clone(), &base);
    let mut v = api.list_capes(offset, &sort).await.map_err(err)?;
    v["active"] = Value::String(st.active_cape.lock().unwrap().clone());
    v["favorites"] = json!(library::list()
        .iter()
        .filter(|i| i.kind == "fav")
        .filter_map(|i| i.ref_id.clone())
        .collect::<Vec<_>>());
    Ok(v)
}

#[tauri::command]
async fn community_upload(st: State<'_, AppState>, name: String, png_b64: String) -> Result<String, String> {
    let png = decode_png(&png_b64)?;
    cosmetics::validate_cape_png(&png).map_err(err)?;
    let api = api(&st).await?;
    let r = api.upload_cape(name.trim(), png).await;
    if r.is_err() {
        forget_api(&st);
    }
    r.map_err(err)
}

#[tauri::command]
async fn cape_select(st: State<'_, AppState>, id: String) -> Result<(), String> {
    set_cape(&st, &id).await;
    Ok(())
}

#[tauri::command]
async fn cape_favorite(st: State<'_, AppState>, id: String, name: String, url: String, on: bool) -> Result<(), String> {
    if on {
        let base = frost_base(&st)?;
        let png = FrostApi::new(st.http.clone(), &base).download(&url).await.map_err(err)?;
        library::add_favorite(&id, &name, &png).map_err(err)?;
    } else if let Some(item) = library::list().into_iter().find(|i| i.kind == "fav" && i.ref_id.as_deref() == Some(id.as_str())) {
        library::remove(&item.id).map_err(err)?;
    }
    push_library(&st);
    Ok(())
}

#[tauri::command]
async fn cape_delete(st: State<'_, AppState>, id: String) -> Result<(), String> {
    api(&st).await?.delete_cape(&id).await.map_err(err)
}

async fn apply_skin_item(st: &AppState, id: &str) -> Result<String, String> {
    let item = library::get(id).ok_or("That skin is no longer in your library")?;
    let png = library::load_png(id).map_err(err)?;
    let acc = fresh_account(st).await?;
    let variant = if item.slim { SkinVariant::Slim } else { SkinVariant::Classic };
    cosmetics::upload_skin(&st.http, &acc, png, variant).await.map_err(err)?;
    if let Ok(api) = api(st).await {
        let _ = api.bump_skin().await;
    }
    Ok(item.name)
}

async fn handle_game_request(handle: &AppHandle, req: GameRequest) {
    let st = handle.state::<AppState>();
    let msg = match req {
        GameRequest::Cape(id) => set_cape(&st, &id).await,
        GameRequest::Skin(id) => match apply_skin_item(&st, &id).await {
            Ok(name) => format!("Skin applied: {name}"),
            Err(e) => format!("Skin change failed: {e}"),
        },
    };
    notice(&st, msg.clone());
    let _ = handle.emit("game-notice", msg);
}

/// While the game runs: handle radial-menu picks, heartbeat, and keep the game's view of other
/// Frost players (tab snowflake, capes, skin revisions) fresh.
fn spawn_cosmetics_sync(handle: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let old = Duration::from_secs(3600);
        let mut last_hb = std::time::Instant::now() - old;
        let mut last_lookup = std::time::Instant::now() - old;
        let mut last_tab: Vec<String> = Vec::new();
        let mut was_connected = false;
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            let st = handle.state::<AppState>();
            let Some(ipc) = st.ipc.as_ref() else { continue };
            if !ipc.connected() {
                was_connected = false;
                continue;
            }
            for req in ipc.take_requests() {
                handle_game_request(&handle, req).await;
                last_lookup = std::time::Instant::now() - old;
            }
            if !was_connected {
                was_connected = true;
                push_library(&st);
                last_hb = std::time::Instant::now() - old;
                last_lookup = std::time::Instant::now() - old;
            }
            let Ok(api) = api(&st).await else { continue };
            if last_hb.elapsed() > Duration::from_secs(60) {
                if api.heartbeat().await.is_err() {
                    forget_api(&st);
                }
                last_hb = std::time::Instant::now();
            }
            let mut tab = ipc.tab_players();
            if let Some(me) = st.accounts.lock().unwrap().active().map(|a| a.uuid.replace('-', "")) {
                if !tab.contains(&me) {
                    tab.push(me);
                }
            }
            if tab != last_tab || last_lookup.elapsed() > Duration::from_secs(20) {
                if let Ok(players) = api.lookup(&tab).await {
                    ipc.send(ToGame::Cosmetics { players });
                }
                last_tab = tab;
                last_lookup = std::time::Instant::now();
            }
        }
    });
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
fn spawn_bot(
    st: State<'_, AppState>,
    style: CombatStyle,
    difficulty: f32,
    arena: Option<String>,
    kit: Option<bool>,
) -> Result<bool, String> {
    let legacy = st.settings.lock().unwrap().selected_version == "1.8.9";
    let arena = arena.unwrap_or_else(|| "none".to_owned());
    Ok(ipc_of(&st)?.spawn_bot_in("Frost Bot", style, difficulty.clamp(0.1, 1.0), legacy, &arena, kit.unwrap_or(false)))
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

const PERF_GROUPS: [(&str, &[&str]); 2] = [
    ("renderer", &["sodium", "immediatelyfast", "entityculling"]),
    ("logic", &["lithium", "ferrite-core", "modernfix"]),
];

fn perf_slugs(group: &str) -> Option<&'static [&'static str]> {
    PERF_GROUPS.iter().find(|(g, _)| *g == group).map(|(_, s)| *s)
}

/// Which performance mod groups are installed for a version.
#[tauri::command]
fn perf_status(mc: String) -> Value {
    let installed = modrinth::list_installed(&mc);
    let has = |slugs: &[&str]| slugs.iter().all(|s| installed.iter().any(|i| i.slug == *s));
    let any = |slugs: &[&str]| slugs.iter().any(|s| installed.iter().any(|i| i.slug == *s));
    json!({
        "fabric": modrinth::loader_name(&mc) == "fabric",
        "renderer": has(perf_slugs("renderer").unwrap()),
        "renderer_any": any(perf_slugs("renderer").unwrap()),
        "logic": has(perf_slugs("logic").unwrap()),
        "logic_any": any(perf_slugs("logic").unwrap()),
    })
}

/// Install or remove one group of performance mods (Fabric versions only). Failures are reported
/// per mod, so a mod without a build for this Minecraft version does not abort the rest.
#[tauri::command]
async fn perf_group(st: State<'_, AppState>, mc: String, group: String, on: bool) -> Result<Value, String> {
    if !versions::SUPPORTED.contains(&mc.as_str()) {
        return Err(format!("Unsupported version {mc}"));
    }
    if modrinth::loader_name(&mc) != "fabric" {
        return Err("These are Fabric mods. Minecraft 1.8.9 uses Frost's built-in optimizations instead.".into());
    }
    let slugs = perf_slugs(&group).ok_or("unknown group")?;
    let mut results = Vec::new();
    if on {
        for slug in slugs {
            let r = modrinth::install(&st.http, slug, &mc, Arc::new(Progress::default())).await;
            results.push(json!({ "mod": slug, "ok": r.is_ok(), "error": r.err().map(|e| e.to_string()) }));
        }
    } else {
        for item in modrinth::list_installed(&mc) {
            if slugs.contains(&item.slug.as_str()) {
                let r = modrinth::remove(&mc, &item.project_id);
                results.push(json!({ "mod": item.slug, "ok": r.is_ok(), "error": r.err().map(|e| e.to_string()) }));
            }
        }
    }
    Ok(Value::Array(results))
}

/// The base Frost modpack for one version: the agent (always bundled) plus every performance mod.
/// Returns what was installed or removed. Forge 1.8.9 has no Fabric mods; the agent carries its optimizations.
async fn pack_apply(http: &reqwest::Client, mc: &str, on: bool) -> Vec<Value> {
    let mut results = Vec::new();
    if modrinth::loader_name(mc) != "fabric" {
        return results;
    }
    for (_, slugs) in PERF_GROUPS.iter() {
        for slug in slugs.iter() {
            if on {
                let r = modrinth::install(http, slug, mc, Arc::new(Progress::default())).await;
                results.push(json!({ "version": mc, "mod": slug, "ok": r.is_ok(), "error": r.err().map(|e| e.to_string()) }));
            } else if let Some(item) = modrinth::list_installed(mc).into_iter().find(|i| i.slug == *slug) {
                let r = modrinth::remove(mc, &item.project_id);
                results.push(json!({ "version": mc, "mod": slug, "ok": r.is_ok(), "error": r.err().map(|e| e.to_string()) }));
            }
        }
    }
    let _ = std::fs::create_dir_all(modrinth::instance_dir(mc));
    let _ = std::fs::write(modrinth::instance_dir(mc).join(".frost-pack"), if on { "1" } else { "0" });
    results
}

/// Install (or remove) the Frost modpack on one version, or on every supported version.
#[tauri::command]
async fn pack_install(st: State<'_, AppState>, mc: String, all: bool, on: bool) -> Result<Value, String> {
    let mut out = Vec::new();
    if all {
        for v in versions::SUPPORTED {
            out.extend(pack_apply(&st.http, v, on).await);
        }
    } else {
        if !versions::SUPPORTED.contains(&mc.as_str()) {
            return Err(format!("Unsupported version {mc}"));
        }
        out.extend(pack_apply(&st.http, &mc, on).await);
    }
    Ok(Value::Array(out))
}

/// Per version: is the whole modpack in place?
#[tauri::command]
fn pack_status() -> Value {
    let rows: Vec<Value> = versions::SUPPORTED
        .iter()
        .map(|v| {
            let fabric = modrinth::loader_name(v) == "fabric";
            let installed = modrinth::list_installed(v);
            let total: usize = PERF_GROUPS.iter().map(|(_, s)| s.len()).sum();
            let have = PERF_GROUPS.iter().flat_map(|(_, s)| s.iter()).filter(|s| installed.iter().any(|i| i.slug == **s)).count();
            json!({ "version": v, "fabric": fabric, "have": have, "total": total })
        })
        .collect();
    Value::Array(rows)
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
                api_cache: Mutex::new(None),
                active_cape: Mutex::new("frost".to_owned()),
                uploaded_capes: Mutex::new(HashMap::new()),
            });
            spawn_cosmetics_sync(app.handle().clone());

            // Push HUD/QoL settings whenever the game connects and tell the UI about it.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut was = false;
                loop {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    let st = handle.state::<AppState>();
                    if let Some((qol, hud)) = st.ipc.as_ref().and_then(|i| i.take_settings()) {
                        {
                            let mut cur = st.settings.lock().unwrap();
                            cur.qol = qol;
                            cur.hud = hud;
                            let _ = cur.save();
                        }
                        let _ = handle.emit("settings-changed", ());
                    }
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
            lib_list,
            lib_save,
            lib_png,
            lib_delete,
            skin_current,
            community_list,
            community_upload,
            cape_select,
            cape_favorite,
            cape_delete,
            ipc_status,
            spawn_bot,
            clear_bots,
            kb_trainer,
            bedwars_bridge,
            mr_search,
            mr_install,
            perf_status,
            perf_group,
            pack_install,
            pack_status,
            mr_installed,
            mr_remove,
            open_instance_folder,
            open_data_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Frost Client");
}
