//! The launcher window (egui / eframe), liquid-glass style.
//!
//! Layout: custom top bar (wordmark, navigation, settings, avatar, window buttons) and a
//! hero home screen with the selected version, a Launch / Login button and a row of version cards.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;

use eframe::egui::{
    self, pos2, vec2, Align, Color32, Layout, Pos2, Rect, RichText, Rounding, Sense, Stroke, Vec2,
};
use tokio::runtime::Handle;

use crate::auth::{self, Account, AccountStore};
use crate::config::Settings;
use crate::cosmetics::{self, FrostApi, SkinVariant};
use crate::downloader::Progress;
use crate::glass::{self, glass_panel};
use crate::hud_manager::{self, GameStats};
use crate::ipc::{self, IpcHandle, ToGame};
use crate::launch;
use crate::trainer::{CombatStyle, KbProfile};
use crate::versions;

const BLUE: Color32 = Color32::from_rgb(54, 88, 255);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Home,
    Versions,
    Skins,
    Hud,
    Trainer,
    Settings,
}

impl Tab {
    /// Tabs shown in the top navigation (Settings lives behind the gear icon).
    const NAV: [Tab; 5] = [Tab::Home, Tab::Versions, Tab::Skins, Tab::Hud, Tab::Trainer];
    fn label(self) -> &'static str {
        match self {
            Tab::Home => "Home",
            Tab::Versions => "Versions",
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
    signing_in: bool,
    game_running: bool,
    maximized: bool,
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
            tab: Tab::Home,
            status: "Ready".into(),
            error: None,
            busy: false,
            signing_in: false,
            game_running: false,
            maximized: false,
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
                UiEvent::LoggedIn(acc) => {
                    self.signing_in = false;
                    self.status = format!("Signed in as {}", acc.username);
                    self.store.upsert(acc);
                    let _ = self.store.save();
                }
                UiEvent::AccountRefreshed(acc) => {
                    self.store.upsert(acc);
                    let _ = self.store.save();
                }
                UiEvent::Status(s) => self.status = s,
                UiEvent::Error(e) => {
                    self.error = Some(e);
                    self.busy = false;
                    self.signing_in = false;
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

    // ---------------------------------------------------------------------------------------------
    // actions
    // ---------------------------------------------------------------------------------------------

    /// One click: opens the Microsoft sign-in page in the browser, the redirect is caught locally.
    fn start_sign_in(&mut self) {
        self.error = None;
        let Some(client_id) = auth::effective_client_id(&self.settings.ms_client_id) else {
            self.error = Some(
                "This build has no Microsoft app id compiled in. Set the FROST_MS_CLIENT_ID \
                 repository variable and rebuild (see README)."
                    .into(),
            );
            return;
        };
        self.signing_in = true;
        self.status = "Waiting for Microsoft sign-in in your browser...".into();
        let (http, tx, ctx) = (self.http.clone(), self.tx.clone(), self.ctx.clone());
        self.rt.spawn(async move {
            let run = async {
                let ms = auth::browser_login(&http, &client_id, |url| {
                    let _ = open::that(url);
                })
                .await?;
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
            self.start_sign_in();
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
                    let client_id = auth::effective_client_id(&settings.ms_client_id)
                        .ok_or_else(|| anyhow::anyhow!("no Microsoft app id compiled in"))?;
                    account = auth::refresh(&http, &client_id, &account).await?;
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
    // top bar
    // ---------------------------------------------------------------------------------------------

    fn top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("topbar")
            .frame(egui::Frame::none().inner_margin(egui::Margin::symmetric(26.0, 16.0)))
            .show(ctx, |ui| {
                let drag = ui.interact(ui.max_rect(), egui::Id::new("title_drag"), Sense::click_and_drag());
                if drag.drag_started() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                if drag.double_clicked() {
                    self.maximized = !self.maximized;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(self.maximized));
                }
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(26.0), Sense::hover());
                    glass::paint_snowflake(ui.painter(), rect.center(), 12.0, glass::ACCENT);
                    ui.label(RichText::new("FROSTCLIENT").size(24.0).strong().color(Color32::from_rgb(214, 224, 255)));
                    ui.add_space(70.0);

                    for tab in Tab::NAV {
                        let selected = self.tab == tab;
                        let color = if selected { Color32::WHITE } else { Color32::from_white_alpha(120) };
                        let resp = ui
                            .add(egui::Label::new(RichText::new(tab.label()).size(15.0).color(color)).sense(Sense::click()))
                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                        if selected {
                            let y = resp.rect.bottom() + 3.0;
                            ui.painter().line_segment(
                                [pos2(resp.rect.left() + 2.0, y), pos2(resp.rect.left() + 22.0, y)],
                                Stroke::new(2.0, Color32::WHITE),
                            );
                        }
                        if resp.clicked() {
                            self.tab = tab;
                        }
                        ui.add_space(26.0);
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if window_button(ui, WinBtn::Close).clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        if window_button(ui, WinBtn::Maximize).clicked() {
                            self.maximized = !self.maximized;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(self.maximized));
                        }
                        if window_button(ui, WinBtn::Minimize).clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                        }
                        ui.add_space(8.0);
                        // avatar
                        let name = self.store.active().map(|a| a.username.clone());
                        let (rect, resp) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::click());
                        let p = ui.painter();
                        p.rect_filled(rect, 8.0, Color32::from_rgba_unmultiplied(120, 200, 255, if name.is_some() { 90 } else { 30 }));
                        p.rect_stroke(rect, 8.0, Stroke::new(1.0, Color32::from_white_alpha(90)));
                        let letter = name.as_deref().and_then(|n| n.chars().next()).map_or('?', |c| c.to_ascii_uppercase());
                        p.text(rect.center(), egui::Align2::CENTER_CENTER, letter.to_string(), egui::FontId::proportional(16.0), Color32::WHITE);
                        if resp.on_hover_text(name.as_deref().unwrap_or("Not signed in")).clicked() {
                            self.tab = Tab::Settings;
                        }
                        ui.add_space(6.0);
                        if window_button(ui, WinBtn::Gear).clicked() {
                            self.tab = Tab::Settings;
                        }
                    });
                });
            });
    }

    fn content(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(egui::Frame::none().inner_margin(egui::Margin { left: 14.0, right: 14.0, top: 2.0, bottom: 14.0 }))
            .show(ctx, |ui| {
                if self.tab == Tab::Home {
                    self.tab_home(ctx, ui);
                    return;
                }
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    ui.add_space(6.0);
                    if let Some(err) = self.error.clone() {
                        self.error_card(ui, err);
                    }
                    match self.tab {
                        Tab::Home => {}
                        Tab::Versions => self.tab_versions(ui),
                        Tab::Skins => self.tab_skins(ui),
                        Tab::Hud => self.tab_hud(ctx, ui),
                        Tab::Trainer => self.tab_trainer(ui),
                        Tab::Settings => self.tab_settings(ui),
                    }
                });
            });
    }

    fn error_card(&mut self, ui: &mut egui::Ui, err: String) {
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

    // ---------------------------------------------------------------------------------------------
    // home (hero) screen
    // ---------------------------------------------------------------------------------------------

    fn tab_home(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        let hero = ui.max_rect();
        let painter = ui.painter().clone();
        rounded_gradient(
            &painter,
            hero,
            20.0,
            Color32::from_rgb(40, 76, 150),
            Color32::from_rgb(10, 16, 34),
        );
        let t = ctx.input(|i| i.time) as f32;
        paint_aurora(&painter, hero, t);
        paint_snow(&painter, hero, t);

        let version = self.settings.selected_version.clone();
        let (title_sub, _) = version_info(&version);

        // --- title block --------------------------------------------------------------------------
        let block = Rect::from_min_size(
            pos2(hero.left() + 40.0, hero.top() + hero.height() * 0.16),
            vec2(hero.width() * 0.6, 220.0),
        );
        ui.allocate_ui_at_rect(block, |ui| {
            ui.label(RichText::new(format!("{version} Fabric")).size(64.0).strong().color(Color32::from_rgb(232, 238, 255)));
            ui.label(RichText::new(title_sub).size(15.0).color(Color32::from_white_alpha(150)));
            ui.add_space(14.0);

            if let Some(err) = self.error.clone() {
                ui.label(RichText::new(err.clone()).size(13.0).color(Color32::from_rgb(255, 150, 150)));
                if ui.small_button("Dismiss").clicked() {
                    self.error = None;
                }
                ui.add_space(6.0);
            }

            let logged_in = self.store.active().is_some();
            ui.horizontal(|ui| {
                if logged_in {
                    let label = if self.game_running { "Running..." } else { "Launch" };
                    let btn = egui::Button::new(RichText::new(label).size(17.0).strong().color(Color32::WHITE))
                        .fill(BLUE)
                        .rounding(Rounding::same(9.0))
                        .min_size(vec2(130.0, 42.0));
                    if ui.add_enabled(!self.busy && !self.game_running, btn).clicked() {
                        let _ = self.settings.save();
                        self.start_launch();
                    }
                } else {
                    let label = if self.signing_in { "Waiting for browser..." } else { "Login" };
                    let btn = egui::Button::new(RichText::new(label).size(17.0).strong().color(Color32::WHITE))
                        .fill(BLUE)
                        .rounding(Rounding::same(9.0))
                        .min_size(vec2(170.0, 42.0));
                    if ui.add_enabled(!self.signing_in, btn).clicked() {
                        self.start_sign_in();
                    }
                }
                let opts = egui::Button::new(RichText::new("Options").size(14.0))
                    .fill(Color32::from_white_alpha(18))
                    .rounding(Rounding::same(9.0))
                    .min_size(vec2(90.0, 42.0));
                if ui.add(opts).clicked() {
                    self.tab = Tab::Versions;
                }
            });
            ui.add_space(8.0);
            if self.busy {
                ui.add(
                    egui::ProgressBar::new(self.progress.fraction())
                        .animate(true)
                        .desired_width(320.0)
                        .text(self.status.clone()),
                );
            } else if let Some(acc) = self.store.active() {
                ui.label(RichText::new(format!("Signed in as {}  -  {}", acc.username, self.status)).size(12.0).color(Color32::from_white_alpha(130)));
            } else {
                ui.label(
                    RichText::new("Opens the Microsoft sign-in in your browser. That's it.")
                        .size(12.0)
                        .color(Color32::from_white_alpha(130)),
                );
            }
        });

        // --- version cards ------------------------------------------------------------------------
        let n = versions::SUPPORTED.len() as f32;
        let gap = 18.0;
        let side = 36.0;
        let more_w = 64.0;
        let card_h = 150.0;
        let avail = hero.width() - side * 2.0 - more_w - gap * n;
        let card_w = (avail / n).clamp(150.0, 270.0);
        let y = hero.bottom() - 28.0 - card_h;
        let mut x = hero.left() + side;
        for (i, v) in versions::SUPPORTED.iter().enumerate() {
            let rect = Rect::from_min_size(pos2(x, y), vec2(card_w, card_h));
            let selected = self.settings.selected_version == *v;
            let resp = ui.interact(rect, egui::Id::new(("vcard", i)), Sense::click());
            let (c1, c2) = card_colors(i);
            let lift = if resp.hovered() { 1.15 } else { 1.0 };
            rounded_gradient(&painter, rect, 14.0, scale_color(c1, lift), scale_color(c2, lift));
            paint_card_art(&painter, rect, i, t);
            // darken the lower part for legible text
            rounded_gradient(
                &painter,
                Rect::from_min_max(pos2(rect.left(), rect.top() + card_h * 0.45), rect.max),
                14.0,
                Color32::from_black_alpha(0),
                Color32::from_black_alpha(150),
            );
            painter.text(
                pos2(rect.left() + 18.0, rect.bottom() - 18.0),
                egui::Align2::LEFT_BOTTOM,
                format!("{v} Fabric"),
                egui::FontId::proportional(22.0),
                Color32::WHITE,
            );
            let stroke = if selected {
                Stroke::new(2.0, Color32::from_rgb(110, 140, 255))
            } else {
                Stroke::new(1.0, Color32::from_white_alpha(40))
            };
            painter.rect_stroke(rect, 14.0, stroke);
            if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                self.settings.selected_version = (*v).to_owned();
                let _ = self.settings.save();
            }
            x += card_w + gap;
        }
        // "more" card
        let more = Rect::from_min_size(pos2(x, y), vec2(more_w, card_h));
        let resp = ui.interact(more, egui::Id::new("vcard_more"), Sense::click());
        painter.rect_filled(more, 14.0, Color32::from_white_alpha(if resp.hovered() { 40 } else { 24 }));
        painter.rect_stroke(more, 14.0, Stroke::new(1.0, Color32::from_white_alpha(40)));
        for r in 0..3 {
            for c in 0..3 {
                painter.circle_filled(
                    more.center() + vec2((c as f32 - 1.0) * 11.0, (r as f32 - 1.0) * 11.0),
                    2.6,
                    Color32::WHITE,
                );
            }
        }
        if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            self.tab = Tab::Versions;
        }

        // light animation only while this screen is visible (~15 fps keeps CPU near idle)
        ctx.request_repaint_after(Duration::from_millis(66));
    }

    // ---------------------------------------------------------------------------------------------
    // other tabs
    // ---------------------------------------------------------------------------------------------

    fn tab_versions(&mut self, ui: &mut egui::Ui) {
        glass_panel(ui, 100, 18.0, |ui| {
            ui.label(RichText::new("Game version").size(16.0).strong());
            ui.add_space(6.0);
            for v in versions::SUPPORTED {
                let (sub, _) = version_info(v);
                ui.horizontal(|ui| {
                    ui.radio_value(&mut self.settings.selected_version, (*v).to_owned(), RichText::new(*v).size(16.0));
                    ui.label(RichText::new(sub).size(12.0).color(glass::TEXT_DIM));
                });
            }
        });
        ui.add_space(12.0);
        glass_panel(ui, 100, 18.0, |ui| {
            ui.label(RichText::new("Performance").size(16.0).strong());
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label("Memory");
                ui.add(egui::Slider::new(&mut self.settings.ram_mb, 1024..=16384).step_by(256.0).suffix(" MB"));
            });
            ui.add_space(4.0);
            ui.label("JVM arguments");
            ui.add(egui::TextEdit::multiline(&mut self.settings.jvm_args).desired_rows(2).desired_width(f32::INFINITY));
            ui.add_space(8.0);
            if ui.button("Save").clicked() {
                match self.settings.save() {
                    Ok(()) => self.status = "Settings saved".into(),
                    Err(e) => self.error = Some(format!("{e:#}")),
                }
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
            ui.add(egui::Slider::new(&mut q.zoom_fov, 30.0..=60.0).text("zoom FOV"));
            ui.add(egui::Slider::new(&mut q.chunk_render_cap, 4..=32).text("chunk cap"));
        });
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
                    let label = if self.signing_in { "Waiting for browser..." } else { "Login with Microsoft" };
                    if ui.add_enabled(!self.signing_in, egui::Button::new(label)).clicked() {
                        self.start_sign_in();
                    }
                }
            }
        });
        ui.add_space(12.0);
        glass_panel(ui, 100, 18.0, |ui| {
            ui.label(RichText::new("Launcher settings").size(16.0).strong());
            ui.add_space(6.0);
            egui::Grid::new("settings").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
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
            });
            ui.add_space(6.0);
            ui.collapsing("Advanced", |ui| {
                ui.label(RichText::new("Override the built-in Microsoft app id (normally leave empty).").size(12.0).color(glass::TEXT_DIM));
                ui.add(egui::TextEdit::singleline(&mut self.settings.ms_client_id).desired_width(420.0));
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
        bg.rect_filled(screen, 16.0, Color32::from_rgba_unmultiplied(12, 16, 28, 175));
        bg.rect_stroke(screen.shrink(0.5), 16.0, Stroke::new(1.0, Color32::from_white_alpha(48)));

        self.top_bar(ctx);
        self.content(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        let _ = self.settings.save();
    }
}

// ------------------------------------------------------------------------------------------------
// drawing helpers
// ------------------------------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum WinBtn {
    Minimize,
    Maximize,
    Close,
    Gear,
}

