use crate::config::TerraneConfig;
use crate::models::{layer::LayerGroup, Layer};
use crate::store::{build_session_cache, PostgresStore, SessionCache, SqliteStore, Store};
use crate::utils::cascaded::CascadedCircuits;
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
    pub config: TerraneConfig,
    pub layers: Arc<RwLock<Vec<Layer>>>,
    pub styles: Arc<RwLock<HashMap<String, String>>>,
    pub styles_meta: Arc<RwLock<HashMap<String, StyleMeta>>>,
    pub store: Option<Arc<dyn Store>>,
    /// 会话缓存 (会话快速层; 元数据存储为真源)
    pub session_cache: Option<Arc<dyn SessionCache>>,
    pub pg_pools: Arc<Mutex<HashMap<String, deadpool_postgres::Pool>>>,
    /// MySQL 连接池缓存 (按数据源名称; 惰性构建并缓存, 仿 pg_pools)
    pub mysql_pools: Arc<Mutex<HashMap<String, mysql_async::Pool>>>,
    /// MongoDB 客户端缓存 (按数据源名称; mongodb::Client 内部自带连接池)
    pub mongo_clients: Arc<Mutex<HashMap<String, mongodb::Client>>>,
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
    /// GeoWebCache 瓦片缓存引擎 (默认本地后端)
    pub tile_cache: Option<TileCache>,
    /// 图层级 Redis 瓦片缓存 (按数据源名称; 惰性构建并缓存)
    pub redis_tile_caches: Arc<Mutex<HashMap<String, TileCache>>>,
    /// 级联 WMS 熔断器 (按上游 URL 隔离)
    pub cascaded_circuits: Arc<CascadedCircuits>,
    /// OGC API - Processes 任务存储 (jobID -> OgcJob; 首版为同步执行)
    pub ogc_jobs: Arc<Mutex<HashMap<String, crate::services::ogc_processes::OgcJob>>>,
    /// WFS 要素锁注册表 (LockFeature / GetFeatureWithLock; 进程内内存实现)
    pub wfs_locks: Arc<crate::utils::wfs_lock::WfsLockRegistry>,
    /// OGC 服务设置缓存 (service -> settings; 元数据存储为真源, 写后即更新)
    pub service_settings: Arc<RwLock<HashMap<String, crate::models::ServiceSettings>>>,
    /// 瓦片种子任务表 (jobID -> SeedJob; GWC 风格 seed/truncate)
    pub seed_jobs: crate::utils::tile_seed::SeedJobTable,
}

impl AppState {
    pub async fn new(config: TerraneConfig) -> Self {
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
                    .unwrap_or("terrane.sqlite");
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

        // 构建会话缓存 (会话快速层; 元数据存储为真源; 内置默认 local)
        let session_cache = build_session_cache(&config.cache);
        if session_cache.is_some() {
            tracing::info!(
                "Session cache backend: {} (ttl {}s)",
                config.cache.kind,
                config.cache.session_ttl_secs
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
                                .with_bounds(crate::models::BoundingBox::new(
                                    crate::models::CoordinateReferenceSystem::from_epsg(
                                        &layer_config.srs,
                                    ),
                                    crate::models::Bounds::new(
                                        layer_config.bounds.minx,
                                        layer_config.bounds.miny,
                                        layer_config.bounds.maxx,
                                        layer_config.bounds.maxy,
                                    ),
                                ))
                                .with_style(layer_config.style.clone())
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

                    layer.cache_store = db_layer.cache_store.clone();

                    if !all_layers.iter().any(|l| l.name == layer.name) {
                        all_layers.push(layer);
                    }
                }
            }
        }

