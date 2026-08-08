//! Raster data store abstraction — persistence for raster files
//! (GeoTIFF / WorldImage / ArcGrid).
//!
//! Decoupled from the metadata store so raster data can live on a dedicated
//! volume / NFS / object storage. Backends are selected via
//! [`crate::config::RasterConfig::kind`]:
//! - `local` — local directory (default; one file per coverage, supports
//!   NFS / object-storage mounts)
//!
//! Future backends: `s3` / `minio` object storage (implement [`RasterStore`]
//! and register it in [`build_raster_store`]).

pub mod local;

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;

use crate::config::GeoServerConfig;
use crate::store::StoreError;

/// Raster data store abstraction — raw raster byte persistence.
///
/// Implement this trait (e.g. an S3/MinIO backend) and register it in
/// [`build_raster_store`] to add a new backend.
#[async_trait]
pub trait RasterStore: Send + Sync {
    /// Store raw raster bytes under a logical key (e.g. data source / coverage name).
    async fn put(&self, key: &str, data: &[u8]) -> Result<(), StoreError>;
    /// Read raw raster bytes by key.
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, StoreError>;
    /// Resolve a key to a local filesystem path.
    ///
    /// The local backend returns the real file path (used by WCS to read
    /// directly); object-storage backends return `None` and callers must
    /// stream via [`RasterStore::get`].
    fn local_path(&self, key: &str) -> Option<PathBuf>;
    /// Delete the stored raster for a key.
    async fn delete(&self, key: &str) -> Result<(), StoreError>;
    /// List all stored raster keys.
    async fn list(&self) -> Result<Vec<String>, StoreError>;
}

/// Build the raster data store from the effective [`crate::config::RasterConfig`].
///
/// Returns `None` if the configured backend cannot be initialized.
pub async fn build_raster_store(config: &GeoServerConfig) -> Option<Arc<dyn RasterStore>> {
    let rc = config.effective_raster();
    match rc.kind.as_str() {
        // Local directory backend (default). Future: "s3" / "minio".
        _ => {
            let dir = rc
                .dir
                .clone()
                .unwrap_or_else(|| config.data_dir.join("rasters"));
            Some(Arc::new(local::LocalRasterStore::new(dir)))
        },
    }
}