fn window_button(ui: &mut egui::Ui, kind: WinBtn) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::click());
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(rect, 8.0, Color32::from_white_alpha(28));
    }
    let c = rect.center();
    let s = Stroke::new(1.6, Color32::from_white_alpha(if resp.hovered() { 255 } else { 190 }));
    match kind {
        WinBtn::Minimize => p.line_segment([c + vec2(-6.0, 4.0), c + vec2(6.0, 4.0)], s),
        WinBtn::Maximize => p.rect_stroke(Rect::from_center_size(c, Vec2::splat(11.0)), 2.0, s),
        WinBtn::Close => {
            p.line_segment([c + vec2(-5.5, -5.5), c + vec2(5.5, 5.5)], s);
            p.line_segment([c + vec2(-5.5, 5.5), c + vec2(5.5, -5.5)], s);
        }
        WinBtn::Gear => {
            p.circle_stroke(c, 4.0, s);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0;
                let d = vec2(a.cos(), a.sin());
                p.line_segment([c + d * 6.5, c + d * 9.0], s);
            }
        }
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Vertical gradient inside a rounded rectangle, built as a vertex-coloured triangle strip.
fn rounded_gradient(painter: &egui::Painter, rect: Rect, radius: f32, top: Color32, bottom: Color32) {
    let r = radius.min(rect.height() * 0.5).min(rect.width() * 0.5);
    let mut ys: Vec<f32> = Vec::new();
    for k in 0..=4 {
        ys.push(rect.top() + r * k as f32 / 4.0);
    }
    let mid_steps = 16;
    for k in 1..mid_steps {
        ys.push(rect.top() + r + (rect.height() - 2.0 * r) * k as f32 / mid_steps as f32);
    }
    for k in 0..=4 {
        ys.push(rect.bottom() - r + r * k as f32 / 4.0);
    }
    let mut mesh = egui::Mesh::default();
    for (i, &y) in ys.iter().enumerate() {
        let dy = (y - rect.top()).min(rect.bottom() - y).max(0.0);
        let inset = if dy < r { r - (r * r - (r - dy) * (r - dy)).max(0.0).sqrt() } else { 0.0 };
        let t = ((y - rect.top()) / rect.height().max(1.0)).clamp(0.0, 1.0);
        let col = top.lerp_to_gamma(bottom, t);
        mesh.colored_vertex(pos2(rect.left() + inset, y), col);
        mesh.colored_vertex(pos2(rect.right() - inset, y), col);
        if i > 0 {
            let a = (2 * (i - 1)) as u32;
            let b = (2 * i) as u32;
            mesh.add_triangle(a, a + 1, b);
            mesh.add_triangle(a + 1, b + 1, b);
        }
    }
    painter.add(egui::Shape::mesh(mesh));
}

