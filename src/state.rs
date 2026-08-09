use crate::config::GeoServerConfig;
use crate::models::{layer::LayerGroup, Feature, Layer};
use crate::store::{
    build_raster_store, build_session_cache, build_vector_store, PostgresStore, RasterStore,
    SessionCache, SqliteStore, Store, VectorStore,
};
use crate::utils::sld_parser;
use crate::utils::tile_cache::TileCache;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct StyleMeta {
    pub title: String,
    pub is_builtin: bool,
    pub format: crate::models::style::StyleFormat,
}

pub type StyleMap = HashMap<String, String>;

/// 端点统计 (用于监控)
#[derive(Debug, Clone, Serialize)]
pub struct EndpointStats {
    pub count: u64,
    pub error_count: u64,
    pub avg_duration_ms: f64,
    pub max_duration_ms: f64,
}

impl Default for EndpointStats {
    fn default() -> Self {
        EndpointStats {
            count: 0,
            error_count: 0,
            avg_duration_ms: 0.0,
            max_duration_ms: 0.0,
        }
    }
}

/// 请求记录 (用于监控)
#[derive(Debug, Clone, Serialize)]
pub struct RequestRecord {
    pub id: u64,
    pub timestamp: String,
    pub method: String,
    pub path: String,
    pub status: u16,
    pub duration_ms: f64,
    pub user_agent: String,
    pub remote_addr: String,
}

pub struct AppState {
    pub config: GeoServerConfig,
    pub layers: Arc<RwLock<Vec<Layer>>>,
    pub features: Arc<RwLock<HashMap<String, Vec<Feature>>>>,
    pub styles: Arc<RwLock<HashMap<String, String>>>,
    pub styles_meta: Arc<RwLock<HashMap<String, StyleMeta>>>,
    pub store: Option<Arc<dyn Store>>,
    /// 矢量数据存储 (图层要素; 与元数据存储分离, 见 config::VectorConfig)
    pub vector_store: Option<Arc<dyn VectorStore>>,
    /// 栅格数据存储 (GeoTIFF/WorldImage/ArcGrid 文件; 见 config::RasterConfig)
    pub raster_store: Option<Arc<dyn RasterStore>>,
    /// 会话缓存 (会话快速层; 元数据存储为真源, 见 config::CacheConfig)
    pub session_cache: Option<Arc<dyn SessionCache>>,
    pub pg_pools: Arc<Mutex<HashMap<String, deadpool_postgres::Pool>>>,
    pub layer_groups: Arc<RwLock<Vec<LayerGroup>>>,
    pub start_time: Instant,
    pub request_count: AtomicU64,
    pub error_count: AtomicU64,
    /// 监控: 端点统计
    pub endpoint_stats: Arc<RwLock<HashMap<String, EndpointStats>>>,
    /// 监控: HTTP 方法统计
    pub method_stats: Arc<RwLock<HashMap<String, u64>>>,
    /// 监控: 状态码统计
    pub status_code_stats: Arc<RwLock<HashMap<u16, u64>>>,
    /// 监控: 最近请求日志 (最多 10000 条)
    pub request_log: Arc<RwLock<Vec<RequestRecord>>>,
    /// 监控: 近5分钟请求计数 (每秒重置)
    pub recent_request_count: AtomicU64,
    /// GeoWebCache 瓦片缓存引擎
    pub tile_cache: Option<TileCache>,
    /// OGC API - Processes 任务存储 (jobID -> OgcJob; 首版为同步执行)
    pub ogc_jobs: Arc<Mutex<HashMap<String, crate::services::ogc_processes::OgcJob>>>,
}

