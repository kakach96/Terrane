//! PostgreSQL 存储后端 — 集群部署时使用。
//!
//! 保存全部配置元数据 + 空间要素(GeoJSON) + 会话信息。
//! 连接信息来自 `config.metadata` (kind = "postgres")。

use async_trait::async_trait;
use chrono::Utc;
use deadpool_postgres::{ManagerConfig, Pool, RecyclingMethod, Runtime};
use tokio_postgres::NoTls;

use crate::config::MetadataConfig;
use crate::handlers::CreateWorkspaceRequest;
use crate::models::permission::Permission;
use crate::models::sql_view::SqlView;
use crate::models::{DataSource, DataSourceConnection, DataSourceType};

use super::types::{
    AuditLogRecord, Layer, LayerGroupRecord, NamespaceRecord, SessionRecord, StyleRecord, Workspace,
};
use super::{Store, StoreError};

fn now() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn parse_ds_type(type_str: &str) -> DataSourceType {
    match type_str {
        "postgis" => DataSourceType::Postgis,
        "mysql" => DataSourceType::Mysql,
        "mongo" => DataSourceType::Mongo,
        "shapefile" => DataSourceType::Shapefile,
        "geotiff" => DataSourceType::Geotiff,
        "geopackage" => DataSourceType::Geopackage,
        "geojson" => DataSourceType::GeoJson,
        "worldimage" => DataSourceType::WorldImage,
        "cascaded_wms" => DataSourceType::CascadedWms,
        "redis" => DataSourceType::Redis,
        "image_mosaic" => DataSourceType::ImageMosaic,
        "image_pyramid" => DataSourceType::ImagePyramid,
        "arcgrid" => DataSourceType::ArcGrid,
        _ => DataSourceType::Postgis,
    }
}

pub struct PostgresStore {
    pool: Pool,
    schema: String,
}

impl PostgresStore {
    /// 根据配置构建连接池并初始化表结构。
    pub async fn new(cfg: &MetadataConfig) -> Result<Self, StoreError> {
        let mut pg_cfg = deadpool_postgres::Config::new();
        let pg = &cfg.postgres;
        let host = if pg.host.eq_ignore_ascii_case("localhost") {
            "127.0.0.1".to_string()
        } else {
            pg.host.clone()
        };
        pg_cfg.host = Some(host);
        pg_cfg.port = Some(pg.port);
        pg_cfg.dbname = Some(pg.instance.clone());
        // 通过 search_path 将表建到指定 schema
        pg_cfg.options = Some(format!("-csearch_path={}", pg.schema));
        pg_cfg.user = Some(pg.user.clone());
        pg_cfg.password = Some(pg.password.clone());
        pg_cfg.connect_timeout = Some(std::time::Duration::from_secs(10));
        pg_cfg.pool = Some(deadpool_postgres::PoolConfig {
            max_size: pg.pool_size as usize,
            ..Default::default()
        });
        pg_cfg.manager = Some(ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        });

        let pool = pg_cfg.create_pool(Some(Runtime::Tokio1), NoTls)?;

