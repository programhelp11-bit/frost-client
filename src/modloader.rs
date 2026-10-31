//! Mod loader installation: Fabric (1.21.x / 26.x) and Forge (1.8.9), plus Fabric API and the
//! Frost in-game mod. Everything lands in the per-version instance directory.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use serde_json::Value;

use crate::config::data_dir;
use crate::downloader::Job;
use crate::versions;

pub const FORGE_189: &str = "1.8.9-11.15.1.2318-1.8.9";

static RESOURCE_DIR: OnceLock<PathBuf> = OnceLock::new();

/// The app's bundled resource directory (set once at startup by the Tauri shell).
pub fn set_resource_dir(p: PathBuf) {
    let _ = RESOURCE_DIR.set(p);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Loader {
    Fabric,
    Forge,
}

pub fn loader_for(mc_version: &str) -> Loader {
    if versions::is_legacy(mc_version) {
        Loader::Forge
    } else {
        Loader::Fabric
    }
}

/// Everything a loader adds on top of the vanilla version.
#[derive(Debug, Default)]
pub struct LoaderPatch {
    pub main_class: Option<String>,
    /// Extra classpath libraries (downloaded by the caller before launch).
    pub libs: Vec<Job>,
    pub jvm_args: Vec<String>,
    pub game_args: Vec<String>,
    /// Forge 1.8.9 replaces the whole legacy `minecraftArguments` string.
    pub legacy_args: Option<String>,
    /// Mod jars to download into `mods/` (e.g. Fabric API).
    pub mod_downloads: Vec<Job>,
    pub warnings: Vec<String>,
}

/// `group:artifact:version[:classifier]` -> `group/path/artifact/version/artifact-version[-classifier].jar`
pub fn maven_path(name: &str) -> Result<String> {
    let mut parts = name.split(':');
    let group = parts.next().ok_or_else(|| anyhow!("bad maven name {name}"))?;
    let artifact = parts.next().ok_or_else(|| anyhow!("bad maven name {name}"))?;
    let version = parts.next().ok_or_else(|| anyhow!("bad maven name {name}"))?;
    let classifier = parts.next();
    let file = match classifier {
        Some(c) => format!("{artifact}-{version}-{c}.jar"),
        None => format!("{artifact}-{version}.jar"),
    };
    Ok(format!("{}/{artifact}/{version}/{file}", group.replace('.', "/")))
}

/// Find the Frost mod jar for this Minecraft version in the usual places.
pub fn find_frost_mod(mc_version: &str) -> Option<PathBuf> {
    let prefix = if loader_for(mc_version) == Loader::Forge {
        "frost-forge-1.8.9-".to_owned()
    } else {
        format!("frost-fabric-{mc_version}-")
    };
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Ok(d) = std::env::var("FROST_AGENT_DIR") {
        dirs.push(PathBuf::from(d));
    }
    if let Some(r) = RESOURCE_DIR.get() {
        dirs.push(r.join("agent"));
        dirs.push(r.clone());
    }
    dirs.push(data_dir().join("agent"));
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            dirs.push(dir.join("agent"));
            dirs.push(dir.join("resources").join("agent"));
            dirs.push(dir.join("../lib/frost-client/agent"));
            dirs.push(dir.join("../lib/Frost Client/agent"));
        }
    }
    for dir in dirs {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with(&prefix) && name.ends_with(".jar") && !name.contains("sources") {
                return Some(entry.path());
            }
        }
    }
    None
}

/// Copy the Frost mod into `mods/`, replacing older Frost jars.
pub fn install_frost_mod(mc_version: &str, mods_dir: &Path) -> Result<bool> {
    std::fs::create_dir_all(mods_dir)?;
    if let Ok(rd) = std::fs::read_dir(mods_dir) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            if n.starts_with("frost-fabric-") || n.starts_with("frost-forge-") {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    match find_frost_mod(mc_version) {
        Some(src) => {
            let dst = mods_dir.join(src.file_name().ok_or_else(|| anyhow!("bad mod path"))?);
            std::fs::copy(&src, &dst).with_context(|| format!("copy {}", src.display()))?;
            Ok(true)
        }
        None => Ok(false),
    }
}

// ------------------------------------------------------------------------------------------------
// Fabric
// ------------------------------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct FabricLoaderEntry {
    loader: FabricLoaderInfo,
}

#[derive(Debug, Deserialize)]
struct FabricLoaderInfo {
    version: String,
    #[serde(default)]
    stable: bool,
}

