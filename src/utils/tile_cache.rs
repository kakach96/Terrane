//! # GeoWebCache — 瓦片缓存引擎
//!
//! 提供瓦片缓存引擎 (启用开关 / 过期策略 / 命中率统计) 与 Gridset 定义。
//! 瓦片字节的持久化委托给 [`crate::store::cache::TileCacheBackend`]
//! (本地磁盘, 未来可扩展 Redis/S3)。
//! 缓存路径: `<cache_dir>/<layer>/<gridset>/<zoom>/<x>/<y>.png`

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::config::CacheConfig;
use crate::store::cache::{
    build_tile_cache_backend, TileCacheBackend, TileCacheKey, TileCacheStats,
};
use crate::store::StoreError;

// ---------------------------------------------------------------------------
// Gridset 定义
// ---------------------------------------------------------------------------

/// 常见瓦片网格集
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Gridset {
    /// EPSG:4326 — 全球地理坐标系 (1x1 顶层瓦片)
    #[serde(rename = "EPSG:4326")]
    Epsg4326,
    /// EPSG:3857 — Web Mercator (Google/Bing/OSM 标准)
    #[serde(rename = "EPSG:3857")]
    Epsg3857,
    /// EPSG:900913 — Google Mercator (EPSG:3857 别名)
    #[serde(rename = "EPSG:900913")]
    Epsg900913,
}

impl Gridset {
    pub fn name(&self) -> &'static str {
        match self {
            Gridset::Epsg4326 => "EPSG:4326",
            Gridset::Epsg3857 => "EPSG:3857",
            Gridset::Epsg900913 => "EPSG:900913",
        }
    }

    /// 获取该 Gridset 的顶层瓦片数 (1x1, 2x1, 或 2x2)
    pub fn top_level_tiles(&self) -> (u32, u32) {
        match self {
            Gridset::Epsg4326 => (2, 1),                       // 全球分 2x1
            Gridset::Epsg3857 | Gridset::Epsg900913 => (1, 1), // 全球分 1x1
        }
    }

    /// 最大缩放级别
    pub fn max_zoom(&self) -> u32 {
        match self {
            Gridset::Epsg4326 => 21,
            Gridset::Epsg3857 | Gridset::Epsg900913 => 20,
        }
    }

    /// 瓦片大小 (像素)
    pub fn tile_size(&self) -> u32 {
        256
    }

    /// 将 tile 坐标转为地理范围 (minx, miny, maxx, maxy)
    pub fn tile_bounds(&self, z: u32, x: u32, y: u32) -> Option<(f64, f64, f64, f64)> {
        let n = 2.0_f64.powi(z as i32);
        match self {
            Gridset::Epsg4326 => {
                let minx = (x as f64 / n) * 360.0 - 180.0;
                let maxx = ((x + 1) as f64 / n) * 360.0 - 180.0;
                let miny = (y as f64 / n) * 180.0 - 90.0;
                let maxy = ((y + 1) as f64 / n) * 180.0 - 90.0;
                Some((minx, miny, maxx, maxy))
            },
            Gridset::Epsg3857 | Gridset::Epsg900913 => {
                let minx = (x as f64 / n) * 360.0 - 180.0;
                let maxx = ((x + 1) as f64 / n) * 360.0 - 180.0;
                let sin_lat = |y: f64| -> f64 {
                    let v = std::f64::consts::PI * (1.0 - 2.0 * y / n);
                    v.cos().recip().ln().atan().to_degrees()
                };
                let miny = sin_lat(y as f64 + 1.0).max(-85.0511);
                let maxy = sin_lat(y as f64).min(85.0511);
                Some((minx, miny, maxx, maxy))
            },
        }
    }
}