fn scale_color(c: Color32, k: f32) -> Color32 {
    let f = |v: u8| ((f32::from(v) * k).min(255.0)) as u8;
    Color32::from_rgb(f(c.r()), f(c.g()), f(c.b()))
}

/// Soft coloured glows behind the hero (stacked translucent circles, slowly drifting).
fn paint_aurora(painter: &egui::Painter, rect: Rect, t: f32) {
    let blobs = [
        (0.78, 0.30, 260.0, Color32::from_rgb(120, 80, 255), 0.0),
        (0.62, 0.55, 220.0, Color32::from_rgb(60, 190, 255), 1.7),
        (0.92, 0.62, 200.0, Color32::from_rgb(255, 110, 190), 3.1),
    ];
    for (fx, fy, r, col, phase) in blobs {
        let c = pos2(
            rect.left() + rect.width() * fx + (t * 0.25 + phase).sin() * 24.0,
            rect.top() + rect.height() * fy + (t * 0.2 + phase).cos() * 16.0,
        );
        for k in 0..14 {
            let rr = r * (1.0 - k as f32 / 14.0);
            painter.circle_filled(c, rr, Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), 3));
        }
    }
}

/// Deterministic drifting snow (no allocations, no RNG state).
fn paint_snow(painter: &egui::Painter, rect: Rect, t: f32) {
    let n = 70;
    for i in 0..n {
        let fi = i as f32;
        let sx = ((fi * 12.9898).sin() * 43758.547).fract().abs();
        let sy = ((fi * 78.233).sin() * 12345.678).fract().abs();
        let speed = 14.0 + 26.0 * (((fi * 3.1).sin() * 999.0).fract().abs());
        let size = 1.0 + 2.2 * (((fi * 5.7).sin() * 777.0).fract().abs());
        let x = rect.left() + rect.width() * sx + (t * 0.6 + fi).sin() * 14.0;
        let y = rect.top() + ((sy * rect.height() + t * speed) % rect.height());
        if x > rect.left() && x < rect.right() {
            painter.circle_filled(pos2(x, y), size, Color32::from_white_alpha((70.0 + 90.0 * size / 3.2) as u8));
        }
    }
}

