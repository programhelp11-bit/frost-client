//! Microsoft authentication for Minecraft: Java Edition.
//!
//! Flow (official, no password ever touches Frost):
//! 1. OAuth2 *device code* flow against `login.microsoftonline.com` (user enters a code in the browser)
//! 2. Xbox Live user token (XBL)
//! 3. XSTS token for `rp://api.minecraftservices.com/`
//! 4. `login_with_xbox` -> Minecraft access token
//! 5. Profile lookup (uuid + name)
//!
//! You must register your own Azure application (public client, "Mobile and desktop
//! applications", device-code enabled, permission `XboxLive.signin`) and put its
//! client id in Settings or in the `FROST_MS_CLIENT_ID` environment variable.

use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::config::data_dir;

const DEVICE_CODE_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
const TOKEN_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const XBL_URL: &str = "https://user.auth.xboxlive.com/user/authenticate";
const XSTS_URL: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
const MC_LOGIN_URL: &str = "https://api.minecraftservices.com/authentication/login_with_xbox";
const MC_PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";
const SCOPE: &str = "XboxLive.signin offline_access";

#[derive(Debug, Clone, Deserialize)]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    #[serde(default = "default_interval")]
    pub interval: u64,
}

fn default_interval() -> u64 {
    5
}

#[derive(Debug, Clone, Deserialize)]
pub struct MsTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub username: String,
    /// Undashed UUID as returned by Mojang.
    pub uuid: String,
    pub mc_access_token: String,
    pub mc_expires_at: u64,
    pub ms_refresh_token: Option<String>,
}

impl Account {
    pub fn is_expired(&self) -> bool {
        now_secs() + 60 >= self.mc_expires_at
    }
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[derive(Debug, Deserialize)]
struct TokenError {
    error: String,
    error_description: Option<String>,
}

/// Step 1a: request a device code; show `user_code` + `verification_uri` to the user.
pub async fn start_device_flow(http: &reqwest::Client, client_id: &str) -> Result<DeviceCode> {
    if client_id.trim().is_empty() {
        bail!("Microsoft client id is empty – set it in Settings or FROST_MS_CLIENT_ID");
    }
    let resp = http
        .post(DEVICE_CODE_URL)
        .form(&[("client_id", client_id), ("scope", SCOPE)])
        .send()
        .await?;
    if !resp.status().is_success() {
        let text = resp.text().await.unwrap_or_default();
        bail!("device code request failed: {text}");
    }
    Ok(resp.json().await?)
}

/// Step 1b: poll until the user finished signing in (or the code expired).
pub async fn poll_device_flow(
    http: &reqwest::Client,
    client_id: &str,
    code: &DeviceCode,
) -> Result<MsTokens> {
    let deadline = std::time::Instant::now() + Duration::from_secs(code.expires_in);
    let mut interval = code.interval.max(1);
    loop {
        if std::time::Instant::now() > deadline {
            bail!("sign-in code expired");
        }
        tokio::time::sleep(Duration::from_secs(interval)).await;
        let resp = http
            .post(TOKEN_URL)
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("client_id", client_id),
                ("device_code", code.device_code.as_str()),
            ])
            .send()
            .await?;
        if resp.status().is_success() {
            return Ok(resp.json().await?);
        }
        let err: TokenError = resp.json().await.context("parse token error")?;
        match err.error.as_str() {
            "authorization_pending" => {}
            "slow_down" => interval += 5,
            other => bail!(
                "sign-in failed: {other} {}",
                err.error_description.unwrap_or_default()
            ),
        }
    }
}

async fn refresh_ms(http: &reqwest::Client, client_id: &str, refresh: &str) -> Result<MsTokens> {
    let resp = http
        .post(TOKEN_URL)
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", client_id),
            ("refresh_token", refresh),
            ("scope", SCOPE),
        ])
        .send()
        .await?;
    if !resp.status().is_success() {
        bail!("refresh failed: {}", resp.text().await.unwrap_or_default());
    }
    Ok(resp.json().await?)
}

#[derive(Debug, Deserialize)]
struct XboxResponse {
    #[serde(rename = "Token")]
    token: String,
    #[serde(rename = "DisplayClaims")]
    display_claims: DisplayClaims,
}

#[derive(Debug, Deserialize)]
struct DisplayClaims {
    xui: Vec<Xui>,
}

#[derive(Debug, Deserialize)]
struct Xui {
    uhs: String,
}