#[derive(Debug, Deserialize)]
struct FabricLib {
    name: String,
    url: Option<String>,
    sha1: Option<String>,
    size: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct FabricArgs {
    #[serde(default)]
    jvm: Vec<Value>,
    #[serde(default)]
    game: Vec<Value>,
}

#[derive(Debug, Deserialize)]
struct FabricProfile {
    #[serde(rename = "mainClass")]
    main_class: String,
    arguments: Option<FabricArgs>,
    libraries: Vec<FabricLib>,
}

fn string_args(items: &[Value]) -> Vec<String> {
    items.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect()
}

pub async fn prepare_fabric(http: &reqwest::Client, mc: &str, mods_dir: &Path) -> Result<LoaderPatch> {
    let loaders: Vec<FabricLoaderEntry> = http
        .get(format!("https://meta.fabricmc.net/v2/versions/loader/{mc}"))
        .send()
        .await?
        .error_for_status()
        .with_context(|| format!("Fabric does not list Minecraft {mc}"))?
        .json()
        .await?;
    let loader = loaders
        .iter()
        .find(|l| l.loader.stable)
        .or_else(|| loaders.first())
        .ok_or_else(|| anyhow!("no Fabric loader for {mc}"))?
        .loader
        .version
        .clone();

    let profile: FabricProfile = http
        .get(format!("https://meta.fabricmc.net/v2/versions/loader/{mc}/{loader}/profile/json"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await
        .context("parse Fabric profile")?;

    let libs_dir = data_dir().join("libraries");
    let mut patch = LoaderPatch { main_class: Some(profile.main_class), ..Default::default() };
    for lib in &profile.libraries {
        let rel = maven_path(&lib.name)?;
        let base = lib.url.clone().unwrap_or_else(|| "https://maven.fabricmc.net/".into());
        patch.libs.push(Job {
            url: format!("{}{}", if base.ends_with('/') { base } else { format!("{base}/") }, rel),
            path: libs_dir.join(&rel),
            sha1: lib.sha1.clone(),
            size: lib.size,
        });
    }
    if let Some(args) = &profile.arguments {
        patch.jvm_args = string_args(&args.jvm);
        patch.game_args = string_args(&args.game);
    }

    // Fabric API from Modrinth (newest build for this Minecraft version).
    match fabric_api_job(http, mc, mods_dir).await {
        Ok(job) => patch.mod_downloads.push(job),
        Err(e) => patch.warnings.push(format!("Fabric API not found for {mc}: {e:#}")),
    }
    Ok(patch)
}

#[derive(Debug, Deserialize)]
struct ModrinthVersion {
    files: Vec<ModrinthFile>,
}

#[derive(Debug, Deserialize)]
struct ModrinthFile {
    url: String,
    filename: String,
    #[serde(default)]
    primary: bool,
    hashes: ModrinthHashes,
    size: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct ModrinthHashes {
    sha1: Option<String>,
}

async fn fabric_api_job(http: &reqwest::Client, mc: &str, mods_dir: &Path) -> Result<Job> {
    let versions: Vec<ModrinthVersion> = http
        .get("https://api.modrinth.com/v2/project/fabric-api/version")
        .query(&[
            ("game_versions", format!("[\"{mc}\"]")),
            ("loaders", "[\"fabric\"]".to_owned()),
        ])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let v = versions.first().ok_or_else(|| anyhow!("no release"))?;
    let f = v.files.iter().find(|f| f.primary).or_else(|| v.files.first()).ok_or_else(|| anyhow!("no file"))?;
    // drop older Fabric API jars so only one is loaded
    if let Ok(rd) = std::fs::read_dir(mods_dir) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            if n.starts_with("fabric-api-") && n != f.filename {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    Ok(Job {
        url: f.url.clone(),
        path: mods_dir.join(&f.filename),
        sha1: f.hashes.sha1.clone(),
        size: f.size,
    })
}

// ------------------------------------------------------------------------------------------------
// Forge 1.8.9 (installer without processors: libraries + universal jar)
// ------------------------------------------------------------------------------------------------

fn fix_forge_url(base: &str) -> String {
    let b = base.replace("http://files.minecraftforge.net/maven/", "https://maven.minecraftforge.net/");
    let b = b.replace("http://", "https://");
    if b.ends_with('/') {
        b
    } else {
        format!("{b}/")
    }
}

async fn first_ok(http: &reqwest::Client, urls: &[String]) -> Option<String> {
    for u in urls {
        if let Ok(r) = http.head(u).send().await {
            if r.status().is_success() {
                return Some(u.clone());
            }
        }
    }
    None
}

pub async fn prepare_forge(http: &reqwest::Client) -> Result<LoaderPatch> {
    let root = data_dir();
    let installer = root.join("installers").join(format!("forge-{FORGE_189}-installer.jar"));
    if !installer.exists() {
        let url = format!(
            "https://maven.minecraftforge.net/net/minecraftforge/forge/{FORGE_189}/forge-{FORGE_189}-installer.jar"
        );
        let bytes = http.get(&url).send().await?.error_for_status()?.bytes().await?;
        tokio::fs::create_dir_all(installer.parent().unwrap()).await?;
        tokio::fs::write(&installer, &bytes).await?;
    }

    let libs_dir = root.join("libraries");
    let installer2 = installer.clone();
    let libs_dir2 = libs_dir.clone();
    let profile: Value = tokio::task::spawn_blocking(move || -> Result<Value> {
        let file = std::fs::File::open(&installer2)?;
        let mut zip = zip::ZipArchive::new(file)?;
        let profile: Value = {
            let mut f = zip.by_name("install_profile.json")?;
            serde_json::from_reader(&mut f)?
        };
        // extract the universal jar into the libraries tree
        let install_path = profile["install"]["path"].as_str().unwrap_or_default().to_owned();
        let file_path = profile["install"]["filePath"].as_str().unwrap_or_default().to_owned();
        if !install_path.is_empty() && !file_path.is_empty() {
            let rel = maven_path(&install_path)?;
            let dst = libs_dir2.join(rel);
            if !dst.exists() {
                std::fs::create_dir_all(dst.parent().unwrap())?;
                let mut f = zip.by_name(&file_path)?;
                let mut out = std::fs::File::create(&dst)?;
                std::io::copy(&mut f, &mut out)?;
            }
        }
        Ok(profile)
    })
    .await
    .map_err(|e| anyhow!("forge installer task: {e}"))??;

    let info = &profile["versionInfo"];
    let install_path = profile["install"]["path"].as_str().unwrap_or_default().to_owned();
    let mut patch = LoaderPatch {
        main_class: info["mainClass"].as_str().map(str::to_owned),
        legacy_args: info["minecraftArguments"].as_str().map(str::to_owned),
        jvm_args: vec![
            "-Dfml.ignoreInvalidMinecraftCertificates=true".into(),
            "-Dfml.ignorePatchDiscrepancies=true".into(),
        ],
        ..Default::default()
    };

    let mut universal_added = false;
    for lib in info["libraries"].as_array().ok_or_else(|| anyhow!("forge profile without libraries"))? {
        let Some(name) = lib["name"].as_str() else { continue };
        if lib["clientreq"].as_bool() == Some(false) {
            continue;
        }
        let rel = maven_path(name)?;
        let path = libs_dir.join(&rel);
        if name == install_path {
            // already extracted from the installer; only needs to be on the classpath
            patch.libs.push(Job { url: String::new(), path, sha1: None, size: None });
            universal_added = true;
            continue;
        }
        let mut candidates: Vec<String> = Vec::new();
        if let Some(base) = lib["url"].as_str() {
            candidates.push(format!("{}{rel}", fix_forge_url(base)));
        }
        for base in [
            "https://libraries.minecraft.net/",
            "https://maven.minecraftforge.net/",
            "https://repo1.maven.org/maven2/",
        ] {
            candidates.push(format!("{base}{rel}"));
        }
        if path.exists() {
            patch.libs.push(Job { url: String::new(), path, sha1: None, size: None });
            continue;
        }
        let url = first_ok(http, &candidates)
            .await
            .ok_or_else(|| anyhow!("library {name} not found on any known maven"))?;
        patch.libs.push(Job { url, path, sha1: None, size: None });
    }
    if !universal_added {
        bail!("Forge profile did not contain the forge library");
    }
    Ok(patch)
}

pub async fn prepare(http: &reqwest::Client, mc: &str, mods_dir: &Path) -> Result<LoaderPatch> {
    match loader_for(mc) {
        Loader::Fabric => prepare_fabric(http, mc, mods_dir).await,
        Loader::Forge => prepare_forge(http).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maven_paths() {
        assert_eq!(
            maven_path("net.fabricmc:fabric-loader:0.18.1").unwrap(),
            "net/fabricmc/fabric-loader/0.18.1/fabric-loader-0.18.1.jar"
        );
        assert_eq!(
            maven_path("org.lwjgl:lwjgl:3.3.3:natives-linux").unwrap(),
            "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-linux.jar"
        );
    }

    #[test]
    fn forge_url_fix() {
        assert_eq!(
            fix_forge_url("http://files.minecraftforge.net/maven/"),
            "https://maven.minecraftforge.net/"
        );
    }

    #[test]
    fn loader_selection() {
        assert_eq!(loader_for("1.8.9"), Loader::Forge);
        assert_eq!(loader_for("26.2"), Loader::Fabric);
    }
}
