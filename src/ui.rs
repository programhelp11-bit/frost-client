//! The launcher window (egui / eframe), liquid-glass style.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;

use eframe::egui::{self, Align, Color32, Layout, RichText, Rounding, Sense, Stroke, Vec2};
use tokio::runtime::Handle;

use crate::auth::{self, Account, AccountStore, DeviceCode};
use crate::config::Settings;
use crate::cosmetics::{self, FrostApi, SkinVariant};
use crate::downloader::Progress;
use crate::glass::{self, glass_panel};
use crate::hud_manager::{self, GameStats, HudKind};
use crate::ipc::{self, IpcHandle, ToGame};
use crate::launch;
use crate::trainer::{CombatStyle, KbProfile};
use crate::versions;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Play,
    Skins,
    Hud,
    Trainer,
    Settings,
}

impl Tab {
    const ALL: [Tab; 5] = [Tab::Play, Tab::Skins, Tab::Hud, Tab::Trainer, Tab::Settings];
    fn label(self) -> &'static str {
        match self {
            Tab::Play => "Play",
            Tab::Skins => "Skins & Capes",
            Tab::Hud => "HUD & QoL",
            Tab::Trainer => "Trainer",
            Tab::Settings => "Settings",
        }
    }
}

#[derive(Clone, Copy)]
enum PickTarget {
    Skin,
    Cape,
}

enum UiEvent {
    DeviceCode(DeviceCode),
    LoggedIn(Account),
    AccountRefreshed(Account),
    Status(String),
    Error(String),
    GameExited,
    FilePicked(PickTarget, Vec<u8>),
    IpcReady(IpcHandle),
    Busy(bool),
}

pub struct FrostApp {
    rt: Handle,
    http: reqwest::Client,
    ctx: egui::Context,
    tx: mpsc::Sender<UiEvent>,
    rx: mpsc::Receiver<UiEvent>,

    settings: Settings,
    store: AccountStore,
    tab: Tab,
    status: String,
    error: Option<String>,
    busy: bool,
    game_running: bool,
    device_code: Option<DeviceCode>,
    progress: Arc<Progress>,
    ipc: Option<IpcHandle>,

    skin_bytes: Option<Vec<u8>>,
    skin_tex: Option<egui::TextureHandle>,
    skin_variant: SkinVariant,
    cape_bytes: Option<Vec<u8>>,
    cape_tex: Option<egui::TextureHandle>,

    hud_edit: bool,
    demo_stats: GameStats,

    bot_style: CombatStyle,
    bot_difficulty: f32,
    bot_name: String,
    kb_enabled: bool,
}

impl FrostApp {
    pub fn new(cc: &eframe::CreationContext<'_>, rt: Handle) -> Self {
        glass::apply_theme(&cc.egui_ctx);
        glass::enable_backdrop_blur(cc);

        let (tx, rx) = mpsc::channel();
        let settings = Settings::load();
        let http = reqwest::Client::builder()
            .user_agent(concat!("FrostClient/", env!("CARGO_PKG_VERSION")))
            .pool_max_idle_per_host(2)
            .build()
            .expect("http client");

        // start the loopback bridge for the in-game agent
        {
            let tx = tx.clone();
            let ctx = cc.egui_ctx.clone();
            let port = settings.ipc_port;
            rt.spawn(async move {
                match ipc::serve(port).await {
                    Ok(h) => {
                        let _ = tx.send(UiEvent::IpcReady(h));
                    }
                    Err(e) => {
                        let _ = tx.send(UiEvent::Error(format!("IPC bridge could not start: {e}")));
                    }
                }
                ctx.request_repaint();
            });
        }

        Self {
            rt,
            http,
            ctx: cc.egui_ctx.clone(),
            tx,
            rx,
            settings,
            store: AccountStore::load(),
            tab: Tab::Play,
            status: "Ready".into(),
            error: None,
            busy: false,
            game_running: false,
            device_code: None,
            progress: Arc::new(Progress::default()),
            ipc: None,
            skin_bytes: None,
            skin_tex: None,
            skin_variant: SkinVariant::Classic,
            cape_bytes: None,
            cape_tex: None,
            hud_edit: false,
            demo_stats: GameStats::demo(),
            bot_style: CombatStyle::Sword,
            bot_difficulty: 0.6,
            bot_name: "FrostBot".into(),
            kb_enabled: false,
        }
    }