        let features_layers = if all_layers.is_empty() {
            // 开箱即用: 首次启动自动注册内置示例数据 (opt-in via [samples] enabled),
            // 再创建内置 world 图层 (store = metadata)。
            let mut seeded: Vec<Layer> = Vec::new();
            if config.samples.enabled {
                if let Some(ref store) = store {
                    seeded = crate::utils::samples::seed_samples(&config, store).await;
                }
            }

            // metadata 数据源为内置选项, 被当作普通数据源看待: postgres 元数据模式
            // 复用同一 PG 发布业务表 / sqlite 元数据模式不承载业务表。
            let world_layer = Layer::new(
                "world".to_string(),
                "World".to_string(),
                "default".to_string(),
                crate::models::METADATA_DATA_SOURCE.to_string(),
                crate::models::CoordinateReferenceSystem::EPSG4326,
            );
            if let Some(ref store) = store {
                // 持久化到元数据库 (下次启动从 DB 加载, 不重复创建)
                let _ = store
                    .create_layer(&crate::store::types::Layer {
                        name: "world".to_string(),
                        title: "World".to_string(),
                        workspace: "default".to_string(),
                        store: crate::models::METADATA_DATA_SOURCE.to_string(),
                        srs: "EPSG:4326".to_string(),
                        abstract_text: None,
                        native_name: None,
                        enabled: true,
                        minx: -180.0,
                        miny: -90.0,
                        maxx: 180.0,
                        maxy: 90.0,
                        cache_store: None,
                        created: String::new(),
                        modified: String::new(),
                    })
                    .await;
            }
            seeded.push(world_layer);
            seeded
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

        // GeoWebCache 初始化 (内置默认 local 瓦片缓存)
        let tile_cache = Some(TileCache::new(config.cache.clone()));

        // 异步初始化瓦片缓存后端
        if let Some(ref cache) = tile_cache {
            if let Err(e) = cache.init().await {
                eprintln!("[GWC] 瓦片缓存初始化失败: {}", e);
            }
        }

        // 级联 WMS 熔断器 (阈值 0 = 禁用; 按上游 URL 隔离)
        let cascaded_circuits = Arc::new(CascadedCircuits::new(
            config.server.cascaded_circuit_threshold,
            std::time::Duration::from_secs(config.server.cascaded_circuit_reset_secs),
        ));

        // 从元数据存储加载 OGC 服务设置 (service -> settings)
        let mut service_settings_init: HashMap<String, crate::models::ServiceSettings> =
            HashMap::new();
        if let Some(ref store) = store {
            if let Ok(map) = store.get_service_settings().await {
                service_settings_init = map;
            }
        }

        // 目录定时刷新: 多副本部署时周期性地从元数据存储重载图层/样式/图层组,
        // 收敛副本间内存缓存差异 (`[server] catalog_refresh_secs`, 0 = 禁用)
        let catalog_refresh_secs = config.server.catalog_refresh_secs;
        if catalog_refresh_secs > 0 && store.is_some() {
            tracing::info!(
                "Catalog refresh enabled: reload from metadata store every {}s",
                catalog_refresh_secs
            );
        }

        let app_state = AppState {
            config,
            layers: Arc::new(RwLock::new(features_layers)),
            styles: Arc::new(RwLock::new(default_styles)),
            styles_meta: Arc::new(RwLock::new(styles_meta_map)),
            layer_groups: Arc::new(RwLock::new(layer_groups_init)),
            store,
            session_cache,
            pg_pools: Arc::new(Mutex::new(HashMap::new())),
            mysql_pools: Arc::new(Mutex::new(HashMap::new())),
            mongo_clients: Arc::new(Mutex::new(HashMap::new())),
            start_time: Instant::now(),
            request_count: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            endpoint_stats: Arc::new(RwLock::new(HashMap::new())),
            method_stats: Arc::new(RwLock::new(HashMap::new())),
            status_code_stats: Arc::new(RwLock::new(HashMap::new())),
            request_log: Arc::new(RwLock::new(Vec::with_capacity(10000))),
            recent_request_count: AtomicU64::new(0),
            tile_cache,
            redis_tile_caches: Arc::new(Mutex::new(HashMap::new())),
            cascaded_circuits,
            ogc_jobs: Arc::new(Mutex::new(HashMap::new())),
            wfs_locks: Arc::new(crate::utils::wfs_lock::WfsLockRegistry::new()),
            service_settings: Arc::new(RwLock::new(service_settings_init)),
            seed_jobs: Arc::new(Mutex::new(HashMap::new())),
        };

        // 启动目录定时刷新任务 (独立 tokio 任务, 不影响请求路径):
        // 从元数据存储周期重载图层/样式/图层组到内存缓存, 收敛多副本间差异。
        if catalog_refresh_secs > 0 {
            let refresh_layers = app_state.layers.clone();
            let refresh_styles = app_state.styles.clone();
            let refresh_styles_meta = app_state.styles_meta.clone();
            let refresh_groups = app_state.layer_groups.clone();
            let refresh_store = app_state.store.clone();
            tokio::spawn(async move {
                let mut ticker =
                    tokio::time::interval(std::time::Duration::from_secs(catalog_refresh_secs));
                loop {
                    ticker.tick().await;
                    refresh_catalog_from_store(
                        &refresh_store,
                        &refresh_layers,
                        &refresh_styles,
                        &refresh_styles_meta,
                        &refresh_groups,
                    )
                    .await;
                }
            });
        }

        app_state
    }