        let store = PostgresStore {
            pool,
            schema: pg.schema.clone(),
        };
        store.init_db().await?;
        Ok(store)
    }

    async fn init_db(&self) -> Result<(), StoreError> {
        let client = self.pool.get().await?;
        // 确保目标 schema 存在 (search_path 指向它, 未存在时建表会失败)
        client
            .batch_execute(&format!("CREATE SCHEMA IF NOT EXISTS {}", self.schema))
            .await?;
        client.batch_execute(
            r#"
            CREATE TABLE IF NOT EXISTS workspaces (
                name TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                enabled BOOLEAN NOT NULL DEFAULT TRUE,
                layer_count INTEGER NOT NULL DEFAULT 0,
                created TEXT,
                modified TEXT
            );

            CREATE TABLE IF NOT EXISTS data_sources (
                name TEXT PRIMARY KEY,
                type TEXT NOT NULL,
                workspace TEXT,
                enabled BOOLEAN NOT NULL DEFAULT TRUE,
                host TEXT,
                port INTEGER,
                database_name TEXT,
                schema_name TEXT DEFAULT 'public',
                username TEXT,
                password TEXT,
                file_path TEXT,
                file_storage_type TEXT DEFAULT 'local',
                s3_endpoint TEXT,
                s3_region TEXT,
                s3_bucket TEXT,
                s3_access_key TEXT,
                s3_secret_key TEXT,
                created TEXT,
                modified TEXT
            );

            CREATE TABLE IF NOT EXISTS layers (
                name TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                workspace TEXT NOT NULL,
                store TEXT NOT NULL,
                srs TEXT DEFAULT 'EPSG:4326',
                abstract_text TEXT,
                native_name TEXT,
                enabled BOOLEAN NOT NULL DEFAULT TRUE,
                minx FLOAT8 DEFAULT -180,
                miny FLOAT8 DEFAULT -90,
                maxx FLOAT8 DEFAULT 180,
                maxy FLOAT8 DEFAULT 90,
                cache_store TEXT,
                created TEXT,
                modified TEXT
            );

            CREATE TABLE IF NOT EXISTS namespaces (
                prefix TEXT PRIMARY KEY,
                uri TEXT NOT NULL,
                isolated BOOLEAN NOT NULL DEFAULT FALSE,
                workspace TEXT,
                created TEXT,
                modified TEXT
            );

            CREATE TABLE IF NOT EXISTS sql_views (
                name TEXT PRIMARY KEY,
                sql TEXT NOT NULL,
                workspace TEXT NOT NULL,
                store TEXT NOT NULL,
                geometry_column TEXT DEFAULT 'geom',
                geometry_type TEXT DEFAULT 'Geometry',
                crs TEXT DEFAULT 'EPSG:4326',
                parameters TEXT DEFAULT '[]',
                description TEXT,
                created TEXT,
                modified TEXT
            );

            CREATE TABLE IF NOT EXISTS users (
                username TEXT PRIMARY KEY,
                password_hash TEXT NOT NULL,
                salt TEXT NOT NULL,
                role TEXT NOT NULL DEFAULT 'user',
                enabled BOOLEAN NOT NULL DEFAULT TRUE,
                created TEXT,
                modified TEXT
            );

            CREATE TABLE IF NOT EXISTS audit_log (
                id BIGSERIAL PRIMARY KEY,
                username TEXT NOT NULL,
                action TEXT NOT NULL,
                resource TEXT,
                detail TEXT,
                ip_address TEXT,
                timestamp TEXT
            );

            CREATE TABLE IF NOT EXISTS permissions (
                id BIGSERIAL PRIMARY KEY,
                username TEXT NOT NULL DEFAULT '*',
                role TEXT NOT NULL DEFAULT '*',
                resource_type TEXT NOT NULL,
                resource_name TEXT NOT NULL DEFAULT '*',
                access_mode TEXT NOT NULL DEFAULT 'read',
                effect TEXT NOT NULL DEFAULT 'allow',
                priority INTEGER DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS styles (
                name TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                format TEXT NOT NULL DEFAULT 'SLD',
                is_builtin BOOLEAN NOT NULL DEFAULT FALSE,
                content TEXT NOT NULL,
                created TEXT,
                modified TEXT
            );

            CREATE TABLE IF NOT EXISTS layer_groups (
                name TEXT PRIMARY KEY,
                title TEXT,
                layers TEXT DEFAULT '[]',
                styles TEXT DEFAULT '[]',
                created TEXT,
                modified TEXT
            );

            CREATE TABLE IF NOT EXISTS sessions (
                jti TEXT PRIMARY KEY,
                username TEXT NOT NULL,
                role TEXT NOT NULL,
                issued_at TEXT,
                expires_at TEXT,
                last_seen_at TEXT,
                revoked BOOLEAN NOT NULL DEFAULT FALSE,
                user_agent TEXT,
                ip_address TEXT
            );

            INSERT INTO permissions (username, role, resource_type, resource_name, access_mode, effect, priority)
            SELECT '*', 'admin', '*', '*', 'admin', 'allow', 100
            WHERE NOT EXISTS (
                SELECT 1 FROM permissions
                WHERE username = '*' AND role = 'admin' AND resource_type = '*'
                  AND resource_name = '*' AND access_mode = 'admin' AND effect = 'allow'
            );

            -- S3 object-storage columns (migrate existing data_sources tables)
            ALTER TABLE data_sources ADD COLUMN IF NOT EXISTS s3_endpoint TEXT;
            ALTER TABLE data_sources ADD COLUMN IF NOT EXISTS s3_region TEXT;
            ALTER TABLE data_sources ADD COLUMN IF NOT EXISTS s3_bucket TEXT;
            ALTER TABLE data_sources ADD COLUMN IF NOT EXISTS s3_access_key TEXT;
            ALTER TABLE data_sources ADD COLUMN IF NOT EXISTS s3_secret_key TEXT;

            -- Layer-level tile cache backend data source (migrate existing layers tables)
            ALTER TABLE layers ADD COLUMN IF NOT EXISTS cache_store TEXT;
            "#,
        )
        .await?;
        Ok(())
    }

    fn row_to_data_source(row: &tokio_postgres::Row) -> Result<DataSource, StoreError> {
        let host: Option<String> = row.try_get(4)?;
        let port: Option<i32> = row.try_get(5)?;
        let db: Option<String> = row.try_get(6)?;
        let schema: Option<String> = row.try_get(7)?;
        let username: Option<String> = row.try_get(8)?;
        let password: Option<String> = row.try_get(9)?;
        let file_path: Option<String> = row.try_get(10)?;
        let file_storage: Option<String> = row.try_get(11)?;
        let s3_endpoint: Option<String> = row.try_get(12)?;
        let s3_region: Option<String> = row.try_get(13)?;
        let s3_bucket: Option<String> = row.try_get(14)?;
        let s3_access_key: Option<String> = row.try_get(15)?;
        let s3_secret_key: Option<String> = row.try_get(16)?;
        let created: Option<String> = row.try_get(17)?;
        let modified: Option<String> = row.try_get(18)?;

        Ok(DataSource {
            name: row.try_get(0)?,
            data_source_type: parse_ds_type(&row.try_get::<_, String>(1)?),
            workspace: row.try_get(2)?,
            enabled: row.try_get(3)?,
            connection: Some(DataSourceConnection {
                host,
                port: port.map(|p| p as u16),
                database: db,
                schema: Some(schema.unwrap_or_else(|| "public".to_string())),
                username,
                password,
                file_path,
                file_storage_type: file_storage,
                s3_endpoint,
                s3_region,
                s3_bucket,
                s3_access_key,
                s3_secret_key,
            }),
            created,
            modified,
        })
    }

    fn parse_role(role_str: &str) -> crate::auth::UserRole {
        match role_str {
            "admin" => crate::auth::UserRole::Admin,
            "manager" => crate::auth::UserRole::Manager,
            "guest" => crate::auth::UserRole::Guest,
            _ => crate::auth::UserRole::User,
        }
    }
}

type DbParams = Vec<Box<dyn tokio_postgres::types::ToSql + Send + Sync>>;

fn to_refs(params: &DbParams) -> Vec<&(dyn tokio_postgres::types::ToSql + Sync)> {
    params
        .iter()
        .map(|b| b.as_ref() as &(dyn tokio_postgres::types::ToSql + Sync))
        .collect()
}

#[async_trait]
impl Store for PostgresStore {
    // ---- 工作空间 ----

    async fn get_workspace(&self, name: &str) -> Result<Option<Workspace>, StoreError> {
        let client = self.pool.get().await?;
        let rows = client
            .query(
                "SELECT name, title, description, enabled, layer_count, created, modified
                 FROM workspaces WHERE name = $1",
                &[&name],
            )
            .await?;
        if let Some(row) = rows.first() {
            Ok(Some(Workspace {
                name: row.try_get(0)?,
                title: row.try_get(1)?,
                enabled: row.try_get(3)?,
                layer_count: row.try_get(4)?,
                description: row.try_get(2)?,
                created: row.try_get(5)?,
                modified: row.try_get(6)?,
            }))
        } else {
            Ok(None)
        }
    }

    async fn get_all_workspaces(&self) -> Result<Vec<Workspace>, StoreError> {
        let client = self.pool.get().await?;
        let rows = client
            .query(
                "SELECT name, title, description, enabled, layer_count, created, modified
                 FROM workspaces ORDER BY name",
                &[],
            )
            .await?;
        rows.iter()
            .map(|row| {
                Ok(Workspace {
                    name: row.try_get(0)?,
                    title: row.try_get(1)?,
                    enabled: row.try_get(3)?,
                    layer_count: row.try_get(4)?,
                    description: row.try_get(2)?,
                    created: row.try_get(5)?,
                    modified: row.try_get(6)?,
                })
            })
            .collect()
    }

    async fn create_workspace(
        &self,
        request: &CreateWorkspaceRequest,
    ) -> Result<Workspace, StoreError> {
        let ts = now();
        let title = request
            .title
            .clone()
            .unwrap_or_else(|| request.name.clone());
        let description = request.description.clone().unwrap_or_default();
        let client = self.pool.get().await?;
        client
            .execute(
                "INSERT INTO workspaces (name, title, description, enabled, layer_count, created, modified)
                 VALUES ($1, $2, $3, $4, $5, $6, $7)",
                &[&request.name, &title, &description, &true, &0, &ts, &ts],
            )
            .await?;
        Ok(Workspace {
            name: request.name.clone(),
            title,
            enabled: true,
            layer_count: 0,
            description,
            created: ts.clone(),
            modified: ts,
        })
    }