    // ---------------------------------------------------------------------------------------------
    // events from background tasks
    // ---------------------------------------------------------------------------------------------

    fn poll_events(&mut self) {
        while let Ok(ev) = self.rx.try_recv() {
            match ev {
                UiEvent::DeviceCode(c) => {
                    let _ = open::that(&c.verification_uri);
                    self.ctx.copy_text(c.user_code.clone());
                    self.device_code = Some(c);
                }
                UiEvent::LoggedIn(acc) => {
                    self.device_code = None;
                    self.status = format!("Signed in as {}", acc.username);
                    self.store.upsert(acc);
                    let _ = self.store.save();
                    self.busy = false;
                }
                UiEvent::AccountRefreshed(acc) => {
                    self.store.upsert(acc);
                    let _ = self.store.save();
                }
                UiEvent::Status(s) => self.status = s,
                UiEvent::Error(e) => {
                    self.error = Some(e);
                    self.busy = false;
                    self.device_code = None;
                }
                UiEvent::Busy(b) => self.busy = b,
                UiEvent::GameExited => {
                    self.game_running = false;
                    self.busy = false;
                    self.status = "Game closed".into();
                }
                UiEvent::IpcReady(h) => self.ipc = Some(h),
                UiEvent::FilePicked(target, bytes) => self.on_file_picked(target, bytes),
            }
        }
    }

    fn on_file_picked(&mut self, target: PickTarget, bytes: Vec<u8>) {
        let check = match target {
            PickTarget::Skin => cosmetics::validate_skin_png(&bytes),
            PickTarget::Cape => cosmetics::validate_cape_png(&bytes),
        };
        if let Err(e) = check {
            self.error = Some(e.to_string());
            return;
        }
        let tex = load_png_texture(&self.ctx, "frost_pick", &bytes);
        match target {
            PickTarget::Skin => {
                self.skin_tex = tex;
                self.skin_bytes = Some(bytes);
            }
            PickTarget::Cape => {
                self.cape_tex = tex;
                self.cape_bytes = Some(bytes);
            }
        }
    }

    fn send(&self, ev: UiEvent) {
        let _ = self.tx.send(ev);
        self.ctx.request_repaint();
    }

    // ---------------------------------------------------------------------------------------------
    // actions
    // ---------------------------------------------------------------------------------------------

    fn start_sign_in(&mut self) {
        self.error = None;
        self.busy = true;
        let (http, tx, ctx) = (self.http.clone(), self.tx.clone(), self.ctx.clone());
        let client_id = self.settings.ms_client_id.clone();
        self.rt.spawn(async move {
            let run = async {
                let code = auth::start_device_flow(&http, &client_id).await?;
                let _ = tx.send(UiEvent::DeviceCode(code.clone()));
                ctx.request_repaint();
                let ms = auth::poll_device_flow(&http, &client_id, &code).await?;
                auth::login_with_ms(&http, &ms).await
            };
            let _ = match run.await {
                Ok(acc) => tx.send(UiEvent::LoggedIn(acc)),
                Err(e) => tx.send(UiEvent::Error(format!("{e:#}"))),
            };
            ctx.request_repaint();
        });
    }

