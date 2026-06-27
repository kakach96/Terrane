//! # GeoWebCache — 瓦片缓存引擎
//!
//! 提供瓦片磁盘缓存、Gridset 定义、缓存统计等功能。
//! 缓存路径: `<data_dir>/gwc/<layer>/<gridset>/<zoom>/<x>/<y>.png`

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::fs;
use serde::{Deserialize, Serialize};
use tracing::{info, debug, warn};

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
            Gridset::Epsg4326 => (2, 1),  // 全球分 2x1
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
            }
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
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 缓存配置
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GwcConfig {
    /// 瓦片缓存根目录 (默认: "<data_dir>/gwc")
    pub cache_dir: PathBuf,
    /// 缓存元数据目录
    pub meta_dir: PathBuf,
    /// 瓦片过期时间 (秒, 0=永不过期)
    #[serde(default = "default_expire")]
    pub expire_after_secs: u64,
    /// 最大缓存瓦片数 (0=无限制)
    #[serde(default)]
    pub max_tiles: u64,
    /// 是否启用磁盘缓存
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// 默认 gridset
    #[serde(default = "default_gridset")]
    pub default_gridset: String,
}

fn default_expire() -> u64 { 86400 }  // 24 小时
fn default_enabled() -> bool { true }
fn default_gridset() -> String { "EPSG:4326".to_string() }

impl Default for GwcConfig {
    fn default() -> Self {
        GwcConfig {
            cache_dir: PathBuf::from("./data/gwc"),
            meta_dir: PathBuf::from("./data/gwc/meta"),
            expire_after_secs: 86400,
            max_tiles: 100_000,
            enabled: true,
            default_gridset: "EPSG:4326".to_string(),
        }
    }
}

impl GwcConfig {
    /// 瓦片缓存文件路径: <cache_dir>/<layer>/<gridset>/<z>/<x>/<y>.png
    pub fn tile_path(&self, layer: &str, gridset: &str, z: u32, x: u32, y: u32) -> PathBuf {
        self.cache_dir.join(layer).join(gridset)
            .join(z.to_string()).join(x.to_string())
            .join(format!("{}.png", y))
    }

    /// 元数据文件路径
    pub fn meta_path(&self, layer: &str) -> PathBuf {
        self.meta_dir.join(format!("{}.json", layer))
    }
}

// ---------------------------------------------------------------------------
// 缓存统计
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Clone, Serialize)]
pub struct TileCacheStats {
    pub hits: u64,
    pub misses: u64,
    pub total_tiles: u64,
    pub cache_size_bytes: u64,
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
    pub config: GwcConfig,
    hits: AtomicU64,
    misses: AtomicU64,
}

impl TileCache {
    pub fn new(config: GwcConfig) -> Self {
        Self {
            config,
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        }
    }

    /// 初始化缓存目录
    pub async fn init(&self) -> Result<(), std::io::Error> {
        if !self.config.enabled {
            info!("[GWC] 瓦片缓存已禁用");
            return Ok(());
        }
        fs::create_dir_all(&self.config.cache_dir).await?;
        fs::create_dir_all(&self.config.meta_dir).await?;
        info!("[GWC] 瓦片缓存初始化完成: {:?}", self.config.cache_dir);
        Ok(())
    }

    /// 检查瓦片是否在缓存中
    pub async fn get(&self, layer: &str, gridset: &str, z: u32, x: u32, y: u32) -> Option<Vec<u8>> {
        if !self.config.enabled {
            return None;
        }

        let path = self.config.tile_path(layer, gridset, z, x, y);
        if !path.exists() {
            self.misses.fetch_add(1, Ordering::Relaxed);
            debug!("[GWC] MISS  layer={} z={} x={} y={}", layer, z, x, y);
            return None;
        }

        // 检查是否过期
        if self.config.expire_after_secs > 0 {
            if let Ok(metadata) = fs::metadata(&path).await {
                if let Ok(modified) = metadata.modified() {
                    if let Ok(elapsed) = std::time::SystemTime::now().duration_since(modified) {
                        if elapsed.as_secs() > self.config.expire_after_secs {
                            debug!("[GWC] EXPIRED layer={} z={} x={} y={}", layer, z, x, y);
                            // 异步删除过期瓦片
                            let path_clone = path.clone();
                            tokio::spawn(async move {
                                let _ = fs::remove_file(&path_clone).await;
                            });
                            self.misses.fetch_add(1, Ordering::Relaxed);
                            return None;
                        }
                    }
                }
            }
        }

        match fs::read(&path).await {
            Ok(data) => {
                self.hits.fetch_add(1, Ordering::Relaxed);
                debug!("[GWC] HIT  layer={} z={} x={} y={} ({} bytes)", layer, z, x, y, data.len());
                Some(data)
            }
            Err(e) => {
                warn!("[GWC] READ_ERROR layer={} z={} x={} y={}: {}", layer, z, x, y, e);
                self.misses.fetch_add(1, Ordering::Relaxed);
                None
            }
        }
    }

