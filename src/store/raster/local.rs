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
