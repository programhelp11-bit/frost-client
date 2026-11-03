//! Frost Client cosmetics backend.
//!
//! * Players prove who they are through Mojang's session server (`join` on the client,
//!   `hasJoined` here), so nobody can act as somebody else and no Minecraft token reaches us.
//! * Community capes: anybody signed in can upload a cape; everyone can pick any of them.
//!   The built-in Frost cape (`frost`) is free for every Frost player and is the default.
//! * Presence + lookup: the launcher heartbeats while the game runs and asks which of the
//!   players in the tab list are Frost users, which cape they picked and their skin revision.
//!
//! Configuration (environment): `PORT` (8080), `DATA_DIR` (./data), `ADMIN_TOKEN` (optional,
//! allows deleting any cape via the `x-admin-token` header).

use std::collections::HashMap;
use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::extract::{DefaultBodyLimit, Multipart, Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::Mutex;

const FROST_CAPE: &[u8] = include_bytes!("../assets/frost_cape.png");
const FROST_ID: &str = "frost";
const NO_CAPE: &str = "none";
const ONLINE_SECS: u64 = 180;
const MAX_CAPE_BYTES: usize = 256 * 1024;
const MAX_PER_AUTHOR: usize = 20;
const UPLOAD_COOLDOWN: Duration = Duration::from_secs(20);
const CHALLENGE_TTL: Duration = Duration::from_secs(120);
const TOKEN_TTL: Duration = Duration::from_secs(6 * 3600);

type ApiErr = (StatusCode, String);

fn err<S: Into<String>>(code: StatusCode, msg: S) -> ApiErr {
    (code, msg.into())
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn sha1_hex(bytes: &[u8]) -> String {
    use sha1::{Digest, Sha1};
    let digest = Sha1::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

// ------------------------------------------------------------------------------------------------
// Validation
// ------------------------------------------------------------------------------------------------

/// Capes: 64x32 atlas (also 2x..16x) or the classic 22x17 area (and multiples).
pub fn validate_cape_png(bytes: &[u8]) -> Result<(u32, u32), String> {
    if bytes.len() > MAX_CAPE_BYTES {
        return Err(format!("cape too large ({} KiB, max {} KiB)", bytes.len() / 1024, MAX_CAPE_BYTES / 1024));
    }
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
        .map_err(|_| "not a valid PNG".to_owned())?;
    let (w, h) = (img.width(), img.height());
    let atlas = w % 64 == 0 && h * 2 == w && w <= 1024;
    let classic = w % 22 == 0 && h % 17 == 0 && w / 22 == h / 17 && w <= 704;
    if atlas || classic {
        Ok((w, h))
    } else {
        Err(format!("cape must be 64x32 or 22x17 (got {w}x{h})"))
    }
}

fn clean_name(raw: &str) -> Result<String, String> {
    let name: String = raw.trim().chars().filter(|c| !c.is_control()).collect();
    let len = name.chars().count();
    if !(3..=24).contains(&len) {
        return Err("cape name must be 3-24 characters".into());
    }
    Ok(name)
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 32 && id.chars().all(|c| c.is_ascii_alphanumeric())
}

// ------------------------------------------------------------------------------------------------
// Data model
// ------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapeRec {
    pub id: String,
    pub name: String,
    pub author_uuid: String,
    pub author_name: String,
    pub created: u64,
    pub hash: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Player {
    pub name: String,
    /// `None` = default (the free Frost cape), `Some("none")` = no cape, otherwise a cape id.
    pub cape: Option<String>,
    pub last_seen: u64,
    pub skin_rev: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Entry {
    pub frost: bool,
    pub cape_url: Option<String>,
    pub cape_hash: Option<String>,
    pub skin_rev: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Db {
    pub capes: Vec<CapeRec>,
    pub players: HashMap<String, Player>,
}

impl Db {
    fn builtin_hash() -> String {
        sha1_hex(FROST_CAPE)
    }

    pub fn cape_exists(&self, id: &str) -> bool {
        id == FROST_ID || self.capes.iter().any(|c| c.id == id)
    }

    pub fn effective_cape(&self, p: &Player) -> Option<String> {
        match p.cape.as_deref() {
            None => Some(FROST_ID.to_owned()),
            Some(NO_CAPE) => None,
            Some(id) if self.cape_exists(id) => Some(id.to_owned()),
            Some(_) => Some(FROST_ID.to_owned()), // the chosen cape was deleted
        }
    }

    pub fn cape_hash(&self, id: &str) -> Option<String> {
        if id == FROST_ID {
            return Some(Self::builtin_hash());
        }
        self.capes.iter().find(|c| c.id == id).map(|c| c.hash.clone())
    }

    pub fn users_of(&self, id: &str) -> usize {
        self.players.values().filter(|p| self.effective_cape(p).as_deref() == Some(id)).count()
    }

    /// Frost status of the given players; only players with a recent heartbeat count.
    pub fn lookup(&self, uuids: &[String], now: u64) -> HashMap<String, Entry> {
        let mut out = HashMap::new();
        for uuid in uuids.iter().take(100) {
            let Some(p) = self.players.get(uuid) else { continue };
            if p.last_seen + ONLINE_SECS < now {
                continue;
            }
            let cape = self.effective_cape(p);
            out.insert(
                uuid.clone(),
                Entry {
                    frost: true,
                    cape_url: cape.as_ref().map(|id| format!("/v1/capes/{id}.png")),
                    cape_hash: cape.as_ref().and_then(|id| self.cape_hash(id)),
                    skin_rev: p.skin_rev,
                },
            );
        }
        out
    }
}

// ------------------------------------------------------------------------------------------------
// App state
// ------------------------------------------------------------------------------------------------

#[derive(Default)]
struct Core {
    db: Db,
    challenges: HashMap<String, (String, Instant)>,
    tokens: HashMap<String, (String, String, Instant)>,
    last_upload: HashMap<String, Instant>,
}

#[derive(Clone)]
struct App {
    core: Arc<Mutex<Core>>,
    data_dir: Arc<PathBuf>,
    http: reqwest::Client,
    admin: Arc<Option<String>>,
}

impl App {
    fn capes_dir(&self) -> PathBuf {
        self.data_dir.join("capes")
    }

    async fn save(&self) {
        let json = {
            let core = self.core.lock().await;
            serde_json::to_vec_pretty(&core.db)
        };
        if let Ok(bytes) = json {
            let tmp = self.data_dir.join("db.json.tmp");
            if tokio::fs::write(&tmp, bytes).await.is_ok() {
                let _ = tokio::fs::rename(&tmp, self.data_dir.join("db.json")).await;
            }
        }
    }

    async fn auth(&self, headers: &HeaderMap) -> Result<(String, String), ApiErr> {
        let token = headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or_else(|| err(StatusCode::UNAUTHORIZED, "missing token"))?;
        let core = self.core.lock().await;
        match core.tokens.get(token) {
            Some((uuid, name, at)) if at.elapsed() < TOKEN_TTL => Ok((uuid.clone(), name.clone())),
            _ => Err(err(StatusCode::UNAUTHORIZED, "session expired, sign in again")),
        }
    }
}

fn random_hex(bytes: usize) -> String {
    (0..bytes).map(|_| format!("{:02x}", rand::random::<u8>())).collect()
}

// ------------------------------------------------------------------------------------------------
// Handlers
// ------------------------------------------------------------------------------------------------

async fn health() -> &'static str {
    "ok"
}

#[derive(Deserialize)]
struct ChallengeReq {
    uuid: String,
}

async fn challenge(State(app): State<App>, Json(req): Json<ChallengeReq>) -> Result<Json<Value>, ApiErr> {
    let uuid = req.uuid.replace('-', "").to_lowercase();
    if uuid.len() != 32 || !uuid.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(err(StatusCode::BAD_REQUEST, "bad uuid"));
    }
    let server_id = random_hex(10);
    let mut core = app.core.lock().await;
    core.challenges.retain(|_, (_, at)| at.elapsed() < CHALLENGE_TTL);
    core.challenges.insert(server_id.clone(), (uuid, Instant::now()));
    Ok(Json(json!({ "server_id": server_id })))
}

#[derive(Deserialize)]
struct LoginReq {
    username: String,
    server_id: String,
}

async fn login(State(app): State<App>, Json(req): Json<LoginReq>) -> Result<Json<Value>, ApiErr> {
    let uuid = {
        let mut core = app.core.lock().await;
        match core.challenges.remove(&req.server_id) {
            Some((uuid, at)) if at.elapsed() < CHALLENGE_TTL => uuid,
            _ => return Err(err(StatusCode::BAD_REQUEST, "unknown or expired challenge")),
        }
    };
    let resp = app
        .http
        .get("https://sessionserver.mojang.com/session/minecraft/hasJoined")
        .query(&[("username", req.username.as_str()), ("serverId", req.server_id.as_str())])
        .send()
        .await
        .map_err(|e| err(StatusCode::BAD_GATEWAY, format!("Mojang unreachable: {e}")))?;
    if resp.status() != reqwest::StatusCode::OK {
        return Err(err(StatusCode::UNAUTHORIZED, "Mojang did not confirm the session"));
    }
    let profile: Value = resp
        .json()
        .await
        .map_err(|e| err(StatusCode::BAD_GATEWAY, format!("bad Mojang answer: {e}")))?;
    let id = profile["id"].as_str().unwrap_or_default().to_lowercase();
    if id != uuid {
        return Err(err(StatusCode::UNAUTHORIZED, "session belongs to another account"));
    }
    let name = profile["name"].as_str().unwrap_or(&req.username).to_owned();
    let token = random_hex(24);
    let mut core = app.core.lock().await;
    core.tokens.retain(|_, (_, _, at)| at.elapsed() < TOKEN_TTL);
    core.tokens.insert(token.clone(), (uuid.clone(), name.clone(), Instant::now()));
    let p = core.db.players.entry(uuid).or_default();
    p.name = name;
    p.last_seen = now_secs();
    Ok(Json(json!({ "token": token })))
}

#[derive(Deserialize)]
struct ListQuery {
    #[serde(default)]
    offset: usize,
    limit: Option<usize>,
    sort: Option<String>,
}

async fn list_capes(State(app): State<App>, Query(q): Query<ListQuery>) -> Json<Value> {
    let core = app.core.lock().await;
    let db = &core.db;
    let mut items: Vec<Value> = db
        .capes
        .iter()
        .map(|c| {
            json!({
                "id": c.id, "name": c.name, "author": c.author_name, "created": c.created,
                "users": db.users_of(&c.id), "builtin": false, "url": format!("/v1/capes/{}.png", c.id),
            })
        })
        .collect();
    if q.sort.as_deref() == Some("popular") {
        items.sort_by(|a, b| b["users"].as_u64().cmp(&a["users"].as_u64()));
    } else {
        items.sort_by(|a, b| b["created"].as_u64().cmp(&a["created"].as_u64()));
    }
    let total = items.len() + 1;
    let limit = q.limit.unwrap_or(24).clamp(1, 60);
    let mut page: Vec<Value> = Vec::new();
    if q.offset == 0 {
        page.push(json!({
            "id": FROST_ID, "name": "Frost", "author": "Frost Client", "created": 0,
            "users": db.users_of(FROST_ID), "builtin": true, "url": "/v1/capes/frost.png",
        }));
    }
    let skip = q.offset.saturating_sub(1);
    page.extend(items.into_iter().skip(skip).take(limit.saturating_sub(page.len())));
    Json(json!({ "capes": page, "total": total }))
}

async fn cape_png(State(app): State<App>, Path(file): Path<String>) -> Result<Response, ApiErr> {
    let id = file.strip_suffix(".png").unwrap_or(&file);
    let bytes = if id == FROST_ID {
        FROST_CAPE.to_vec()
    } else {
        if !valid_id(id) {
            return Err(err(StatusCode::BAD_REQUEST, "bad id"));
        }
        tokio::fs::read(app.capes_dir().join(format!("{id}.png")))
            .await
            .map_err(|_| err(StatusCode::NOT_FOUND, "no such cape"))?
    };
    Ok(([(header::CONTENT_TYPE, "image/png"), (header::CACHE_CONTROL, "public, max-age=3600")], bytes).into_response())
}

async fn upload_cape(State(app): State<App>, headers: HeaderMap, mut mp: Multipart) -> Result<Json<Value>, ApiErr> {
    let (uuid, author) = app.auth(&headers).await?;
    let mut name = String::new();
    let mut file: Option<Vec<u8>> = None;
    while let Some(field) = mp.next_field().await.map_err(|e| err(StatusCode::BAD_REQUEST, e.to_string()))? {
        let fname = field.name().map(str::to_owned);
        match fname.as_deref() {
            Some("name") => name = field.text().await.map_err(|e| err(StatusCode::BAD_REQUEST, e.to_string()))?,
            Some("file") => {
                file = Some(field.bytes().await.map_err(|e| err(StatusCode::BAD_REQUEST, e.to_string()))?.to_vec())
            }
            _ => {}
        }
    }
    let name = clean_name(&name).map_err(|m| err(StatusCode::BAD_REQUEST, m))?;
    let png = file.ok_or_else(|| err(StatusCode::BAD_REQUEST, "missing file"))?;
    validate_cape_png(&png).map_err(|m| err(StatusCode::UNPROCESSABLE_ENTITY, m))?;

    let id = random_hex(8);
    {
        let mut core = app.core.lock().await;
        if let Some(at) = core.last_upload.get(&uuid) {
            if at.elapsed() < UPLOAD_COOLDOWN {
                return Err(err(StatusCode::TOO_MANY_REQUESTS, "slow down, wait a few seconds between uploads"));
            }
        }
        if core.db.capes.iter().filter(|c| c.author_uuid == uuid).count() >= MAX_PER_AUTHOR {
            return Err(err(StatusCode::CONFLICT, format!("you already have {MAX_PER_AUTHOR} capes, delete one first")));
        }
        core.last_upload.insert(uuid.clone(), Instant::now());
        core.db.capes.push(CapeRec {
            id: id.clone(),
            name,
            author_uuid: uuid,
            author_name: author,
            created: now_secs(),
            hash: sha1_hex(&png),
        });
    }
    tokio::fs::create_dir_all(app.capes_dir()).await.map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    tokio::fs::write(app.capes_dir().join(format!("{id}.png")), &png)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    app.save().await;
    Ok(Json(json!({ "id": id })))
}

async fn delete_cape(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Result<StatusCode, ApiErr> {
    if !valid_id(&id) || id == FROST_ID {
        return Err(err(StatusCode::BAD_REQUEST, "cannot delete this cape"));
    }
    let is_admin = match (app.admin.as_deref(), headers.get("x-admin-token").and_then(|v| v.to_str().ok())) {
        (Some(want), Some(got)) => want == got,
        _ => false,
    };
    let uuid = if is_admin { String::new() } else { app.auth(&headers).await?.0 };
    {
        let mut core = app.core.lock().await;
        let Some(pos) = core.db.capes.iter().position(|c| c.id == id) else {
            return Err(err(StatusCode::NOT_FOUND, "no such cape"));
        };
        if !is_admin && core.db.capes[pos].author_uuid != uuid {
            return Err(err(StatusCode::FORBIDDEN, "only the author can delete a cape"));
        }
        core.db.capes.remove(pos);
    }
    let _ = tokio::fs::remove_file(app.capes_dir().join(format!("{id}.png"))).await;
    app.save().await;
    Ok(StatusCode::NO_CONTENT)
}

async fn get_me(State(app): State<App>, headers: HeaderMap) -> Result<Json<Value>, ApiErr> {
    let (uuid, _) = app.auth(&headers).await?;
    let core = app.core.lock().await;
    let p = core.db.players.get(&uuid).cloned().unwrap_or_default();
    Ok(Json(json!({
        "cape": core.db.effective_cape(&p).unwrap_or_else(|| NO_CAPE.to_owned()),
        "skin_rev": p.skin_rev,
    })))
}

#[derive(Deserialize)]
struct MeReq {
    cape: Option<String>,
    #[serde(default)]
    bump_skin: bool,
}

async fn put_me(State(app): State<App>, headers: HeaderMap, Json(req): Json<MeReq>) -> Result<Json<Value>, ApiErr> {
    let (uuid, name) = app.auth(&headers).await?;
    {
        let mut core = app.core.lock().await;
        if let Some(cape) = &req.cape {
            if cape != NO_CAPE && !core.db.cape_exists(cape) {
                return Err(err(StatusCode::NOT_FOUND, "no such cape"));
            }
        }
        let p = core.db.players.entry(uuid).or_default();
        p.name = name;
        if let Some(cape) = req.cape {
            p.cape = Some(cape);
        }
        if req.bump_skin {
            p.skin_rev = now_secs();
        }
        p.last_seen = now_secs();
    }
    app.save().await;
    Ok(Json(json!({ "ok": true })))
}

async fn presence(State(app): State<App>, headers: HeaderMap) -> Result<StatusCode, ApiErr> {
    let (uuid, name) = app.auth(&headers).await?;
    let mut core = app.core.lock().await;
    let p = core.db.players.entry(uuid).or_default();
    p.name = name;
    p.last_seen = now_secs();
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct LookupReq {
    uuids: Vec<String>,
}

async fn lookup(State(app): State<App>, Json(req): Json<LookupReq>) -> Json<HashMap<String, Entry>> {
    let uuids: Vec<String> = req.uuids.iter().map(|u| u.replace('-', "").to_lowercase()).collect();
    let core = app.core.lock().await;
    Json(core.db.lookup(&uuids, now_secs()))
}

// ------------------------------------------------------------------------------------------------

fn router(app: App) -> Router {
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/auth/challenge", post(challenge))
        .route("/v1/auth/login", post(login))
        .route("/v1/capes", get(list_capes).post(upload_cape))
        .route("/v1/capes/:file", get(cape_png).delete(delete_cape))
        .route("/v1/me", get(get_me).put(put_me))
        .route("/v1/presence", post(presence))
        .route("/v1/cosmetics/lookup", post(lookup))
        .layer(DefaultBodyLimit::max(MAX_CAPE_BYTES * 2))
        .with_state(app)
}

fn load_db(dir: &FsPath) -> Db {
    std::fs::read(dir.join("db.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();
    let port: u16 = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8080);
    let data_dir = PathBuf::from(std::env::var("DATA_DIR").unwrap_or_else(|_| "data".into()));
    std::fs::create_dir_all(data_dir.join("capes"))?;
    let app = App {
        core: Arc::new(Mutex::new(Core { db: load_db(&data_dir), ..Core::default() })),
        data_dir: Arc::new(data_dir),
        http: reqwest::Client::builder().timeout(Duration::from_secs(10)).build()?,
        admin: Arc::new(std::env::var("ADMIN_TOKEN").ok().filter(|t| !t.is_empty())),
    };
    // persist last_seen / selections regularly (presence does not save on every beat)
    let saver = app.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(300)).await;
            saver.save().await;
        }
    });
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await?;
    tracing::info!("frost-server listening on port {port}");
    axum::serve(listener, router(app)).await?;
    Ok(())
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
    fn cape_validation() {
        assert!(validate_cape_png(&png(64, 32)).is_ok());
        assert!(validate_cape_png(&png(22, 17)).is_ok());
        assert!(validate_cape_png(&png(128, 64)).is_ok());
        assert!(validate_cape_png(&png(10, 10)).is_err());
        assert!(validate_cape_png(b"nope").is_err());
    }

    #[test]
    fn builtin_cape_is_a_valid_64x32_png() {
        assert_eq!(validate_cape_png(FROST_CAPE), Ok((64, 32)));
    }

    #[test]
    fn names_are_cleaned() {
        assert_eq!(clean_name("  Icy Wings ").unwrap(), "Icy Wings");
        assert!(clean_name("ab").is_err());
        assert!(clean_name(&"x".repeat(25)).is_err());
    }

    #[test]
    fn default_cape_is_frost_and_offline_players_are_hidden() {
        let mut db = Db::default();
        db.players.insert("a".into(), Player { last_seen: 1000, ..Player::default() });
        db.players.insert("b".into(), Player { last_seen: 10, ..Player::default() });
        let out = db.lookup(&["a".into(), "b".into(), "c".into()], 1050);
        assert_eq!(out.len(), 1);
        let a = &out["a"];
        assert!(a.frost);
        assert_eq!(a.cape_url.as_deref(), Some("/v1/capes/frost.png"));
        assert_eq!(a.cape_hash, Some(Db::builtin_hash()));
    }

    #[test]
    fn cape_choice_none_and_deleted_cape_fallback() {
        let mut db = Db::default();
        db.capes.push(CapeRec {
            id: "c1".into(),
            name: "Cool".into(),
            author_uuid: "u".into(),
            author_name: "U".into(),
            created: 1,
            hash: "h".into(),
        });
        let none = Player { cape: Some(NO_CAPE.into()), last_seen: 100, ..Player::default() };
        assert_eq!(db.effective_cape(&none), None);
        let custom = Player { cape: Some("c1".into()), last_seen: 100, ..Player::default() };
        assert_eq!(db.effective_cape(&custom).as_deref(), Some("c1"));
        db.capes.clear();
        assert_eq!(db.effective_cape(&custom).as_deref(), Some(FROST_ID));
    }

    #[test]
    fn users_are_counted_per_cape() {
        let mut db = Db::default();
        db.players.insert("a".into(), Player::default());
        db.players.insert("b".into(), Player { cape: Some(NO_CAPE.into()), ..Player::default() });
        assert_eq!(db.users_of(FROST_ID), 1);
    }

    #[test]
    fn db_json_roundtrip() {
        let mut db = Db::default();
        db.players.insert("a".into(), Player { name: "A".into(), skin_rev: 5, ..Player::default() });
        let bytes = serde_json::to_vec(&db).unwrap();
        let back: Db = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(back.players["a"].skin_rev, 5);
    }
}
