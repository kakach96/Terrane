//! Tile cache backend abstraction — tile byte persistence.
//!
//! [`crate::utils::tile_cache::TileCache`] is the cache *engine* (enabled /
//! expiry / hit-rate statistics); the actual byte storage is delegated to a
//! [`TileCacheBackend`]. The local backend persists tiles on disk under
//! `<cache_dir>/<layer>/<gridset>/<z>/<x>/<y>.png`.
//!
//! Future backends: Redis / S3.

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use serde::Serialize;
use tokio::fs;

use crate::config::CacheConfig;
use crate::store::StoreError;

/// Cache key identifying a single tile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileCacheKey {
    pub layer: String,
    pub gridset: String,
    pub z: u32,
    pub x: u32,
    pub y: u32,
}

impl TileCacheKey {
    /// Canonical string form of the key (used by non-disk backends later).
    pub fn as_string(&self) -> String {
        format!("tile:{}:{}:{}:{}:{}", self.layer, self.gridset, self.z, self.x, self.y)
    }
}

/// Tile cache statistics.
#[derive(Debug, Default, Clone, Serialize)]
pub struct TileCacheStats {
    pub hits: u64,
    pub misses: u64,
    pub total_tiles: u64,
    pub cache_size_bytes: u64,
}

/// Tile cache backend abstraction — tile byte persistence.
#[async_trait]
pub trait TileCacheBackend: Send + Sync {
    /// Initialize the backend (create directories, connect, etc.).
    async fn init(&self) -> Result<(), StoreError>;
    /// Look up a cached tile.
    async fn get(&self, key: &TileCacheKey) -> Option<Vec<u8>>;
    /// Store a tile (may be a no-op when caching is disabled).
    async fn put(&self, key: &TileCacheKey, data: &[u8]);
    /// Remove all tiles for a layer.
    async fn clear_layer(&self, layer: &str) -> Result<u64, StoreError>;
    /// Remove all cached tiles.
    async fn clear_all(&self) -> Result<u64, StoreError>;
    /// Expensive backend stats (tile count + size on disk / in store).
    async fn disk_stats(&self) -> TileCacheStats;
}

/// Local (disk) tile cache backend.
pub struct LocalTileCacheBackend {
    config: CacheConfig,
}

impl LocalTileCacheBackend {
    pub fn new(config: CacheConfig) -> Self {
        Self { config }
    }

    /// Tile file path: <cache_dir>/<layer>/<gridset>/<z>/<x>/<y>.png
    ///
    /// Gridset names like `EPSG:4326` contain `:` which is an invalid character
    /// in Windows file/folder names, so the filesystem component is sanitized
    /// (`:` -> `_`) while the logical [`TileCacheKey`] keeps the canonical name.
    fn tile_path(&self, key: &TileCacheKey) -> PathBuf {
        let safe_gridset: String = key
            .gridset
            .chars()
            .map(|c| if c == ':' { '_' } else { c })
            .collect();
        self.config
            .cache_dir
            .join(&key.layer)
            .join(&safe_gridset)
            .join(key.z.to_string())
            .join(key.x.to_string())
            .join(format!("{}.png", key.y))
    }
}

#[async_trait]
impl TileCacheBackend for LocalTileCacheBackend {
    async fn init(&self) -> Result<(), StoreError> {
        if !self.config.enabled {
            tracing::info!("[GWC] tile cache disabled");
            return Ok(());
        }
        fs::create_dir_all(&self.config.cache_dir)
            .await
            .map_err(|e| StoreError::Other(e.to_string()))?;
        fs::create_dir_all(&self.config.meta_dir)
            .await
            .map_err(|e| StoreError::Other(e.to_string()))?;
        tracing::info!("[GWC] tile cache initialized: {:?}", self.config.cache_dir);
        Ok(())
    }