    async fn update_workspace(
        &self,
        name: &str,
        title: Option<String>,
        description: Option<String>,
        enabled: Option<bool>,
    ) -> Result<(), StoreError> {
        let ts = now();
        let client = self.pool.get().await?;
        let mut sets = vec!["modified = $1".to_string()];
        let mut params: DbParams = vec![Box::new(ts)];
        if let Some(t) = &title {
            params.push(Box::new(t.clone()));
            sets.push(format!("title = ${}", params.len()));
        }
        if let Some(d) = &description {
            params.push(Box::new(d.clone()));
            sets.push(format!("description = ${}", params.len()));
        }
        if let Some(e) = enabled {
            params.push(Box::new(e));
            sets.push(format!("enabled = ${}", params.len()));
        }
        params.push(Box::new(name.to_string()));
        let sql = format!(
            "UPDATE workspaces SET {} WHERE name = ${}",
            sets.join(", "),
            params.len()
        );
        client.execute(&sql, &to_refs(&params)).await?;
        Ok(())
    }

    async fn delete_workspace(&self, name: &str) -> Result<(), StoreError> {
        let client = self.pool.get().await?;
        client
            .execute("DELETE FROM workspaces WHERE name = $1", &[&name])
            .await?;
        Ok(())
    }

    // ---- 命名空间 ----

    async fn get_namespace(&self, prefix: &str) -> Result<Option<NamespaceRecord>, StoreError> {
        let client = self.pool.get().await?;
        let rows = client
            .query(
                "SELECT prefix, uri, isolated, workspace, created, modified
                 FROM namespaces WHERE prefix = $1",
                &[&prefix],
            )
            .await?;
        if let Some(row) = rows.first() {
            Ok(Some(NamespaceRecord {
                prefix: row.try_get(0)?,
                uri: row.try_get(1)?,
                isolated: row.try_get(2)?,
                workspace: row.try_get(3)?,
                created: row.try_get(4)?,
                modified: row.try_get(5)?,
            }))
        } else {
            Ok(None)
        }
    }

    async fn get_all_namespaces(&self) -> Result<Vec<NamespaceRecord>, StoreError> {
        let client = self.pool.get().await?;
        let rows = client
            .query(
                "SELECT prefix, uri, isolated, workspace, created, modified
                 FROM namespaces ORDER BY prefix",
                &[],
            )
            .await?;
        rows.iter()
            .map(|row| {
                Ok(NamespaceRecord {
                    prefix: row.try_get(0)?,
                    uri: row.try_get(1)?,
                    isolated: row.try_get(2)?,
                    workspace: row.try_get(3)?,
                    created: row.try_get(4)?,
                    modified: row.try_get(5)?,
                })
            })
            .collect()
    }

    async fn create_namespace(
        &self,
        prefix: &str,
        uri: &str,
        workspace: Option<&str>,
        isolated: bool,
    ) -> Result<NamespaceRecord, StoreError> {
        let ts = now();
        let client = self.pool.get().await?;
        client
            .execute(
                "INSERT INTO namespaces (prefix, uri, isolated, workspace, created, modified)
                 VALUES ($1, $2, $3, $4, $5, $6)",
                &[&prefix, &uri, &isolated, &workspace, &ts, &ts],
            )
            .await?;
        Ok(NamespaceRecord {
            prefix: prefix.to_string(),
            uri: uri.to_string(),
            isolated,
            workspace: workspace.map(|s| s.to_string()),
            created: ts.clone(),
            modified: ts,
        })
    }

    async fn update_namespace(
        &self,
        prefix: &str,
        uri: Option<String>,
        isolated: Option<bool>,
        workspace: Option<String>,
    ) -> Result<(), StoreError> {
        let ts = now();
        let client = self.pool.get().await?;
        let mut sets = vec!["modified = $1".to_string()];
        let mut params: DbParams = vec![Box::new(ts)];
        if let Some(u) = &uri {
            params.push(Box::new(u.clone()));
            sets.push(format!("uri = ${}", params.len()));
        }
        if let Some(i) = isolated {
            params.push(Box::new(i));
            sets.push(format!("isolated = ${}", params.len()));
        }
        if let Some(w) = &workspace {
            params.push(Box::new(w.clone()));
            sets.push(format!("workspace = ${}", params.len()));
        }
        params.push(Box::new(prefix.to_string()));
        let sql = format!(
            "UPDATE namespaces SET {} WHERE prefix = ${}",
            sets.join(", "),
            params.len()
        );
        client.execute(&sql, &to_refs(&params)).await?;
        Ok(())
    }

    async fn delete_namespace(&self, prefix: &str) -> Result<(), StoreError> {
        let client = self.pool.get().await?;
        client
            .execute("DELETE FROM namespaces WHERE prefix = $1", &[&prefix])
            .await?;
        Ok(())
    }

    // ---- 数据源 ----

    async fn get_data_source(&self, name: &str) -> Result<Option<DataSource>, StoreError> {
        let client = self.pool.get().await?;
        let rows = client
            .query(
                "SELECT name, type, workspace, enabled, host, port, database_name, schema_name,
                        username, password, file_path, file_storage_type, s3_endpoint, s3_region,
                        s3_bucket, s3_access_key, s3_secret_key, created, modified
                 FROM data_sources WHERE name = $1",
                &[&name],
            )
            .await?;
        if let Some(row) = rows.first() {
            Ok(Some(Self::row_to_data_source(row)?))
        } else {
            Ok(None)
        }
    }

    async fn get_all_data_sources(&self) -> Result<Vec<DataSource>, StoreError> {
        let client = self.pool.get().await?;
        let rows = client
            .query(
                "SELECT name, type, workspace, enabled, host, port, database_name, schema_name,
                        username, password, file_path, file_storage_type, s3_endpoint, s3_region,
                        s3_bucket, s3_access_key, s3_secret_key, created, modified
                 FROM data_sources ORDER BY name",
                &[],
            )
            .await?;
        rows.iter().map(Self::row_to_data_source).collect()
    }

