//! Cosmetics & network layer.
//!
//! * **Universal skin** – the skin is uploaded to Mojang's official profile API
//!   (`POST /minecraft/profile/skins`). Because it is stored on the Minecraft account itself,
//!   *every* player (vanilla, Lunar, Fabric ... ) sees it. No Frost client is required.
//! * **Frost-exclusive capes** – the PNG goes to the Frost backend, not to Mojang. The game agent
//!   asks the backend which players have a cape and only Frost clients render it, so vanilla
//!   players never see it.
//! * **Tab snowflake** – the backend's presence registry tells the agent which players are
//!   Frost users; the agent prefixes their tab-list name with [`TAB_PREFIX`].
//!
//! Frost backend authentication re-uses Mojang's session server: the backend hands out a random
//! `server_id`, the client calls `session/minecraft/join` with its access token and the backend
//! verifies it with `hasJoined`. The Minecraft access token itself never leaves the client.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::auth::Account;

pub const SNOWFLAKE: char = '\u{2744}'; // ❄
pub const TAB_PREFIX: &str = "\u{2744} ";

const MAX_IMAGE_BYTES: usize = 1024 * 1024;
const SKIN_API: &str = "https://api.minecraftservices.com/minecraft/profile/skins";
const SESSION_JOIN: &str = "https://sessionserver.mojang.com/session/minecraft/join";

/// Name shown in the tab list for a player.
pub fn tab_display_name(name: &str, is_frost_user: bool) -> String {
    if is_frost_user {
        format!("{TAB_PREFIX}{name}")
    } else {
        name.to_owned()
    }
}

// ------------------------------------------------------------------------------------------------
// Image validation
// ------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageInfo {
    pub width: u32,
    pub height: u32,
}

fn decode_png(bytes: &[u8]) -> Result<ImageInfo> {
    if bytes.len() > MAX_IMAGE_BYTES {
        bail!("image too large ({} KiB, max 1024 KiB)", bytes.len() / 1024);
    }
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
        .context("not a valid PNG file")?;
    Ok(ImageInfo { width: img.width(), height: img.height() })
}

/// Skins: 64x64 (modern) or 64x32 (legacy), also integer multiples for HD skins.
pub fn validate_skin_png(bytes: &[u8]) -> Result<ImageInfo> {
    let info = decode_png(bytes)?;
    let ok = info.width % 64 == 0
        && (info.height == info.width || info.height * 2 == info.width)
        && info.width <= 1024;
    if !ok {
        bail!("skin must be 64x64 or 64x32 (got {}x{})", info.width, info.height);
    }
    Ok(info)
}

/// Capes: 64x32 texture layout (also 2x/4x/8x) or the classic 22x17 cape area.
pub fn validate_cape_png(bytes: &[u8]) -> Result<ImageInfo> {
    let info = decode_png(bytes)?;
    let atlas = info.width % 64 == 0 && info.height * 2 == info.width && info.width <= 1024;
    let classic = info.width % 22 == 0 && info.height % 17 == 0 && info.width / 22 == info.height / 17;
    if !(atlas || classic) {
        bail!("cape must be 64x32 or 22x17 (got {}x{})", info.width, info.height);
    }
    Ok(info)
}

// ------------------------------------------------------------------------------------------------
// Universal skin changer (Mojang API)
// ------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SkinVariant {
    Classic,
    Slim,
}

impl SkinVariant {
    fn as_str(self) -> &'static str {
        match self {
            SkinVariant::Classic => "classic",
            SkinVariant::Slim => "slim",
        }
    }
}

pub async fn upload_skin(
    http: &reqwest::Client,
    account: &Account,
    png: Vec<u8>,
    variant: SkinVariant,
) -> Result<()> {
    validate_skin_png(&png)?;
    let part = reqwest::multipart::Part::bytes(png)
        .file_name("skin.png")
        .mime_str("image/png")?;
    let form = reqwest::multipart::Form::new()
        .text("variant", variant.as_str())
        .part("file", part);
    let resp = http
        .post(SKIN_API)
        .bearer_auth(&account.mc_access_token)
        .multipart(form)
        .send()
        .await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        bail!("Mojang rejected the skin ({status}): {body}");
    }
    Ok(())
}

// ------------------------------------------------------------------------------------------------
// Frost backend: capes + presence
// ------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CosmeticEntry {
    /// Player is currently using Frost Client (gets the tab snowflake).
    pub frost: bool,
    pub cape_url: Option<String>,
    pub cape_hash: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Challenge {
    server_id: String,
}

#[derive(Debug, Deserialize)]
struct Session {
    token: String,
}

pub struct FrostApi {
    http: reqwest::Client,
    base: String,
    token: Option<String>,
}

impl FrostApi {
    pub fn new(http: reqwest::Client, base: &str) -> Self {
        Self { http, base: base.trim_end_matches('/').to_owned(), token: None }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }

    fn bearer(&self) -> Result<&str> {
        self.token.as_deref().ok_or_else(|| anyhow!("not logged in to the Frost backend"))
    }