// ---------------------------------------------------------------------------
// 缓存条目元数据
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TileMetaEntry {
    pub layer: String,
    pub gridset: String,
    pub z: u32,
    pub x: u32,
    pub y: u32,
    pub size_bytes: u64,
    pub created_at: String,
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerTileMeta {
    pub layer: String,
    pub gridsets: Vec<String>,
    pub tiles: Vec<TileMetaEntry>,
    pub last_updated: String,
}

// ---------------------------------------------------------------------------
// 缓存引擎
// ---------------------------------------------------------------------------

pub struct TileCache {
    pub config: CacheConfig,
    backend: Arc<dyn TileCacheBackend>,
    hits: AtomicU64,
    misses: AtomicU64,
}

impl TileCache {
    /// 使用默认 (本地磁盘) 后端创建瓦片缓存引擎。
    pub fn new(config: CacheConfig) -> Self {
        Self::with_backend(config.clone(), build_tile_cache_backend(&config))
    }

    /// 使用指定后端创建瓦片缓存引擎 (供未来 Redis/S3 等后端使用)。
    pub fn with_backend(config: CacheConfig, backend: Arc<dyn TileCacheBackend>) -> Self {
        Self {
            config,
            backend,
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        }
    }

    /// 初始化缓存后端
    pub async fn init(&self) -> Result<(), StoreError> {
        self.backend.init().await
    }

    /// 检查瓦片是否在缓存中
    pub async fn get(&self, layer: &str, gridset: &str, z: u32, x: u32, y: u32) -> Option<Vec<u8>> {
        if !self.config.enabled {
            return None;
        }
        let key = TileCacheKey {
            layer: layer.to_string(),
            gridset: gridset.to_string(),
            z,
            x,
            y,
        };
        match self.backend.get(&key).await {
            Some(data) => {
                self.hits.fetch_add(1, Ordering::Relaxed);
                Some(data)
            },
            None => {
                self.misses.fetch_add(1, Ordering::Relaxed);
                None
            },
        }
    }

    /// 保存瓦片到缓存
    pub async fn put(&self, layer: &str, gridset: &str, z: u32, x: u32, y: u32, data: &[u8]) {
        if !self.config.enabled {
            return;
        }
        let key = TileCacheKey {
            layer: layer.to_string(),
            gridset: gridset.to_string(),
            z,
            x,
            y,
        };
        self.backend.put(&key, data).await;
    }

    /// 清除指定图层的所有缓存
    pub async fn clear_layer(&self, layer: &str) -> Result<u64, StoreError> {
        self.backend.clear_layer(layer).await
    }

    /// 清除所有缓存
    pub async fn clear_all(&self) -> Result<u64, StoreError> {
        self.backend.clear_all().await
    }

    /// 获取缓存统计 (命中/未命中)
    pub fn stats(&self) -> TileCacheStats {
        TileCacheStats {
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
            total_tiles: 0,
            cache_size_bytes: 0,
        }
    }

    /// 遍历缓存目录统计瓦片数量和大小 (异步，可能较慢)
    pub async fn calculate_disk_stats(&self) -> TileCacheStats {
        let mut st = self.backend.disk_stats().await;
        st.hits = self.hits.load(Ordering::Relaxed);
        st.misses = self.misses.load(Ordering::Relaxed);
        st
    }

    /// 缓存命中率
    pub fn hit_rate(&self) -> f64 {
        let hits = self.hits.load(Ordering::Relaxed) as f64;
        let misses = self.misses.load(Ordering::Relaxed) as f64;
        let total = hits + misses;
        if total > 0.0 {
            hits / total
        } else {
            0.0
        }
    }
}

// ---------------------------------------------------------------------------
// 测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CacheConfig;

    #[test]
    fn test_gridset_epsg4326_name() {
        assert_eq!(Gridset::Epsg4326.name(), "EPSG:4326");
        assert_eq!(Gridset::Epsg3857.name(), "EPSG:3857");
    }

    #[test]
    fn test_gridset_top_level_tiles() {
        assert_eq!(Gridset::Epsg4326.top_level_tiles(), (2, 1));
        assert_eq!(Gridset::Epsg3857.top_level_tiles(), (1, 1));
    }

    #[test]
    fn test_gridset_max_zoom() {
        assert!(Gridset::Epsg4326.max_zoom() > 0);
        assert_eq!(Gridset::Epsg4326.tile_size(), 256);
    }

    #[test]
    fn test_tile_bounds_epsg4326_z0() {
        // z=0, n=2^0=1, 瓦片覆盖全球
        let bounds = Gridset::Epsg4326.tile_bounds(0, 0, 0).unwrap();
        assert!((bounds.0 - (-180.0)).abs() < 0.001); // minx
        assert!((bounds.1 - (-90.0)).abs() < 0.001); // miny
        assert!((bounds.2 - 180.0).abs() < 0.001); // maxx = 全球
        assert!((bounds.3 - 90.0).abs() < 0.001); // maxy
    }

    #[test]
    fn test_tile_bounds_epsg4326_z1() {
        // z=1, x=1, y=0 (右上角)
        let bounds = Gridset::Epsg4326.tile_bounds(1, 1, 0).unwrap();
        assert_eq!(bounds.0, 0.0); // minx
        assert_eq!(bounds.1, -90.0); // miny
        assert_eq!(bounds.2, 180.0); // maxx
        assert_eq!(bounds.3, 0.0); // maxy
    }

    #[test]
    fn test_tile_bounds_epsg3857_z0() {
        let bounds = Gridset::Epsg3857.tile_bounds(0, 0, 0).unwrap();
        assert!(bounds.0 < -100.0); // 应该在 -180 附近
        assert!(bounds.1 <= -85.0); // 应该在 -85.0511 附近
        assert!(bounds.2 > 100.0); // 应该在 180 附近
        assert!(bounds.3 >= 85.0); // 应该在 85.0511 附近
    }

    #[test]
    fn test_cache_config_default() {
        let config = CacheConfig::default();
        assert_eq!(config.kind, "local");
        assert!(config.enabled);
        assert_eq!(config.expire_after_secs, 86400);
        assert_eq!(config.max_tiles, 100_000);
        assert_eq!(config.default_gridset, "EPSG:4326");
        assert_eq!(config.session_ttl_secs, 86400);
    }

    #[tokio::test]
    async fn test_tile_put_get_disk_layout() {
        let mut config = CacheConfig::default();
        let dir = std::env::temp_dir().join(format!("terrane-gwc-test-{}", std::process::id()));
        config.cache_dir = dir.clone();
        config.meta_dir = dir.join("meta");
        let cache = TileCache::new(config);
        let _ = cache.init().await;

        // put 一个瓦片, 验证落在 <dir>/test_layer/EPSG_4326/5/10/15.png
        // (gridset 中的 ':' 在文件系统路径中被消毒为 '_', 兼容 Windows)
        cache
            .put("test_layer", "EPSG:4326", 5, 10, 15, b"tile-bytes")
            .await;
        let expected = dir
            .join("test_layer")
            .join("EPSG_4326")
            .join("5")
            .join("10")
            .join("15.png");
        assert!(
            expected.exists(),
            "tile file should exist at {:?}",
            expected
        );

        let got = cache.get("test_layer", "EPSG:4326", 5, 10, 15).await;
        assert_eq!(got.as_deref(), Some(&b"tile-bytes"[..]));

        // 清理
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_tile_cache_new() {
        let config = CacheConfig::default();
        let cache = TileCache::new(config);
        assert_eq!(cache.stats().hits, 0);
        assert_eq!(cache.stats().misses, 0);
        assert_eq!(cache.hit_rate(), 0.0);
    }

    #[test]
    fn test_hit_rate_calculation() {
        let config = CacheConfig::default();
        let cache = TileCache::new(config);
        // 初始状态
        assert_eq!(cache.hit_rate(), 0.0);
        // 部分命中: 2 hits, 3 misses = 40%
        cache.hits.fetch_add(2, Ordering::Relaxed);
        cache.misses.fetch_add(3, Ordering::Relaxed);
        let rate = cache.hit_rate();
        assert!((rate - 0.4).abs() < 0.001);
    }
}