    /// 保存瓦片到缓存
    pub async fn put(&self, layer: &str, gridset: &str, z: u32, x: u32, y: u32, data: &[u8]) {
        if !self.config.enabled {
            return;
        }

        let path = self.config.tile_path(layer, gridset, z, x, y);

        // 创建目录
        if let Some(parent) = path.parent() {
            if let Err(e) = fs::create_dir_all(parent).await {
                warn!("[GWC] 无法创建缓存目录 {:?}: {}", parent, e);
                return;
            }
        }

        match fs::write(&path, data).await {
            Ok(_) => {
                debug!("[GWC] PUT  layer={} z={} x={} y={} ({} bytes)", layer, z, x, y, data.len());
            }
            Err(e) => {
                warn!("[GWC] WRITE_ERROR layer={} z={} x={} y={}: {}", layer, z, x, y, e);
            }
        }
    }

    /// 清除指定图层的所有缓存
    pub async fn clear_layer(&self, layer: &str) -> Result<u64, std::io::Error> {
        let layer_dir = self.config.cache_dir.join(layer);
        if !layer_dir.exists() {
            return Ok(0);
        }
        let count = count_files(&layer_dir).await;
        fs::remove_dir_all(&layer_dir).await?;
        info!("[GWC] 已清除图层 '{}' 的 {} 个缓存瓦片", layer, count);
        Ok(count)
    }

    /// 清除所有缓存
    pub async fn clear_all(&self) -> Result<u64, std::io::Error> {
        let mut total = 0u64;
        let mut entries = fs::read_dir(&self.config.cache_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_dir() {
                let count = count_files(&entry.path()).await;
                fs::remove_dir_all(&entry.path()).await?;
                total += count;
            }
        }
        info!("[GWC] 已清除全部 {} 个缓存瓦片", total);
        Ok(total)
    }

    /// 获取缓存统计
    pub fn stats(&self) -> TileCacheStats {
        TileCacheStats {
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
            total_tiles: 0, // 运行时计算开销大，按需从文件系统统计
            cache_size_bytes: 0,
        }
    }

    /// 遍历缓存目录统计瓦片数量和大小 (异步，可能较慢)
    pub async fn calculate_disk_stats(&self) -> TileCacheStats {
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
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
            total_tiles: total,
            cache_size_bytes: size,
        }
    }

    /// 缓存命中率
    pub fn hit_rate(&self) -> f64 {
        let hits = self.hits.load(Ordering::Relaxed) as f64;
        let misses = self.misses.load(Ordering::Relaxed) as f64;
        let total = hits + misses;
        if total > 0.0 { hits / total } else { 0.0 }
    }
}

// ---------------------------------------------------------------------------
// 辅助函数
// ---------------------------------------------------------------------------

/// 递归统计目录下的文件数 (使用 Box::pin 避免递归 async fn 大小问题)
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

/// 递归统计目录下的文件数和总大小 (使用迭代，避免递归 async fn)
async fn count_files_and_size(dir: &Path) -> (u64, u64) {
    let mut stack = vec![dir.to_path_buf()];
    let mut count = 0u64;
    let mut size = 0u64;
    while let Some(current) = stack.pop() {
        if let Ok(mut entries) = fs::read_dir(&current).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                if entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
                    stack.push(entry.path());
                } else if let Ok(meta) = entry.metadata().await {
                    count += 1;
                    size += meta.len();
                }
            }
        }
    }
    (count, size)
}
