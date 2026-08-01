//! PostgreSQL 业务数据存储 — 仅管理 `features` 表 (图层要素)。
//!
//! 用于独立 postgres 业务存储 (kind = "postgres"), 或复用 postgres 元数据存储
//! (kind = "metadata" 且元数据为 postgres)。只创建/使用 `features` 表, 不会
//! 在业务数据库中创建元数据表。

use async_trait::async_trait;
use deadpool_postgres::{ManagerConfig, RecyclingMethod, Runtime, Pool};
use tokio_postgres::NoTls;

use crate::config::PostgresConfig;
use crate::models::Feature;
use crate::store::StoreError;

/// PostgreSQL 业务数据存储
pub struct PostgresBusinessStore {
    pool: Pool,
    schema: String,
}

impl PostgresBusinessStore {
    /// 根据连接配置构建连接池并确保 `features` 表存在。
    pub async fn new(cfg: &PostgresConfig) -> Result<Self, StoreError> {
        let mut pg_cfg = deadpool_postgres::Config::new();
        let host = if cfg.host.eq_ignore_ascii_case("localhost") {
            "127.0.0.1".to_string()
        } else {
            cfg.host.clone()
        };
        pg_cfg.host = Some(host);
        pg_cfg.port = Some(cfg.port);
        pg_cfg.dbname = Some(cfg.instance.clone());
        // 通过 search_path 将表建到指定 schema
        pg_cfg.options = Some(format!("-csearch_path={}", cfg.schema));
        pg_cfg.user = Some(cfg.user.clone());
        pg_cfg.password = Some(cfg.password.clone());
        pg_cfg.connect_timeout = Some(std::time::Duration::from_secs(10));
        pg_cfg.pool = Some(deadpool_postgres::PoolConfig {
            max_size: cfg.pool_size as usize,
            ..Default::default()
        });
        pg_cfg.manager = Some(ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        });

        let pool = pg_cfg.create_pool(Some(Runtime::Tokio1), NoTls)?;
        let store = PostgresBusinessStore {
            pool,
            schema: cfg.schema.clone(),
        };
        store.init().await?;
        Ok(store)
    }

    async fn init(&self) -> Result<(), StoreError> {
        let client = self.pool.get().await?;
        // 确保目标 schema 存在 (search_path 指向它, 未存在时建表会失败)
        client
            .batch_execute(&format!("CREATE SCHEMA IF NOT EXISTS {}", self.schema))
            .await?;
        client
            .batch_execute(
                r#"
                CREATE TABLE IF NOT EXISTS features (
                    layer_name TEXT NOT NULL,
                    feature_id TEXT NOT NULL,
                    geometry TEXT NOT NULL,
                    properties TEXT,
                    PRIMARY KEY (layer_name, feature_id)
                );
                "#,
            )
            .await?;
        Ok(())
    }
}

#[async_trait]
impl super::BusinessStore for PostgresBusinessStore {
    async fn save_features(&self, layer_name: &str, features: &[Feature]) -> Result<usize, StoreError> {
        let mut client = self.pool.get().await?;
        let tx = client.transaction().await?;
        tx.execute("DELETE FROM features WHERE layer_name = $1", &[&layer_name]).await?;
        let mut count = 0;
        for feature in features {
            let geometry = serde_json::to_string(&feature.geometry)?;
            let properties = serde_json::to_string(&feature.properties)?;
            tx.execute(
                "INSERT INTO features (layer_name, feature_id, geometry, properties)
                 VALUES ($1, $2, $3, $4)
                 ON CONFLICT (layer_name, feature_id) DO UPDATE
                 SET geometry = EXCLUDED.geometry, properties = EXCLUDED.properties",
                &[&layer_name, &feature.id, &geometry, &properties],
            )
            .await?;
            count += 1;
        }
        tx.commit().await?;
        Ok(count)
    }

    async fn load_features(&self, layer_name: &str) -> Result<Vec<Feature>, StoreError> {
        let client = self.pool.get().await?;
        let rows = client
            .query(
                "SELECT feature_id, geometry, properties FROM features
                 WHERE layer_name = $1 ORDER BY feature_id",
                &[&layer_name],
            )
            .await?;
        rows.iter()
            .map(|row| {
                let id: String = row.try_get(0)?;
                let geometry_str: String = row.try_get(1)?;
                let properties_str: Option<String> = row.try_get(2)?;
                let geometry = serde_json::from_str(&geometry_str)
                    .unwrap_or(crate::models::GeoJsonGeometry::Point { coordinates: vec![0.0, 0.0] });
                let properties = properties_str
                    .and_then(|s| serde_json::from_str(&s).ok())
                    .unwrap_or_default();
                Ok(Feature::with_id(id, geometry, properties))
            })
            .collect()
    }

    async fn delete_features(&self, layer_name: &str) -> Result<usize, StoreError> {
        let client = self.pool.get().await?;
        let res = client
            .execute("DELETE FROM features WHERE layer_name = $1", &[&layer_name])
            .await?;
        Ok(res as usize)
    }
}
