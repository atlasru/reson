use crate::error::{Error, Result};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::sync::{Mutex, Semaphore};

pub struct ArtworkCache {
    root: PathBuf,
    client: reqwest::Client,
    lock: Mutex<()>,
    downloads: Semaphore,
    hosts: Vec<String>,
    limit: std::sync::atomic::AtomicU64,
}
impl ArtworkCache {
    pub fn new(root: PathBuf, limit_mb: u64, hosts: Vec<String>) -> Result<Self> {
        std::fs::create_dir_all(&root)?;
        cleanup(&root, limit_mb * 1024 * 1024)?;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| Error::Network)?;
        Ok(Self {
            root,
            client,
            lock: Mutex::new(()),
            downloads: Semaphore::new(4),
            hosts,
            limit: std::sync::atomic::AtomicU64::new(limit_mb * 1024 * 1024),
        })
    }
    pub async fn get(&self, raw: &str) -> Result<PathBuf> {
        let url = url::Url::parse(raw).map_err(|_| Error::Malformed)?;
        if url.scheme() != "https"
            || !url.host_str().is_some_and(|host| {
                self.hosts
                    .iter()
                    .any(|h| host == h || host.ends_with(&format!(".{h}")))
            })
            || !url.username().is_empty()
            || url.password().is_some()
            || url.port().is_some_and(|p| p != 443)
        {
            return Err(Error::Invalid("Invalid artwork URL".into()));
        }
        let key = format!("{:x}", Sha256::digest(raw.as_bytes()));
        let path = self.root.join(format!("{key}.jpg"));
        let _permit = self
            .downloads
            .acquire()
            .await
            .map_err(|_| Error::Cancelled)?;
        {
            let _guard = self.lock.lock().await;
            if path.exists() {
                let metadata = tokio::fs::metadata(&path).await?;
                if metadata.len() > 0 {
                    touch(&path)?;
                    return Ok(path);
                }
            }
        }
        let mut response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|_| Error::Network)?;
        if !response.status().is_success()
            || !response
                .headers()
                .get("content-type")
                .and_then(|s| s.to_str().ok())
                .is_some_and(|s| {
                    matches!(
                        s.split(';').next(),
                        Some("image/jpeg" | "image/png" | "image/webp")
                    )
                })
        {
            return Err(Error::Malformed);
        }
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| Error::Network)? {
            if body.len() + chunk.len() > 4_000_000 {
                return Err(Error::Malformed);
            }
            body.extend_from_slice(&chunk);
        }
        let valid = body.starts_with(&[0xff, 0xd8, 0xff])
            || body.starts_with(b"\x89PNG\r\n\x1a\n")
            || (body.starts_with(b"RIFF") && body.get(8..12) == Some(b"WEBP"));
        if !valid {
            return Err(Error::Malformed);
        }
        let _guard = self.lock.lock().await;
        if path.exists() {
            return Ok(path);
        }
        cleanup(
            &self.root,
            self.limit
                .load(std::sync::atomic::Ordering::Relaxed)
                .saturating_sub(body.len() as u64),
        )?;
        let temp = self.root.join(format!("{key}.tmp"));
        tokio::fs::write(&temp, &body).await?;
        tokio::fs::rename(temp, &path).await?;
        Ok(path)
    }
    pub async fn set_limit(&self, mb: u64) -> Result<()> {
        let _guard = self.lock.lock().await;
        self.limit
            .store(mb * 1024 * 1024, std::sync::atomic::Ordering::Relaxed);
        cleanup(&self.root, mb * 1024 * 1024)
    }
    pub async fn clear(&self) -> Result<()> {
        let _guard = self.lock.lock().await;
        cleanup(&self.root, 0)
    }
    pub fn size(&self) -> u64 {
        files(&self.root).iter().map(|(_, n, _)| n).sum()
    }
}
fn touch(path: &Path) -> Result<()> {
    std::fs::OpenOptions::new()
        .write(true)
        .open(path)?
        .set_modified(std::time::SystemTime::now())?;
    Ok(())
}
fn files(root: &Path) -> Vec<(PathBuf, u64, std::time::SystemTime)> {
    std::fs::read_dir(root)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            if !m.is_file() {
                return None;
            }
            Some((
                e.path(),
                m.len(),
                m.modified().unwrap_or(std::time::UNIX_EPOCH),
            ))
        })
        .collect()
}
pub fn cleanup(root: &Path, limit: u64) -> Result<()> {
    let mut entries = files(root);
    let mut bytes = entries.iter().map(|(_, n, _)| n).sum::<u64>();
    entries.sort_by_key(|(_, _, time)| *time);
    for (path, n, _) in entries {
        if bytes <= limit {
            break;
        }
        std::fs::remove_file(path)?;
        bytes = bytes.saturating_sub(n);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn limit_evicts_old_entries() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("one.jpg"), vec![0; 100]).unwrap();
        std::fs::write(d.path().join("two.jpg"), vec![0; 100]).unwrap();
        cleanup(d.path(), 100).unwrap();
        assert_eq!(files(d.path()).iter().map(|(_, n, _)| n).sum::<u64>(), 100);
        cleanup(d.path(), 0).unwrap();
        assert!(files(d.path()).is_empty());
    }
}
