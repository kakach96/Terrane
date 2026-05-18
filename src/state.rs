use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use tokio::sync::RwLock;
use std::time::Instant;
use crate::config::GeoServerConfig;
use crate::models::{Layer, Feature};
use crate::store::SqliteStore;

pub struct AppState {
    pub config: GeoServerConfig,
    pub layers: Arc<RwLock<Vec<Layer>>>,
    pub features: Arc<RwLock<HashMap<String, Vec<Feature>>>>,
    pub styles: Arc<RwLock<HashMap<String, String>>>,
    pub store: Option<Arc<SqliteStore>>,
    pub pg_pools: Arc<Mutex<HashMap<String, deadpool_postgres::Pool>>>,
    pub start_time: Instant,
    pub request_count: AtomicU64,
    pub error_count: AtomicU64,
}

impl AppState {
    pub async fn new(config: GeoServerConfig) -> Self {
        let sqlite_path = config.database.sqlite_path.to_str().unwrap_or("geoserver.sqlite");
        let store = match SqliteStore::new(sqlite_path).await {
            Ok(s) => Some(Arc::new(s)),
            Err(e) => {
                eprintln!("Failed to initialize SQLite store: {}", e);
                None
            }
        };

        let config_layers: Vec<Layer> = config.workspaces.iter()
            .flat_map(|workspace| {
                workspace.stores.iter().flat_map(|store| {
                    store.layers.iter().map(|layer_config| {
                        Layer::new(
                            layer_config.name.clone(),
                            layer_config.title.clone(),
                            workspace.name.clone(),
                            store.name.clone(),
                            crate::models::CoordinateReferenceSystem::from_epsg(&layer_config.srs),
                        ).with_bounds(crate::models::BoundingBox::new(
                            crate::models::CoordinateReferenceSystem::from_epsg(&layer_config.srs),
                            crate::models::Bounds::new(
                                layer_config.bounds.minx,
                                layer_config.bounds.miny,
                                layer_config.bounds.maxx,
                                layer_config.bounds.maxy,
                            ),
                        ))
                    }).collect::<Vec<_>>()
                }).collect::<Vec<_>>()
            }).collect();

        let mut all_layers = config_layers.clone();

        if let Some(ref store) = store {
            if let Ok(db_layers) = store.get_all_layers().await {
                for db_layer in db_layers {
                    let layer = Layer::new(
                        db_layer.name.clone(),
                        db_layer.title.clone(),
                        db_layer.workspace.clone(),
                        db_layer.store.clone(),
                        crate::models::CoordinateReferenceSystem::from_epsg(&db_layer.srs),
                    ).with_bounds(crate::models::BoundingBox::new(
                        crate::models::CoordinateReferenceSystem::from_epsg(&db_layer.srs),
                        crate::models::Bounds::new(
                            db_layer.minx,
                            db_layer.miny,
                            db_layer.maxx,
                            db_layer.maxy,
                        ),
                    ));

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

        AppState {
            config,
            layers: Arc::new(RwLock::new(features_layers)),
            features: Arc::new(RwLock::new(HashMap::new())),
            styles: Arc::new(RwLock::new(HashMap::new())),
            store,
            pg_pools: Arc::new(Mutex::new(HashMap::new())),
            start_time: Instant::now(),
            request_count: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
        }
    }

    /// 获取或创建指定数据源的 PostgreSQL 连接池
    pub fn get_pg_pool(&self, ds_name: &str, conn_info: &crate::models::DataSourceConnection) -> deadpool_postgres::Pool {
        let start = Instant::now();

        let t1 = Instant::now();
        let mut pools = self.pg_pools.lock().unwrap();
        let lock_elapsed = t1.elapsed();
        tracing::debug!("[get_pg_pool] Mutex 锁定耗时: {:?}", lock_elapsed);

        if let Some(pool) = pools.get(ds_name) {
            tracing::debug!("[get_pg_pool] 命中缓存, ds_name={}, 总耗时: {:?}", ds_name, start.elapsed());
            return pool.clone();
        }

        tracing::debug!("[get_pg_pool] 缓存未命中, 开始创建新连接池, ds_name={}, host={}, port={}", ds_name, conn_info.host, conn_info.port);

        let t2 = Instant::now();
        let mut cfg = deadpool_postgres::Config::new();
        // 将 localhost 转为 127.0.0.1 以避免 IPv6 解析导致的连接超时
        let host = if conn_info.host.eq_ignore_ascii_case("localhost") {
            "127.0.0.1".to_string()
        } else {
            conn_info.host.clone()
        };
        cfg.host = Some(host);
        cfg.port = Some(conn_info.port);
        cfg.dbname = Some(conn_info.database.clone());
        cfg.user = Some(conn_info.username.clone());
        cfg.password = conn_info.password.clone();
        // 设置连接超时，避免因网络问题长时间挂起
        cfg.connect_timeout = Some(std::time::Duration::from_secs(self.config.server.connect_timeout_secs));
        cfg.manager = Some(deadpool_postgres::ManagerConfig {
            recycling_method: deadpool_postgres::RecyclingMethod::Fast,
        });

        let pool = cfg.create_pool(
            Some(deadpool_postgres::Runtime::Tokio1),
            tokio_postgres::NoTls,
        ).expect("Failed to create PG pool");
        let create_elapsed = t2.elapsed();
        tracing::debug!("[get_pg_pool] create_pool 耗时: {:?}", create_elapsed);

        pools.insert(ds_name.to_string(), pool.clone());
        tracing::debug!("[get_pg_pool] 新连接池已创建并缓存, ds_name={}, 总耗时: {:?}", ds_name, start.elapsed());
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
