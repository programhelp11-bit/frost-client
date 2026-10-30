//! Liquid-glass HUD model, layout editor and renderer.
//!
//! The same widgets are drawn (a) in the launcher's HUD editor / live preview and (b) by the
//! Frost overlay renderer; the in-game agent streams [`GameStats`] over the IPC bridge and
//! receives the saved [`HudLayout`] + [`QolSettings`] from the launcher.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Vec2};
use serde::{Deserialize, Serialize};

use crate::glass;

// ------------------------------------------------------------------------------------------------
// Data model
// ------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HudKind {
    Cps,
    Fps,
    Ping,
    Keystrokes,
    ArmorStatus,
    PotionStatus,
    Direction,
    ReachDisplay,
    Coordinates,
}

impl HudKind {
    pub const ALL: [HudKind; 9] = [
        HudKind::Cps,
        HudKind::Fps,
        HudKind::Ping,
        HudKind::Keystrokes,
        HudKind::ArmorStatus,
        HudKind::PotionStatus,
        HudKind::Direction,
        HudKind::ReachDisplay,
        HudKind::Coordinates,
    ];

    pub fn label(self) -> &'static str {
        match self {
            HudKind::Cps => "CPS",
            HudKind::Fps => "FPS",
            HudKind::Ping => "Ping",
            HudKind::Keystrokes => "Keystrokes",
            HudKind::ArmorStatus => "Armor Status",
            HudKind::PotionStatus => "Potion Status",
            HudKind::Direction => "Direction",
            HudKind::ReachDisplay => "Reach",
            HudKind::Coordinates => "Coordinates / Biome",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HudWidget {
    pub kind: HudKind,
    pub enabled: bool,
    pub pos: [f32; 2],
    pub scale: f32,
    /// Glass tint opacity 0..=255.
    pub opacity: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HudLayout {
    pub widgets: Vec<HudWidget>,
    pub snap_grid: f32,
}

impl Default for HudLayout {
    fn default() -> Self {
        let mut widgets = Vec::new();
        let mut y = 20.0;
        for kind in HudKind::ALL {
            let enabled = matches!(kind, HudKind::Cps | HudKind::Fps | HudKind::Ping | HudKind::Keystrokes);
            widgets.push(HudWidget { kind, enabled, pos: [20.0, y], scale: 1.0, opacity: 120 });
            y += 56.0;
        }
        Self { widgets, snap_grid: 4.0 }
    }
}

/// Quality-of-life toggles. Applied in-game by the agent (shader / render hooks).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct QolSettings {
    pub fullbright: bool,
    pub minimal_viewmodel: bool,
    pub item_physics: bool,
    pub dynamic_fov: bool,
    pub zoom_enabled: bool,
    pub zoom_key: String,
    pub zoom_fov: f32,
    pub show_coordinates: bool,
    pub show_biome: bool,
    pub hitboxes: bool,
    pub fps_boost: bool,
    pub chunk_optimizer: bool,
    pub chunk_render_cap: u32,
}

impl Default for QolSettings {
    fn default() -> Self {
        Self {
            fullbright: false,
            minimal_viewmodel: false,
            item_physics: false,
            dynamic_fov: true,
            zoom_enabled: true,
            zoom_key: "C".into(),
            zoom_fov: 30.0,
            show_coordinates: true,
            show_biome: true,
            hitboxes: false,
            fps_boost: true,
            chunk_optimizer: true,
            chunk_render_cap: 12,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Keys {
    pub w: bool,
    pub a: bool,
    pub s: bool,
    pub d: bool,
    pub space: bool,
    pub lmb: bool,
    pub rmb: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArmorPiece {
    /// 0 = helmet .. 3 = boots
    pub slot: u8,
    pub name: String,
    pub durability_pct: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PotionEffect {
    pub name: String,
    pub amplifier: u8,
    pub remaining_secs: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GameStats {
    pub fps: u32,
    pub ping_ms: u32,
    pub cps_left: u32,
    pub cps_right: u32,
    pub keys: Keys,
    pub armor: Vec<ArmorPiece>,
    pub potions: Vec<PotionEffect>,
    pub yaw: f32,
    pub last_reach: Option<f32>,
    pub pos: [f64; 3],
    pub biome: String,
}

impl GameStats {
    /// Fake data for the launcher's live preview.
    pub fn demo() -> Self {
        Self {
            fps: 612,
            ping_ms: 23,
            cps_left: 9,
            cps_right: 3,
            keys: Keys { w: true, a: false, s: false, d: true, space: false, lmb: true, rmb: false },
            armor: vec![
                ArmorPiece { slot: 0, name: "Diamond Helmet".into(), durability_pct: 0.92 },
                ArmorPiece { slot: 1, name: "Diamond Chestplate".into(), durability_pct: 0.71 },
                ArmorPiece { slot: 2, name: "Diamond Leggings".into(), durability_pct: 0.64 },
                ArmorPiece { slot: 3, name: "Diamond Boots".into(), durability_pct: 0.88 },
            ],
            potions: vec![
                PotionEffect { name: "Speed II".into(), amplifier: 1, remaining_secs: 94 },
                PotionEffect { name: "Strength".into(), amplifier: 0, remaining_secs: 41 },
            ],
            yaw: 135.0,
            last_reach: Some(2.94),
            pos: [124.5, 64.0, -310.2],
            biome: "Plains".into(),
        }
    }
}

// ------------------------------------------------------------------------------------------------
// CPS counter
// ------------------------------------------------------------------------------------------------

/// Sliding one-second window click counter.
#[derive(Debug, Default)]
pub struct CpsCounter {
    clicks: VecDeque<Instant>,
}

impl CpsCounter {
    pub fn click_at(&mut self, now: Instant) {
        self.clicks.push_back(now);
        self.prune(now);
    }

    pub fn click(&mut self) {
        self.click_at(Instant::now());
    }

    fn prune(&mut self, now: Instant) {
        while let Some(front) = self.clicks.front() {
            if now.duration_since(*front) > Duration::from_secs(1) {
                self.clicks.pop_front();
            } else {
                break;
            }
        }
    }

    pub fn cps_at(&mut self, now: Instant) -> u32 {
        self.prune(now);
        self.clicks.len() as u32
    }
}

/// 16-point compass label for a Minecraft yaw (0 = south, 90 = west).
pub fn compass(yaw: f32) -> &'static str {
    const NAMES: [&str; 16] = [
        "S", "SSW", "SW", "WSW", "W", "WNW", "NW", "NNW", "N", "NNE", "NE", "ENE", "E", "ESE", "SE", "SSE",
    ];
    let norm = yaw.rem_euclid(360.0);
    let idx = ((norm + 11.25) / 22.5) as usize % 16;
    NAMES[idx]
}

// ------------------------------------------------------------------------------------------------
// Rendering
// ------------------------------------------------------------------------------------------------

fn text_size(scale: f32) -> f32 {
    15.0 * scale
}

/// Render all enabled HUD widgets. In `edit` mode widgets can be dragged and snap to the grid.
/// Widget positions are relative to `bounds.min` (the game viewport / preview rectangle).
/// Returns `true` if the layout changed this frame (so the caller can persist it).
pub fn render_hud(
    ctx: &egui::Context,
    layout: &mut HudLayout,
    stats: &GameStats,
    edit: bool,
    bounds: Rect,
) -> bool {
    let mut changed = false;
    let grid = layout.snap_grid.max(1.0);

    for (i, w) in layout.widgets.iter_mut().enumerate() {
        if !w.enabled {
            continue;
        }
        let id = egui::Id::new(("frost_hud", i));
        let area = egui::Area::new(id)
            .order(egui::Order::Foreground)
            .fixed_pos(Pos2::new(bounds.min.x + w.pos[0], bounds.min.y + w.pos[1]))
            .show(ctx, |ui| {
                let size = widget_contents(ui, w, stats);
                let rect = Rect::from_min_size(ui.min_rect().min, size);
                if edit {
                    ui.painter().rect_stroke(
                        rect.expand(3.0),
                        10.0,
                        egui::Stroke::new(1.0, glass::ACCENT),
                    );
                    return Some(ui.interact(rect, id.with("drag"), Sense::drag()));
                }
                None
            });

        if let Some(resp) = area.inner {
            if resp.dragged() {
                let delta = resp.drag_delta();
                w.pos[0] += delta.x;
                w.pos[1] += delta.y;
                changed = true;
            }
            if resp.drag_stopped() {
                w.pos[0] = (w.pos[0] / grid).round() * grid;
                w.pos[1] = (w.pos[1] / grid).round() * grid;
                changed = true;
            }
            if resp.hovered() {
                // scroll wheel resizes the hovered widget in edit mode
                let scroll = ctx.input(|i| i.raw_scroll_delta.y);
                if scroll != 0.0 {
                    w.scale = (w.scale + scroll.signum() * 0.05).clamp(0.6, 2.5);
                    changed = true;
                }
            }
        }
        // keep widgets on screen
        w.pos[0] = w.pos[0].clamp(0.0, (bounds.width() - 40.0).max(0.0));
        w.pos[1] = w.pos[1].clamp(0.0, (bounds.height() - 30.0).max(0.0));
    }
    changed
}

/// Draws one widget; returns its size.
fn widget_contents(ui: &mut egui::Ui, w: &HudWidget, s: &GameStats) -> Vec2 {
    let frame = glass::glass_frame(w.opacity, 12.0 * w.scale)
        .inner_margin(egui::Margin::symmetric(10.0 * w.scale, 6.0 * w.scale));
    let sz = text_size(w.scale);
    let out = frame.show(ui, |ui| {
        ui.spacing_mut().item_spacing = Vec2::splat(3.0 * w.scale);
        match w.kind {
            HudKind::Cps => {
                line(ui, "CPS", &format!("{} | {}", s.cps_left, s.cps_right), sz);
            }
            HudKind::Fps => line(ui, "FPS", &s.fps.to_string(), sz),
            HudKind::Ping => line(ui, "Ping", &format!("{} ms", s.ping_ms), sz),
            HudKind::Direction => line(ui, "Dir", &format!("{} {:.0}°", compass(s.yaw), s.yaw.rem_euclid(360.0)), sz),
            HudKind::ReachDisplay => match s.last_reach {
                Some(r) => line(ui, "Reach", &format!("{r:.2} m"), sz),
                None => line(ui, "Reach", "-", sz),
            },
            HudKind::Coordinates => {
                line(ui, "XYZ", &format!("{:.1} {:.1} {:.1}", s.pos[0], s.pos[1], s.pos[2]), sz);
                if !s.biome.is_empty() {
                    line(ui, "Biome", &s.biome, sz);
                }
            }
            HudKind::Keystrokes => keystrokes(ui, &s.keys, w.scale),
            HudKind::ArmorStatus => {
                if s.armor.is_empty() {
                    line(ui, "Armor", "none", sz);
                }
                for a in &s.armor {
                    durability_row(ui, a, sz);
                }
            }
            HudKind::PotionStatus => {
                if s.potions.is_empty() {
                    line(ui, "Effects", "none", sz);
                }
                for p in &s.potions {
                    line(ui, &p.name, &format!("{}:{:02}", p.remaining_secs / 60, p.remaining_secs % 60), sz);
                }
            }
        }
    });
    glass::decorate(ui.painter(), out.response.rect, 12.0 * w.scale);
    out.response.rect.size()
}

fn line(ui: &mut egui::Ui, label: &str, value: &str, size: f32) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).size(size * 0.85).color(glass::TEXT_DIM));
        ui.label(egui::RichText::new(value).size(size).strong().color(glass::TEXT));
    });
}

fn durability_row(ui: &mut egui::Ui, a: &ArmorPiece, size: f32) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(&a.name).size(size * 0.85).color(glass::TEXT_DIM));
        let (rect, _) = ui.allocate_exact_size(Vec2::new(size * 3.0, size * 0.45), Sense::hover());
        let p = ui.painter();
        p.rect_filled(rect, 4.0, Color32::from_white_alpha(25));
        let mut fill = rect;
        fill.set_width(rect.width() * a.durability_pct.clamp(0.0, 1.0));
        let c = if a.durability_pct > 0.5 {
            glass::ACCENT
        } else if a.durability_pct > 0.2 {
            Color32::from_rgb(255, 200, 90)
        } else {
            Color32::from_rgb(255, 100, 100)
        };
        p.rect_filled(fill, 4.0, c);
    });
}

