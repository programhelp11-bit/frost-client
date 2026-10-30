use std::collections::HashMap;

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use serde_json::Value;

pub const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

/// Versions Frost Client officially supports.
pub const SUPPORTED: &[&str] = &["1.8.9", "1.21.10", "1.21.11", "26.1.2", "26.2"];

/// 1.8.9 uses the old combat system (no attack cooldown) and `minecraftArguments`.
pub fn is_legacy(id: &str) -> bool {
    id == "1.8.9"
}

pub fn current_os() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    }
}

#[derive(Debug, Deserialize)]
pub struct Manifest {
    pub versions: Vec<ManifestEntry>,
}

#[derive(Debug, Deserialize)]
pub struct ManifestEntry {
    pub id: String,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FileRef {
    pub url: String,
    pub sha1: Option<String>,
    pub size: Option<u64>,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetIndexRef {
    pub id: String,
    pub url: String,
    pub sha1: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JavaVersion {
    #[serde(rename = "majorVersion")]
    pub major_version: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OsRule {
    pub name: Option<String>,
    pub arch: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Rule {
    pub action: String,
    pub os: Option<OsRule>,
    pub features: Option<HashMap<String, bool>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LibDownloads {
    pub artifact: Option<FileRef>,
    pub classifiers: Option<HashMap<String, FileRef>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Library {
    pub name: String,
    pub downloads: Option<LibDownloads>,
    pub rules: Option<Vec<Rule>>,
    pub natives: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Arguments {
    #[serde(default)]
    pub game: Vec<Value>,
    #[serde(default)]
    pub jvm: Vec<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClientDownloads {
    pub client: FileRef,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VersionJson {
    pub id: String,
    #[serde(rename = "mainClass")]
    pub main_class: String,
    pub arguments: Option<Arguments>,
    #[serde(rename = "minecraftArguments")]
    pub minecraft_arguments: Option<String>,
    #[serde(rename = "assetIndex")]
    pub asset_index: AssetIndexRef,
    pub assets: String,
    pub downloads: ClientDownloads,
    pub libraries: Vec<Library>,
    #[serde(rename = "javaVersion")]
    pub java_version: Option<JavaVersion>,
}

#[derive(Debug, Deserialize)]
pub struct AssetIndex {
    pub objects: HashMap<String, AssetObject>,
}

#[derive(Debug, Deserialize)]
pub struct AssetObject {
    pub hash: String,
    pub size: u64,
}

/// Evaluate Mojang rule lists. No rules = allowed. Last matching rule wins.
/// Feature-gated rules (demo mode, custom resolution, ...) never match.
pub fn rules_allow(rules: &Option<Vec<Rule>>) -> bool {
    let Some(rules) = rules else { return true };
    if rules.is_empty() {
        return true;
    }
    let mut allowed = false;
    for r in rules {
        if r.features.is_some() {
            continue;
        }
        let os_ok = match &r.os {
            None => true,
            Some(os) => {
                let name_ok = os.name.as_deref().map_or(true, |n| n == current_os());
                let arch_ok = os.arch.as_deref().map_or(true, |a| {
                    // "x86" rules only target 32-bit machines.
                    if a == "x86" {
                        cfg!(target_arch = "x86")
                    } else {
                        true
                    }
                });
                name_ok && arch_ok
            }
        };
        if os_ok {
            allowed = r.action == "allow";
        }
    }
    allowed
}

pub async fn fetch_manifest(http: &reqwest::Client) -> Result<Manifest> {
    http.get(MANIFEST_URL)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await
        .context("parse version manifest")
}

pub async fn fetch_version(http: &reqwest::Client, id: &str) -> Result<VersionJson> {
    let manifest = fetch_manifest(http).await?;
    let entry = manifest
        .versions
        .iter()
        .find(|v| v.id == id)
        .ok_or_else(|| anyhow!("version {id} not found in Mojang manifest"))?;
    http.get(&entry.url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await
        .context("parse version json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_default_allow() {
        assert!(rules_allow(&None));
    }

    #[test]
    fn rules_disallow_other_os() {
        let other = if current_os() == "windows" { "osx" } else { "windows" };
        let rules = Some(vec![
            Rule { action: "allow".into(), os: None, features: None },
            Rule {
                action: "disallow".into(),
                os: Some(OsRule { name: Some(other.into()), arch: None }),
                features: None,
            },
        ]);
        assert!(rules_allow(&rules));
        let rules = Some(vec![Rule {
            action: "allow".into(),
            os: Some(OsRule { name: Some(other.into()), arch: None }),
            features: None,
        }]);
        assert!(!rules_allow(&rules));
    }

    #[test]
    fn supported_list_contains_legacy() {
        assert!(SUPPORTED.contains(&"1.8.9"));
        assert!(is_legacy("1.8.9"));
        assert!(!is_legacy("1.21.10"));
    }
}