async fn xbox_post(
    http: &reqwest::Client,
    url: &str,
    body: serde_json::Value,
) -> Result<XboxResponse> {
    let resp = http
        .post(url)
        .header("Accept", "application/json")
        .json(&body)
        .send()
        .await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        // 2148916233 = no Xbox account, 2148916238 = child account without family
        bail!("Xbox request failed ({status}): {text}");
    }
    Ok(resp.json().await?)
}

#[derive(Debug, Deserialize)]
struct McLogin {
    access_token: String,
    expires_in: u64,
}

#[derive(Debug, Deserialize)]
struct McProfile {
    id: String,
    name: String,
}

/// Steps 2-5: exchange a Microsoft access token for a Minecraft session.
pub async fn login_with_ms(http: &reqwest::Client, ms: &MsTokens) -> Result<Account> {
    let xbl = xbox_post(
        http,
        XBL_URL,
        json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": format!("d={}", ms.access_token),
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT",
        }),
    )
    .await
    .context("Xbox Live authentication")?;

    let xsts = xbox_post(
        http,
        XSTS_URL,
        json!({
            "Properties": { "SandboxId": "RETAIL", "UserTokens": [xbl.token] },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT",
        }),
    )
    .await
    .context("XSTS authorization")?;

    let uhs = xsts
        .display_claims
        .xui
        .first()
        .ok_or_else(|| anyhow!("XSTS response without user hash"))?
        .uhs
        .clone();

    let mc: McLogin = http
        .post(MC_LOGIN_URL)
        .json(&json!({ "identityToken": format!("XBL3.0 x={uhs};{}", xsts.token) }))
        .send()
        .await?
        .error_for_status()
        .context("Minecraft login_with_xbox")?
        .json()
        .await?;

    let profile_resp = http
        .get(MC_PROFILE_URL)
        .bearer_auth(&mc.access_token)
        .send()
        .await?;
    if profile_resp.status() == reqwest::StatusCode::NOT_FOUND {
        bail!("this Microsoft account does not own Minecraft: Java Edition");
    }
    let profile: McProfile = profile_resp.error_for_status()?.json().await?;

    Ok(Account {
        username: profile.name,
        uuid: profile.id,
        mc_access_token: mc.access_token,
        mc_expires_at: now_secs() + mc.expires_in,
        ms_refresh_token: ms.refresh_token.clone(),
    })
}

/// Silent re-login using the stored Microsoft refresh token.
pub async fn refresh(http: &reqwest::Client, client_id: &str, acc: &Account) -> Result<Account> {
    let refresh = acc
        .ms_refresh_token
        .as_deref()
        .ok_or_else(|| anyhow!("no refresh token stored – sign in again"))?;
    let ms = refresh_ms(http, client_id, refresh).await?;
    let mut fresh = login_with_ms(http, &ms).await?;
    if fresh.ms_refresh_token.is_none() {
        fresh.ms_refresh_token = acc.ms_refresh_token.clone();
    }
    Ok(fresh)
}

/// Accounts persisted to `accounts.json` (file mode 0600 on Unix).
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AccountStore {
    pub accounts: Vec<Account>,
    pub active: Option<String>,
}

impl AccountStore {
    fn path() -> PathBuf {
        data_dir().join("accounts.json")
    }

    pub fn load() -> Self {
        std::fs::read_to_string(Self::path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, serde_json::to_vec_pretty(self)?)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    pub fn upsert(&mut self, acc: Account) {
        self.active = Some(acc.uuid.clone());
        match self.accounts.iter_mut().find(|a| a.uuid == acc.uuid) {
            Some(slot) => *slot = acc,
            None => self.accounts.push(acc),
        }
    }

    pub fn active(&self) -> Option<&Account> {
        let id = self.active.as_ref()?;
        self.accounts.iter().find(|a| &a.uuid == id)
    }

    pub fn remove_active(&mut self) {
        if let Some(id) = self.active.take() {
            self.accounts.retain(|a| a.uuid != id);
            self.active = self.accounts.first().map(|a| a.uuid.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn acc(id: &str) -> Account {
        Account {
            username: id.into(),
            uuid: id.into(),
            mc_access_token: "t".into(),
            mc_expires_at: 0,
            ms_refresh_token: None,
        }
    }

    #[test]
    fn store_upsert_and_remove() {
        let mut s = AccountStore::default();
        s.upsert(acc("a"));
        s.upsert(acc("b"));
        assert_eq!(s.active().unwrap().uuid, "b");
        s.remove_active();
        assert_eq!(s.active().unwrap().uuid, "a");
    }

    #[test]
    fn expiry_check() {
        assert!(acc("x").is_expired());
    }
}
