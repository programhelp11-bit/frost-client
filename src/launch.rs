//! Prepares (downloads) and starts a Minecraft instance.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use serde_json::Value;

use crate::auth::Account;
use crate::config::{data_dir, Settings};
use crate::downloader::{download_all, Job, Progress};
use crate::{java, modloader};
use crate::versions::{self, rules_allow, AssetIndex, VersionJson};

const ASSET_BASE: &str = "https://resources.download.minecraft.net";

pub struct LaunchPlan {
    pub java: String,
    pub args: Vec<String>,
    pub game_dir: PathBuf,
    /// Non-fatal problems (e.g. the Frost mod jar was not bundled).
    pub warnings: Vec<String>,
}

fn cp_separator() -> &'static str {
    if cfg!(windows) {
        ";"
    } else {
        ":"
    }
}

fn arch_bits() -> &'static str {
    if cfg!(target_pointer_width = "64") {
        "64"
    } else {
        "32"
    }
}

/// Download everything the version needs and build the java command line.
pub async fn prepare(
    http: &reqwest::Client,
    settings: &Settings,
    account: &Account,
    version_id: &str,
    progress: Arc<Progress>,
) -> Result<LaunchPlan> {
    let root = data_dir();
    let libs_dir = root.join("libraries");
    let assets_dir = root.join("assets");
    let ver_dir = root.join("versions").join(version_id);
    let natives_dir = ver_dir.join("natives");
    let game_dir = root.join("instances").join(version_id);
    tokio::fs::create_dir_all(&game_dir).await?;

    let ver: VersionJson = versions::fetch_version(http, version_id).await?;
    let mods_dir = game_dir.join("mods");
    let mut warnings: Vec<String> = Vec::new();

    // Mod loader (Fabric / Forge), Fabric API and the Frost mod itself.
    let mut patch = modloader::prepare(http, version_id, &mods_dir)
        .await
        .with_context(|| format!("install mod loader for {version_id}"))?;
    warnings.append(&mut patch.warnings);
    match modloader::install_frost_mod(version_id, &mods_dir) {
        Ok(true) => {}
        Ok(false) => warnings.push(format!(
            "Frost mod for {version_id} was not found - the game starts without Frost HUD/bots."
        )),
        Err(e) => warnings.push(format!("could not install Frost mod: {e:#}")),
    }

    // --- phase 1: client jar, libraries, asset index -------------------------------------------
    let client_jar = ver_dir.join(format!("{version_id}.jar"));
    let mut jobs = vec![Job {
        url: ver.downloads.client.url.clone(),
        path: client_jar.clone(),
        sha1: ver.downloads.client.sha1.clone(),
        size: ver.downloads.client.size,
    }];

    let mut classpath: Vec<PathBuf> = Vec::new();
    let mut native_jars: Vec<PathBuf> = Vec::new();

    for lib in &ver.libraries {
        if !rules_allow(&lib.rules) {
            continue;
        }
        let Some(dl) = &lib.downloads else { continue };
        if let Some(art) = &dl.artifact {
            if let Some(rel) = &art.path {
                let path = libs_dir.join(rel);
                jobs.push(Job {
                    url: art.url.clone(),
                    path: path.clone(),
                    sha1: art.sha1.clone(),
                    size: art.size,
                });
                // Modern versions ship natives as plain artifacts with an OS suffix.
                if lib.name.contains(":natives-") {
                    native_jars.push(path.clone());
                }
                classpath.push(path);
            }
        }
        // Legacy (1.8.x) natives live in classifiers.
        if let (Some(natives), Some(classifiers)) = (&lib.natives, &dl.classifiers) {
            if let Some(key) = natives.get(versions::current_os()) {
                let key = key.replace("${arch}", arch_bits());
                if let Some(art) = classifiers.get(&key) {
                    if let Some(rel) = &art.path {
                        let path = libs_dir.join(rel);
                        jobs.push(Job {
                            url: art.url.clone(),
                            path: path.clone(),
                            sha1: art.sha1.clone(),
                            size: art.size,
                        });
                        native_jars.push(path);
                    }
                }
            }
        }
    }

    // Loader libraries go first on the classpath so they win over vanilla duplicates.
    let mut loader_cp: Vec<PathBuf> = Vec::new();
    for lib in std::mem::take(&mut patch.libs) {
        loader_cp.push(lib.path.clone());
        if !lib.url.is_empty() {
            jobs.push(lib);
        }
    }
    jobs.append(&mut patch.mod_downloads);
    classpath.splice(0..0, loader_cp);

    let index_path = assets_dir.join("indexes").join(format!("{}.json", ver.asset_index.id));
    jobs.push(Job {
        url: ver.asset_index.url.clone(),
        path: index_path.clone(),
        sha1: ver.asset_index.sha1.clone(),
        size: None,
    });
    download_all(http, jobs, 16, progress.clone()).await?;

    // --- phase 2: asset objects (parallel, hash-checked, skipped when present) -----------------
    let index: AssetIndex = serde_json::from_slice(&tokio::fs::read(&index_path).await?)
        .context("parse asset index")?;
    let asset_jobs: Vec<Job> = index
        .objects
        .values()
        .map(|o| {
            let prefix = &o.hash[..2];
            Job {
                url: format!("{ASSET_BASE}/{prefix}/{}", o.hash),
                path: assets_dir.join("objects").join(prefix).join(&o.hash),
                sha1: Some(o.hash.clone()),
                size: Some(o.size),
            }
        })
        .collect();
    download_all(http, asset_jobs, 48, progress).await?;

    // --- natives -------------------------------------------------------------------------------
    extract_natives(native_jars, natives_dir.clone()).await?;

    if settings.auto_optimize {
        let _ = crate::optimize::apply(&game_dir, version_id);
    }

    // --- arguments -----------------------------------------------------------------------------
    classpath.push(client_jar);
    let classpath_str = classpath
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(cp_separator());

    let mut vars: HashMap<&str, String> = HashMap::new();
    vars.insert("auth_player_name", account.username.clone());
    vars.insert("auth_uuid", account.uuid.clone());
    vars.insert("auth_access_token", account.mc_access_token.clone());
    vars.insert("auth_session", format!("token:{}:{}", account.mc_access_token, account.uuid));
    vars.insert("user_type", "msa".into());
    vars.insert("user_properties", "{}".into());
    vars.insert("clientid", String::new());
    vars.insert("auth_xuid", String::new());
    vars.insert("version_name", version_id.into());
    vars.insert("version_type", "release".into());
    vars.insert("game_directory", game_dir.to_string_lossy().into_owned());
    vars.insert("assets_root", assets_dir.to_string_lossy().into_owned());
    vars.insert("game_assets", assets_dir.to_string_lossy().into_owned());
    vars.insert("assets_index_name", ver.asset_index.id.clone());
    vars.insert("natives_directory", natives_dir.to_string_lossy().into_owned());
    vars.insert("launcher_name", "FrostClient".into());
    vars.insert("launcher_version", env!("CARGO_PKG_VERSION").into());
    vars.insert("classpath", classpath_str);

    let mut jvm: Vec<String> = Vec::new();
    jvm.push(format!("-Xmx{}M", settings.ram_mb));
    jvm.push(format!("-Xms{}M", settings.ram_mb.min(512)));
    jvm.extend(settings.jvm_args.split_whitespace().map(str::to_owned));

    // The Frost mod reads these to find the launcher's IPC bridge.
    jvm.push(format!("-Dfrost.ipc.port={}", settings.ipc_port));
    jvm.push(format!("-Dfrost.uuid={}", account.uuid));
    jvm.extend(patch.jvm_args.iter().map(|a| a.trim().to_owned()).filter(|a| !a.is_empty()));

    let mut game: Vec<String> = Vec::new();
    if let Some(args) = &ver.arguments {
        jvm.extend(flatten_args(&args.jvm));
        game.extend(flatten_args(&args.game));
    } else {
        jvm.push("-Djava.library.path=${natives_directory}".into());
        jvm.push("-cp".into());
        jvm.push("${classpath}".into());
        let legacy = patch.legacy_args.clone().or_else(|| ver.minecraft_arguments.clone());
        if let Some(legacy) = legacy {
            game.extend(legacy.split_whitespace().map(str::to_owned));
        }
    }
    game.extend(patch.game_args.iter().cloned());

    let mut args: Vec<String> = jvm.into_iter().map(|a| substitute(&a, &vars)).collect();
    args.push(patch.main_class.clone().unwrap_or_else(|| ver.main_class.clone()));
    args.extend(game.into_iter().map(|a| substitute(&a, &vars)));

    let java = if settings.java_path.trim().is_empty() {
        let major = ver.java_version.as_ref().map(|j| j.major_version).unwrap_or(8);
        match java::ensure_java(http, major).await {
            Ok(p) => p,
            Err(e) => {
                warnings.push(format!("automatic Java {major} download failed ({e:#}); using system Java"));
                find_java(settings)?
            }
        }
    } else {
        find_java(settings)?
    };
    Ok(LaunchPlan { java, args, game_dir, warnings })
}

