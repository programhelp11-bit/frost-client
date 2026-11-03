use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::hud_manager::{HudLayout, QolSettings};

/// Root data directory (`%APPDATA%/FrostClient` or `~/.local/share/FrostClient`).
pub fn data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("FrostClient")
}

/// Seconds since the Unix epoch.
pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn settings_path() -> PathBuf {
    data_dir().join("settings.json")
}

/// Tuned G1 flags for a game client: short pauses, early concurrent cycles, young-gen headroom.
pub const OPTIMIZED_JVM_ARGS: &str = "-XX:+UseG1GC -XX:+ParallelRefProcEnabled -XX:MaxGCPauseMillis=40 \
-XX:+UnlockExperimentalVMOptions -XX:+DisableExplicitGC -XX:G1NewSizePercent=30 -XX:G1MaxNewSizePercent=40 \
-XX:G1HeapRegionSize=8M -XX:G1ReservePercent=20 -XX:G1HeapWastePercent=5 -XX:G1MixedGCCountTarget=4 \
-XX:InitiatingHeapOccupancyPercent=15 -XX:G1MixedGCLiveThresholdPercent=90 -XX:G1RSetUpdatingPauseTimePercent=5 \
-XX:SurvivorRatio=32 -XX:+PerfDisableSharedMem -XX:MaxTenuringThreshold=1";

/// The flags older builds shipped; replaced by [`OPTIMIZED_JVM_ARGS`] when found unchanged.
const OLD_DEFAULT_JVM_ARGS: &str = "-XX:+UseG1GC -XX:MaxGCPauseMillis=50 -XX:+DisableExplicitGC";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub selected_version: String,
    /// Maximum heap in MiB.
    pub ram_mb: u32,
    /// Extra JVM arguments, whitespace separated.
    pub jvm_args: String,
    /// Empty = auto (JAVA_HOME / PATH).
    pub java_path: String,
    /// Frost cosmetics backend (capes, presence).
    pub frost_api: String,
    /// Optional override of the built-in Azure application id (normally empty: the id is
    /// compiled in from FROST_MS_CLIENT_ID so users never have to configure anything).
    pub ms_client_id: String,
    pub ipc_port: u16,
    pub hud: HudLayout,
    pub qol: QolSettings,
    /// Unix seconds of the last launch per Minecraft version.
    pub last_played: HashMap<String, u64>,
    /// What the launcher window does while the game runs: "keep" | "hide".
    pub while_playing: String,
    /// Falling snowflakes in the launcher background.
    pub snow: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            selected_version: "1.21.11".into(),
            ram_mb: 3072,
            jvm_args: OPTIMIZED_JVM_ARGS.split_whitespace().collect::<Vec<_>>().join(" "),
            java_path: String::new(),
            frost_api: option_env!("FROST_API_URL").unwrap_or("").trim().into(),
            ms_client_id: String::new(),
            ipc_port: 25585,
            hud: HudLayout::default(),
            qol: QolSettings::default(),
            last_played: HashMap::new(),
            while_playing: "keep".into(),
            snow: true,
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        let mut s: Settings = std::fs::read_to_string(settings_path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        if s.frost_api.trim().is_empty() || s.frost_api.contains("frostclient.example") {
            s.frost_api = Settings::default().frost_api;
        }
        if s.jvm_args.trim() == OLD_DEFAULT_JVM_ARGS {
            s.jvm_args = Settings::default().jvm_args;
        }
        s
    }

    pub fn save(&self) -> Result<()> {
        let path = settings_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).context("create data dir")?;
        }
        std::fs::write(&path, serde_json::to_vec_pretty(self)?).context("write settings")?;
        Ok(())
    }
}
