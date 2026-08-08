//! PostgreSQL 矢量数据存储 — 每个图层一张物理表 (与 PostGIS 数据源一致的模型)。
//!
//! 用于独立 postgres 矢量存储 (kind = "postgres"), 或复用 postgres 元数据存储
//! (kind = "metadata" 且元数据为 postgres)。每个图层对应一张 `biz_<layer>` 表,
//! 与 PostGIS 数据源"一图层一表"的逻辑保持一致, 便于 metadata 数据源复用
//! 相同的表列表 / 要素读写路径。

use async_trait::async_trait;
use deadpool_postgres::{ManagerConfig, Pool, RecyclingMethod, Runtime};
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
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '_' {
                    c
                } else {
                    '_'
                }
            })
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

    async fn ensure_table(
        &self,
        client: &deadpool_postgres::Client,
        layer_name: &str,
    ) -> Result<(), StoreError> {
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
    async fn save_features(
        &self,
        layer_name: &str,
        features: &[Feature],
    ) -> Result<usize, StoreError> {
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
                let geometry = serde_json::from_str(&geometry_str).unwrap_or(
                    crate::models::GeoJsonGeometry::Point {
                        coordinates: vec![0.0, 0.0],
                    },
                );
                let properties = properties_str
                    .and_then(|s| serde_json::from_str(&s).ok())
                    .unwrap_or_default();
                Ok(Feature::with_id(id, geometry, properties))
            })
            .collect()
    }

    async fn delete_features(&self, layer_name: &str) -> Result<usize, StoreError> {
        let client = self.pool.get().await?;
        // 表不存在时返回 0 (与 SqliteStore 一致: 删除行并返回行数, 保留表结构)
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
            return Ok(0);
        }
        let table = self.table_name(layer_name);
        let res = client
            .execute(&format!("DELETE FROM {}", table), &[])
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Feature, GeoJsonGeometry};
    use crate::store::vector::VectorStore;

    /// 构造一个指向本地 PostGIS 的矢量存储配置。
    /// 连接参数可用 `GEOSERVER_TEST_PG_*` 环境变量覆盖, 默认匹配本地开发栈
    /// (`docker compose --profile postgres` 或本机 postgis 容器)。
    fn test_pg_config(schema: &str) -> PostgresConfig {
        let host = std::env::var("GEOSERVER_TEST_PG_HOST").unwrap_or_else(|_| "127.0.0.1".into());
        let port: u16 = std::env::var("GEOSERVER_TEST_PG_PORT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5432);
        let user = std::env::var("GEOSERVER_TEST_PG_USER").unwrap_or_else(|_| "postgres".into());
        let password =
            std::env::var("GEOSERVER_TEST_PG_PASSWORD").unwrap_or_else(|_| "kakach2026".into());
        let instance = std::env::var("GEOSERVER_TEST_PG_DB").unwrap_or_else(|_| "postgres".into());

        PostgresConfig {
            host,
            port,
            instance,
            schema: schema.to_string(),
            user,
            password,
            pool_size: 2,
        }
    }

    /// 删除测试 schema (清理)。
    async fn drop_test_schema(cfg: &PostgresConfig) {
        let mut pg_cfg = deadpool_postgres::Config::new();
        pg_cfg.host = Some(cfg.host.clone());
        pg_cfg.port = Some(cfg.port);
        pg_cfg.dbname = Some(cfg.instance.clone());
        pg_cfg.user = Some(cfg.user.clone());
        pg_cfg.password = Some(cfg.password.clone());
        let pool = pg_cfg
            .create_pool(Some(deadpool_postgres::Runtime::Tokio1), NoTls)
            .unwrap();
        if let Ok(client) = pool.get().await {
            let _ = client
                .batch_execute(&format!("DROP SCHEMA IF EXISTS {} CASCADE", cfg.schema))
                .await;
        }
    }

    #[actix_rt::test]
    #[ignore = "requires a live PostGIS (e.g. docker compose --profile postgres)"]
    async fn test_live_postgres_vector_store() {
        // 每进程独立 schema, 避免并行运行互相清理
        let schema = format!("terrane_vec_test_{}", std::process::id());
        let cfg = test_pg_config(&schema);

        // 1. 连接 + 确保 schema
        let store = PostgresVectorStore::new(&cfg)
            .await
            .expect("应能连接 PostGIS 并初始化 schema");

        // 2. 要素保存/读取往返
        let feature = Feature::new(
            GeoJsonGeometry::Point {
                coordinates: vec![10.0, 20.0],
            },
            std::collections::HashMap::new(),
        );
        let saved = store.save_features("vec_layer", &[feature]).await.unwrap();
        assert_eq!(saved, 1, "PostGIS 应保存 1 个要素");

        let feats = store.load_features("vec_layer").await.unwrap();
        assert_eq!(feats.len(), 1, "PostGIS 应能读回 1 个要素");
        if let GeoJsonGeometry::Point { coordinates } = &feats[0].geometry {
            assert!((coordinates[0] - 10.0).abs() < 1e-6);
            assert!((coordinates[1] - 20.0).abs() < 1e-6);
        } else {
            panic!("读回的要素应为点, 实际: {:?}", feats[0].geometry);
        }

        // 3. list_tables 应包含该图层
        let tables = store.list_tables().await.unwrap();
        assert!(
            tables.iter().any(|t| t == "vec_layer"),
            "list_tables 应包含 vec_layer, 实际: {:?}",
            tables
        );

        // 4. 删除后为空
        let deleted = store.delete_features("vec_layer").await.unwrap();
        assert!(deleted > 0);
        assert!(store.load_features("vec_layer").await.unwrap().is_empty());

        // 5. 清理测试 schema
        drop_test_schema(&cfg).await;
    }
}