    fn start_launch(&mut self) {
        let Some(account) = self.store.active().cloned() else {
            self.error = Some("Sign in with Microsoft first.".into());
            return;
        };
        self.error = None;
        self.busy = true;
        self.progress = Arc::new(Progress::default());
        let version = self.settings.selected_version.clone();
        let settings = self.settings.clone();
        let (http, tx, ctx) = (self.http.clone(), self.tx.clone(), self.ctx.clone());
        let progress = self.progress.clone();
        let ipc = self.ipc.clone();
        let rt = self.rt.clone();

        self.rt.spawn(async move {
            let result: anyhow::Result<()> = async {
                let mut account = account;
                if account.is_expired() {
                    let _ = tx.send(UiEvent::Status("Refreshing Microsoft session...".into()));
                    account = auth::refresh(&http, &settings.ms_client_id, &account).await?;
                    let _ = tx.send(UiEvent::AccountRefreshed(account.clone()));
                }
                let _ = tx.send(UiEvent::Status(format!("Downloading Minecraft {version}...")));
                ctx.request_repaint();
                let plan = launch::prepare(&http, &settings, &account, &version, progress).await?;
                let _ = tx.send(UiEvent::Status("Starting game...".into()));
                let mut child = launch::spawn(&plan)?;
                let _ = tx.send(UiEvent::Status(format!("Playing {version}")));
                let _ = tx.send(UiEvent::Busy(false));

                let alive = Arc::new(AtomicBool::new(true));
                if let Some(ipc) = ipc {
                    spawn_frost_services(&rt, http.clone(), settings.clone(), account.clone(), ipc, alive.clone());
                }
                child.wait().await?;
                alive.store(false, Ordering::Relaxed);
                Ok(())
            }
            .await;
            if let Err(e) = result {
                let _ = tx.send(UiEvent::Error(format!("{e:#}")));
            }
            let _ = tx.send(UiEvent::GameExited);
            ctx.request_repaint();
        });
        self.game_running = true;
    }

    fn pick_png(&self, target: PickTarget) {
        // The native dialog blocks, so it gets its own short-lived thread (not a runtime worker).
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        std::thread::spawn(move || {
            let picked = rfd::FileDialog::new().add_filter("PNG image", &["png"]).pick_file();
            if let Some(path) = picked {
                match std::fs::read(&path) {
                    Ok(bytes) => {
                        let _ = tx.send(UiEvent::FilePicked(target, bytes));
                    }
                    Err(e) => {
                        let _ = tx.send(UiEvent::Error(format!("cannot read file: {e}")));
                    }
                }
                ctx.request_repaint();
            }
        });
    }

    fn apply_skin(&mut self) {
        let (Some(bytes), Some(acc)) = (self.skin_bytes.clone(), self.store.active().cloned()) else {
            self.error = Some("Choose a skin and sign in first.".into());
            return;
        };
        let (http, tx, ctx, variant) = (self.http.clone(), self.tx.clone(), self.ctx.clone(), self.skin_variant);
        self.status = "Uploading skin...".into();
        self.rt.spawn(async move {
            let ev = match cosmetics::upload_skin(&http, &acc, bytes, variant).await {
                Ok(()) => UiEvent::Status("Skin applied - visible to every player".into()),
                Err(e) => UiEvent::Error(format!("{e:#}")),
            };
            let _ = tx.send(ev);
            ctx.request_repaint();
        });
    }