    /// 事件驱动目录刷新: REST 写路径 (图层/样式/图层组/数据源 CRUD) 成功后
    /// 立即从元数据存储重载内存目录, 消除"写后读旧"窗口 (周期刷新之外)。
    pub async fn refresh_catalog(&self) {
        refresh_catalog_from_store(
            &self.store,
            &self.layers,
            &self.styles,
            &self.styles_meta,
            &self.layer_groups,
        )
        .await;
    }

    /// 获取或创建指定数据源的 PostgreSQL 连接池
    pub fn get_pg_pool(
        &self,
        ds_name: &str,
        conn_info: &crate::models::DataSourceConnection,
    ) -> deadpool_postgres::Pool {
        let start = Instant::now();

        // 解析 ${ENV_VAR} 引用的凭据 (K8s Secrets 注入), 仅本地副本, 不落库
        let mut resolved = conn_info.clone();
        crate::utils::secrets::resolve_connection_secrets(&mut resolved);
        let conn_info = &resolved;

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
        // 集群连接: host 为完整连接串 (postgres://…, 含多主机 URL) 时交给
        // deadpool 的 url 字段解析; 否则解析逗号分隔主机列表 (见 utils/cluster.rs)。
        // 先用 tokio-postgres 校验连接串, 避免无效输入在 create_pool 处 panic。
        let mut cfg = deadpool_postgres::Config::new();
        if crate::utils::cluster::is_connection_string(host_str) {
            match <tokio_postgres::Config as std::str::FromStr>::from_str(host_str) {
                Ok(_) => cfg.url = Some(host_str.to_string()),
                Err(e) => {
                    tracing::warn!(
                        "[get_pg_pool] 无效的连接串, 回退默认主机: {}, err={}",
                        host_str,
                        e
                    );
                    cfg.host = Some("127.0.0.1".to_string());
                    cfg.port = Some(5432);
                },
            }
        } else {
            let hosts = crate::utils::cluster::parse_host_list(
                conn_info.host.as_deref(),
                conn_info.port,
                5432,
            );
            if hosts.len() == 1 {
                cfg.host = Some(hosts[0].host.clone());
                cfg.port = Some(hosts[0].port);
            } else {
                // 多主机: deadpool/tokio-postgres 原生按序故障转移
                cfg.hosts = Some(hosts.iter().map(|h| h.host.clone()).collect());
                cfg.ports = Some(hosts.iter().map(|h| h.port).collect());
            }
        }
        // 连接串模式下 dbname/user/password 已含在 URI 中, 不再以字段默认值覆盖。
        if !crate::utils::cluster::is_connection_string(host_str) {
            cfg.dbname = conn_info
                .database
                .clone()
                .or_else(|| Some("terrane".to_string()));
            cfg.user = conn_info
                .username
                .clone()
                .or_else(|| Some("postgres".to_string()));
            cfg.password = conn_info.password.clone();
        }
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

    /// 获取或创建指定数据源的 MySQL 连接池 (按数据源名称缓存)。
    pub fn get_mysql_pool(
        &self,
        ds_name: &str,
        conn_info: &crate::models::DataSourceConnection,
    ) -> mysql_async::Pool {
        let mut resolved = conn_info.clone();
        crate::utils::secrets::resolve_connection_secrets(&mut resolved);
        let conn_info = &resolved;

        let mut pools = self.mysql_pools.lock().unwrap();
        if let Some(pool) = pools.get(ds_name) {
            return pool.clone();
        }

        let database = conn_info
            .database
            .clone()
            .unwrap_or_else(|| "terrane".to_string());
        let user = conn_info
            .username
            .clone()
            .unwrap_or_else(|| "root".to_string());
        let password = conn_info.password.clone();

        // 集群连接: 逗号分隔主机列表 → 依次 TCP 探活, 池绑定第一个可达节点
        // (mysql_async 连接池为单主机; 未探到可达节点时退回首主机)。
        let hosts =
            crate::utils::cluster::parse_host_list(conn_info.host.as_deref(), conn_info.port, 3306);
        let chosen = if hosts.len() > 1 {
            crate::utils::cluster::first_reachable_tcp(
                &hosts,
                std::time::Duration::from_secs(self.config.server.connect_timeout_secs.clamp(1, 3)),
            )
            .unwrap_or_else(|| hosts[0].clone())
        } else {
            hosts[0].clone()
        };
        tracing::debug!(
            "[get_mysql_pool] ds_name={}, 主机列表 {} 项, 选中 {}:{}",
            ds_name,
            hosts.len(),
            chosen.host,
            chosen.port
        );

        let opts = mysql_async::OptsBuilder::default()
            .ip_or_hostname(chosen.host)
            .tcp_port(chosen.port)
            .db_name(Some(database))
            .user(Some(user))
            .pass(password)
            // 连接等待超时 (秒), 避免网络问题长时间挂起
            .wait_timeout(Some(self.config.server.connect_timeout_secs.max(1) as usize));
        let pool = mysql_async::Pool::new(opts);
        pools.insert(ds_name.to_string(), pool.clone());
        tracing::debug!("[get_mysql_pool] 新连接池已创建并缓存, ds_name={}", ds_name);
        pool
    }

    /// 获取或创建指定数据源的 MongoDB 客户端 (按数据源名称缓存)。
    /// 获取或创建指定数据源的 MongoDB 客户端 (按数据源名称缓存)。
    pub async fn get_mongo_client(
        &self,
        ds_name: &str,
        conn_info: &crate::models::DataSourceConnection,
    ) -> mongodb::Client {
        {
            let clients = self.mongo_clients.lock().unwrap();
            if let Some(client) = clients.get(ds_name) {
                return client.clone();
            }
        }

        let mut resolved = conn_info.clone();
        crate::utils::secrets::resolve_connection_secrets(&mut resolved);
        let conn_info = &resolved;

        // 集群连接: host 为 mongodb:// / mongodb+srv:// 完整 URI 时直接使用;
        // 否则由逗号分隔主机列表 + replica_set 组装副本集 URI (见 utils/cluster.rs)。
        let uri = crate::utils::cluster::mongo_uri_from_connection(conn_info);

        let client = match mongodb::Client::with_uri_str(&uri).await {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(
                    "[get_mongo_client] 无效的 MongoDB URI, 回退默认主机: {}, err={}",
                    uri,
                    e
                );
                mongodb::Client::with_uri_str("mongodb://127.0.0.1:27017")
                    .await
                    .unwrap_or_else(|_| unreachable!("default mongodb URI is valid"))
            },
        };
        let mut clients = self.mongo_clients.lock().unwrap();
        clients.insert(ds_name.to_string(), client.clone());
        tracing::debug!(
            "[get_mongo_client] 新客户端已创建并缓存, ds_name={}",
            ds_name
        );
        client
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
            if let Some(cache_store) = updates.cache_store {
                layer.cache_store = cache_store;
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
            true
        } else {
            false
        }
    }