fn card_colors(i: usize) -> (Color32, Color32) {
    const PAL: [((u8, u8, u8), (u8, u8, u8)); 5] = [
        ((120, 150, 90), (60, 90, 60)),
        ((240, 150, 190), (110, 70, 140)),
        ((90, 160, 230), (50, 70, 150)),
        ((230, 170, 80), (130, 70, 50)),
        ((120, 110, 230), (50, 40, 120)),
    ];
    let ((a, b, c), (d, e, f)) = PAL[i % PAL.len()];
    (Color32::from_rgb(a, b, c), Color32::from_rgb(d, e, f))
}

/// Simple abstract art for the cards: a big faint snowflake plus a few drifting flakes.
fn paint_card_art(painter: &egui::Painter, rect: Rect, i: usize, t: f32) {
    let center = pos2(rect.center().x + (t * 0.3 + i as f32).sin() * 6.0, rect.top() + rect.height() * 0.38);
    glass::paint_snowflake(painter, center, rect.height() * 0.26, Color32::from_white_alpha(70));
}

/// (subtitle, is_legacy) for a version id.
fn version_info(id: &str) -> (&'static str, bool) {
    match id {
        "1.8.9" => ("Legacy PvP - 1.8 combat", true),
        "1.21.10" => ("Modern PvP - 1.21.10", false),
        "1.21.11" => ("Modern PvP - 1.21.11", false),
        "26.1.2" => ("Latest - 26.1.2", false),
        "26.2" => ("Newest - 26.2", false),
        _ => ("Custom version", false),
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

// `Pos2` is used by helper signatures in doc examples; keep the import meaningful.
#[allow(dead_code)]
fn _origin() -> Pos2 {
    pos2(0.0, 0.0)
}