    async fn get(&self, key: &TileCacheKey) -> Option<Vec<u8>> {
        let path = self.tile_path(key);
        if !path.exists() {
            return None;
        }

        // Expiry check based on file mtime (only for the disk backend).
        if self.config.expire_after_secs > 0 {
            if let Ok(metadata) = fs::metadata(&path).await {
                if let Ok(modified) = metadata.modified() {
                    if let Ok(elapsed) = std::time::SystemTime::now().duration_since(modified) {
                        if elapsed.as_secs() > self.config.expire_after_secs {
                            tracing::debug!(
                                "[GWC] EXPIRED layer={} z={} x={} y={}",
                                key.layer, key.z, key.x, key.y
                            );
                            let path_clone = path.clone();
                            tokio::spawn(async move {
                                let _ = fs::remove_file(&path_clone).await;
                            });
                            return None;
                        }
                    }
                }
            }
        }

        match fs::read(&path).await {
            Ok(data) => Some(data),
            Err(e) => {
                tracing::warn!(
                    "[GWC] READ_ERROR layer={} z={} x={} y={}: {}",
                    key.layer, key.z, key.x, key.y, e
                );
                None
            }
        }
    }

    async fn put(&self, key: &TileCacheKey, data: &[u8]) {
        let path = self.tile_path(key);

        if let Some(parent) = path.parent() {
            if let Err(e) = fs::create_dir_all(parent).await {
                tracing::warn!("[GWC] cannot create cache dir {:?}: {}", parent, e);
                return;
            }
        }

        if let Err(e) = fs::write(&path, data).await {
            tracing::warn!(
                "[GWC] WRITE_ERROR layer={} z={} x={} y={}: {}",
                key.layer, key.z, key.x, key.y, e
            );
        }
    }

    async fn clear_layer(&self, layer: &str) -> Result<u64, StoreError> {
        let layer_dir = self.config.cache_dir.join(layer);
        if !layer_dir.exists() {
            return Ok(0);
        }
        let count = count_files(&layer_dir).await;
        fs::remove_dir_all(&layer_dir)
            .await
            .map_err(|e| StoreError::Other(e.to_string()))?;
        tracing::info!("[GWC] cleared {} cached tiles for layer '{}'", count, layer);
        Ok(count)
    }

    async fn clear_all(&self) -> Result<u64, StoreError> {
        let mut total = 0u64;
        let mut entries = fs::read_dir(&self.config.cache_dir)
            .await
            .map_err(|e| StoreError::Other(e.to_string()))?;
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| StoreError::Other(e.to_string()))?
        {
            if entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
                total += count_files(&entry.path()).await;
                fs::remove_dir_all(&entry.path())
                    .await
                    .map_err(|e| StoreError::Other(e.to_string()))?;
            }
        }
        tracing::info!("[GWC] cleared all {} cached tiles", total);
        Ok(total)
    }

    async fn disk_stats(&self) -> TileCacheStats {
        let mut total = 0u64;
        let mut size = 0u64;

        if let Ok(mut entries) = fs::read_dir(&self.config.cache_dir).await {
            while let Some(entry) = entries.next_entry().await.unwrap_or(None) {
                if entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
                    let (c, s) = count_files_and_size(&entry.path()).await;
                    total += c;
                    size += s;
                }
            }
        }

        TileCacheStats {
            total_tiles: total,
            cache_size_bytes: size,
            ..Default::default()
        }
    }
}

/// Build the tile cache backend selected by [`CacheConfig::kind`].
///
/// Future: `"redis"` -> a Redis-backed [`TileCacheBackend`].
pub fn build_tile_cache_backend(config: &CacheConfig) -> Arc<dyn TileCacheBackend> {
    Arc::new(LocalTileCacheBackend::new(config.clone()))
}

/// Recursively count files under a directory.
async fn count_files(dir: &Path) -> u64 {
    let mut stack = vec![dir.to_path_buf()];
    let mut count = 0u64;
    while let Some(current) = stack.pop() {
        if let Ok(mut entries) = fs::read_dir(&current).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                if entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
                    stack.push(entry.path());
                } else {
                    count += 1;
                }
            }
        }
    }
    count
}

/// Recursively count files and total size under a directory.
async fn count_files_and_size(dir: &Path) -> (u64, u64) {
    let mut stack = vec![dir.to_path_buf()];
    let mut count = 0u64;
    let mut size = 0u64;
    while let Some(current) = stack.pop() {
        if let Ok(mut entries) = fs::read_dir(&current).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                if entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
                    stack.push(entry.path());
                } else {
                    count += 1;
                    if let Ok(meta) = entry.metadata().await {
                        size += meta.len();
                    }
                }
            }
        }
    }
    (count, size)
}