/// Start the game. stdout/stderr go to `<instance>/frost-latest.log`.
pub fn spawn(plan: &LaunchPlan) -> Result<tokio::process::Child> {
    let log = std::fs::File::create(plan.game_dir.join("frost-latest.log"))?;
    let log_err = log.try_clone()?;
    let child = tokio::process::Command::new(&plan.java)
        .args(&plan.args)
        .current_dir(&plan.game_dir)
        .stdout(log)
        .stderr(log_err)
        .kill_on_drop(false)
        .spawn()
        .with_context(|| format!("start java at {}", plan.java))?;
    Ok(child)
}

fn find_java(settings: &Settings) -> Result<String> {
    if !settings.java_path.trim().is_empty() {
        return Ok(settings.java_path.trim().to_owned());
    }
    let exe = if cfg!(windows) { "javaw.exe" } else { "java" };
    if let Ok(home) = std::env::var("JAVA_HOME") {
        let p = Path::new(&home).join("bin").join(exe);
        if p.exists() {
            return Ok(p.to_string_lossy().into_owned());
        }
    }
    // Fall back to PATH lookup by the OS.
    Ok(exe.to_owned())
}

/// Expand Mojang's mixed string / conditional-object argument arrays.
fn flatten_args(items: &[Value]) -> Vec<String> {
    let mut out = Vec::new();
    for item in items {
        match item {
            Value::String(s) => out.push(s.clone()),
            Value::Object(obj) => {
                let rules: Option<Vec<versions::Rule>> = obj
                    .get("rules")
                    .and_then(|r| serde_json::from_value(r.clone()).ok());
                if !rules_allow(&rules) {
                    continue;
                }
                match obj.get("value") {
                    Some(Value::String(s)) => out.push(s.clone()),
                    Some(Value::Array(a)) => {
                        out.extend(a.iter().filter_map(|v| v.as_str().map(str::to_owned)))
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    out
}

fn substitute(template: &str, vars: &HashMap<&str, String>) -> String {
    let mut out = template.to_owned();
    for (k, v) in vars {
        out = out.replace(&format!("${{{k}}}"), v);
    }
    out
}

async fn extract_natives(jars: Vec<PathBuf>, dest: PathBuf) -> Result<()> {
    tokio::task::spawn_blocking(move || -> Result<()> {
        std::fs::create_dir_all(&dest)?;
        for jar in jars {
            let file = std::fs::File::open(&jar)?;
            let mut zip = zip::ZipArchive::new(file)?;
            for i in 0..zip.len() {
                let mut entry = zip.by_index(i)?;
                if entry.is_dir() || entry.name().starts_with("META-INF") {
                    continue;
                }
                // `enclosed_name` rejects path traversal ("zip slip").
                let Some(rel) = entry.enclosed_name() else { continue };
                let Some(file_name) = rel.file_name() else { continue };
                let out_path = dest.join(file_name);
                if out_path.exists() {
                    continue;
                }
                let mut out = std::fs::File::create(&out_path)?;
                std::io::copy(&mut entry, &mut out)?;
            }
        }
        Ok(())
    })
    .await
    .map_err(|e| anyhow!("natives task panicked: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitutes_placeholders() {
        let mut vars = HashMap::new();
        vars.insert("auth_player_name", "Steve".to_string());
        assert_eq!(substitute("--username ${auth_player_name}", &vars), "--username Steve");
    }

    #[test]
    fn flattens_plain_and_conditional_args() {
        let items: Vec<Value> = serde_json::from_str(
            r#"["--a", {"rules":[{"action":"allow","features":{"is_demo_user":true}}],"value":"--demo"},
                {"rules":[{"action":"allow"}],"value":["--b","--c"]}]"#,
        )
        .unwrap();
        assert_eq!(flatten_args(&items), vec!["--a", "--b", "--c"]);
    }
}
