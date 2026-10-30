use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use futures::stream::{self, StreamExt};
use sha1::{Digest, Sha1};
use tokio::io::AsyncWriteExt;

#[derive(Debug, Clone)]
pub struct Job {
    pub url: String,
    pub path: PathBuf,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

#[derive(Debug, Default)]
pub struct Progress {
    pub done: AtomicUsize,
    pub total: AtomicUsize,
}

impl Progress {
    pub fn fraction(&self) -> f32 {
        let total = self.total.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }
        self.done.load(Ordering::Relaxed) as f32 / total as f32
    }
}

/// Streaming SHA-1 of a file (64 KiB buffer, constant memory).
pub fn sha1_file(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha1::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Cheap check first (size), expensive check (hash) only when sizes agree.
async fn is_up_to_date(job: &Job) -> bool {
    let Ok(meta) = tokio::fs::metadata(&job.path).await else {
        return false;
    };
    if let Some(size) = job.size {
        if meta.len() != size {
            return false;
        }
    }
    match &job.sha1 {
        None => true,
        Some(expected) => {
            let path = job.path.clone();
            let expected = expected.clone();
            tokio::task::spawn_blocking(move || {
                sha1_file(&path).map(|h| h.eq_ignore_ascii_case(&expected)).unwrap_or(false)
            })
            .await
            .unwrap_or(false)
        }
    }
}

async fn fetch_one(http: &reqwest::Client, job: &Job) -> Result<()> {
    if let Some(parent) = job.path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let tmp = job.path.with_extension("frost-part");
    let mut resp = http.get(&job.url).send().await?.error_for_status()?;
    let mut file = tokio::fs::File::create(&tmp).await?;
    let mut hasher = Sha1::new();
    while let Some(chunk) = resp.chunk().await? {
        hasher.update(&chunk);
        file.write_all(&chunk).await?;
    }
    file.flush().await?;
    drop(file);
    if let Some(expected) = &job.sha1 {
        let got = hex::encode(hasher.finalize());
        if !got.eq_ignore_ascii_case(expected) {
            let _ = tokio::fs::remove_file(&tmp).await;
            bail!("hash mismatch for {} (expected {expected}, got {got})", job.url);
        }
    }
    tokio::fs::rename(&tmp, &job.path).await?;
    Ok(())
}

async fn run_job(http: &reqwest::Client, job: Job, progress: &Progress) -> Result<()> {
    if !is_up_to_date(&job).await {
        let mut last_err = None;
        for attempt in 0..3u64 {
            match fetch_one(http, &job).await {
                Ok(()) => {
                    last_err = None;
                    break;
                }
                Err(e) => {
                    last_err = Some(e);
                    tokio::time::sleep(std::time::Duration::from_millis(300 * (attempt + 1))).await;
                }
            }
        }
        if let Some(e) = last_err {
            return Err(e).with_context(|| format!("download {}", job.url));
        }
    }
    progress.done.fetch_add(1, Ordering::Relaxed);
    Ok(())
}

/// Download all jobs with bounded parallelism. Existing, valid files are skipped.
pub async fn download_all(
    http: &reqwest::Client,
    jobs: Vec<Job>,
    concurrency: usize,
    progress: Arc<Progress>,
) -> Result<()> {
    progress.total.fetch_add(jobs.len(), Ordering::Relaxed);
    let results: Vec<Result<()>> = stream::iter(jobs)
        .map(|job| {
            let progress = progress.clone();
            async move { run_job(http, job, &progress).await }
        })
        .buffer_unordered(concurrency.max(1))
        .collect()
        .await;
    for r in results {
        r?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha1_of_known_content() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.txt");
        std::fs::write(&p, b"abc").unwrap();
        assert_eq!(sha1_file(&p).unwrap(), "a9993e364706816aba3e25717850c26c9cd0d89d");
    }

    #[tokio::test]
    async fn up_to_date_detects_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.txt");
        std::fs::write(&p, b"abc").unwrap();
        let ok = Job {
            url: String::new(),
            path: p.clone(),
            sha1: Some("a9993e364706816aba3e25717850c26c9cd0d89d".into()),
            size: Some(3),
        };
        assert!(is_up_to_date(&ok).await);
        let bad = Job { sha1: Some("00".into()), ..ok };
        assert!(!is_up_to_date(&bad).await);
    }
}
