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

// ------------------------------------------------------------------------------------------------
// One-click browser login (authorization code + PKCE, loopback redirect)
// ------------------------------------------------------------------------------------------------

const AUTHORIZE_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize";

/// The Azure application id baked into the build (`FROST_MS_CLIENT_ID` at compile time), so end
/// users never configure anything. A non-empty settings value or a runtime env var overrides it.
/// Azure application ids are GUIDs (8-4-4-4-12 hex digits).
pub fn is_valid_client_id(id: &str) -> bool {
    let parts: Vec<&str> = id.split('-').collect();
    parts.len() == 5
        && [8usize, 4, 4, 4, 12].iter().zip(&parts).all(|(n, p)| p.len() == *n && p.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Every configured candidate in priority order: settings override, compiled-in id, env var.
fn client_id_candidates(settings_override: &str) -> Vec<String> {
    let mut v = vec![settings_override.trim().to_owned()];
    if let Some(id) = option_env!("FROST_MS_CLIENT_ID") {
        v.push(id.trim().to_owned());
    }
    if let Ok(id) = std::env::var("FROST_MS_CLIENT_ID") {
        v.push(id.trim().to_owned());
    }
    v.retain(|s| !s.is_empty());
    v
}

/// The Azure application id baked into the build (`FROST_MS_CLIENT_ID` at compile time), so end
/// users never configure anything. A non-empty settings value or a runtime env var overrides it.
/// Only well-formed GUIDs are accepted; anything else behaves as "not configured".
pub fn effective_client_id(settings_override: &str) -> Option<String> {
    client_id_candidates(settings_override).into_iter().find(|c| is_valid_client_id(c))
}

/// A client id is configured but is not a valid GUID (e.g. someone typed an app *name*).
pub fn invalid_client_id(settings_override: &str) -> Option<String> {
    if effective_client_id(settings_override).is_some() {
        return None;
    }
    client_id_candidates(settings_override).into_iter().next()
}

/// RFC 7636 S256 code challenge.
pub fn pkce_challenge(verifier: &str) -> String {
    use base64::Engine as _;
    use sha2::{Digest, Sha256};
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn random_string(len: usize) -> String {
    use rand::distributions::Alphanumeric;
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..len).map(|_| rng.sample(Alphanumeric) as char).collect()
}

#[derive(Debug, PartialEq, Eq)]
pub struct Callback {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

/// Parse the request line of the redirect, e.g. `GET /?code=abc&state=xyz HTTP/1.1`.
/// Returns `None` for unrelated requests (favicon, probes).
pub fn parse_callback(request_line: &str) -> Option<Callback> {
    let path = request_line.split_whitespace().nth(1)?;
    let url = reqwest::Url::parse(&format!("http://localhost{path}")).ok()?;
    let mut cb = Callback { code: None, state: None, error: None };
    for (k, v) in url.query_pairs() {
        match k.as_ref() {
            "code" => cb.code = Some(v.into_owned()),
            "state" => cb.state = Some(v.into_owned()),
            "error" => cb.error = Some(v.into_owned()),
            _ => {}
        }
    }
    if cb.code.is_some() || cb.error.is_some() {
        Some(cb)
    } else {
        None
    }
}

async fn respond(sock: &mut tokio::net::TcpStream, status: &str, title: &str, text: &str) {
    use tokio::io::AsyncWriteExt;
    let body = format!(
        "<!doctype html><html><head><meta charset=utf-8><title>Frost Client</title></head>\
         <body style=\"margin:0;height:100vh;display:flex;align-items:center;justify-content:center;\
         background:radial-gradient(circle at 30% 20%,#1d3557,#0b1020);font-family:system-ui,sans-serif;color:#eaf2ff\">\
         <div style=\"text-align:center;padding:40px 56px;border-radius:24px;background:rgba(255,255,255,.08);\
         border:1px solid rgba(255,255,255,.2)\"><div style=\"font-size:48px\">&#10052;</div>\
         <h1 style=\"margin:8px 0\">{title}</h1><p style=\"color:#a0b0c8\">{text}</p></div></body></html>"
    );
    let resp = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = sock.write_all(resp.as_bytes()).await;
    let _ = sock.shutdown().await;
}

async fn wait_for_code(listener: &tokio::net::TcpListener, state: &str) -> Result<String> {
    use tokio::io::AsyncReadExt;
    loop {
        let (mut sock, _) = listener.accept().await?;
        let mut buf = vec![0u8; 8192];
        let n = sock.read(&mut buf).await.unwrap_or(0);
        let req = String::from_utf8_lossy(&buf[..n]).into_owned();
        let line = req.lines().next().unwrap_or("");
        match parse_callback(line) {
            Some(cb) => {
                if let Some(err) = cb.error {
                    respond(&mut sock, "400 Bad Request", "Sign-in cancelled", "You can close this tab.").await;
                    bail!("Microsoft sign-in failed: {err}");
                }
                if cb.state.as_deref() != Some(state) {
                    respond(&mut sock, "400 Bad Request", "Sign-in failed", "State mismatch.").await;
                    bail!("sign-in state mismatch");
                }
                respond(&mut sock, "200 OK", "Signed in", "You can close this tab and return to Frost Client.").await;
                return cb.code.ok_or_else(|| anyhow!("no code in redirect"));
            }
            None => respond(&mut sock, "404 Not Found", "Not found", "").await,
        }
    }
}

/// Open the Microsoft sign-in page in the user's browser and wait for the redirect.
/// `open_url` is called once with the authorization URL.
pub async fn browser_login(
    http: &reqwest::Client,
    client_id: &str,
    open_url: impl FnOnce(&str),
) -> Result<MsTokens> {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.context("bind loopback")?;
    let port = listener.local_addr()?.port();
    let redirect = format!("http://localhost:{port}");
    let verifier = random_string(64);
    let challenge = pkce_challenge(&verifier);
    let state = random_string(24);

    let url = reqwest::Url::parse_with_params(
        AUTHORIZE_URL,
        &[
            ("client_id", client_id),
            ("response_type", "code"),
            ("redirect_uri", redirect.as_str()),
            ("scope", SCOPE),
            ("code_challenge", challenge.as_str()),
            ("code_challenge_method", "S256"),
            ("state", state.as_str()),
            ("response_mode", "query"),
            ("prompt", "select_account"),
        ],
    )?;
    open_url(url.as_str());

    let code = tokio::time::timeout(Duration::from_secs(300), wait_for_code(&listener, &state))
        .await
        .map_err(|_| anyhow!("sign-in timed out"))??;

    let resp = http
        .post(TOKEN_URL)
        .form(&[
            ("client_id", client_id),
            ("grant_type", "authorization_code"),
            ("code", code.as_str()),
            ("redirect_uri", redirect.as_str()),
            ("code_verifier", verifier.as_str()),
            ("scope", SCOPE),
        ])
        .send()
        .await?;
    if !resp.status().is_success() {
        bail!("token exchange failed: {}", resp.text().await.unwrap_or_default());
    }
    Ok(resp.json().await?)
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

    #[test]
    fn client_id_must_be_a_guid() {
        assert!(is_valid_client_id("3f2c1a9e-8b4d-4c1e-9a77-0123456789ab"));
        assert!(!is_valid_client_id("FROST"));
        assert!(!is_valid_client_id(""));
        assert!(!is_valid_client_id("3f2c1a9e-8b4d-4c1e-9a77-0123456789a"));
    }

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
    fn pkce_matches_rfc7636_vector() {
        assert_eq!(
            pkce_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn callback_parsing() {
        let cb = parse_callback("GET /?code=abc%20d&state=xyz HTTP/1.1").unwrap();
        assert_eq!(cb.code.as_deref(), Some("abc d"));
        assert_eq!(cb.state.as_deref(), Some("xyz"));
        assert!(parse_callback("GET /favicon.ico HTTP/1.1").is_none());
        let err = parse_callback("GET /?error=access_denied HTTP/1.1").unwrap();
        assert_eq!(err.error.as_deref(), Some("access_denied"));
    }

    #[test]
    fn settings_override_wins() {
        assert_eq!(effective_client_id(" my-id ").as_deref(), Some("my-id"));
    }

    #[test]
    fn expiry_check() {
        assert!(acc("x").is_expired());
    }
}
