//! Tile cache backend abstraction — tile byte persistence.
//!
//! [`crate::utils::tile_cache::TileCache`] is the cache *engine* (enabled /
//! expiry / hit-rate statistics); the actual byte storage is delegated to a
//! [`TileCacheBackend`]. Backends:
//! - **local** — tiles on disk under `<cache_dir>/<layer>/<gridset>/<z>/<x>/<y>.png`
//!   (default, single-node / dev)
//! - **redis** — tiles as Redis string keys (`tile:{layer}:{gridset}:{z}:{x}:{y}`,
//!   TTL from `expire_after_secs`), shared across replicas in cloud deployments
//!
//! Backend selected via [`CacheConfig::kind`] (`local` | `redis`).

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use async_trait::async_trait;
use redis::aio::ConnectionManager;
use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::config::CacheConfig;
use crate::store::StoreError;

use super::redis::RedisConn;

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
        format!(
            "tile:{}:{}:{}:{}:{}",
            self.layer, self.gridset, self.z, self.x, self.y
        )
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

/// Metastore: one JSON file per `(layer, gridset)` recording every cached tile
/// (size + mtime). Enables fast stats and seed resume without directory walks.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct LayerMeta {
    layer: String,
    gridset: String,
    tiles: Vec<MetaTile>,
    last_updated: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MetaTile {
    z: u32,
    x: u32,
    y: u32,
    size: u64,
    mtime_unix: u64,
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

    /// Metastore file path: <meta_dir>/<layer>/<gridset>.json
    fn meta_path(&self, key: &TileCacheKey) -> PathBuf {
        let safe_gridset: String = key
            .gridset
            .chars()
            .map(|c| if c == ':' { '_' } else { c })
            .collect();
        self.config
            .meta_dir
            .join(&key.layer)
            .join(format!("{}.json", safe_gridset))
    }

    async fn read_meta(&self, path: &Path) -> Option<LayerMeta> {
        let bytes = fs::read(path).await.ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    async fn write_meta(&self, path: &Path, meta: &LayerMeta) {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent).await;
        }
        if let Ok(bytes) = serde_json::to_vec(meta) {
            let _ = fs::write(path, bytes).await;
        }
    }

    /// Record a tile in the metastore (upsert by z/x/y).
    async fn update_meta(&self, key: &TileCacheKey, size: u64) {
        let path = self.meta_path(key);
        let mut meta = self.read_meta(&path).await.unwrap_or(LayerMeta {
            layer: key.layer.clone(),
            gridset: key.gridset.clone(),
            tiles: vec![],
            last_updated: String::new(),
        });
        meta.tiles
            .retain(|t| !(t.z == key.z && t.x == key.x && t.y == key.y));
        meta.tiles.push(MetaTile {
            z: key.z,
            x: key.x,
            y: key.y,
            size,
            mtime_unix: now_unix(),
        });
        meta.last_updated = now_str();
        self.write_meta(&path, &meta).await;
    }

    /// Enforce `layer_quota_bytes`: when a layer's total cached size exceeds
    /// the quota, evict the oldest tiles (by file mtime) until under quota.
    /// Evicted tiles are also removed from the metastore.
    async fn enforce_quota(&self, layer: &str) {
        let quota = self.config.layer_quota_bytes;
        if quota == 0 {
            return;
        }
        let layer_dir = self.config.cache_dir.join(layer);
        let files = collect_layer_files(&layer_dir).await;
        let mut total: u64 = files.iter().map(|f| f.1).sum();
        if total <= quota {
            return;
        }
        // Oldest first (mtime ascending).
        let mut by_age: Vec<&(PathBuf, u64, u64)> = files.iter().collect();
        by_age.sort_by_key(|(_, _, mtime)| *mtime);
        let mut evicted: Vec<(u32, u32, u32)> = Vec::new();
        for (path, size, _) in by_age {
            if total <= quota {
                break;
            }
            if fs::remove_file(&path).await.is_ok() {
                total = total.saturating_sub(*size);
                if let Some((z, x, y)) = tile_xyz_from_path(path) {
                    evicted.push((z, x, y));
                }
            }
        }
        // 同步 metastore: 移除被淘汰的瓦片
        if !evicted.is_empty() {
            let meta_layer_dir = self.config.meta_dir.join(layer);
            if let Ok(mut entries) = fs::read_dir(&meta_layer_dir).await {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    let p = entry.path();
                    if let Some(mut meta) = self.read_meta(&p).await {
                        meta.tiles.retain(|t| {
                            !evicted
                                .iter()
                                .any(|(z, x, y)| *z == t.z && *x == t.x && *y == t.y)
                        });
                        self.write_meta(&p, &meta).await;
                    }
                }
            }
        }
        tracing::info!(
            "[GWC] quota eviction for layer '{}': {} bytes after eviction",
            layer,
            total
        );
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
                                key.layer,
                                key.z,
                                key.x,
                                key.y
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
                    key.layer,
                    key.z,
                    key.x,
                    key.y,
                    e
                );
                None
            },
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
                key.layer,
                key.z,
                key.x,
                key.y,
                e
            );
            return;
        }

        // Metastore + disk quota bookkeeping (best-effort, never blocks reads).
        self.update_meta(key, data.len() as u64).await;
        self.enforce_quota(&key.layer).await;
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
        // 同步清理 metastore
        let _ = fs::remove_dir_all(self.config.meta_dir.join(layer)).await;
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
            // meta_dir 是 cache_dir 的子目录 (默认 ./data/gwc/meta), 跳过
            if entry.path() == self.config.meta_dir {
                continue;
            }
            if entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
                total += count_files(&entry.path()).await;
                fs::remove_dir_all(&entry.path())
                    .await
                    .map_err(|e| StoreError::Other(e.to_string()))?;
            }
        }
        // 同步清理 metastore
        let _ = fs::remove_dir_all(&self.config.meta_dir).await;
        tracing::info!("[GWC] cleared all {} cached tiles", total);
        Ok(total)
    }

    async fn disk_stats(&self) -> TileCacheStats {
        // Fast path: aggregate the metastore files.
        let mut total = 0u64;
        let mut size = 0u64;
        let mut meta_found = false;
        if let Ok(mut entries) = fs::read_dir(&self.config.meta_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                if entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
                    // <meta_dir>/<layer>/<gridset>.json
                    if let Ok(mut inner) = fs::read_dir(entry.path()).await {
                        while let Ok(Some(file)) = inner.next_entry().await {
                            let name = file.file_name().to_string_lossy().to_string();
                            if !name.ends_with(".json") {
                                continue;
                            }
                            meta_found = true;
                            if let Some(meta) = self.read_meta(&file.path()).await {
                                total += meta.tiles.len() as u64;
                                size += meta.tiles.iter().map(|t| t.size).sum::<u64>();
                            }
                        }
                    }
                }
            }
        }
        if meta_found {
            return TileCacheStats {
                total_tiles: total,
                cache_size_bytes: size,
                ..Default::default()
            };
        }

        // Fallback: directory walk (no metastore yet).
        let mut total = 0u64;
        let mut size = 0u64;

        if let Ok(mut entries) = fs::read_dir(&self.config.cache_dir).await {
            while let Some(entry) = entries.next_entry().await.unwrap_or(None) {
                if entry.path() == self.config.meta_dir {
                    continue;
                }
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
/// The `local` backend is the built-in default. Layer-level Redis caches are
/// built explicitly from a Redis data source (see
/// [`RedisTileCacheBackend::new`]), not via global cache config.
pub fn build_tile_cache_backend(config: &CacheConfig) -> Arc<dyn TileCacheBackend> {
    Arc::new(LocalTileCacheBackend::new(config.clone()))
}

/// Redis tile cache backend — tiles stored as Redis string keys.
///
/// Key: `tile:{layer}:{gridset}:{z}:{x}:{y}` (matches
/// [`TileCacheKey::as_string`]). TTL derives from `expire_after_secs`
/// (0 = no expiry). Clears scan matching prefixes so `clear_layer` /
/// `clear_all` work on the shared store.
pub struct RedisTileCacheBackend {
    expire_after_secs: u64,
    conn: RedisConn,
}

impl RedisTileCacheBackend {
    /// Create a Redis-backed tile cache backend for the given Redis URL
    /// (e.g. from a `type = "redis"` data source connection).
    pub fn new(redis_url: &str, expire_after_secs: u64) -> Self {
        RedisTileCacheBackend {
            expire_after_secs,
            conn: RedisConn::new(redis_url),
        }
    }
}

/// Redis `SCAN` cursor helper: collect keys matching `pattern` in batches.
async fn scan_redis_keys(
    conn: &mut ConnectionManager,
    pattern: &str,
    batch: u64,
) -> Result<Vec<String>, StoreError> {
    let mut cursor: u64 = 0;
    let mut keys = Vec::new();
    loop {
        let (next, batch_keys): (u64, Vec<String>) = redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg(pattern)
            .arg("COUNT")
            .arg(batch)
            .query_async(conn)
            .await
            .map_err(|e| StoreError::Other(format!("Redis SCAN failed: {}", e)))?;
        keys.extend(batch_keys);
        cursor = next;
        if cursor == 0 {
            break;
        }
    }
    Ok(keys)
}

#[async_trait]
impl TileCacheBackend for RedisTileCacheBackend {
    async fn init(&self) -> Result<(), StoreError> {
        self.conn.ping().await?;
        tracing::info!(
            "[GWC] Redis tile cache backend ready (ttl {}s)",
            self.expire_after_secs
        );
        Ok(())
    }

    async fn get(&self, key: &TileCacheKey) -> Option<Vec<u8>> {
        let mut conn = self.conn.conn().await.ok()?;
        redis::cmd("GET")
            .arg(key.as_string())
            .query_async::<Option<Vec<u8>>>(&mut conn)
            .await
            .unwrap_or(None)
    }

    async fn put(&self, key: &TileCacheKey, data: &[u8]) {
        let mut conn = match self.conn.conn().await {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("[GWC] Redis unavailable, tile not cached: {}", e);
                return;
            },
        };
        let k = key.as_string();
        let res = if self.expire_after_secs > 0 {
            redis::cmd("SET")
                .arg(&k)
                .arg(data)
                .arg("EX")
                .arg(self.expire_after_secs)
                .query_async::<String>(&mut conn)
                .await
        } else {
            redis::cmd("SET")
                .arg(&k)
                .arg(data)
                .query_async::<String>(&mut conn)
                .await
        };
        if let Err(e) = res {
            tracing::warn!(
                "[GWC] Redis write error layer={} z={} x={} y={}: {}",
                key.layer,
                key.z,
                key.x,
                key.y,
                e
            );
        }
    }

    async fn clear_layer(&self, layer: &str) -> Result<u64, StoreError> {
        let mut conn = self.conn.conn().await?;
        let pattern = format!("tile:{}:*", layer);
        let keys = scan_redis_keys(&mut conn, &pattern, 500).await?;
        if !keys.is_empty() {
            redis::cmd("DEL")
                .arg(keys.clone())
                .query_async::<u64>(&mut conn)
                .await
                .map_err(|e| StoreError::Other(format!("Redis DEL failed: {}", e)))?;
        }
        tracing::info!(
            "[GWC] Redis: cleared {} cached tiles for layer '{}'",
            keys.len(),
            layer
        );
        Ok(keys.len() as u64)
    }

    async fn clear_all(&self) -> Result<u64, StoreError> {
        let mut conn = self.conn.conn().await?;
        let keys = scan_redis_keys(&mut conn, "tile:*", 500).await?;
        if !keys.is_empty() {
            redis::cmd("DEL")
                .arg(keys.clone())
                .query_async::<u64>(&mut conn)
                .await
                .map_err(|e| StoreError::Other(format!("Redis DEL failed: {}", e)))?;
        }
        tracing::info!("[GWC] Redis: cleared all {} cached tiles", keys.len());
        Ok(keys.len() as u64)
    }

    async fn disk_stats(&self) -> TileCacheStats {
        let mut conn = match self.conn.conn().await {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("[GWC] Redis stats unavailable: {}", e);
                return TileCacheStats::default();
            },
        };
        let keys = match scan_redis_keys(&mut conn, "tile:*", 500).await {
            Ok(k) => k,
            Err(e) => {
                tracing::warn!("[GWC] Redis stats scan failed: {}", e);
                return TileCacheStats::default();
            },
        };
        // Sum value sizes via STRLEN (batched) for an approximate cache size.
        let mut size = 0u64;
        for chunk in keys.chunks(100) {
            let mut pipe = redis::pipe();
            for k in chunk {
                pipe.cmd("STRLEN").arg(k);
            }
            let lens: Vec<u64> = match pipe.query_async(&mut conn).await {
                Ok(l) => l,
                Err(_) => break,
            };
            size += lens.iter().sum::<u64>();
        }
        TileCacheStats {
            total_tiles: keys.len() as u64,
            cache_size_bytes: size,
            ..Default::default()
        }
    }
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

/// Collect every tile file under a layer directory as `(path, size, mtime_unix)`.
async fn collect_layer_files(dir: &Path) -> Vec<(PathBuf, u64, u64)> {
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        if let Ok(mut entries) = fs::read_dir(&current).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                if entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
                    stack.push(entry.path());
                } else {
                    let size = entry.metadata().await.map(|m| m.len()).unwrap_or(0);
                    let mtime = entry
                        .metadata()
                        .await
                        .and_then(|m| m.modified())
                        .ok()
                        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                        .map(|d| d.as_nanos() as u64)
                        .unwrap_or(0);
                    files.push((entry.path(), size, mtime));
                }
            }
        }
    }
    files
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn now_str() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// Reverse-map a tile file path to `(z, x, y)`: `<...>/<z>/<x>/<y>.png`.
fn tile_xyz_from_path(path: &Path) -> Option<(u32, u32, u32)> {
    let y = path.file_stem()?.to_str()?.parse::<u32>().ok()?;
    let parent = path.parent()?;
    let x = parent.file_name()?.to_str()?.parse::<u32>().ok()?;
    let z = parent
        .parent()?
        .file_name()?
        .to_str()?
        .parse::<u32>()
        .ok()?;
    Some((z, x, y))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CacheConfig;

    fn test_config(tag: &str) -> CacheConfig {
        let dir =
            std::env::temp_dir().join(format!("terrane-tile-test-{}-{}", tag, std::process::id()));
        CacheConfig {
            kind: "local".to_string(),
            cache_dir: dir.clone(),
            meta_dir: dir.join("meta"),
            expire_after_secs: 0,
            max_tiles: 0,
            layer_quota_bytes: 0,
            enabled: true,
            default_gridset: "EPSG:4326".to_string(),
            session_ttl_secs: 300,
        }
    }

    fn sample_key() -> TileCacheKey {
        TileCacheKey {
            layer: "world".to_string(),
            gridset: "EPSG:4326".to_string(),
            z: 0,
            x: 0,
            y: 0,
        }
    }

    fn test_config_with_quota(tag: &str, layer_quota_bytes: u64) -> CacheConfig {
        let mut cfg = test_config(tag);
        cfg.layer_quota_bytes = layer_quota_bytes;
        cfg
    }

    #[actix_rt::test]
    async fn test_put_get_roundtrip() {
        let backend = LocalTileCacheBackend::new(test_config("put"));
        backend.init().await.unwrap();
        let key = sample_key();

        assert!(backend.get(&key).await.is_none(), "未 put 前应 miss");
        backend.put(&key, b"tile-bytes-123").await;
        assert_eq!(backend.get(&key).await, Some(b"tile-bytes-123".to_vec()));

        std::fs::remove_dir_all(&backend.config.cache_dir).ok();
    }

    #[actix_rt::test]
    async fn test_tile_path_sanitizes_gridset() {
        let backend = LocalTileCacheBackend::new(test_config("path"));
        let p = backend.tile_path(&sample_key());
        let s = p.to_string_lossy().to_string();
        assert!(
            s.contains("EPSG_4326"),
            "gridset 中的 ':' 应消毒为 '_', 实际: {}",
            s
        );
        assert!(!s.contains("EPSG:4326"), "路径中不应含 ':'");
        assert!(s.ends_with("0.png"));
    }

    #[actix_rt::test]
    async fn test_clear_layer_and_clear_all() {
        let backend = LocalTileCacheBackend::new(test_config("clear"));
        backend.init().await.unwrap();

        let k1 = sample_key();
        let k2 = TileCacheKey {
            layer: "world".into(),
            gridset: "EPSG:4326".into(),
            z: 1,
            x: 1,
            y: 1,
        };
        let k3 = TileCacheKey {
            layer: "ocean".into(),
            gridset: "EPSG:4326".into(),
            z: 0,
            x: 0,
            y: 0,
        };
        backend.put(&k1, b"a").await;
        backend.put(&k2, b"b").await;
        backend.put(&k3, b"c").await;

        let cleared = backend.clear_layer("world").await.unwrap();
        assert_eq!(cleared, 2, "world 图层应清除 2 张瓦片");

        assert!(backend.get(&k1).await.is_none());
        assert!(backend.get(&k3).await.is_some(), "其他图层瓦片应保留");

        let all = backend.clear_all().await.unwrap();
        assert_eq!(all, 1, "clear_all 应清除剩余 1 张");
        assert!(backend.get(&k3).await.is_none());

        std::fs::remove_dir_all(&backend.config.cache_dir).ok();
    }

    #[actix_rt::test]
    async fn test_disk_stats() {
        let backend = LocalTileCacheBackend::new(test_config("stats"));
        backend.init().await.unwrap();
        backend.put(&sample_key(), b"12345").await;

        let stats = backend.disk_stats().await;
        assert_eq!(stats.total_tiles, 1);
        assert!(stats.cache_size_bytes >= 5);

        std::fs::remove_dir_all(&backend.config.cache_dir).ok();
    }

    #[actix_rt::test]
    async fn test_metastore_written_and_stats_fast_path() {
        let backend = LocalTileCacheBackend::new(test_config("meta"));
        backend.init().await.unwrap();
        let k1 = sample_key();
        let k2 = TileCacheKey {
            layer: "world".into(),
            gridset: "EPSG:4326".into(),
            z: 0,
            x: 0,
            y: 1,
        };
        backend.put(&k1, b"aaa").await;
        backend.put(&k2, b"bbbb").await;

        // Metastore 文件存在且记录两条
        let meta_path = backend.meta_path(&k1);
        assert!(meta_path.exists(), "metastore 文件应写入: {:?}", meta_path);
        let meta = backend.read_meta(&meta_path).await.expect("metastore 可读");
        assert_eq!(meta.layer, "world");
        assert_eq!(meta.tiles.len(), 2, "应记录 2 条瓦片元数据");

        // disk_stats 走 metastore 快路径 (total 精确)
        let stats = backend.disk_stats().await;
        assert_eq!(stats.total_tiles, 2);
        assert_eq!(stats.cache_size_bytes, 7);

        std::fs::remove_dir_all(&backend.config.cache_dir).ok();
    }

    #[actix_rt::test]
    async fn test_layer_quota_evicts_oldest() {
        // 配额仅容纳 1 片: 第二片写入后触发 LRU 淘汰, 层内只剩 1 片
        let backend = LocalTileCacheBackend::new(test_config_with_quota("quota", 5));
        backend.init().await.unwrap();
        let k1 = sample_key(); // (0,0,0)
        let k2 = TileCacheKey {
            layer: "world".into(),
            gridset: "EPSG:4326".into(),
            z: 0,
            x: 0,
            y: 1,
        };
        backend.put(&k1, b"aaaaa").await; // 5 字节, 等于配额
        backend.put(&k2, b"bbbbb").await; // 再写 5 字节 → 10 > 5 → 淘汰最老 (k1)

        assert!(
            backend.get(&k1).await.is_none(),
            "配额淘汰应移除最早写入的瓦片 k1"
        );
        assert!(backend.get(&k2).await.is_some(), "最新写入的瓦片 k2 应保留");

        let stats = backend.disk_stats().await;
        assert_eq!(stats.total_tiles, 1, "配额生效后层内应只剩 1 片");

        std::fs::remove_dir_all(&backend.config.cache_dir).ok();
    }
}