    fn cape_action(&mut self, upload: bool) {
        let Some(acc) = self.store.active().cloned() else {
            self.error = Some("Sign in first.".into());
            return;
        };
        let bytes = if upload {
            match self.cape_bytes.clone() {
                Some(b) => Some(b),
                None => {
                    self.error = Some("Choose a cape PNG first.".into());
                    return;
                }
            }
        } else {
            None
        };
        let (http, tx, ctx) = (self.http.clone(), self.tx.clone(), self.ctx.clone());
        let base = self.settings.frost_api.clone();
        self.status = "Talking to the Frost backend...".into();
        self.rt.spawn(async move {
            let res: anyhow::Result<&'static str> = async {
                let mut api = FrostApi::new(http, &base);
                api.login(&acc).await?;
                match bytes {
                    Some(b) => {
                        api.upload_cape(b).await?;
                        Ok("Cape uploaded - visible to other Frost users")
                    }
                    None => {
                        api.delete_cape().await?;
                        Ok("Cape removed")
                    }
                }
            }
            .await;
            let ev = match res {
                Ok(msg) => UiEvent::Status(msg.into()),
                Err(e) => UiEvent::Error(format!("{e:#}")),
            };
            let _ = tx.send(ev);
            ctx.request_repaint();
        });
    }

    fn push_settings_to_game(&self) {
        let _ = self.settings.save();
        if let Some(ipc) = &self.ipc {
            ipc.send(ToGame::Settings { qol: self.settings.qol.clone(), hud: self.settings.hud.clone() });
        }
    }

    // ---------------------------------------------------------------------------------------------
    // layout
    // ---------------------------------------------------------------------------------------------

    fn title_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("title")
            .frame(egui::Frame::none().inner_margin(egui::Margin::symmetric(16.0, 10.0)))
            .show(ctx, |ui| {
                let drag = ui.interact(ui.max_rect(), egui::Id::new("title_drag"), Sense::click_and_drag());
                if drag.drag_started() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::hover());
                    glass::paint_snowflake(ui.painter(), rect.center(), 11.0, glass::ACCENT);
                    ui.label(RichText::new("Frost Client").size(18.0).strong());
                    ui.label(RichText::new(env!("CARGO_PKG_VERSION")).size(12.0).color(glass::TEXT_DIM));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button(" X ").clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        if ui.button(" - ").clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                        }
                    });
                });
            });
    }

    fn sidebar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("nav")
            .exact_width(200.0)
            .resizable(false)
            .frame(egui::Frame::none().inner_margin(egui::Margin::symmetric(14.0, 8.0)))
            .show(ctx, |ui| {
                ui.add_space(6.0);
                for tab in Tab::ALL {
                    let selected = self.tab == tab;
                    let text = RichText::new(tab.label()).size(15.0);
                    let btn = egui::Button::new(if selected { text.strong() } else { text })
                        .fill(if selected {
                            Color32::from_rgba_unmultiplied(120, 200, 255, 60)
                        } else {
                            Color32::from_white_alpha(8)
                        })
                        .stroke(Stroke::new(1.0, Color32::from_white_alpha(if selected { 110 } else { 28 })))
                        .rounding(Rounding::same(12.0))
                        .min_size(Vec2::new(ui.available_width(), 40.0));
                    if ui.add(btn).clicked() {
                        self.tab = tab;
                    }
                    ui.add_space(4.0);
                }
                ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                    ui.add_space(6.0);
                    let connected = self.ipc.as_ref().map_or(false, |i| i.connected());
                    ui.label(
                        RichText::new(if connected { "Game agent: connected" } else { "Game agent: not connected" })
                            .size(12.0)
                            .color(if connected { glass::ACCENT } else { glass::TEXT_DIM }),
                    );
                });
            });
    }

    fn content(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(egui::Frame::none().inner_margin(egui::Margin { left: 6.0, right: 18.0, top: 8.0, bottom: 14.0 }))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    if let Some(err) = self.error.clone() {
                        let mut dismiss = false;
                        glass_panel(ui, 120, 14.0, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(err).color(Color32::from_rgb(255, 140, 140)));
                                if ui.button("Dismiss").clicked() {
                                    dismiss = true;
                                }
                            });
                        });
                        if dismiss {
                            self.error = None;
                        }
                        ui.add_space(10.0);
                    }
                    match self.tab {
                        Tab::Play => self.tab_play(ui),
                        Tab::Skins => self.tab_skins(ui),
                        Tab::Hud => self.tab_hud(ctx, ui),
                        Tab::Trainer => self.tab_trainer(ui),
                        Tab::Settings => self.tab_settings(ui),
                    }
                });
            });
    }

    // ---------------------------------------------------------------------------------------------
    // tabs
    // ---------------------------------------------------------------------------------------------

    fn tab_play(&mut self, ui: &mut egui::Ui) {
        glass_panel(ui, 100, 18.0, |ui| {
            ui.label(RichText::new("Account").size(16.0).strong());
            ui.add_space(6.0);
            match self.store.active().cloned() {
                Some(acc) => {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&acc.username).size(18.0).strong());
                        ui.label(RichText::new(format!("({})", acc.uuid)).size(11.0).color(glass::TEXT_DIM));
                        if ui.button("Sign out").clicked() {
                            self.store.remove_active();
                            let _ = self.store.save();
                        }
                    });
                }
                None => {
                    if let Some(code) = self.device_code.clone() {
                        ui.label("Open the page in your browser and enter this code (copied to clipboard):");
                        ui.label(RichText::new(&code.user_code).size(30.0).strong().color(glass::ACCENT));
                        if ui.link(&code.verification_uri).clicked() {
                            let _ = open::that(&code.verification_uri);
                        }
                        ui.label(RichText::new("Waiting for you to finish signing in...").color(glass::TEXT_DIM));
                    } else if ui
                        .add_enabled(!self.busy, egui::Button::new("Sign in with Microsoft").min_size(Vec2::new(220.0, 36.0)))
                        .clicked()
                    {
                        self.start_sign_in();
                    }
                }
            }
        });
        ui.add_space(12.0);

        glass_panel(ui, 100, 18.0, |ui| {
            ui.label(RichText::new("Game").size(16.0).strong());
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label("Version");
                egui::ComboBox::from_id_source("version")
                    .selected_text(self.settings.selected_version.clone())
                    .show_ui(ui, |ui| {
                        for v in versions::SUPPORTED {
                            ui.selectable_value(&mut self.settings.selected_version, (*v).to_owned(), *v);
                        }
                    });
                if versions::is_legacy(&self.settings.selected_version) {
                    ui.label(RichText::new("legacy PvP (1.8 combat)").size(12.0).color(glass::TEXT_DIM));
                }
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label("Memory");
                ui.add(egui::Slider::new(&mut self.settings.ram_mb, 1024..=16384).step_by(256.0).suffix(" MB"));
            });
            ui.add_space(12.0);
            let can_play = !self.busy && !self.game_running;
            let label = if self.game_running { "Running..." } else { "PLAY" };
            let play = egui::Button::new(RichText::new(label).size(20.0).strong())
                .fill(Color32::from_rgba_unmultiplied(120, 200, 255, 90))
                .stroke(Stroke::new(1.0, glass::ACCENT))
                .rounding(Rounding::same(14.0))
                .min_size(Vec2::new(ui.available_width(), 52.0));
            if ui.add_enabled(can_play, play).clicked() {
                let _ = self.settings.save();
                self.start_launch();
            }
            if self.busy {
                ui.add_space(8.0);
                ui.add(egui::ProgressBar::new(self.progress.fraction()).animate(true).text(self.status.clone()));
                self.ctx.request_repaint_after(Duration::from_millis(100));
            } else {
                ui.add_space(6.0);
                ui.label(RichText::new(&self.status).color(glass::TEXT_DIM));
            }
        });
    }

    fn tab_skins(&mut self, ui: &mut egui::Ui) {
        glass_panel(ui, 100, 18.0, |ui| {
            ui.label(RichText::new("Skin changer").size(16.0).strong());
            ui.label(
                RichText::new("Uploaded to your Minecraft account - every player sees it, with or without Frost.")
                    .size(12.0)
                    .color(glass::TEXT_DIM),
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if let Some(tex) = &self.skin_tex {
                    let size = tex.size_vec2() * 3.0;
                    ui.image((tex.id(), size.min(Vec2::new(192.0, 192.0))));
                }
                ui.vertical(|ui| {
                    if ui.button("Choose skin (.png)").clicked() {
                        self.pick_png(PickTarget::Skin);
                    }
                    ui.horizontal(|ui| {
                        ui.radio_value(&mut self.skin_variant, SkinVariant::Classic, "Classic");
                        ui.radio_value(&mut self.skin_variant, SkinVariant::Slim, "Slim");
                    });
                    if ui.add_enabled(self.skin_bytes.is_some(), egui::Button::new("Apply skin")).clicked() {
                        self.apply_skin();
                    }
                });
            });
        });
        ui.add_space(12.0);

        glass_panel(ui, 100, 18.0, |ui| {
            ui.label(RichText::new("Frost cape").size(16.0).strong());
            ui.label(
                RichText::new("Stored on the Frost backend - only other Frost Client users see it.")
                    .size(12.0)
                    .color(glass::TEXT_DIM),
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if let Some(tex) = &self.cape_tex {
                    let size = tex.size_vec2() * 4.0;
                    ui.image((tex.id(), size.min(Vec2::new(256.0, 160.0))));
                }
                ui.vertical(|ui| {
                    if ui.button("Choose cape (.png)").clicked() {
                        self.pick_png(PickTarget::Cape);
                    }
                    if ui.add_enabled(self.cape_bytes.is_some(), egui::Button::new("Upload cape")).clicked() {
                        self.cape_action(true);
                    }
                    if ui.button("Remove cape").clicked() {
                        self.cape_action(false);
                    }
                });
            });
        });
        ui.add_space(12.0);

        glass_panel(ui, 100, 18.0, |ui| {
            ui.label(RichText::new("Tab list icon").size(16.0).strong());
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::hover());
                glass::paint_snowflake(ui.painter(), rect.center(), 9.0, glass::ACCENT);
                let name = self.store.active().map_or("YourName", |a| a.username.as_str()).to_owned();
                ui.label(name);
                ui.label(RichText::new("Frost users get this snowflake in the tab list.").size(12.0).color(glass::TEXT_DIM));
            });
        });
        ui.add_space(8.0);
        ui.label(RichText::new(&self.status).color(glass::TEXT_DIM));
    }

    fn tab_hud(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        glass_panel(ui, 100, 18.0, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("HUD layout").size(16.0).strong());
                ui.checkbox(&mut self.hud_edit, "Edit mode (drag to move, scroll to resize)");
                if ui.button("Apply to game").clicked() {
                    self.push_settings_to_game();
                    self.status = "HUD & QoL settings saved".into();
                }
            });
            ui.add_space(6.0);
            for w in self.settings.hud.widgets.iter_mut() {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut w.enabled, w.kind.label());
                    ui.add(egui::Slider::new(&mut w.scale, 0.6..=2.5).text("size"));
                    let mut op = f32::from(w.opacity);
                    if ui.add(egui::Slider::new(&mut op, 20.0..=240.0).text("glass")).changed() {
                        w.opacity = op as u8;
                    }
                });
            }
        });
        ui.add_space(12.0);

        // live preview "viewport" the HUD is drawn into
        let height = 300.0;
        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
        ui.painter().rect_filled(rect, 16.0, Color32::from_rgba_unmultiplied(6, 10, 18, 170));
        ui.painter().rect_stroke(rect, 16.0, Stroke::new(1.0, Color32::from_white_alpha(40)));
        ui.painter().text(
            rect.right_bottom() - Vec2::new(12.0, 10.0),
            egui::Align2::RIGHT_BOTTOM,
            "live preview",
            egui::FontId::proportional(11.0),
            glass::TEXT_DIM,
        );
        let live = self.ipc.as_ref().filter(|i| i.connected()).map(|i| i.stats());
        let stats = live.unwrap_or_else(|| self.demo_stats.clone());
        if hud_manager::render_hud(ctx, &mut self.settings.hud, &stats, self.hud_edit, rect) {
            let _ = self.settings.save();
        }
        ui.add_space(12.0);

        glass_panel(ui, 100, 18.0, |ui| {
            ui.label(RichText::new("Quality of life").size(16.0).strong());
            let q = &mut self.settings.qol;
            egui::Grid::new("qol").num_columns(2).spacing([24.0, 6.0]).show(ui, |ui| {
                ui.checkbox(&mut q.fullbright, "Fullbright");
                ui.checkbox(&mut q.minimal_viewmodel, "Minimal viewmodel");
                ui.end_row();
                ui.checkbox(&mut q.item_physics, "Item physics");
                ui.checkbox(&mut q.dynamic_fov, "Dynamic FOV");
                ui.end_row();
                ui.checkbox(&mut q.zoom_enabled, "Zoom toggle");
                ui.checkbox(&mut q.hitboxes, "Hitboxes visualizer");
                ui.end_row();
                ui.checkbox(&mut q.show_coordinates, "Coordinates");
                ui.checkbox(&mut q.show_biome, "Biome");
                ui.end_row();
                ui.checkbox(&mut q.fps_boost, "FPS boost");
                ui.checkbox(&mut q.chunk_optimizer, "Chunk optimizer");
                ui.end_row();
            });
            ui.add(egui::Slider::new(&mut q.zoom_fov, 5.0..=60.0).text("zoom FOV"));
            ui.add(egui::Slider::new(&mut q.chunk_render_cap, 4..=32).text("chunk cap"));
        });
        let _ = HudKind::ALL; // keeps the enum import meaningful for editors
    }

    fn tab_trainer(&mut self, ui: &mut egui::Ui) {
        let connected = self.ipc.as_ref().map_or(false, |i| i.connected());
        glass_panel(ui, 100, 18.0, |ui| {
            ui.label(RichText::new("PvP trainer bots").size(16.0).strong());
            ui.label(
                RichText::new("Singleplayer only. Start the game with Frost Client and open a singleplayer world.")
                    .size(12.0)
                    .color(glass::TEXT_DIM),
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label("Style");
                egui::ComboBox::from_id_source("style")
                    .selected_text(self.bot_style.label())
                    .show_ui(ui, |ui| {
                        for s in CombatStyle::ALL {
                            ui.selectable_value(&mut self.bot_style, s, s.label());
                        }
                    });
                ui.label("Name");
                ui.add(egui::TextEdit::singleline(&mut self.bot_name).desired_width(120.0));
            });
            ui.add(egui::Slider::new(&mut self.bot_difficulty, 0.0..=1.0).text("difficulty"));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.add_enabled(connected, egui::Button::new("Spawn bot")).clicked() {
                    let legacy = versions::is_legacy(&self.settings.selected_version);
                    if let Some(ipc) = &self.ipc {
                        ipc.spawn_bot(&self.bot_name, self.bot_style, self.bot_difficulty, legacy);
                    }
                }
                if ui.add_enabled(connected, egui::Button::new("Clear bots")).clicked() {
                    if let Some(ipc) = &self.ipc {
                        ipc.clear_bots();
                    }
                }
            });
            if !connected {
                ui.label(RichText::new("Game agent not connected.").color(glass::TEXT_DIM));
            }
        });
        ui.add_space(12.0);

        glass_panel(ui, 100, 18.0, |ui| {
            ui.label(RichText::new("Knockback trainer").size(16.0).strong());
            if ui.checkbox(&mut self.kb_enabled, "Enabled").changed() {
                if let Some(ipc) = &self.ipc {
                    let profile = if versions::is_legacy(&self.settings.selected_version) {
                        KbProfile::legacy_1_8()
                    } else {
                        KbProfile::modern()
                    };
                    ipc.send(ToGame::KnockbackTrainer { enabled: self.kb_enabled, profile });
                }
            }
            if let Some(ipc) = &self.ipc {
                let kb = ipc.shared.lock().unwrap().trainer.kb.clone();
                ui.label(format!(
                    "Combo {}   Best {}   Landed {}   Taken {}",
                    kb.combo, kb.best_combo, kb.hits_landed, kb.hits_taken
                ));
            }
        });
    }

    fn tab_settings(&mut self, ui: &mut egui::Ui) {
        glass_panel(ui, 100, 18.0, |ui| {
            ui.label(RichText::new("Launcher settings").size(16.0).strong());
            ui.add_space(6.0);
            egui::Grid::new("settings").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
                ui.label("Max memory");
                ui.add(egui::Slider::new(&mut self.settings.ram_mb, 1024..=16384).step_by(256.0).suffix(" MB"));
                ui.end_row();
                ui.label("JVM arguments");
                ui.add(egui::TextEdit::multiline(&mut self.settings.jvm_args).desired_rows(2).desired_width(420.0));
                ui.end_row();
                ui.label("Java path");
                ui.add(
                    egui::TextEdit::singleline(&mut self.settings.java_path)
                        .hint_text("auto (JAVA_HOME / PATH)")
                        .desired_width(420.0),
                );
                ui.end_row();
                ui.label("Frost backend");
                ui.add(egui::TextEdit::singleline(&mut self.settings.frost_api).desired_width(420.0));
                ui.end_row();
                ui.label("Microsoft client id");
                ui.add(egui::TextEdit::singleline(&mut self.settings.ms_client_id).desired_width(420.0));
                ui.end_row();
            });
            ui.add_space(8.0);
            if ui.button("Save").clicked() {
                match self.settings.save() {
                    Ok(()) => self.status = "Settings saved".into(),
                    Err(e) => self.error = Some(format!("{e:#}")),
                }
            }
        });
    }
}

