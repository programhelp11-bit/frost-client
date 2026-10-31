//! Modrinth integration: search, install (with required dependencies), list and remove
//! content (mods, resource packs, shaders) inside a Frost instance.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::config::data_dir;
use crate::downloader::{download_all, Job, Progress};
use crate::modloader::{loader_for, Loader};

const API: &str = "https://api.modrinth.com/v2";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Mod,
    ResourcePack,
    Shader,
}

impl Kind {
    pub fn api_name(self) -> &'static str {
        match self {
            Kind::Mod => "mod",
            Kind::ResourcePack => "resourcepack",
            Kind::Shader => "shader",
        }
    }

    pub fn folder(self) -> &'static str {
        match self {
            Kind::Mod => "mods",
            Kind::ResourcePack => "resourcepacks",
            Kind::Shader => "shaderpacks",
        }
    }
}

pub fn loader_name(mc: &str) -> &'static str {
    match loader_for(mc) {
        Loader::Fabric => "fabric",
        Loader::Forge => "forge",
    }
}

pub fn instance_dir(mc: &str) -> PathBuf {
    data_dir().join("instances").join(mc)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hit {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub author: String,
    pub downloads: u64,
    pub icon_url: Option<String>,
    #[serde(default)]
    pub categories: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchPage {
    pub hits: Vec<Hit>,
    pub total_hits: u64,
    pub offset: u32,
    pub limit: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
    pub query: String,
    pub mc_version: String,
    pub kind: Kind,
    pub category: Option<String>,
    /// relevance | downloads | follows | newest | updated
    pub sort: String,
    pub offset: u32,
    pub limit: u32,
}

pub async fn search(http: &reqwest::Client, q: &SearchQuery) -> Result<SearchPage> {
    let mut facets: Vec<Vec<String>> = vec![
        vec![format!("project_type:{}", q.kind.api_name())],
        vec![format!("versions:{}", q.mc_version)],
    ];
    // resource packs and shaders are loader-agnostic (shaders need Iris/OptiFine, handled by the user)
    if q.kind == Kind::Mod {
        facets.push(vec![format!("categories:{}", loader_name(&q.mc_version))]);
    }
    if let Some(c) = q.category.as_deref().filter(|c| !c.is_empty()) {
        facets.push(vec![format!("categories:{c}")]);
    }
    let index = match q.sort.as_str() {
        "downloads" | "follows" | "newest" | "updated" => q.sort.clone(),
        _ => "relevance".to_owned(),
    };
    let resp = http
        .get(format!("{API}/search"))
        .query(&[
            ("query", q.query.clone()),
            ("facets", serde_json::to_string(&facets)?),
            ("index", index),
            ("offset", q.offset.to_string()),
            ("limit", q.limit.clamp(1, 50).to_string()),
        ])
        .send()
        .await?
        .error_for_status()
        .context("Modrinth search")?;
    #[derive(Deserialize)]
    struct Raw {
        hits: Vec<Hit>,
        total_hits: u64,
        offset: u32,
        limit: u32,
    }
    let raw: Raw = resp.json().await.context("parse Modrinth search")?;
    Ok(SearchPage { hits: raw.hits, total_hits: raw.total_hits, offset: raw.offset, limit: raw.limit })
}

// ------------------------------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct Version {
    id: String,
    version_number: String,
    files: Vec<VFile>,
    #[serde(default)]
    dependencies: Vec<Dep>,
}

#[derive(Debug, Deserialize)]
struct VFile {
    url: String,
    filename: String,
    #[serde(default)]
    primary: bool,
    hashes: Hashes,
    size: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct Hashes {
    sha1: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Dep {
    project_id: Option<String>,
    dependency_type: String,
}

#[derive(Debug, Deserialize)]
struct ProjectInfo {
    id: String,
    slug: String,
    title: String,
    project_type: String,
    icon_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Installed {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub kind: Kind,
    pub filename: String,
    pub version: String,
    pub icon_url: Option<String>,
    /// Pulled in automatically as a dependency of another project.
    #[serde(default)]
    pub dependency: bool,
}

fn index_path(instance: &Path) -> PathBuf {
    instance.join("frost-content.json")
}

pub fn list_installed(mc: &str) -> Vec<Installed> {
    let items: Vec<Installed> = std::fs::read_to_string(index_path(&instance_dir(mc)))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    // forget entries whose file was deleted by hand
    let inst = instance_dir(mc);
    items.into_iter().filter(|i| inst.join(i.kind.folder()).join(&i.filename).exists()).collect()
}

fn save_index(instance: &Path, items: &[Installed]) -> Result<()> {
    std::fs::create_dir_all(instance)?;
    std::fs::write(index_path(instance), serde_json::to_vec_pretty(items)?)?;
    Ok(())
}

async fn project(http: &reqwest::Client, id: &str) -> Result<ProjectInfo> {
    Ok(http
        .get(format!("{API}/project/{id}"))
        .send()
        .await?
        .error_for_status()
        .with_context(|| format!("project {id}"))?
        .json()
        .await?)
}

async fn best_version(http: &reqwest::Client, id: &str, mc: &str, kind: Kind) -> Result<Version> {
    let mut query = vec![("game_versions", format!("[\"{mc}\"]"))];
    if kind == Kind::Mod {
        query.push(("loaders", format!("[\"{}\"]", loader_name(mc))));
    }
    let versions: Vec<Version> = http
        .get(format!("{API}/project/{id}/version"))
        .query(&query)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    versions
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("no build of this project for Minecraft {mc}"))
}

fn kind_from(api: &str) -> Option<Kind> {
    match api {
        "mod" => Some(Kind::Mod),
        "resourcepack" => Some(Kind::ResourcePack),
        "shader" => Some(Kind::Shader),
        _ => None,
    }
}

fn safe_filename(name: &str) -> Result<String> {
    let p = Path::new(name);
    match p.file_name().and_then(|f| f.to_str()) {
        Some(f) if f == name && !f.is_empty() => Ok(f.to_owned()),
        _ => bail!("refusing suspicious file name {name:?}"),
    }
}

/// Install `project` for the instance of `mc`, including required dependencies.
/// Returns everything that was newly installed (the project first).
pub async fn install(
    http: &reqwest::Client,
    project_id: &str,
    mc: &str,
    progress: Arc<Progress>,
) -> Result<Vec<Installed>> {
    let inst = instance_dir(mc);
    let mut index = list_installed(mc);
    let mut queue: Vec<(String, bool)> = vec![(project_id.to_owned(), false)];
    let mut seen: HashSet<String> = HashSet::new();
    let mut added: Vec<Installed> = Vec::new();
    let mut jobs: Vec<Job> = Vec::new();

    while let Some((pid, is_dep)) = queue.pop() {
        if !seen.insert(pid.clone()) || seen.len() > 24 {
            continue;
        }
        if index.iter().any(|i| i.project_id == pid) {
            continue;
        }
        let info = project(http, &pid).await?;
        let Some(kind) = kind_from(&info.project_type) else {
            if is_dep {
                continue;
            }
            bail!("unsupported content type {}", info.project_type);
        };
        let ver = match best_version(http, &info.id, mc, kind).await {
            Ok(v) => v,
            Err(e) if is_dep => {
                tracing::warn!("dependency {} skipped: {e:#}", info.slug);
                continue;
            }
            Err(e) => return Err(e),
        };
        let file = ver
            .files
            .iter()
            .find(|f| f.primary)
            .or_else(|| ver.files.first())
            .ok_or_else(|| anyhow!("version without files"))?;
        let filename = safe_filename(&file.filename)?;
        let dir = inst.join(kind.folder());
        jobs.push(Job {
            url: file.url.clone(),
            path: dir.join(&filename),
            sha1: file.hashes.sha1.clone(),
            size: file.size,
        });
        let entry = Installed {
            project_id: info.id.clone(),
            slug: info.slug,
            title: info.title,
            kind,
            filename,
            version: ver.version_number.clone(),
            icon_url: info.icon_url,
            dependency: is_dep,
        };
        index.push(entry.clone());
        added.push(entry);
        for d in &ver.dependencies {
            if d.dependency_type == "required" {
                if let Some(dp) = &d.project_id {
                    queue.push((dp.clone(), true));
                }
            }
        }
        let _ = &ver.id;
    }

    download_all(http, jobs, 4, progress).await?;
    save_index(&inst, &index)?;
    Ok(added)
}

pub fn remove(mc: &str, project_id: &str) -> Result<()> {
    let inst = instance_dir(mc);
    let mut index = list_installed(mc);
    if let Some(pos) = index.iter().position(|i| i.project_id == project_id) {
        let item = index.remove(pos);
        let path = inst.join(item.kind.folder()).join(safe_filename(&item.filename)?);
        let _ = std::fs::remove_file(path);
        save_index(&inst, &index)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filenames_are_validated() {
        assert!(safe_filename("sodium-1.0.jar").is_ok());
        assert!(safe_filename("../evil.jar").is_err());
        assert!(safe_filename("a/b.jar").is_err());
    }

    #[test]
    fn loaders() {
        assert_eq!(loader_name("1.8.9"), "forge");
        assert_eq!(loader_name("1.21.11"), "fabric");
    }

    #[test]
    fn facets_json() {
        let f = vec![vec!["project_type:mod".to_string()]];
        assert_eq!(serde_json::to_string(&f).unwrap(), "[[\"project_type:mod\"]]");
    }
}