fn keystrokes(ui: &mut egui::Ui, k: &Keys, scale: f32) {
    let cell = 34.0 * scale;
    let gap = 4.0 * scale;
    let total = Vec2::new(cell * 3.0 + gap * 2.0, cell * 2.0 + gap + cell * 0.7 + gap);
    let (rect, _) = ui.allocate_exact_size(total, Sense::hover());
    let p = ui.painter();
    let key = |col: f32, row: f32, w_cells: f32, h: f32, label: &str, down: bool| {
        let min = rect.min + Vec2::new(col * (cell + gap), row * (cell + gap));
        let r = Rect::from_min_size(min, Vec2::new(cell * w_cells + gap * (w_cells - 1.0), h));
        let fill = if down {
            Color32::from_rgba_unmultiplied(120, 200, 255, 150)
        } else {
            Color32::from_white_alpha(20)
        };
        p.rect_filled(r, 8.0, fill);
        p.rect_stroke(r, 8.0, egui::Stroke::new(1.0, Color32::from_white_alpha(if down { 140 } else { 45 })));
        p.text(r.center(), Align2::CENTER_CENTER, label, FontId::proportional(13.0 * scale), glass::TEXT);
    };
    key(1.0, 0.0, 1.0, cell, "W", k.w);
    key(0.0, 1.0, 1.0, cell, "A", k.a);
    key(1.0, 1.0, 1.0, cell, "S", k.s);
    key(2.0, 1.0, 1.0, cell, "D", k.d);
    let row2 = 2.0 * (cell + gap);
    let half = (cell * 3.0 + gap * 2.0 - gap) / 2.0;
    let mouse = |x: f32, label: &str, down: bool| {
        let r = Rect::from_min_size(rect.min + Vec2::new(x, row2), Vec2::new(half, cell * 0.7));
        let fill = if down {
            Color32::from_rgba_unmultiplied(120, 200, 255, 150)
        } else {
            Color32::from_white_alpha(20)
        };
        p.rect_filled(r, 8.0, fill);
        p.rect_stroke(r, 8.0, egui::Stroke::new(1.0, Color32::from_white_alpha(if down { 140 } else { 45 })));
        p.text(r.center(), Align2::CENTER_CENTER, label, FontId::proportional(12.0 * scale), glass::TEXT);
    };
    mouse(0.0, "LMB", k.lmb);
    mouse(half + gap, "RMB", k.rmb);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cps_window_slides() {
        let mut c = CpsCounter::default();
        let t0 = Instant::now();
        for i in 0..10 {
            c.click_at(t0 + Duration::from_millis(i * 50));
        }
        assert_eq!(c.cps_at(t0 + Duration::from_millis(500)), 10);
        assert_eq!(c.cps_at(t0 + Duration::from_millis(1300)), 4);
        assert_eq!(c.cps_at(t0 + Duration::from_secs(3)), 0);
    }

    #[test]
    fn compass_directions() {
        assert_eq!(compass(0.0), "S");
        assert_eq!(compass(90.0), "W");
        assert_eq!(compass(180.0), "N");
        assert_eq!(compass(270.0), "E");
        assert_eq!(compass(-90.0), "E");
    }

    #[test]
    fn default_layout_has_all_kinds() {
        let l = HudLayout::default();
        assert_eq!(l.widgets.len(), HudKind::ALL.len());
    }
}
