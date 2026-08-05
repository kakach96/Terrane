//! PostgreSQL 矢量数据存储 — 每个图层一张物理表 (与 PostGIS 数据源一致的模型)。
//!
//! 用于独立 postgres 矢量存储 (kind = "postgres"), 或复用 postgres 元数据存储
//! (kind = "metadata" 且元数据为 postgres)。每个图层对应一张 `biz_<layer>` 表,
//! 与 PostGIS 数据源"一图层一表"的逻辑保持一致, 便于 metadata 数据源复用
//! 相同的表列表 / 要素读写路径。

use async_trait::async_trait;
use deadpool_postgres::{ManagerConfig, RecyclingMethod, Runtime, Pool};
use tokio_postgres::NoTls;

use crate::config::PostgresConfig;
use crate::models::Feature;
use crate::store::StoreError;

/// 业务表前缀 (避免与元数据表冲突)
const BIZ_TABLE_PREFIX: &str = "biz_";

/// PostgreSQL 矢量数据存储
pub struct PostgresVectorStore {
    pool: Pool,
    schema: String,
}

impl PostgresVectorStore {
    /// 根据连接配置构建连接池并确保 schema 存在。
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
        let store = PostgresVectorStore {
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
        Ok(())
    }

    /// 图层名 → 物理表名 (加前缀 + 转义, 防注入)
    fn table_name(&self, layer_name: &str) -> String {
        let safe: String = layer_name
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' })
            .collect();
        format!("\"{}{}\"", BIZ_TABLE_PREFIX, safe)
    }

    /// 物理表名 → 图层名 (去掉前缀)
    fn layer_name_from_table(&self, table: &str) -> String {
        table
            .strip_prefix(BIZ_TABLE_PREFIX)
            .unwrap_or(table)
            .to_string()
    }

    async fn ensure_table(&self, client: &deadpool_postgres::Client, layer_name: &str) -> Result<(), StoreError> {
        let table = self.table_name(layer_name);
        client
            .batch_execute(&format!(
                "CREATE TABLE IF NOT EXISTS {} (
                    feature_id TEXT NOT NULL PRIMARY KEY,
                    geometry TEXT NOT NULL,
                    properties TEXT
                )",
                table
            ))
            .await?;
        Ok(())
    }
}

#[async_trait]
impl super::VectorStore for PostgresVectorStore {
    async fn save_features(&self, layer_name: &str, features: &[Feature]) -> Result<usize, StoreError> {
        let mut client = self.pool.get().await?;
        let table = self.table_name(layer_name);
        let tx = client.transaction().await?;
        tx.batch_execute(&format!(
            "CREATE TABLE IF NOT EXISTS {} (
                feature_id TEXT NOT NULL PRIMARY KEY,
                geometry TEXT NOT NULL,
                properties TEXT
            )",
            table
        ))
        .await?;
        tx.execute(&format!("DELETE FROM {}", table), &[]).await?;
        let mut count = 0;
        for feature in features {
            let geometry = serde_json::to_string(&feature.geometry)?;
            let properties = serde_json::to_string(&feature.properties)?;
            tx.execute(
                &format!(
                    "INSERT INTO {} (feature_id, geometry, properties)
                     VALUES ($1, $2, $3)
                     ON CONFLICT (feature_id) DO UPDATE
                     SET geometry = EXCLUDED.geometry, properties = EXCLUDED.properties",
                    table
                ),
                &[&feature.id, &geometry, &properties],
            )
            .await?;
            count += 1;
        }
        tx.commit().await?;
        Ok(count)
    }

    async fn load_features(&self, layer_name: &str) -> Result<Vec<Feature>, StoreError> {
        let client = self.pool.get().await?;
        let table = self.table_name(layer_name);
        // 表不存在时返回空
        let exists: bool = client
            .query_one(
                "SELECT EXISTS (
                    SELECT 1 FROM information_schema.tables
                    WHERE table_schema = current_schema() AND table_name = $1
                 )",
                &[&format!("{}{}", BIZ_TABLE_PREFIX, layer_name)],
            )
            .await?
            .get(0);
        if !exists {
            return Ok(Vec::new());
        }

        let rows = client
            .query(
                &format!(
                    "SELECT feature_id, geometry, properties FROM {} ORDER BY feature_id",
                    table
                ),
                &[],
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
        let table = self.table_name(layer_name);
        let res = client
            .execute(&format!("DROP TABLE IF EXISTS {}", table), &[])
            .await?;
        Ok(res as usize)
    }

    async fn list_tables(&self) -> Result<Vec<String>, StoreError> {
        let client = self.pool.get().await?;
        let rows = client
            .query(
                "SELECT table_name FROM information_schema.tables
                 WHERE table_schema = current_schema()
                 AND table_name LIKE $1
                 ORDER BY table_name",
                &[&format!("{}%", BIZ_TABLE_PREFIX)],
            )
            .await?;
        Ok(rows
            .iter()
            .map(|row| {
                let t: String = row.get(0);
                self.layer_name_from_table(&t)
            })
            .collect())
    }
}

