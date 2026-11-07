//! Frost Client – launcher + client runtime library.
//!
//! Module overview:
//! - [`config`]       persistent settings & paths
//! - [`versions`]     Mojang manifest / version-JSON model, supported versions
//! - [`downloader`]   parallel, hash-verified downloads
//! - [`auth`]         Microsoft device-code -> XBL -> XSTS -> Minecraft login
//! - [`launch`]       classpath / argument assembly and process spawn
//! - [`cosmetics`]    universal skin changer, Frost-only capes, tab snowflake
//! - [`hud_manager`]  HUD / QoL data model
//! - [`trainer`]      PvP bot brains + knockback trainer
//! - [`ipc`]          local JSON-lines bridge between launcher and in-game agent

pub mod auth;
pub mod botlab;
pub mod config;
pub mod cosmetics;
pub mod downloader;
pub mod hud_manager;
pub mod modloader;
pub mod modrinth;
pub mod ipc;
pub mod java;
pub mod launch;
pub mod library;
pub mod trainer;
pub mod versions;