impl eframe::App for FrostApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_events();

        // window background: translucent tint + border (the OS blur sits behind this)
        let bg = ctx.layer_painter(egui::LayerId::background());
        let screen = ctx.screen_rect();
        bg.rect_filled(screen, 16.0, Color32::from_rgba_unmultiplied(10, 14, 26, 150));
        bg.rect_filled(
            egui::Rect::from_min_max(screen.min, egui::pos2(screen.max.x, screen.min.y + screen.height() * 0.3)),
            Rounding { nw: 16.0, ne: 16.0, sw: 0.0, se: 0.0 },
            Color32::from_white_alpha(7),
        );
        bg.rect_stroke(screen.shrink(0.5), 16.0, Stroke::new(1.0, Color32::from_white_alpha(48)));

        self.title_bar(ctx);
        self.sidebar(ctx);
        self.content(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        let _ = self.settings.save();
    }
}

fn load_png_texture(ctx: &egui::Context, name: &str, bytes: &[u8]) -> Option<egui::TextureHandle> {
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    let color = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
    Some(ctx.load_texture(name, color, egui::TextureOptions::NEAREST))
}

/// Background tasks that run while the game is alive: Frost presence + cosmetics sync.
fn spawn_frost_services(
    rt: &Handle,
    http: reqwest::Client,
    settings: Settings,
    account: Account,
    ipc: IpcHandle,
    alive: Arc<AtomicBool>,
) {
    rt.spawn(async move {
        // Push HUD/QoL as soon as the agent connects (wait up to 2 minutes).
        for _ in 0..240 {
            if ipc.connected() || !alive.load(Ordering::Relaxed) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        ipc.send(ToGame::Settings { qol: settings.qol.clone(), hud: settings.hud.clone() });

        let mut api = FrostApi::new(http, &settings.frost_api);
        if let Err(e) = api.login(&account).await {
            tracing::info!("Frost backend unavailable, cosmetics disabled: {e:#}");
            return;
        }
        let mut cache = cosmetics::CosmeticsCache::new(Duration::from_secs(120));
        let mut tick = 0u32;
        while alive.load(Ordering::Relaxed) {
            if tick % 12 == 0 {
                let _ = api.heartbeat().await; // every 60 s
            }
            let players = ipc.shared.lock().unwrap().tab_players.clone();
            let missing = cache.missing(players.iter());
            if !missing.is_empty() {
                if let Ok(found) = api.lookup(&missing).await {
                    cache.insert_all(found);
                    ipc.send(ToGame::Cosmetics { players: cache.snapshot() });
                }
            }
            tick = tick.wrapping_add(1);
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });
}
