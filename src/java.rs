//! Automatic Java runtime download (Eclipse Temurin via the Adoptium API), so players never
//! have to install Java themselves.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};

use crate::config::data_dir;

fn os_name() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "mac"
    } else {
        "linux"
    }
}

fn arch_name() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "x64"
    }
}

fn java_exe(dir: &Path) -> PathBuf {
    let exe = if cfg!(windows) { "javaw.exe" } else { "java" };
    dir.join("bin").join(exe)
}

/// Locate `bin/java(w)` inside an extracted runtime (the archive has one top-level folder).
fn find_home(root: &Path) -> Option<PathBuf> {
    if java_exe(root).exists() {
        return Some(root.to_path_buf());
    }
    for entry in std::fs::read_dir(root).ok()?.flatten() {
        let p = entry.path();
        if p.is_dir() {
            if java_exe(&p).exists() {
                return Some(p);
            }
            // macOS layout: Contents/Home
            let mac = p.join("Contents").join("Home");
            if java_exe(&mac).exists() {
                return Some(mac);
            }
        }
    }
    None
}

/// Returns the path to a Java executable of at least `major`, downloading Temurin if needed.
pub async fn ensure_java(http: &reqwest::Client, major: u32) -> Result<String> {
    let root = data_dir().join("runtime").join(format!("jre-{major}"));
    if let Some(home) = find_home(&root) {
        return Ok(java_exe(&home).to_string_lossy().into_owned());
    }
    let url = format!(
        "https://api.adoptium.net/v3/binary/latest/{major}/ga/{}/{}/jre/hotspot/normal/eclipse",
        os_name(),
        arch_name()
    );
    let bytes = http
        .get(&url)
        .send()
        .await?
        .error_for_status()
        .with_context(|| format!("download Java {major}"))?
        .bytes()
        .await?;
    let dest = root.clone();
    tokio::task::spawn_blocking(move || -> Result<()> {
        std::fs::create_dir_all(&dest)?;
        if cfg!(windows) {
            let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes.to_vec()))?;
            for i in 0..zip.len() {
                let mut entry = zip.by_index(i)?;
                let Some(rel) = entry.enclosed_name() else { continue };
                let out = dest.join(rel);
                if entry.is_dir() {
                    std::fs::create_dir_all(&out)?;
                } else {
                    if let Some(parent) = out.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    let mut f = std::fs::File::create(&out)?;
                    std::io::copy(&mut entry, &mut f)?;
                }
            }
        } else {
            let gz = flate2::read::GzDecoder::new(std::io::Cursor::new(bytes.to_vec()));
            tar::Archive::new(gz).unpack(&dest)?;
        }
        Ok(())
    })
    .await
    .map_err(|e| anyhow!("java extract task: {e}"))??;

    match find_home(&root) {
        Some(home) => Ok(java_exe(&home).to_string_lossy().into_owned()),
        None => bail!("Java {major} was downloaded but no java executable was found"),
    }
}