    /// 解析图层使用的瓦片缓存引擎。
    ///
    /// - 图层设置了 `cache_store` (指向 `type = "redis"` 的数据源) → 返回该
    ///   Redis 数据源驱动的瓦片缓存 (惰性构建并按数据源缓存, 多副本共享)。
    /// - 否则 (未设置 / 数据源缺失 / 非 redis / 连接信息不完整) → 回退到默认
    ///   (本地磁盘) 缓存, 保证缓存始终可用。
    pub async fn tile_cache_for(&self, layer: &Layer) -> Option<TileCache> {
        let ds_name = match layer.cache_store.as_deref() {
            Some(name) if !name.trim().is_empty() => name,
            _ => return self.tile_cache.clone(),
        };

        // 已构建过的 Redis 缓存直接复用
        {
            let caches = self.redis_tile_caches.lock().unwrap();
            if let Some(cache) = caches.get(ds_name) {
                return Some(cache.clone());
            }
        }

        // 校验数据源: 必须是 type = "redis" 且启用; 任何解析失败都回退默认缓存
        let store = match self.store.as_ref() {
            Some(s) => s.clone(),
            None => return self.tile_cache.clone(),
        };
        let ds = match store.get_data_source(ds_name).await {
            Ok(Some(ds)) => ds,
            _ => return self.tile_cache.clone(),
        };
        if ds.data_source_type != crate::models::DataSourceType::Redis || !ds.enabled {
            return self.tile_cache.clone();
        }
        let conn = match ds.connection.as_ref() {
            Some(c) => c,
            None => return self.tile_cache.clone(),
        };
        let url = match crate::store::cache::redis::redis_url_from_connection(conn) {
            Some(u) => u,
            None => return self.tile_cache.clone(),
        };

        let mut cache_config = self.config.cache.clone();
        cache_config.enabled = true;
        let backend: std::sync::Arc<dyn crate::store::cache::TileCacheBackend> =
            std::sync::Arc::new(crate::store::cache::RedisTileCacheBackend::new(
                &url,
                cache_config.expire_after_secs,
            ));
        let cache = TileCache::with_backend(cache_config, backend);

        let mut caches = self.redis_tile_caches.lock().unwrap();
        caches.insert(ds_name.to_string(), cache.clone());
        tracing::info!(
            "[GWC] layer '{}' uses Redis cache data source '{}' ({})",
            layer.name,
            ds_name,
            url
        );
        Some(cache)
    }
}

