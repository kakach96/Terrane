//! Local directory raster data store — one file per coverage.
//!
//! The directory can be mounted as NFS / object storage (e.g. via a FUSE or
//! CSI driver). File layout: `<dir>/<key>.tif` (a `.tif` extension is appended
//! when the key carries no raster extension).

use std::path::PathBuf;

use crate::store::StoreError;

/// Local directory raster data store.
pub struct LocalRasterStore {
    dir: PathBuf,
}

impl LocalRasterStore {
    /// Create a local raster store (directory is lazily created on write).
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

    /// Key -> canonical raster file path (ensures a `.tif` extension).
    fn tif_path(&self, key: &str) -> PathBuf {
        let mut p = self.file_path(key);
        if p.extension().is_none() {
            p.set_extension("tif");
        }
        p
    }
}

#[async_trait::async_trait]
impl super::RasterStore for LocalRasterStore {
    async fn put(&self, key: &str, data: &[u8]) -> Result<(), StoreError> {
        std::fs::create_dir_all(&self.dir)?;
        let path = self.tif_path(key);
        std::fs::write(path, data)?;
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, StoreError> {
        let path = self.tif_path(key);
        if !path.exists() {
            return Ok(None);
        }
        Ok(Some(std::fs::read(path)?))
    }

    fn local_path(&self, key: &str) -> Option<PathBuf> {
        Some(self.tif_path(key))
    }

    async fn delete(&self, key: &str) -> Result<(), StoreError> {
        let path = self.tif_path(key);
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
            if name.ends_with(".tif") || name.ends_with(".tiff") {
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
    use super::super::RasterStore;
    use std::path::PathBuf;

    fn temp_dir(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("terrane-raster-{}-{}", tag, std::process::id()))
    }

    #[actix_rt::test]
    async fn test_put_get_roundtrip() {
        let dir = temp_dir("rt");
        let store = LocalRasterStore::new(dir.clone());
        store.put("cov1", b"tiff-bytes").await.unwrap();
        assert_eq!(store.get("cov1").await.unwrap(), Some(b"tiff-bytes".to_vec()));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[actix_rt::test]
    async fn test_local_path_adds_tif_extension() {
        let dir = temp_dir("ext");
        let store = LocalRasterStore::new(dir.clone());
        let p = store.local_path("cov1").unwrap();
        assert!(p.to_string_lossy().ends_with("cov1.tif"), "无扩展名的 key 应补 .tif, 实际: {:?}", p);

        // 已有扩展名则保留
        store.put("cov2.tif", b"x").await.unwrap();
        let p2 = store.local_path("cov2.tif").unwrap();
        assert!(p2.to_string_lossy().ends_with("cov2.tif"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[actix_rt::test]
    async fn test_get_missing_returns_none() {
        let dir = temp_dir("miss");
        let store = LocalRasterStore::new(dir.clone());
        assert!(store.get("missing").await.unwrap().is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[actix_rt::test]
    async fn test_delete_and_list() {
        let dir = temp_dir("dl");
        let store = LocalRasterStore::new(dir.clone());
        store.put("a.tif", b"a").await.unwrap();
        store.put("b.tiff", b"b").await.unwrap();
        store.put("c.png", b"c").await.unwrap();

        let mut keys = store.list().await.unwrap();
        keys.sort();
        assert_eq!(keys, vec!["a.tif".to_string(), "b.tiff".to_string()], "仅 .tif/.tiff 应列出");

        store.delete("a.tif").await.unwrap();
        assert!(store.get("a.tif").await.unwrap().is_none());
        std::fs::remove_dir_all(&dir).ok();
    }
}
