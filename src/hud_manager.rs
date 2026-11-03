//! HUD data model shared between the launcher UI and the in-game agent.
//!
//! The in-game agent streams [`GameStats`] over the IPC bridge and receives the saved
//! [`HudLayout`] + [`QolSettings`] from the launcher. Rendering happens in the launcher's web UI
//! (preview) and in the Java agent (in game).

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};


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
    BwResources,
    BwBlocks,
    BridgeSpeed,
}

impl HudKind {
    pub const ALL: [HudKind; 12] = [
        HudKind::Cps,
        HudKind::Fps,
        HudKind::Ping,
        HudKind::Keystrokes,
        HudKind::ArmorStatus,
        HudKind::PotionStatus,
        HudKind::Direction,
        HudKind::ReachDisplay,
        HudKind::Coordinates,
        HudKind::BwResources,
        HudKind::BwBlocks,
        HudKind::BridgeSpeed,
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
            HudKind::BwResources => "Bedwars resources",
            HudKind::BwBlocks => "Bedwars blocks",
            HudKind::BridgeSpeed => "Bridge speed",
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
    /// Turn clouds off (cheap frames in big views).
    pub no_clouds: bool,
    /// Disable entity shadow blobs.
    pub no_entity_shadows: bool,
    /// Biome colour blending radius 1 instead of up to 7x7 (cheaper chunk meshing).
    pub low_biome_blend: bool,
    /// Cap the simulation distance (entity / block-tick range).
    pub cap_sim_distance: bool,
    pub sim_distance_cap: u32,
    /// Remove the blurred menu background.
    pub no_menu_blur: bool,
    /// Fast graphics / flat lighting (1.8.9 only; modern versions use Sodium via the performance pack).
    pub fast_graphics: bool,
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
            no_clouds: false,
            no_entity_shadows: true,
            low_biome_blend: true,
            cap_sim_distance: true,
            sim_distance_cap: 8,
            no_menu_blur: true,
            fast_graphics: true,
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
    pub bw_iron: u32,
    pub bw_gold: u32,
    pub bw_diamond: u32,
    pub bw_emerald: u32,
    pub bw_blocks: u32,
    pub bridge_bps: f32,
    pub bridge_best: f32,
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
            bw_iron: 64,
            bw_gold: 12,
            bw_diamond: 3,
            bw_emerald: 1,
            bw_blocks: 96,
            bridge_bps: 7.4,
            bridge_best: 9.1,
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