impl AppState {
    pub async fn new(config: GeoServerConfig) -> Self {
        // 根据配置选择元数据存储后端: "postgres" (集群) / "sqlite" (默认, 本地开发)
        let store: Option<Arc<dyn Store>> = match config.metadata.kind.as_str() {
            "postgres" => match PostgresStore::new(&config.metadata).await {
                Ok(s) => {
                    tracing::info!(
                        "Metadata store backend: PostgreSQL ({})",
                        config.metadata.postgres.instance
                    );
                    Some(Arc::new(s))
                },
                Err(e) => {
                    eprintln!("Failed to initialize PostgreSQL store: {}", e);
                    None
                },
            },
            _ => {
                let sqlite_path = config
                    .metadata
                    .sqlite_path
                    .to_str()
                    .unwrap_or("geoserver.sqlite");
                match SqliteStore::new(sqlite_path).await {
                    Ok(s) => {
                        tracing::info!("Metadata store backend: SQLite ({})", sqlite_path);
                        Some(Arc::new(s))
                    },
                    Err(e) => {
                        eprintln!("Failed to initialize SQLite store: {}", e);
                        None
                    },
                }
            },
        };

        // 构建矢量数据存储 (元数据与矢量数据分离; 见 config::VectorConfig)
        let vector_store = build_vector_store(&config).await;
        match &vector_store {
            Some(_) => {
                let eff = config.effective_vector();
                let detail = match eff.kind.as_str() {
                    "metadata" => "reuse metadata store".to_string(),
                    "postgres" => format!("PostgreSQL ({})", eff.postgres.instance),
                    _ => eff
                        .dir
                        .as_ref()
                        .map(|d| d.to_string_lossy().to_string())
                        .unwrap_or_default(),
                };
                tracing::info!("Vector data store backend: {} ({})", eff.kind, detail);
            },
            None => {
                tracing::warn!("Vector data store backend: none");
            },
        }

        // 构建栅格数据存储 (见 config::RasterConfig)
        let raster_store = build_raster_store(&config).await;
        match &raster_store {
            Some(_) => {
                let eff = config.effective_raster();
                let dir = eff
                    .dir
                    .as_ref()
                    .map(|d| d.to_string_lossy().to_string())
                    .unwrap_or_default();
                tracing::info!("Raster data store backend: {} ({})", eff.kind, dir);
            },
            None => {
                tracing::warn!("Raster data store backend: none");
            },
        }

        // 构建会话缓存 (瓦片缓存之外的会话快速层; 见 config::CacheConfig)
        let eff_cache = config.effective_cache();
        let session_cache = build_session_cache(&eff_cache);
        if session_cache.is_some() {
            tracing::info!(
                "Session cache backend: {} (ttl {}s)",
                eff_cache.kind,
                eff_cache.session_ttl_secs
            );
        }

        let config_layers: Vec<Layer> = config
            .workspaces
            .iter()
            .flat_map(|workspace| {
                workspace
                    .stores
                    .iter()
                    .flat_map(|store| {
                        store
                            .layers
                            .iter()
                            .map(|layer_config| {
                                Layer::new(
                                    layer_config.name.clone(),
                                    layer_config.title.clone(),
                                    workspace.name.clone(),
                                    store.name.clone(),
                                    crate::models::CoordinateReferenceSystem::from_epsg(
                                        &layer_config.srs,
                                    ),
                                )
                                .with_bounds(
                                    crate::models::BoundingBox::new(
                                        crate::models::CoordinateReferenceSystem::from_epsg(
                                            &layer_config.srs,
                                        ),
                                        crate::models::Bounds::new(
                                            layer_config.bounds.minx,
                                            layer_config.bounds.miny,
                                            layer_config.bounds.maxx,
                                            layer_config.bounds.maxy,
                                        ),
                                    ),
                                )
                            })
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>()
            })
            .collect();

        let mut all_layers = config_layers.clone();

        if let Some(ref store) = store {
            if let Ok(db_layers) = store.get_all_layers().await {
                for db_layer in db_layers {
                    let mut layer = Layer::new(
                        db_layer.name.clone(),
                        db_layer.title.clone(),
                        db_layer.workspace.clone(),
                        db_layer.store.clone(),
                        crate::models::CoordinateReferenceSystem::from_epsg(&db_layer.srs),
                    )
                    .with_bounds(crate::models::BoundingBox::new(
                        crate::models::CoordinateReferenceSystem::from_epsg(&db_layer.srs),
                        crate::models::Bounds::new(
                            db_layer.minx,
                            db_layer.miny,
                            db_layer.maxx,
                            db_layer.maxy,
                        ),
                    ));

                    if let Some(native_name) = db_layer.native_name {
                        layer.native_name = Some(native_name);
                    }

                    if !all_layers.iter().any(|l| l.name == layer.name) {
                        all_layers.push(layer);
                    }
                }
            }
        }

        let features_layers = if all_layers.is_empty() {
            vec![Layer::new(
                "world".to_string(),
                "World".to_string(),
                "default".to_string(),
                "shapes".to_string(),
                crate::models::CoordinateReferenceSystem::EPSG4326,
            )]
        } else {
            all_layers.clone()
        };

        let mut default_styles = HashMap::new();
        let mut styles_meta_map = HashMap::new();

        for builtin in sld_parser::builtin_styles() {
            default_styles.insert(builtin.name.to_string(), builtin.sld.to_string());
            styles_meta_map.insert(
                builtin.name.to_string(),
                StyleMeta {
                    title: builtin.title.to_string(),
                    is_builtin: true,
                    format: crate::models::style::StyleFormat::SLD,
                },
            );
        }

        for layer in &features_layers {
            let style_name = layer
                .styles
                .first()
                .map(|s| s.name.clone())
                .unwrap_or_else(|| "default".to_string());
            if !default_styles.contains_key(&style_name) {
                let sld = sld_parser::default_sld(&layer.name);
                default_styles.insert(style_name.clone(), sld);
                styles_meta_map.entry(style_name).or_insert(StyleMeta {
                    title: layer.title.clone(),
                    is_builtin: false,
                    format: crate::models::style::StyleFormat::SLD,
                });
            }
        }

        // 从存储加载持久化样式 / 图层组 (与内置样式合并)
        let mut layer_groups_init: Vec<LayerGroup> = Vec::new();
        if let Some(ref store) = store {
            if let Ok(style_records) = store.get_all_styles().await {
                for rec in style_records {
                    if !default_styles.contains_key(&rec.name) {
                        default_styles.insert(rec.name.clone(), rec.content.clone());
                        styles_meta_map
                            .entry(rec.name.clone())
                            .or_insert(StyleMeta {
                                title: rec.title.clone(),
                                is_builtin: rec.is_builtin,
                                format: parse_style_format(&rec.format),
                            });
                    }
                }
            }
            if let Ok(group_records) = store.get_all_layer_groups().await {
                for g in group_records {
                    layer_groups_init.push(LayerGroup {
                        name: g.name,
                        title: g.title,
                        layers: g.layers,
                        styles: g.styles,
                    });
                }
            }
        }

        // 初始化默认管理员
        if let Some(ref store) = store {
            crate::auth::ensure_default_admin(store.as_ref()).await;
        }

        // GeoWebCache 初始化 (后端按 [cache].kind 选择; 未配置 [cache] 时保持 None)
        let tile_cache = config
            .cache
            .as_ref()
            .map(|_| TileCache::new(config.effective_cache()));

        // 异步初始化瓦片缓存后端
        if let Some(ref cache) = tile_cache {
            if let Err(e) = cache.init().await {
                eprintln!("[GWC] 瓦片缓存初始化失败: {}", e);
            }
        }

        AppState {
            config,
            layers: Arc::new(RwLock::new(features_layers)),
            features: Arc::new(RwLock::new(HashMap::new())),
            styles: Arc::new(RwLock::new(default_styles)),
            styles_meta: Arc::new(RwLock::new(styles_meta_map)),
            layer_groups: Arc::new(RwLock::new(layer_groups_init)),
            store,
            vector_store,
            raster_store,
            session_cache,
            pg_pools: Arc::new(Mutex::new(HashMap::new())),
            start_time: Instant::now(),
            request_count: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            endpoint_stats: Arc::new(RwLock::new(HashMap::new())),
            method_stats: Arc::new(RwLock::new(HashMap::new())),
            status_code_stats: Arc::new(RwLock::new(HashMap::new())),
            request_log: Arc::new(RwLock::new(Vec::with_capacity(10000))),
            recent_request_count: AtomicU64::new(0),
            tile_cache,
            ogc_jobs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 获取或创建指定数据源的 PostgreSQL 连接池
    pub fn get_pg_pool(
        &self,
        ds_name: &str,
        conn_info: &crate::models::DataSourceConnection,
    ) -> deadpool_postgres::Pool {
        let start = Instant::now();

        let t1 = Instant::now();
        let mut pools = self.pg_pools.lock().unwrap();
        let lock_elapsed = t1.elapsed();
        tracing::debug!("[get_pg_pool] Mutex 锁定耗时: {:?}", lock_elapsed);

        if let Some(pool) = pools.get(ds_name) {
            tracing::debug!(
                "[get_pg_pool] 命中缓存, ds_name={}, 总耗时: {:?}",
                ds_name,
                start.elapsed()
            );
            return pool.clone();
        }

        let host_str = conn_info.host.as_deref().unwrap_or("127.0.0.1");
        let port_u16 = conn_info.port.unwrap_or(5432);
        tracing::debug!(
            "[get_pg_pool] 缓存未命中, 开始创建新连接池, ds_name={}, host={}, port={}",
            ds_name,
            host_str,
            port_u16
        );

        let t2 = Instant::now();
        let mut cfg = deadpool_postgres::Config::new();
        // 将 localhost 转为 127.0.0.1 以避免 IPv6 解析导致的连接超时
        let host = if host_str.eq_ignore_ascii_case("localhost") {
            "127.0.0.1".to_string()
        } else {
            host_str.to_string()
        };
        cfg.host = Some(host);
        cfg.port = Some(port_u16);
        cfg.dbname = conn_info
            .database
            .clone()
            .or_else(|| Some("geoserver".to_string()));
        cfg.user = conn_info
            .username
            .clone()
            .or_else(|| Some("postgres".to_string()));
        cfg.password = conn_info.password.clone();
        // 设置连接超时，避免因网络问题长时间挂起
        cfg.connect_timeout = Some(std::time::Duration::from_secs(
            self.config.server.connect_timeout_secs,
        ));
        cfg.manager = Some(deadpool_postgres::ManagerConfig {
            recycling_method: deadpool_postgres::RecyclingMethod::Fast,
        });

        let pool = cfg
            .create_pool(
                Some(deadpool_postgres::Runtime::Tokio1),
                tokio_postgres::NoTls,
            )
            .expect("Failed to create PG pool");
        let create_elapsed = t2.elapsed();
        tracing::debug!("[get_pg_pool] create_pool 耗时: {:?}", create_elapsed);

        pools.insert(ds_name.to_string(), pool.clone());
        tracing::debug!(
            "[get_pg_pool] 新连接池已创建并缓存, ds_name={}, 总耗时: {:?}",
            ds_name,
            start.elapsed()
        );
        pool
    }

    pub fn increment_request_count(&self) {
        self.request_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_error_count(&self) {
        self.error_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn get_uptime(&self) -> String {
        let duration = self.start_time.elapsed();
        let total_secs = duration.as_secs();
        let days = total_secs / 86400;
        let hours = (total_secs % 86400) / 3600;
        let minutes = (total_secs % 3600) / 60;
        format!("{}天 {}小时 {}分钟", days, hours, minutes)
    }

    pub async fn get_layer_features(&self, layer_name: &str) -> Option<Vec<Feature>> {
        let features = self.features.read().await;
        features.get(layer_name).cloned()
    }

    pub async fn add_layer_features(&self, layer_name: &str, features: Vec<Feature>) {
        let mut features_map = self.features.write().await;
        features_map.insert(layer_name.to_string(), features);
    }

    pub async fn add_feature(&self, layer_name: &str, feature: Feature) {
        let mut features_map = self.features.write().await;
        features_map
            .entry(layer_name.to_string())
            .or_insert_with(Vec::new)
            .push(feature);
    }

    pub async fn get_layer(&self, layer_name: &str) -> Option<Layer> {
        let layers = self.layers.read().await;
        layers.iter().find(|l| l.name == layer_name).cloned()
    }

    pub async fn list_layers(&self) -> Vec<Layer> {
        let layers = self.layers.read().await;
        layers.clone()
    }

    pub async fn get_style(&self, style_name: &str) -> Option<String> {
        let styles = self.styles.read().await;
        styles.get(style_name).cloned()
    }

    pub async fn add_style(&self, style_name: &str, content: String) {
        let mut styles = self.styles.write().await;
        styles.insert(style_name.to_string(), content);
    }

    pub async fn add_layer(&self, layer: Layer) {
        let mut layers = self.layers.write().await;
        layers.push(layer);
    }

    pub async fn update_layer(&self, layer_name: &str, updates: LayerUpdates) -> bool {
        let mut layers = self.layers.write().await;
        if let Some(layer) = layers.iter_mut().find(|l| l.name == layer_name) {
            if let Some(title) = updates.title {
                layer.title = title;
            }
            if let Some(abstract_text) = updates.abstract_text {
                layer.abstract_text = Some(abstract_text);
            }
            if let Some(enabled) = updates.enabled {
                layer.enabled = enabled;
            }
            true
        } else {
            false
        }
    }

    pub async fn delete_layer(&self, layer_name: &str) -> bool {
        let mut layers = self.layers.write().await;
        if let Some(pos) = layers.iter().position(|l| l.name == layer_name) {
            layers.remove(pos);
            let mut features_map = self.features.write().await;
            features_map.remove(layer_name);
            true
        } else {
            false
        }
    }
}

#[derive(Debug, Clone)]
pub struct LayerUpdates {
    pub title: Option<String>,
    pub abstract_text: Option<String>,
    pub enabled: Option<bool>,
}

/// 将存储中的格式字符串解析为 StyleFormat
fn parse_style_format(format: &str) -> crate::models::style::StyleFormat {
    match format {
        "CSS" => crate::models::style::StyleFormat::CSS,
        "YSLD" => crate::models::style::StyleFormat::YSLD,
        "MBStyle" => crate::models::style::StyleFormat::MBStyle,
        _ => crate::models::style::StyleFormat::SLD,
    }
}
