//! File store abstraction — raw byte persistence for file-based data sources
//! (GeoJSON / Shapefile / GeoTIFF / WorldImage / ArcGrid, etc.).
//!
//! Each data source records where its file lives via `DataSourceConnection`
//! (`file_path` + `file_storage_type`), so storage backends are chosen
//! per data source instead of through a global configuration section. The
//! local backend stores files under a data directory; object-storage backends
//! (`s3` / `oss` / `minio`) are reserved for future implementation.

use std::path::PathBuf;

use async_trait::async_trait;

use crate::store::StoreError;

/// A directory-listing entry returned by the browse endpoints.
///
/// - Local browse: `path` is an absolute filesystem path.
/// - S3 browse: `path` is the object key / common prefix.
#[derive(Debug, Clone, serde::Serialize)]
pub struct StoreEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
}

/// File store abstraction — raw file byte persistence.
///
/// Implement this trait (e.g. an S3/MinIO backend) to add a new backend; the
/// caller selects the backend per data source via `file_storage_type`.
#[async_trait]
pub trait FileStore: Send + Sync {
    /// Store raw bytes under a logical key.
    async fn put(&self, key: &str, data: &[u8]) -> Result<(), StoreError>;
    /// Read raw bytes by key.
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, StoreError>;
    /// Resolve a key to a local filesystem path.
    ///
    /// The local backend returns the real file path (used by WCS / readers to
    /// access files directly); object-storage backends return `None` and
    /// callers must stream via [`FileStore::get`].
    fn local_path(&self, key: &str) -> Option<PathBuf>;
    /// Delete the stored file for a key.
    async fn delete(&self, key: &str) -> Result<(), StoreError>;
    /// List all stored keys.
    async fn list(&self) -> Result<Vec<String>, StoreError>;
}

/// Local directory file store — files stored under a data directory.
///
/// The directory can be mounted as NFS / object storage (e.g. via a FUSE or
/// CSI driver), keeping the local backend cloud-friendly.
pub struct LocalFileStore {
    dir: PathBuf,
}

impl LocalFileStore {
    /// Create a local file store (directory is lazily created on write).
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// Key -> safe file name (strip path separators and illegal characters).
    fn file_path(&self, key: &str) -> PathBuf {
        let safe: String = key
            .chars()
            .map(|c| match c {
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
                _ => c,
            })
            .collect();
        self.dir.join(safe)
    }
}

#[async_trait]
impl FileStore for LocalFileStore {
    async fn put(&self, key: &str, data: &[u8]) -> Result<(), StoreError> {
        std::fs::create_dir_all(&self.dir)?;
        std::fs::write(self.file_path(key), data)?;
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, StoreError> {
        let path = self.file_path(key);
        if !path.exists() {
            return Ok(None);
        }
        Ok(Some(std::fs::read(path)?))
    }

    fn local_path(&self, key: &str) -> Option<PathBuf> {
        Some(self.file_path(key))
    }

    async fn delete(&self, key: &str) -> Result<(), StoreError> {
        let path = self.file_path(key);
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }

    async fn list(&self) -> Result<Vec<String>, StoreError> {
        if !self.dir.exists() {
            return Ok(Vec::new());
        }
        let mut keys = Vec::new();
        for entry in std::fs::read_dir(&self.dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            if entry.file_type()?.is_file() {
                keys.push(name);
            }
        }
        keys.sort();
        Ok(keys)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("terrane-filestore-{}-{}", tag, std::process::id()))
    }

    #[actix_rt::test]
    async fn test_put_get_roundtrip() {
        let dir = temp_dir("rt");
        let store = LocalFileStore::new(dir.clone());
        store.put("data/a.geojson", b"geojson-bytes").await.unwrap();
        assert_eq!(
            store.get("data/a.geojson").await.unwrap(),
            Some(b"geojson-bytes".to_vec())
        );
        let p = store.local_path("data/a.geojson").unwrap();
        assert!(p.exists(), "local path 应真实存在");
        assert_eq!(
            store.list().await.unwrap(),
            vec!["data_a.geojson".to_string()]
        );
        store.delete("data/a.geojson").await.unwrap();
        assert!(store.get("data/a.geojson").await.unwrap().is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[actix_rt::test]
    async fn test_safe_key() {
        let dir = temp_dir("safe");
        let store = LocalFileStore::new(dir.clone());
        store.put("a/b:c?.tif", b"x").await.unwrap();
        let p = store.local_path("a/b:c?.tif").unwrap();
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        assert!(
            !name.contains('/') && !name.contains(':'),
            "非法字符应被替换"
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