    async fn create_data_source(
        &self,
        name: &str,
        data_source_type: &DataSourceType,
        workspace: Option<String>,
        enabled: bool,
        connection: &DataSourceConnection,
    ) -> Result<DataSource, StoreError> {
        let ts = now();
        let port: Option<i32> = connection.port.map(|p| p as i32);
        let client = self.pool.get().await?;
        client
            .execute(
                "INSERT INTO data_sources (name, type, workspace, enabled, host, port, database_name,
                        schema_name, username, password, file_path, file_storage_type,
                        s3_endpoint, s3_region, s3_bucket, s3_access_key, s3_secret_key,
                        created, modified)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19)",
                &[
                    &name,
                    &data_source_type.to_string().to_lowercase(),
                    &workspace,
                    &enabled,
                    &connection.host,
                    &port,
                    &connection.database,
                    &connection.schema,
                    &connection.username,
                    &connection.password,
                    &connection.file_path,
                    &connection.file_storage_type,
                    &connection.s3_endpoint,
                    &connection.s3_region,
                    &connection.s3_bucket,
                    &connection.s3_access_key,
                    &connection.s3_secret_key,
                    &ts,
                    &ts,
                ],
            )
            .await?;
        Ok(DataSource {
            name: name.to_string(),
            data_source_type: data_source_type.clone(),
            workspace,
            enabled,
            connection: Some(connection.clone()),
            created: Some(ts.clone()),
            modified: Some(ts),
        })
    }

    async fn update_data_source(
        &self,
        name: &str,
        data_source_type: Option<DataSourceType>,
        workspace: Option<String>,
        enabled: Option<bool>,
        connection: Option<DataSourceConnection>,
    ) -> Result<(), StoreError> {
        let ts = now();
        let client = self.pool.get().await?;
        let mut sets = vec!["modified = $1".to_string()];
        let mut params: DbParams = vec![Box::new(ts)];
        if let Some(t) = &data_source_type {
            let s = t.to_string().to_lowercase();
            params.push(Box::new(s));
            sets.push(format!("type = ${}", params.len()));
        }
        if let Some(w) = &workspace {
            params.push(Box::new(w.clone()));
            sets.push(format!("workspace = ${}", params.len()));
        }
        if let Some(e) = enabled {
            params.push(Box::new(e));
            sets.push(format!("enabled = ${}", params.len()));
        }
        if let Some(c) = &connection {
            params.push(Box::new(c.host.clone()));
            sets.push(format!("host = ${}", params.len()));
            params.push(Box::new(c.port.map(|p| p as i32)));
            sets.push(format!("port = ${}", params.len()));
            params.push(Box::new(c.database.clone()));
            sets.push(format!("database_name = ${}", params.len()));
            params.push(Box::new(c.schema.clone()));
            sets.push(format!("schema_name = ${}", params.len()));
            params.push(Box::new(c.username.clone()));
            sets.push(format!("username = ${}", params.len()));
            params.push(Box::new(c.password.clone()));
            sets.push(format!("password = ${}", params.len()));
            params.push(Box::new(c.file_path.clone()));
            sets.push(format!("file_path = ${}", params.len()));
            params.push(Box::new(c.file_storage_type.clone()));
            sets.push(format!("file_storage_type = ${}", params.len()));
            params.push(Box::new(c.s3_endpoint.clone()));
            sets.push(format!("s3_endpoint = ${}", params.len()));
            params.push(Box::new(c.s3_region.clone()));
            sets.push(format!("s3_region = ${}", params.len()));
            params.push(Box::new(c.s3_bucket.clone()));
            sets.push(format!("s3_bucket = ${}", params.len()));
            params.push(Box::new(c.s3_access_key.clone()));
            sets.push(format!("s3_access_key = ${}", params.len()));
            params.push(Box::new(c.s3_secret_key.clone()));
            sets.push(format!("s3_secret_key = ${}", params.len()));
        }
        params.push(Box::new(name.to_string()));
        let sql = format!(
            "UPDATE data_sources SET {} WHERE name = ${}",
            sets.join(", "),
            params.len()
        );
        client.execute(&sql, &to_refs(&params)).await?;
        Ok(())
    }

    async fn delete_data_source(&self, name: &str) -> Result<(), StoreError> {
        let client = self.pool.get().await?;
        client
            .execute("DELETE FROM data_sources WHERE name = $1", &[&name])
            .await?;
        Ok(())
    }

    // ---- 图层 ----

    async fn get_layer(&self, name: &str) -> Result<Option<Layer>, StoreError> {
        let client = self.pool.get().await?;
        let rows = client
            .query(
                "SELECT name, title, workspace, store, srs, abstract_text, native_name, enabled,
                        minx, miny, maxx, maxy, cache_store, created, modified
                 FROM layers WHERE name = $1",
                &[&name],
            )
            .await?;
        if let Some(row) = rows.first() {
            Ok(Some(Layer {
                name: row.try_get(0)?,
                title: row.try_get(1)?,
                workspace: row.try_get(2)?,
                store: row.try_get(3)?,
                srs: row.try_get(4)?,
                abstract_text: row.try_get(5)?,
                native_name: row.try_get(6)?,
                enabled: row.try_get(7)?,
                minx: row.try_get(8)?,
                miny: row.try_get(9)?,
                maxx: row.try_get(10)?,
                maxy: row.try_get(11)?,
                cache_store: row.try_get(12)?,
                created: row.try_get(13)?,
                modified: row.try_get(14)?,
            }))
        } else {
            Ok(None)
        }
    }

    async fn get_all_layers(&self) -> Result<Vec<Layer>, StoreError> {
        let client = self.pool.get().await?;
        let rows = client
            .query(
                "SELECT name, title, workspace, store, srs, abstract_text, native_name, enabled,
                        minx, miny, maxx, maxy, cache_store, created, modified
                 FROM layers ORDER BY name",
                &[],
            )
            .await?;
        rows.iter()
            .map(|row| {
                Ok(Layer {
                    name: row.try_get(0)?,
                    title: row.try_get(1)?,
                    workspace: row.try_get(2)?,
                    store: row.try_get(3)?,
                    srs: row.try_get(4)?,
                    abstract_text: row.try_get(5)?,
                    native_name: row.try_get(6)?,
                    enabled: row.try_get(7)?,
                    minx: row.try_get(8)?,
                    miny: row.try_get(9)?,
                    maxx: row.try_get(10)?,
                    maxy: row.try_get(11)?,
                    cache_store: row.try_get(12)?,
                    created: row.try_get(13)?,
                    modified: row.try_get(14)?,
                })
            })
            .collect()
    }

    async fn create_layer(&self, layer: &Layer) -> Result<Layer, StoreError> {
        let ts = now();
        let client = self.pool.get().await?;
        client
            .execute(
                "INSERT INTO layers (name, title, workspace, store, srs, abstract_text, native_name,
                        enabled, minx, miny, maxx, maxy, cache_store, created, modified)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)",
                &[
                    &layer.name, &layer.title, &layer.workspace, &layer.store, &layer.srs,
                    &layer.abstract_text, &layer.native_name, &layer.enabled,
                    &layer.minx, &layer.miny, &layer.maxx, &layer.maxy, &layer.cache_store,
                    &ts, &ts,
                ],
            )
            .await?;
        Ok(Layer {
            name: layer.name.clone(),
            title: layer.title.clone(),
            workspace: layer.workspace.clone(),
            store: layer.store.clone(),
            srs: layer.srs.clone(),
            abstract_text: layer.abstract_text.clone(),
            native_name: layer.native_name.clone(),
            enabled: layer.enabled,
            minx: layer.minx,
            miny: layer.miny,
            maxx: layer.maxx,
            maxy: layer.maxy,
            cache_store: layer.cache_store.clone(),
            created: ts.clone(),
            modified: ts,
        })
    }

    async fn update_layer(
        &self,
        name: &str,
        title: Option<String>,
        abstract_text: Option<String>,
        native_name: Option<String>,
        enabled: Option<bool>,
        cache_store: Option<Option<String>>,
    ) -> Result<(), StoreError> {
        let ts = now();
        let client = self.pool.get().await?;
        let mut sets = vec!["modified = $1".to_string()];
        let mut params: DbParams = vec![Box::new(ts)];
        if let Some(t) = &title {
            params.push(Box::new(t.clone()));
            sets.push(format!("title = ${}", params.len()));
        }
        if let Some(a) = &abstract_text {
            params.push(Box::new(a.clone()));
            sets.push(format!("abstract_text = ${}", params.len()));
        }
        if let Some(n) = &native_name {
            params.push(Box::new(n.clone()));
            sets.push(format!("native_name = ${}", params.len()));
        }
        if let Some(e) = enabled {
            params.push(Box::new(e));
            sets.push(format!("enabled = ${}", params.len()));
        }
        // 图层级缓存后端: Some(Some(ds)) = 设置; Some(None) = 清除(回到默认缓存)
        if let Some(cs) = &cache_store {
            params.push(Box::new(cs.clone()));
            sets.push(format!("cache_store = ${}", params.len()));
        }
        params.push(Box::new(name.to_string()));
        let sql = format!(
            "UPDATE layers SET {} WHERE name = ${}",
            sets.join(", "),
            params.len()
        );
        client.execute(&sql, &to_refs(&params)).await?;
        Ok(())
    }

    async fn delete_layer(&self, name: &str) -> Result<(), StoreError> {
        let client = self.pool.get().await?;
        client
            .execute("DELETE FROM layers WHERE name = $1", &[&name])
            .await?;
        Ok(())
    }

    // ---- SQL 视图 ----

    async fn get_sql_view(&self, name: &str) -> Result<Option<SqlView>, StoreError> {
        let client = self.pool.get().await?;
        let rows = client
            .query(
                "SELECT name, sql, workspace, store, geometry_column, geometry_type, crs,
                        parameters, description, created, modified
                 FROM sql_views WHERE name = $1",
                &[&name],
            )
            .await?;
        if let Some(row) = rows.first() {
            let params_str: String = row.try_get(7)?;
            let parameters: Vec<crate::models::sql_view::SqlViewParameter> =
                serde_json::from_str(&params_str).unwrap_or_default();
            Ok(Some(SqlView {
                name: row.try_get(0)?,
                sql: row.try_get(1)?,
                workspace: row.try_get(2)?,
                store: row.try_get(3)?,
                geometry_column: row.try_get(4)?,
                geometry_type: row.try_get(5)?,
                crs: row.try_get(6)?,
                parameters,
                description: row.try_get(8)?,
                created: row.try_get(9)?,
                modified: row.try_get(10)?,
            }))
        } else {
            Ok(None)
        }
    }

    async fn get_all_sql_views(&self) -> Result<Vec<SqlView>, StoreError> {
        let client = self.pool.get().await?;
        let rows = client
            .query(
                "SELECT name, sql, workspace, store, geometry_column, geometry_type, crs,
                        parameters, description, created, modified
                 FROM sql_views ORDER BY name",
                &[],
            )
            .await?;
        rows.iter()
            .map(|row| {
                let params_str: String = row.try_get(7)?;
                let parameters: Vec<crate::models::sql_view::SqlViewParameter> =
                    serde_json::from_str(&params_str).unwrap_or_default();
                Ok(SqlView {
                    name: row.try_get(0)?,
                    sql: row.try_get(1)?,
                    workspace: row.try_get(2)?,
                    store: row.try_get(3)?,
                    geometry_column: row.try_get(4)?,
                    geometry_type: row.try_get(5)?,
                    crs: row.try_get(6)?,
                    parameters,
                    description: row.try_get(8)?,
                    created: row.try_get(9)?,
                    modified: row.try_get(10)?,
                })
            })
            .collect()
    }

    async fn create_sql_view(&self, view: &SqlView) -> Result<(), StoreError> {
        let ts = now();
        let params_json =
            serde_json::to_string(&view.parameters).unwrap_or_else(|_| "[]".to_string());
        let client = self.pool.get().await?;
        client
            .execute(
                "INSERT INTO sql_views (name, sql, workspace, store, geometry_column, geometry_type,
                        crs, parameters, description, created, modified)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
                &[
                    &view.name, &view.sql, &view.workspace, &view.store,
                    &view.geometry_column, &view.geometry_type, &view.crs,
                    &params_json, &view.description, &ts, &ts,
                ],
            )
            .await?;
        Ok(())
    }

    async fn update_sql_view(
        &self,
        name: &str,
        sql: Option<String>,
        geometry_column: Option<String>,
        geometry_type: Option<String>,
        crs: Option<String>,
        parameters: Option<Vec<crate::models::sql_view::SqlViewParameter>>,
        description: Option<String>,
    ) -> Result<(), StoreError> {
        let ts = now();
        let client = self.pool.get().await?;
        let mut sets = vec!["modified = $1".to_string()];
        let mut params: DbParams = vec![Box::new(ts)];
        if let Some(s) = &sql {
            params.push(Box::new(s.clone()));
            sets.push(format!("sql = ${}", params.len()));
        }
        if let Some(g) = &geometry_column {
            params.push(Box::new(g.clone()));
            sets.push(format!("geometry_column = ${}", params.len()));
        }
        if let Some(g) = &geometry_type {
            params.push(Box::new(g.clone()));
            sets.push(format!("geometry_type = ${}", params.len()));
        }
        if let Some(c) = &crs {
            params.push(Box::new(c.clone()));
            sets.push(format!("crs = ${}", params.len()));
        }
        if let Some(p) = &parameters {
            let params_json = serde_json::to_string(p).unwrap_or_else(|_| "[]".to_string());
            params.push(Box::new(params_json));
            sets.push(format!("parameters = ${}", params.len()));
        }
        if let Some(d) = &description {
            params.push(Box::new(d.clone()));
            sets.push(format!("description = ${}", params.len()));
        }
        params.push(Box::new(name.to_string()));
        let sql = format!(
            "UPDATE sql_views SET {} WHERE name = ${}",
            sets.join(", "),
            params.len()
        );
        client.execute(&sql, &to_refs(&params)).await?;
        Ok(())
    }

    async fn delete_sql_view(&self, name: &str) -> Result<(), StoreError> {
        let client = self.pool.get().await?;
        client
            .execute("DELETE FROM sql_views WHERE name = $1", &[&name])
            .await?;
        Ok(())
    }

    // ---- 用户 ----

    async fn get_user(&self, username: &str) -> Result<Option<crate::auth::User>, StoreError> {
        let client = self.pool.get().await?;
        let rows = client
            .query(
                "SELECT username, password_hash, salt, role, enabled, created, modified
                 FROM users WHERE username = $1",
                &[&username],
            )
            .await?;
        if let Some(row) = rows.first() {
            Ok(Some(crate::auth::User {
                username: row.try_get(0)?,
                password_hash: row.try_get(1)?,
                salt: row.try_get(2)?,
                role: Self::parse_role(&row.try_get::<_, String>(3)?),
                enabled: row.try_get(4)?,
                created: row.try_get(5)?,
                modified: row.try_get(6)?,
            }))
        } else {
            Ok(None)
        }
    }

    async fn get_all_users(&self) -> Result<Vec<crate::auth::User>, StoreError> {
        let client = self.pool.get().await?;
        let rows = client
            .query(
                "SELECT username, password_hash, salt, role, enabled, created, modified
                 FROM users ORDER BY username",
                &[],
            )
            .await?;
        rows.iter()
            .map(|row| {
                Ok(crate::auth::User {
                    username: row.try_get(0)?,
                    password_hash: row.try_get(1)?,
                    salt: row.try_get(2)?,
                    role: Self::parse_role(&row.try_get::<_, String>(3)?),
                    enabled: row.try_get(4)?,
                    created: row.try_get(5)?,
                    modified: row.try_get(6)?,
                })
            })
            .collect()
    }

    async fn create_user(
        &self,
        username: &str,
        password_hash: &str,
        salt: &str,
        role: &crate::auth::UserRole,
        enabled: bool,
    ) -> Result<(), StoreError> {
        let ts = now();
        let client = self.pool.get().await?;
        client
            .execute(
                "INSERT INTO users (username, password_hash, salt, role, enabled, created, modified)
                 VALUES ($1, $2, $3, $4, $5, $6, $7)",
                &[&username, &password_hash, &salt, &role.to_string(), &enabled, &ts, &ts],
            )
            .await?;
        Ok(())
    }

    async fn update_user(
        &self,
        username: &str,
        role: Option<&crate::auth::UserRole>,
        enabled: Option<bool>,
        password_hash: Option<&str>,
        salt: Option<&str>,
    ) -> Result<(), StoreError> {
        let ts = now();
        let client = self.pool.get().await?;
        let mut sets = vec!["modified = $1".to_string()];
        let mut params: DbParams = vec![Box::new(ts)];
        if let Some(r) = role {
            let s = r.to_string();
            params.push(Box::new(s));
            sets.push(format!("role = ${}", params.len()));
        }
        if let Some(e) = enabled {
            params.push(Box::new(e));
            sets.push(format!("enabled = ${}", params.len()));
        }
        if let Some(h) = password_hash {
            params.push(Box::new(h.to_string()));
            sets.push(format!("password_hash = ${}", params.len()));
        }
        if let Some(s) = salt {
            params.push(Box::new(s.to_string()));
            sets.push(format!("salt = ${}", params.len()));
        }
        params.push(Box::new(username.to_string()));
        let sql = format!(
            "UPDATE users SET {} WHERE username = ${}",
            sets.join(", "),
            params.len()
        );
        client.execute(&sql, &to_refs(&params)).await?;
        Ok(())
    }

    async fn delete_user(&self, username: &str) -> Result<(), StoreError> {
        let client = self.pool.get().await?;
        client
            .execute("DELETE FROM users WHERE username = $1", &[&username])
            .await?;
        Ok(())
    }

    // ---- 审计日志 ----

    async fn audit_log(
        &self,
        username: &str,
        action: &str,
        resource: Option<&str>,
        detail: Option<&str>,
        ip_address: Option<&str>,
    ) -> Result<(), StoreError> {
        let ts = now();
        let client = self.pool.get().await?;
        client
            .execute(
                "INSERT INTO audit_log (username, action, resource, detail, ip_address, timestamp)
                 VALUES ($1, $2, $3, $4, $5, $6)",
                &[&username, &action, &resource, &detail, &ip_address, &ts],
            )
            .await?;
        Ok(())
    }

    async fn get_audit_logs(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<AuditLogRecord>, StoreError> {
        let client = self.pool.get().await?;
        let rows = client
            .query(
                "SELECT id, username, action, resource, detail, ip_address, timestamp
                 FROM audit_log ORDER BY id DESC LIMIT $1 OFFSET $2",
                &[&(limit as i64), &(offset as i64)],
            )
            .await?;
        rows.iter()
            .map(|row| {
                Ok(AuditLogRecord {
                    id: row.try_get(0)?,
                    username: row.try_get(1)?,
                    action: row.try_get(2)?,
                    resource: row.try_get(3)?,
                    detail: row.try_get(4)?,
                    ip_address: row.try_get(5)?,
                    created_at: row.try_get(6)?,
                })
            })
            .collect()
    }

    // ---- 权限 ----

    async fn get_permissions(&self) -> Result<Vec<Permission>, StoreError> {
        let client = self.pool.get().await?;
        let rows = client
            .query(
                "SELECT id, username, role, resource_type, resource_name, access_mode, effect, priority
                 FROM permissions ORDER BY priority DESC, resource_type, resource_name",
                &[],
            )
            .await?;
        rows.iter()
            .map(|row| {
                Ok(Permission {
                    id: Some(row.try_get(0)?),
                    username: row.try_get(1)?,
                    role: row.try_get(2)?,
                    resource_type: row.try_get(3)?,
                    resource_name: row.try_get(4)?,
                    access_mode: row
                        .try_get::<_, String>(5)?
                        .parse()
                        .unwrap_or(crate::models::permission::AccessMode::Read),
                    effect: row
                        .try_get::<_, String>(6)?
                        .parse()
                        .unwrap_or(crate::models::permission::Effect::Allow),
                    priority: row.try_get(7)?,
                })
            })
            .collect()
    }

    async fn create_permission(&self, p: &Permission) -> Result<i64, StoreError> {
        let client = self.pool.get().await?;
        let row = client
            .query_one(
                "INSERT INTO permissions (username, role, resource_type, resource_name, access_mode, effect, priority)
                 VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
                &[
                    &p.username, &p.role, &p.resource_type, &p.resource_name,
                    &p.access_mode.to_string(), &p.effect.to_string(), &p.priority,
                ],
            )
            .await?;
        Ok(row.try_get(0)?)
    }

    async fn delete_permission(&self, id: i64) -> Result<(), StoreError> {
        let client = self.pool.get().await?;
        client
            .execute("DELETE FROM permissions WHERE id = $1", &[&id])
            .await?;
        Ok(())
    }

    async fn check_permission(
        &self,
        username: &str,
        role: &str,
        resource_type: &str,
        resource_name: &str,
        required_mode: &str,
    ) -> Result<bool, StoreError> {
        let client = self.pool.get().await?;
        let rows = client
            .query(
                "SELECT effect FROM permissions
                 WHERE (username = $1 OR username = '*')
                   AND (role = $2 OR role = '*')
                   AND (resource_type = $3 OR resource_type = '*')
                   AND (resource_name = $4 OR resource_name = '*')
                   AND (access_mode = $5 OR access_mode = 'admin')
                 ORDER BY priority DESC LIMIT 1",
                &[
                    &username,
                    &role,
                    &resource_type,
                    &resource_name,
                    &required_mode,
                ],
            )
            .await?;
        if let Some(row) = rows.first() {
            let effect: String = row.try_get(0)?;
            Ok(effect == "allow")
        } else {
            // 没有匹配规则时，admin 默认有权限，其他人默认无权限
            Ok(role == "admin")
        }
    }

    // ---- 样式 ----

    async fn get_all_styles(&self) -> Result<Vec<StyleRecord>, StoreError> {
        let client = self.pool.get().await?;
        let rows = client
            .query(
                "SELECT name, title, format, is_builtin, content, created, modified
                 FROM styles ORDER BY name",
                &[],
            )
            .await?;
        rows.iter()
            .map(|row| {
                Ok(StyleRecord {
                    name: row.try_get(0)?,
                    title: row.try_get(1)?,
                    format: row.try_get(2)?,
                    is_builtin: row.try_get(3)?,
                    content: row.try_get(4)?,
                    created: row.try_get(5)?,
                    modified: row.try_get(6)?,
                })
            })
            .collect()
    }

    async fn get_style(&self, name: &str) -> Result<Option<StyleRecord>, StoreError> {
        let client = self.pool.get().await?;
        let rows = client
            .query(
                "SELECT name, title, format, is_builtin, content, created, modified
                 FROM styles WHERE name = $1",
                &[&name],
            )
            .await?;
        if let Some(row) = rows.first() {
            Ok(Some(StyleRecord {
                name: row.try_get(0)?,
                title: row.try_get(1)?,
                format: row.try_get(2)?,
                is_builtin: row.try_get(3)?,
                content: row.try_get(4)?,
                created: row.try_get(5)?,
                modified: row.try_get(6)?,
            }))
        } else {
            Ok(None)
        }
    }

    async fn create_style(&self, style: &StyleRecord) -> Result<(), StoreError> {
        let client = self.pool.get().await?;
        client
            .execute(
                "INSERT INTO styles (name, title, format, is_builtin, content, created, modified)
                 VALUES ($1, $2, $3, $4, $5, $6, $7)
                 ON CONFLICT (name) DO UPDATE
                 SET title = EXCLUDED.title, format = EXCLUDED.format,
                     is_builtin = EXCLUDED.is_builtin, content = EXCLUDED.content,
                     modified = EXCLUDED.modified",
                &[
                    &style.name,
                    &style.title,
                    &style.format,
                    &style.is_builtin,
                    &style.content,
                    &style.created,
                    &style.modified,
                ],
            )
            .await?;
        Ok(())
    }

    async fn update_style(
        &self,
        name: &str,
        title: Option<String>,
        format: Option<String>,
        content: Option<String>,
        is_builtin: Option<bool>,
    ) -> Result<(), StoreError> {
        let ts = now();
        let client = self.pool.get().await?;
        let mut sets = vec!["modified = $1".to_string()];
        let mut params: DbParams = vec![Box::new(ts)];
        if let Some(t) = &title {
            params.push(Box::new(t.clone()));
            sets.push(format!("title = ${}", params.len()));
        }
        if let Some(f) = &format {
            params.push(Box::new(f.clone()));
            sets.push(format!("format = ${}", params.len()));
        }
        if let Some(c) = &content {
            params.push(Box::new(c.clone()));
            sets.push(format!("content = ${}", params.len()));
        }
        if let Some(b) = is_builtin {
            params.push(Box::new(b));
            sets.push(format!("is_builtin = ${}", params.len()));
        }
        params.push(Box::new(name.to_string()));
        let sql = format!(
            "UPDATE styles SET {} WHERE name = ${}",
            sets.join(", "),
            params.len()
        );
        client.execute(&sql, &to_refs(&params)).await?;
        Ok(())
    }

    async fn delete_style(&self, name: &str) -> Result<(), StoreError> {
        let client = self.pool.get().await?;
        client
            .execute("DELETE FROM styles WHERE name = $1", &[&name])
            .await?;
        Ok(())
    }

    // ---- 图层组 ----

    async fn get_all_layer_groups(&self) -> Result<Vec<LayerGroupRecord>, StoreError> {
        let client = self.pool.get().await?;
        let rows = client
            .query(
                "SELECT name, title, layers, styles, created, modified
                 FROM layer_groups ORDER BY name",
                &[],
            )
            .await?;
        rows.iter()
            .map(|row| {
                let layers_json: String = row.try_get(2)?;
                let styles_json: String = row.try_get(3)?;
                Ok(LayerGroupRecord {
                    name: row.try_get(0)?,
                    title: row.try_get(1)?,
                    layers: serde_json::from_str(&layers_json).unwrap_or_default(),
                    styles: serde_json::from_str(&styles_json).unwrap_or_default(),
                    created: row.try_get(4)?,
                    modified: row.try_get(5)?,
                })
            })
            .collect()
    }

    async fn get_layer_group(&self, name: &str) -> Result<Option<LayerGroupRecord>, StoreError> {
        let client = self.pool.get().await?;
        let rows = client
            .query(
                "SELECT name, title, layers, styles, created, modified
                 FROM layer_groups WHERE name = $1",
                &[&name],
            )
            .await?;
        if let Some(row) = rows.first() {
            let layers_json: String = row.try_get(2)?;
            let styles_json: String = row.try_get(3)?;
            Ok(Some(LayerGroupRecord {
                name: row.try_get(0)?,
                title: row.try_get(1)?,
                layers: serde_json::from_str(&layers_json).unwrap_or_default(),
                styles: serde_json::from_str(&styles_json).unwrap_or_default(),
                created: row.try_get(4)?,
                modified: row.try_get(5)?,
            }))
        } else {
            Ok(None)
        }
    }

    async fn create_layer_group(&self, group: &LayerGroupRecord) -> Result<(), StoreError> {
        let client = self.pool.get().await?;
        let layers_json = serde_json::to_string(&group.layers).unwrap_or_else(|_| "[]".to_string());
        let styles_json = serde_json::to_string(&group.styles).unwrap_or_else(|_| "[]".to_string());
        client
            .execute(
                "INSERT INTO layer_groups (name, title, layers, styles, created, modified)
                 VALUES ($1, $2, $3, $4, $5, $6)
                 ON CONFLICT (name) DO UPDATE
                 SET title = EXCLUDED.title, layers = EXCLUDED.layers, styles = EXCLUDED.styles,
                     modified = EXCLUDED.modified",
                &[
                    &group.name,
                    &group.title,
                    &layers_json,
                    &styles_json,
                    &group.created,
                    &group.modified,
                ],
            )
            .await?;
        Ok(())
    }

    async fn delete_layer_group(&self, name: &str) -> Result<(), StoreError> {
        let client = self.pool.get().await?;
        client
            .execute("DELETE FROM layer_groups WHERE name = $1", &[&name])
            .await?;
        Ok(())
    }

    // ---- 会话 ----

    async fn create_session(&self, session: &SessionRecord) -> Result<(), StoreError> {
        let client = self.pool.get().await?;
        client
            .execute(
                "INSERT INTO sessions (jti, username, role, issued_at, expires_at, last_seen_at, revoked, user_agent, ip_address)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                 ON CONFLICT (jti) DO UPDATE
                 SET username = EXCLUDED.username, role = EXCLUDED.role,
                     issued_at = EXCLUDED.issued_at, expires_at = EXCLUDED.expires_at,
                     last_seen_at = EXCLUDED.last_seen_at, revoked = EXCLUDED.revoked,
                     user_agent = EXCLUDED.user_agent, ip_address = EXCLUDED.ip_address",
                &[
                    &session.jti, &session.username, &session.role,
                    &session.issued_at, &session.expires_at, &session.last_seen_at,
                    &session.revoked, &session.user_agent, &session.ip_address,
                ],
            )
            .await?;
        Ok(())
    }

    async fn get_session(&self, jti: &str) -> Result<Option<SessionRecord>, StoreError> {
        let client = self.pool.get().await?;
        let rows = client
            .query(
                "SELECT jti, username, role, issued_at, expires_at, last_seen_at, revoked, user_agent, ip_address
                 FROM sessions WHERE jti = $1",
                &[&jti],
            )
            .await?;
        if let Some(row) = rows.first() {
            Ok(Some(SessionRecord {
                jti: row.try_get(0)?,
                username: row.try_get(1)?,
                role: row.try_get(2)?,
                issued_at: row.try_get(3)?,
                expires_at: row.try_get(4)?,
                last_seen_at: row.try_get(5)?,
                revoked: row.try_get(6)?,
                user_agent: row.try_get(7)?,
                ip_address: row.try_get(8)?,
            }))
        } else {
            Ok(None)
        }
    }

    async fn delete_session(&self, jti: &str) -> Result<(), StoreError> {
        let client = self.pool.get().await?;
        client
            .execute("DELETE FROM sessions WHERE jti = $1", &[&jti])
            .await?;
        Ok(())
    }

    async fn delete_user_sessions(&self, username: &str) -> Result<(), StoreError> {
        let client = self.pool.get().await?;
        client
            .execute("DELETE FROM sessions WHERE username = $1", &[&username])
            .await?;
        Ok(())
    }

    async fn cleanup_expired_sessions(&self) -> Result<usize, StoreError> {
        let ts = now();
        let client = self.pool.get().await?;
        let res = client
            .execute("DELETE FROM sessions WHERE expires_at < $1", &[&ts])
            .await?;
        Ok(res as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::Store;
    use super::*;
    use crate::config::PostgresConfig;

    /// 构造一个指向本地 PostGIS 的元数据配置。
    /// 连接参数可用 `GEOSERVER_TEST_PG_*` 环境变量覆盖, 默认匹配本地开发栈
    /// (`docker compose -f build/docker-compose.yml up -d` 或本机 postgis 容器)。
    fn test_metadata_config(schema: &str) -> MetadataConfig {
        let host = std::env::var("GEOSERVER_TEST_PG_HOST").unwrap_or_else(|_| "127.0.0.1".into());
        let port: u16 = std::env::var("GEOSERVER_TEST_PG_PORT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5433);
        let user = std::env::var("GEOSERVER_TEST_PG_USER").unwrap_or_else(|_| "terrane".into());
        let password =
            std::env::var("GEOSERVER_TEST_PG_PASSWORD").unwrap_or_else(|_| "terrane".into());
        let instance = std::env::var("GEOSERVER_TEST_PG_DB").unwrap_or_else(|_| "terrane".into());

        MetadataConfig {
            kind: "postgres".into(),
            sqlite_path: std::path::PathBuf::new(),
            postgres: PostgresConfig {
                host,
                port,
                instance,
                schema: schema.to_string(),
                user,
                password,
                pool_size: 2,
            },
        }
    }

    /// 删除测试 schema (清理) — 走独立连接池, 不依赖 store 的 search_path。
    async fn drop_test_schema(cfg: &MetadataConfig) {
        let pg = &cfg.postgres;
        let mut pg_cfg = deadpool_postgres::Config::new();
        pg_cfg.host = Some(pg.host.clone());
        pg_cfg.port = Some(pg.port);
        pg_cfg.dbname = Some(pg.instance.clone());
        pg_cfg.user = Some(pg.user.clone());
        pg_cfg.password = Some(pg.password.clone());
        let pool = pg_cfg
            .create_pool(Some(deadpool_postgres::Runtime::Tokio1), NoTls)
            .unwrap();
        if let Ok(client) = pool.get().await {
            let _ = client
                .batch_execute(&format!("DROP SCHEMA IF EXISTS {} CASCADE", pg.schema))
                .await;
        }
    }

    #[actix_rt::test]
    #[ignore = "requires a live PostGIS (e.g. docker compose -f build/docker-compose.yml up -d)"]
    async fn test_live_postgres_metadata_store() {
        // 每进程独立 schema, 避免并行运行互相清理
        let schema = format!("terrane_test_{}", std::process::id());
        let cfg = test_metadata_config(&schema);

        // 1. 连接 + 建表
        let store = PostgresStore::new(&cfg)
            .await
            .expect("应能连接 PostGIS 并初始化表结构");

        // 2. workspace CRUD
        store
            .create_workspace(&CreateWorkspaceRequest {
                name: "pg_ws".into(),
                title: Some("PG WS".into()),
                description: None,
            })
            .await
            .unwrap();
        let ws = store.get_workspace("pg_ws").await.unwrap().unwrap();
        assert_eq!(ws.name, "pg_ws");

        // 3. layer CRUD
        let layer = Layer {
            name: "pg_layer".into(),
            title: "PG Layer".into(),
            workspace: "pg_ws".into(),
            store: "pg_store".into(),
            srs: "EPSG:4326".into(),
            abstract_text: None,
            native_name: Some("pg_layer".into()),
            enabled: true,
            minx: -180.0,
            miny: -90.0,
            maxx: 180.0,
            maxy: 90.0,
            cache_store: None,
            created: String::new(),
            modified: String::new(),
        };
        store.create_layer(&layer).await.unwrap();
        let got = store.get_layer("pg_layer").await.unwrap().unwrap();
        assert_eq!(got.title, "PG Layer");

        // 4. user CRUD
        store
            .create_user(
                "pgalice",
                "hash",
                "salt",
                &crate::auth::UserRole::User,
                true,
            )
            .await
            .unwrap();
        assert!(store.get_user("pgalice").await.unwrap().is_some());
        store.delete_user("pgalice").await.unwrap();
        assert!(store.get_user("pgalice").await.unwrap().is_none());

        // 5. 清理测试 schema
        drop_test_schema(&cfg).await;
    }
}