#[derive(Debug, Clone)]
pub struct LayerUpdates {
    pub title: Option<String>,
    pub abstract_text: Option<String>,
    pub enabled: Option<bool>,
    /// 图层级瓦片缓存后端数据源 (Some(ds) = 设置, Some(None) = 清除)
    pub cache_store: Option<Option<String>>,
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

/// 目录刷新: 从元数据存储重载图层/样式/图层组到内存缓存。
///
/// 多副本部署时各副本的内存目录 (`Arc<RwLock<...>>`) 会因本副本的 REST 写入
/// 或外部修改而发散; 周期性调用本函数可从共享元数据存储收敛差异:
/// - 图层: 按名称更新/新增 (不删除, 保留本副本内未持久化/内置的图层)
/// - 样式: 按名称更新/新增 (保留内置样式与本副本独有样式)
/// - 图层组: 按名称更新/新增
///
/// 失败 (存储不可用) 时静默返回, 下个周期重试, 不影响请求路径。
async fn refresh_catalog_from_store(
    store: &Option<Arc<dyn Store>>,
    layers: &Arc<RwLock<Vec<Layer>>>,
    styles: &Arc<RwLock<HashMap<String, String>>>,
    styles_meta: &Arc<RwLock<HashMap<String, StyleMeta>>>,
    layer_groups: &Arc<RwLock<Vec<LayerGroup>>>,
) {
    let store = match store {
        Some(s) => s.clone(),
        None => return,
    };

    // 1. 图层
    if let Ok(db_layers) = store.get_all_layers().await {
        let mut layers_guard = layers.write().await;
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
            layer.cache_store = db_layer.cache_store.clone();
            match layers_guard.iter_mut().find(|l| l.name == layer.name) {
                Some(existing) => *existing = layer,
                None => layers_guard.push(layer),
            }
        }
    }

    // 2. 样式 — 覆盖式更新 (元数据存储为真源; 内置样式不在存储中则保持本地)
    if let Ok(style_records) = store.get_all_styles().await {
        let mut styles_guard = styles.write().await;
        let mut meta_guard = styles_meta.write().await;
        for rec in style_records {
            styles_guard.insert(rec.name.clone(), rec.content.clone());
            meta_guard.insert(
                rec.name.clone(),
                StyleMeta {
                    title: rec.title.clone(),
                    is_builtin: rec.is_builtin,
                    format: parse_style_format(&rec.format),
                },
            );
        }
    }

    // 3. 图层组
    if let Ok(group_records) = store.get_all_layer_groups().await {
        let mut groups_guard = layer_groups.write().await;
        for g in group_records {
            let group = LayerGroup {
                name: g.name.clone(),
                title: g.title,
                layers: g.layers,
                styles: g.styles,
            };
            match groups_guard.iter_mut().find(|x| x.name == group.name) {
                Some(existing) => *existing = group,
                None => groups_guard.push(group),
            }
        }
    }

    tracing::debug!("[catalog] refreshed from metadata store");
}