    /// Prove account ownership via Mojang's session server and obtain a Frost session token.
    pub async fn login(&mut self, account: &Account) -> Result<()> {
        let challenge: Challenge = self
            .http
            .post(self.url("/v1/auth/challenge"))
            .json(&json!({ "uuid": account.uuid }))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let join = self
            .http
            .post(SESSION_JOIN)
            .json(&json!({
                "accessToken": account.mc_access_token,
                "selectedProfile": account.uuid,
                "serverId": challenge.server_id,
            }))
            .send()
            .await?;
        if !join.status().is_success() {
            bail!("Mojang session join failed: {}", join.status());
        }

        let session: Session = self
            .http
            .post(self.url("/v1/auth/login"))
            .json(&json!({ "username": account.username, "server_id": challenge.server_id }))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        self.token = Some(session.token);
        Ok(())
    }

    /// Upload a Frost-exclusive cape (visible only to other Frost users).
    pub async fn upload_cape(&self, png: Vec<u8>) -> Result<()> {
        validate_cape_png(&png)?;
        let part = reqwest::multipart::Part::bytes(png)
            .file_name("cape.png")
            .mime_str("image/png")?;
        let form = reqwest::multipart::Form::new().part("file", part);
        self.http
            .put(self.url("/v1/cape"))
            .bearer_auth(self.bearer()?)
            .multipart(form)
            .send()
            .await?
            .error_for_status()
            .context("cape upload")?;
        Ok(())
    }

    pub async fn delete_cape(&self) -> Result<()> {
        self.http
            .delete(self.url("/v1/cape"))
            .bearer_auth(self.bearer()?)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Announce "this player is online with Frost" (call every ~60 s while the game runs).
    pub async fn heartbeat(&self) -> Result<()> {
        self.http
            .post(self.url("/v1/presence"))
            .bearer_auth(self.bearer()?)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Look up Frost status + cape for up to 100 players at once.
    pub async fn lookup(&self, uuids: &[String]) -> Result<HashMap<String, CosmeticEntry>> {
        if uuids.is_empty() {
            return Ok(HashMap::new());
        }
        let resp = self
            .http
            .post(self.url("/v1/cosmetics/lookup"))
            .json(&json!({ "uuids": uuids }))
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }
}

/// Short-lived cache so the agent does not hit the backend for every nametag render.
pub struct CosmeticsCache {
    ttl: Duration,
    entries: HashMap<String, (Instant, CosmeticEntry)>,
}

impl CosmeticsCache {
    pub fn new(ttl: Duration) -> Self {
        Self { ttl, entries: HashMap::new() }
    }

    pub fn get(&self, uuid: &str) -> Option<&CosmeticEntry> {
        self.entries
            .get(uuid)
            .filter(|(at, _)| at.elapsed() < self.ttl)
            .map(|(_, e)| e)
    }

    /// UUIDs that are unknown or stale and should be looked up.
    pub fn missing<'a>(&self, uuids: impl IntoIterator<Item = &'a String>) -> Vec<String> {
        uuids.into_iter().filter(|u| self.get(u).is_none()).cloned().collect()
    }

    pub fn insert_all(&mut self, map: HashMap<String, CosmeticEntry>) {
        let now = Instant::now();
        for (k, v) in map {
            self.entries.insert(k, (now, v));
        }
    }

    pub fn snapshot(&self) -> HashMap<String, CosmeticEntry> {
        self.entries
            .iter()
            .filter(|(_, (at, _))| at.elapsed() < self.ttl)
            .map(|(k, (_, e))| (k.clone(), e.clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbaImage::new(w, h);
        let mut out = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
            .unwrap();
        out
    }

    #[test]
    fn tab_name_gets_snowflake_only_for_frost_users() {
        assert_eq!(tab_display_name("Steve", true), "\u{2744} Steve");
        assert_eq!(tab_display_name("Steve", false), "Steve");
    }

    #[test]
    fn skin_validation() {
        assert!(validate_skin_png(&png(64, 64)).is_ok());
        assert!(validate_skin_png(&png(64, 32)).is_ok());
        assert!(validate_skin_png(&png(128, 128)).is_ok());
        assert!(validate_skin_png(&png(100, 50)).is_err());
        assert!(validate_skin_png(b"not a png").is_err());
    }

    #[test]
    fn cape_validation() {
        assert!(validate_cape_png(&png(64, 32)).is_ok());
        assert!(validate_cape_png(&png(22, 17)).is_ok());
        assert!(validate_cape_png(&png(44, 34)).is_ok());
        assert!(validate_cape_png(&png(64, 64)).is_err());
    }

    #[test]
    fn cache_reports_missing_and_expires() {
        let mut c = CosmeticsCache::new(Duration::from_millis(30));
        let a = "a".to_string();
        let b = "b".to_string();
        assert_eq!(c.missing([&a, &b]).len(), 2);
        let mut m = HashMap::new();
        m.insert("a".to_string(), CosmeticEntry { frost: true, ..Default::default() });
        c.insert_all(m);
        assert_eq!(c.missing([&a, &b]), vec!["b".to_string()]);
        assert!(c.get("a").unwrap().frost);
        std::thread::sleep(Duration::from_millis(40));
        assert!(c.get("a").is_none());
    }
}
