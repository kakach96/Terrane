//! 矢量数据存储抽象层 — 图层要素 (矢量业务数据) 的持久化。
//!
//! 与元数据存储 ([`crate::store::Store`]) 分离, 便于矢量数据使用独立的
//! 数据库 / NFS / 对象存储等后端。后端通过 [`crate::config::VectorConfig::kind`] 选择:
//! - `local` — 本地目录 (每图层一个 GeoJSON 文件, 支持 NFS/对象存储挂载)
//! - `metadata` — 复用元数据存储 (内置默认选项; 元数据为外部存储时的默认)
//! - `postgres` — 独立 PostgreSQL 矢量存储

pub mod local_dir;
pub mod postgres;

use std::sync::Arc;

use async_trait::async_trait;

use crate::config::GeoServerConfig;
use crate::models::Feature;
use crate::store::{SqliteStore, StoreError};

/// 矢量数据存储抽象 — 图层要素的持久化接口。
///
/// 未来新增后端 (如 s3 / oss / minio 对象存储) 时实现本 trait 并在
/// [`build_vector_store`] 中注册即可。
#[async_trait]
pub trait VectorStore: Send + Sync {
    /// 保存指定图层的全部要素 (覆盖写)
    async fn save_features(&self, layer_name: &str, features: &[Feature]) -> Result<usize, StoreError>;
    /// 加载指定图层的全部要素
    async fn load_features(&self, layer_name: &str) -> Result<Vec<Feature>, StoreError>;
    /// 删除指定图层的全部要素
    async fn delete_features(&self, layer_name: &str) -> Result<usize, StoreError>;
    /// 列出矢量存储中已有的图层表 (数据源表列表使用, 如 metadata 数据源)
    async fn list_tables(&self) -> Result<Vec<String>, StoreError>;
}

/// 复用 SQLite 元数据存储的要素能力 (kind = "metadata" 且元数据为 sqlite 时)。
///
/// 直接复用 [`SqliteStore`] 上已有的 features 表读写逻辑, 不复制实现。
#[async_trait]
impl VectorStore for SqliteStore {
    async fn save_features(&self, layer_name: &str, features: &[Feature]) -> Result<usize, StoreError> {
        SqliteStore::save_features(self, layer_name, features)
            .await
            .map_err(StoreError::from)
    }

    async fn load_features(&self, layer_name: &str) -> Result<Vec<Feature>, StoreError> {
        SqliteStore::load_features(self, layer_name).await.map_err(StoreError::from)
    }

    async fn delete_features(&self, layer_name: &str) -> Result<usize, StoreError> {
        SqliteStore::delete_features(self, layer_name).await.map_err(StoreError::from)
    }

    async fn list_tables(&self) -> Result<Vec<String>, StoreError> {
        SqliteStore::list_feature_layers(self).await.map_err(StoreError::from)
    }
}

/// 构建矢量数据存储。
///
/// 按 [`GeoServerConfig::effective_vector`] 解析出的生效配置选择后端。
/// 返回 `None` 表示矢量存储不可用 (如后端初始化失败)。
pub async fn build_vector_store(config: &GeoServerConfig) -> Option<Arc<dyn VectorStore>> {
    let vc = config.effective_vector();
    match vc.kind.as_str() {
        // 复用元数据存储 (内置默认选项; 元数据为非 sqlite 外部存储时的默认)
        "metadata" => match config.metadata.kind.as_str() {
            "postgres" => match postgres::PostgresVectorStore::new(&config.metadata.postgres).await {
                Ok(s) => Some(Arc::new(s)),
                Err(e) => {
                    eprintln!("Failed to initialize vector store (reuse postgres metadata): {}", e);
                    None
                }
            },
            _ => {
                // sqlite 元数据 -> 复用同一 sqlite 文件的 features 表
                let sqlite_path = config
                    .metadata
                    .sqlite_path
                    .to_str()
                    .unwrap_or("geoserver.sqlite");
                match SqliteStore::new(sqlite_path).await {
                    Ok(s) => Some(Arc::new(s)),
                    Err(e) => {
                        eprintln!("Failed to initialize vector store (reuse sqlite metadata): {}", e);
                        None
                    }
                }
            }
        },
        // 独立 PostgreSQL 矢量存储
        "postgres" => match postgres::PostgresVectorStore::new(&vc.postgres).await {
            Ok(s) => Some(Arc::new(s)),
            Err(e) => {
                eprintln!("Failed to initialize vector store (postgres): {}", e);
                None
            }
        },
        // 本地目录 (默认)
        _ => {
            let dir = vc.dir.clone().unwrap_or_else(|| config.data_dir.join("business"));
            Some(Arc::new(local_dir::LocalVectorStore::new(dir)))
        }
    }
}
