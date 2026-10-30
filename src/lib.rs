//! Frost Client – launcher + client runtime library.
//!
//! Module overview:
//! - [`config`]       persistent settings & paths
//! - [`versions`]     Mojang manifest / version-JSON model, supported versions
//! - [`downloader`]   parallel, hash-verified downloads
//! - [`auth`]         Microsoft device-code -> XBL -> XSTS -> Minecraft login
//! - [`launch`]       classpath / argument assembly and process spawn
//! - [`cosmetics`]    universal skin changer, Frost-only capes, tab snowflake
//! - [`hud_manager`]  liquid-glass HUD model + egui renderer + layout editor
//! - [`trainer`]      PvP bot brains + knockback trainer
//! - [`ipc`]          local JSON-lines bridge between launcher and in-game agent
//! - [`glass`]        liquid-glass theme helpers
//! - [`ui`]           the launcher window

pub mod auth;
pub mod config;
pub mod cosmetics;
pub mod downloader;
pub mod glass;
pub mod hud_manager;
pub mod ipc;
pub mod launch;
pub mod trainer;
pub mod ui;
pub mod versions;
