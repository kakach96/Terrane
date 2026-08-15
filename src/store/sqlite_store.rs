use crate::handlers::CreateWorkspaceRequest;
use crate::models::sql_view::SqlView;
use crate::models::{DataSource, DataSourceConnection, DataSourceType};
use chrono::Utc;
use rusqlite::{params, Connection, Result as SqlResult};
use std::sync::{Arc, Mutex};
// 导入并重新导出共享类型，保持 `sqlite_store::Layer` 等旧路径兼容
pub use super::types::{
    AuditLogRecord, Layer, LayerGroupRecord, NamespaceRecord, SessionRecord, StyleRecord, Workspace,
};

fn column_exists(conn: &Connection, table: &str, column: &str) -> bool {
    conn.query_row(
        &format!(
            "SELECT COUNT(*) FROM pragma_table_info('{}') WHERE name='{}'",
            table, column
        ),
        [],
        |row| row.get::<_, i32>(0).map(|c| c > 0),
    )
    .unwrap_or(false)
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

pub struct SqliteStore {
    conn: Arc<Mutex<Connection>>,
}

unsafe impl Send for SqliteStore {}
unsafe impl Sync for SqliteStore {}

impl SqliteStore {
    pub async fn new(path: &str) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;
        Self::init_db(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    fn init_db(conn: &Connection) -> SqlResult<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS workspaces (
                name TEXT PRIMARY KEY,
                title TEXT,
                description TEXT,
                enabled INTEGER DEFAULT 1,
                layer_count INTEGER DEFAULT 0,
                created TEXT,
                modified TEXT
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS data_sources (
                name TEXT PRIMARY KEY,
                type TEXT NOT NULL,
                workspace TEXT,
                enabled INTEGER DEFAULT 1,
                host TEXT,
                port INTEGER,
                database_name TEXT,
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
            )",
            [],
        )?;

        // 检查并添加 schema_name 列（向后兼容）
        if !column_exists(conn, "data_sources", "schema_name") {
            conn.execute(
                "ALTER TABLE data_sources ADD COLUMN schema_name TEXT DEFAULT 'public'",
                [],
            )?;
        }

        // 检查并添加 file_path 列（支持 shapefile/geotiff 文件型数据源）
        if !column_exists(conn, "data_sources", "file_path") {
            conn.execute("ALTER TABLE data_sources ADD COLUMN file_path TEXT", [])?;
        }

        // 检查并添加 file_storage_type 列
        if !column_exists(conn, "data_sources", "file_storage_type") {
            conn.execute(
                "ALTER TABLE data_sources ADD COLUMN file_storage_type TEXT DEFAULT 'local'",
                [],
            )?;
        }

        // 检查并添加 S3 对象存储列 (file_storage_type = "s3" 时使用)
        for s3_col in [
            "s3_endpoint",
            "s3_region",
            "s3_bucket",
            "s3_access_key",
            "s3_secret_key",
        ] {
            if !column_exists(conn, "data_sources", s3_col) {
                conn.execute(
                    &format!("ALTER TABLE data_sources ADD COLUMN {} TEXT", s3_col),
                    [],
                )?;
            }
        }

        conn.execute(
            "CREATE TABLE IF NOT EXISTS layers (
                name TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                workspace TEXT NOT NULL,
                store TEXT NOT NULL,
                srs TEXT DEFAULT 'EPSG:4326',
                abstract_text TEXT,
                native_name TEXT,
                enabled INTEGER DEFAULT 1,
                minx REAL DEFAULT -180,
                miny REAL DEFAULT -90,
                maxx REAL DEFAULT 180,
                maxy REAL DEFAULT 90,
                cache_store TEXT,
                created TEXT,
                modified TEXT
            )",
            [],
        )?;

        // 检查并添加 native_name 列（向后兼容）
        if !column_exists(conn, "layers", "native_name") {
            conn.execute("ALTER TABLE layers ADD COLUMN native_name TEXT", [])?;
        }

        // 检查并添加 cache_store 列（图层级瓦片缓存后端数据源名称; 向后兼容）
        if !column_exists(conn, "layers", "cache_store") {
            conn.execute("ALTER TABLE layers ADD COLUMN cache_store TEXT", [])?;
        }

        // 命名空间表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS namespaces (
                prefix TEXT PRIMARY KEY,
                uri TEXT NOT NULL,
                isolated INTEGER DEFAULT 0,
                workspace TEXT,
                created TEXT,
                modified TEXT
            )",
            [],
        )?;

        // SQL 视图表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS sql_views (
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
            )",
            [],
        )?;

        // 用户表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS users (
                username TEXT PRIMARY KEY,
                password_hash TEXT NOT NULL,
                salt TEXT NOT NULL,
                role TEXT NOT NULL DEFAULT 'user',
                enabled INTEGER DEFAULT 1,
                created TEXT,
                modified TEXT
            )",
            [],
        )?;

        // 操作日志表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS audit_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                username TEXT NOT NULL,
                action TEXT NOT NULL,
                resource TEXT,
                detail TEXT,
                ip_address TEXT,
                timestamp TEXT
            )",
            [],
        )?;

        // 权限表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS permissions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                username TEXT NOT NULL DEFAULT '*',
                role TEXT NOT NULL DEFAULT '*',
                resource_type TEXT NOT NULL,
                resource_name TEXT NOT NULL DEFAULT '*',
                access_mode TEXT NOT NULL DEFAULT 'read',
                effect TEXT NOT NULL DEFAULT 'allow',
                priority INTEGER DEFAULT 0
            )",
            [],
        )?;

        // 默认权限: admin 拥有所有权限
        conn.execute(
            "INSERT OR IGNORE INTO permissions (username, role, resource_type, resource_name, access_mode, effect, priority)
             VALUES ('*', 'admin', '*', '*', 'admin', 'allow', 100)",
            [],
        ).ok();

        // 样式表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS styles (
                name TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                format TEXT NOT NULL DEFAULT 'SLD',
                is_builtin INTEGER DEFAULT 0,
                content TEXT NOT NULL,
                created TEXT,
                modified TEXT
            )",
            [],
        )?;

        // 图层组表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS layer_groups (
                name TEXT PRIMARY KEY,
                title TEXT,
                layers TEXT DEFAULT '[]',
                styles TEXT DEFAULT '[]',
                created TEXT,
                modified TEXT
            )",
            [],
        )?;

        // 会话表 (JWT jti 关联)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS sessions (
                jti TEXT PRIMARY KEY,
                username TEXT NOT NULL,
                role TEXT NOT NULL,
                issued_at TEXT,
                expires_at TEXT,
                last_seen_at TEXT,
                revoked INTEGER DEFAULT 0,
                user_agent TEXT,
                ip_address TEXT
            )",
            [],
        )?;

        Ok(())
    }

    pub async fn get_workspace(&self, name: &str) -> SqlResult<Option<Workspace>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT name, title, description, enabled, layer_count, created, modified 
             FROM workspaces WHERE name = ?",
        )?;

        let mut rows = stmt.query([name])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Workspace {
                name: row.get(0)?,
                title: row.get(1)?,
                enabled: row.get::<_, i32>(3)? == 1,
                layer_count: row.get(4)?,
                description: row.get(2)?,
                created: row.get(5)?,
                modified: row.get(6)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn get_all_workspaces(&self) -> SqlResult<Vec<Workspace>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT name, title, description, enabled, layer_count, created, modified 
             FROM workspaces ORDER BY name",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(Workspace {
                name: row.get(0)?,
                title: row.get(1)?,
                enabled: row.get::<_, i32>(3)? == 1,
                layer_count: row.get(4)?,
                description: row.get(2)?,
                created: row.get(5)?,
                modified: row.get(6)?,
            })
        })?;

        rows.collect()
    }

    pub async fn create_workspace(&self, request: &CreateWorkspaceRequest) -> SqlResult<Workspace> {
        let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let title = request
            .title
            .clone()
            .unwrap_or_else(|| request.name.clone());
        let description = request.description.clone().unwrap_or_default();
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "INSERT INTO workspaces (name, title, description, enabled, layer_count, created, modified)
             VALUES (?, ?, ?, 1, 0, ?, ?)",
            params![
                request.name,
                title,
                description,
                now,
                now
            ],
        )?;

        Ok(Workspace {
            name: request.name.clone(),
            title,
            enabled: true,
            layer_count: 0,
            description,
            created: now.clone(),
            modified: now,
        })
    }

    pub async fn update_workspace(
        &self,
        name: &str,
        title: Option<String>,
        description: Option<String>,
        enabled: Option<bool>,
    ) -> SqlResult<()> {
        let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let conn = self.conn.lock().unwrap();

        let mut updates: Vec<String> = vec!["modified = ?".to_string()];
        let mut values: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(now.clone())];

        if let Some(t) = title {
            updates.push("title = ?".to_string());
            values.push(Box::new(t));
        }
        if let Some(d) = description {
            updates.push("description = ?".to_string());
            values.push(Box::new(d));
        }
        if let Some(e) = enabled {
            updates.push("enabled = ?".to_string());
            values.push(Box::new(e as i32));
        }

        let query = format!(
            "UPDATE workspaces SET {} WHERE name = ?",
            updates.join(", ")
        );
        let mut params: Vec<&dyn rusqlite::types::ToSql> =
            values.iter().map(|v| v.as_ref()).collect();
        params.push(&name);

        conn.execute(&query, params.as_slice())?;
        Ok(())
    }

    pub async fn delete_workspace(&self, name: &str) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM workspaces WHERE name = ?", [name])?;
        Ok(())
    }

    // ========================================================================
    // 命名空间 (Namespaces)
    // ========================================================================

    pub async fn get_namespace(&self, prefix: &str) -> SqlResult<Option<NamespaceRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT prefix, uri, isolated, workspace, created, modified
             FROM namespaces WHERE prefix = ?",
        )?;
        let mut rows = stmt.query([prefix])?;
        if let Some(row) = rows.next()? {
            Ok(Some(NamespaceRecord {
                prefix: row.get(0)?,
                uri: row.get(1)?,
                isolated: row.get::<_, i32>(2)? == 1,
                workspace: row.get(3)?,
                created: row.get(4)?,
                modified: row.get(5)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn get_all_namespaces(&self) -> SqlResult<Vec<NamespaceRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT prefix, uri, isolated, workspace, created, modified
             FROM namespaces ORDER BY prefix",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(NamespaceRecord {
                prefix: row.get(0)?,
                uri: row.get(1)?,
                isolated: row.get::<_, i32>(2)? == 1,
                workspace: row.get(3)?,
                created: row.get(4)?,
                modified: row.get(5)?,
            })
        })?;
        rows.collect()
    }

    pub async fn create_namespace(
        &self,
        prefix: &str,
        uri: &str,
        workspace: Option<&str>,
        isolated: bool,
    ) -> SqlResult<NamespaceRecord> {
        let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO namespaces (prefix, uri, isolated, workspace, created, modified)
             VALUES (?, ?, ?, ?, ?, ?)",
            params![prefix, uri, isolated as i32, workspace, now, now],
        )?;
        Ok(NamespaceRecord {
            prefix: prefix.to_string(),
            uri: uri.to_string(),
            isolated,
            workspace: workspace.map(|s| s.to_string()),
            created: now.clone(),
            modified: now,
        })
    }

    pub async fn update_namespace(
        &self,
        prefix: &str,
        uri: Option<String>,
        isolated: Option<bool>,
        workspace: Option<String>,
    ) -> SqlResult<()> {
        let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let conn = self.conn.lock().unwrap();

        let mut updates: Vec<String> = vec!["modified = ?".to_string()];
        let mut values: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(now.clone())];

        if let Some(ref u) = uri {
            updates.push("uri = ?".to_string());
            values.push(Box::new(u.clone()));
        }
        if let Some(i) = isolated {
            updates.push("isolated = ?".to_string());
            values.push(Box::new(i as i32));
        }
        if let Some(ref w) = workspace {
            updates.push("workspace = ?".to_string());
            values.push(Box::new(w.clone()));
        }

        let query = format!(
            "UPDATE namespaces SET {} WHERE prefix = ?",
            updates.join(", ")
        );
        let mut params: Vec<&dyn rusqlite::types::ToSql> =
            values.iter().map(|v| v.as_ref()).collect();
        params.push(&prefix);
        conn.execute(&query, params.as_slice())?;
        Ok(())
    }

    pub async fn delete_namespace(&self, prefix: &str) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM namespaces WHERE prefix = ?", [prefix])?;
        Ok(())
    }

    /// 从行中读取 DataSource（支持多种 schema 版本）
    fn row_to_data_source(
        row: &rusqlite::Row,
        has_schema: bool,
        has_file: bool,
        has_s3: bool,
    ) -> rusqlite::Result<DataSource> {
        let host: Option<String> = row.get(4)?;
        let port: Option<u16> = row.get(5)?;
        let db: Option<String> = row.get(6)?;

        let schema = if has_schema {
            row.get::<_, Option<String>>(7)?
                .unwrap_or_else(|| "public".to_string())
        } else {
            "public".to_string()
        };

        let (user_idx, pass_idx, file_path_idx, file_storage_idx, created_idx, modified_idx) =
            if has_s3 {
                // name,type,ws,enabled,host,port,db,schema,user,pass,file_path,file_storage,
                // s3_endpoint,s3_region,s3_bucket,s3_access_key,s3_secret_key,created,modified
                (8, 9, 10, 11, 17, 18)
            } else if has_file {
                // name,type,workspace,enabled,host,port,db,schema,user,pass,file_path,file_storage,created,modified
                (8, 9, 10, 11, 12, 13)
            } else if has_schema {
                // name,type,workspace,enabled,host,port,db,schema,user,pass,created,modified
                (8, 9, 10, 11, 12, 13) // file columns at end (dummy), will be None
            } else {
                // name,type,workspace,enabled,host,port,db,user,pass,created,modified
                (7, 8, 99, 99, 9, 10) // file_path/storage idx unused
            };

        let username: Option<String> = if has_schema || has_file {
            row.get(user_idx)?
        } else {
            Some(row.get::<_, String>(user_idx)?)
        };
        let password: Option<String> = row.get(pass_idx)?;

        let file_path: Option<String> = if has_file {
            row.get(file_path_idx)?
        } else {
            None
        };
        let file_storage: Option<String> = if has_file {
            row.get(file_storage_idx)?
        } else {
            None
        };

        let (s3_endpoint, s3_region, s3_bucket, s3_access_key, s3_secret_key) = if has_s3 {
            (
                row.get(12)?,
                row.get(13)?,
                row.get(14)?,
                row.get(15)?,
                row.get(16)?,
            )
        } else {
            (None, None, None, None, None)
        };

        let created: Option<String> = row.get(created_idx)?;
        let modified: Option<String> = row.get(modified_idx)?;

        Ok(DataSource {
            name: row.get(0)?,
            data_source_type: parse_ds_type(&row.get::<_, String>(1)?),
            workspace: row.get(2)?,
            enabled: row.get::<_, i32>(3)? == 1,
            connection: Some(DataSourceConnection {
                host,
                port,
                database: db,
                schema: Some(schema),
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

    pub async fn get_data_source(&self, name: &str) -> SqlResult<Option<DataSource>> {
        let conn = self.conn.lock().unwrap();

        let has_schema = column_exists(&conn, "data_sources", "schema_name");
        let has_file = column_exists(&conn, "data_sources", "file_path");
        let has_s3 = column_exists(&conn, "data_sources", "s3_bucket");

        let sql = if has_s3 {
            "SELECT name, type, workspace, enabled, host, port, database_name, schema_name, username, password, file_path, file_storage_type, s3_endpoint, s3_region, s3_bucket, s3_access_key, s3_secret_key, created, modified
             FROM data_sources WHERE name = ?"
        } else if has_file {
            "SELECT name, type, workspace, enabled, host, port, database_name, schema_name, username, password, file_path, file_storage_type, created, modified
             FROM data_sources WHERE name = ?"
        } else if has_schema {
            "SELECT name, type, workspace, enabled, host, port, database_name, schema_name, username, password, created, modified
             FROM data_sources WHERE name = ?"
        } else {
            "SELECT name, type, workspace, enabled, host, port, database_name, username, password, created, modified
             FROM data_sources WHERE name = ?"
        };

        let mut stmt = conn.prepare(sql)?;
        let mut rows = stmt.query([name])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::row_to_data_source(
                row, has_schema, has_file, has_s3,
            )?))
        } else {
            Ok(None)
        }
    }

    pub async fn get_all_data_sources(&self) -> SqlResult<Vec<DataSource>> {
        let conn = self.conn.lock().unwrap();

        let has_schema = column_exists(&conn, "data_sources", "schema_name");
        let has_file = column_exists(&conn, "data_sources", "file_path");
        let has_s3 = column_exists(&conn, "data_sources", "s3_bucket");

        let sql = if has_s3 {
            "SELECT name, type, workspace, enabled, host, port, database_name, schema_name, username, password, file_path, file_storage_type, s3_endpoint, s3_region, s3_bucket, s3_access_key, s3_secret_key, created, modified
             FROM data_sources ORDER BY name"
        } else if has_file {
            "SELECT name, type, workspace, enabled, host, port, database_name, schema_name, username, password, file_path, file_storage_type, created, modified
             FROM data_sources ORDER BY name"
        } else if has_schema {
            "SELECT name, type, workspace, enabled, host, port, database_name, schema_name, username, password, created, modified
             FROM data_sources ORDER BY name"
        } else {
            "SELECT name, type, workspace, enabled, host, port, database_name, username, password, created, modified
             FROM data_sources ORDER BY name"
        };

        let mut stmt = conn.prepare(sql)?;
        let has_schema_ref = has_schema;
        let has_file_ref = has_file;
        let has_s3_ref = has_s3;
        let rows = stmt.query_map([], move |row| {
            Self::row_to_data_source(row, has_schema_ref, has_file_ref, has_s3_ref)
        })?;

        rows.collect()
    }

    pub async fn create_data_source(
        &self,
        name: &str,
        data_source_type: &DataSourceType,
        workspace: Option<String>,
        enabled: bool,
        connection: &DataSourceConnection,
    ) -> SqlResult<DataSource> {
        let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let conn = self.conn.lock().unwrap();

        let has_file = column_exists(&conn, "data_sources", "file_path");
        let has_s3 = column_exists(&conn, "data_sources", "s3_bucket");

        if has_s3 {
            conn.execute(
                "INSERT INTO data_sources (name, type, workspace, enabled, host, port, database_name, schema_name, username, password, file_path, file_storage_type, s3_endpoint, s3_region, s3_bucket, s3_access_key, s3_secret_key, created, modified)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    name,
                    format!("{}", data_source_type).to_lowercase(),
                    workspace,
                    enabled as i32,
                    connection.host,
                    connection.port,
                    connection.database,
                    connection.schema,
                    connection.username,
                    connection.password,
                    connection.file_path,
                    connection.file_storage_type,
                    connection.s3_endpoint,
                    connection.s3_region,
                    connection.s3_bucket,
                    connection.s3_access_key,
                    connection.s3_secret_key,
                    now.clone(),
                    now
                ],
            )?;
        } else if has_file {
            conn.execute(
                "INSERT INTO data_sources (name, type, workspace, enabled, host, port, database_name, schema_name, username, password, file_path, file_storage_type, created, modified)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    name,
                    format!("{}", data_source_type).to_lowercase(),
                    workspace,
                    enabled as i32,
                    connection.host,
                    connection.port,
                    connection.database,
                    connection.schema,
                    connection.username,
                    connection.password,
                    connection.file_path,
                    connection.file_storage_type,
                    now.clone(),
                    now
                ],
            )?;
        } else {
            conn.execute(
                "INSERT INTO data_sources (name, type, workspace, enabled, host, port, database_name, schema_name, username, password, created, modified)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    name,
                    format!("{}", data_source_type).to_lowercase(),
                    workspace,
                    enabled as i32,
                    connection.host,
                    connection.port,
                    connection.database,
                    connection.schema,
                    connection.username,
                    connection.password,
                    now.clone(),
                    now
                ],
            )?;
        }

        Ok(DataSource {
            name: name.to_string(),
            data_source_type: data_source_type.clone(),
            workspace,
            enabled,
            connection: Some(connection.clone()),
            created: Some(now.clone()),
            modified: Some(now),
        })
    }

    pub async fn update_data_source(
        &self,
        name: &str,
        data_source_type: Option<DataSourceType>,
        workspace: Option<String>,
        enabled: Option<bool>,
        connection: Option<DataSourceConnection>,
    ) -> SqlResult<()> {
        let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let conn = self.conn.lock().unwrap();

        let has_file = column_exists(&conn, "data_sources", "file_path");
        let has_s3 = column_exists(&conn, "data_sources", "s3_bucket");

        let mut updates: Vec<String> = vec!["modified = ?".to_string()];
        let mut values: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(now)];

        if let Some(t) = data_source_type {
            updates.push("type = ?".to_string());
            values.push(Box::new(t.to_string()));
        }
        if let Some(w) = workspace {
            updates.push("workspace = ?".to_string());
            values.push(Box::new(w));
        }
        if let Some(e) = enabled {
            updates.push("enabled = ?".to_string());
            values.push(Box::new(e as i32));
        }
        if let Some(c) = connection {
            updates.push("host = ?".to_string());
            values.push(Box::new(c.host));
            updates.push("port = ?".to_string());
            values.push(Box::new(c.port));
            updates.push("database_name = ?".to_string());
            values.push(Box::new(c.database));
            updates.push("schema_name = ?".to_string());
            values.push(Box::new(c.schema));
            updates.push("username = ?".to_string());
            values.push(Box::new(c.username));
            updates.push("password = ?".to_string());
            values.push(Box::new(c.password));
            if has_file {
                updates.push("file_path = ?".to_string());
                values.push(Box::new(c.file_path));
                updates.push("file_storage_type = ?".to_string());
                values.push(Box::new(c.file_storage_type));
            }
            if has_s3 {
                updates.push("s3_endpoint = ?".to_string());
                values.push(Box::new(c.s3_endpoint));
                updates.push("s3_region = ?".to_string());
                values.push(Box::new(c.s3_region));
                updates.push("s3_bucket = ?".to_string());
                values.push(Box::new(c.s3_bucket));
                updates.push("s3_access_key = ?".to_string());
                values.push(Box::new(c.s3_access_key));
                updates.push("s3_secret_key = ?".to_string());
                values.push(Box::new(c.s3_secret_key));
            }
        }

        let query = format!(
            "UPDATE data_sources SET {} WHERE name = ?",
            updates.join(", ")
        );
        let mut params: Vec<&dyn rusqlite::types::ToSql> =
            values.iter().map(|v| v.as_ref()).collect();
        params.push(&name);

        conn.execute(&query, params.as_slice())?;
        Ok(())
    }

    pub async fn delete_data_source(&self, name: &str) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM data_sources WHERE name = ?", [name])?;
        Ok(())
    }

    pub async fn get_layer(&self, name: &str) -> SqlResult<Option<Layer>> {
        let conn = self.conn.lock().unwrap();

        let has_native_name = column_exists(&conn, "layers", "native_name");
        let has_cache_store = column_exists(&conn, "layers", "cache_store");
        let mut stmt = if has_native_name && has_cache_store {
            conn.prepare(
                "SELECT name, title, workspace, store, srs, abstract_text, native_name, enabled, minx, miny, maxx, maxy, cache_store, created, modified
                 FROM layers WHERE name = ?"
            )?
        } else if has_native_name {
            conn.prepare(
                "SELECT name, title, workspace, store, srs, abstract_text, native_name, enabled, minx, miny, maxx, maxy, created, modified
                 FROM layers WHERE name = ?"
            )?
        } else {
            conn.prepare(
                "SELECT name, title, workspace, store, srs, abstract_text, enabled, minx, miny, maxx, maxy, created, modified
                 FROM layers WHERE name = ?"
            )?
        };

        let mut rows = stmt.query([name])?;
        if let Some(row) = rows.next()? {
            if has_native_name && has_cache_store {
                Ok(Some(Layer {
                    name: row.get(0)?,
                    title: row.get(1)?,
                    workspace: row.get(2)?,
                    store: row.get(3)?,
                    srs: row.get(4)?,
                    abstract_text: row.get(5)?,
                    native_name: row.get(6)?,
                    enabled: row.get::<_, i32>(7)? == 1,
                    minx: row.get(8)?,
                    miny: row.get(9)?,
                    maxx: row.get(10)?,
                    maxy: row.get(11)?,
                    cache_store: row.get(12)?,
                    created: row.get(13)?,
                    modified: row.get(14)?,
                }))
            } else if has_native_name {
                Ok(Some(Layer {
                    name: row.get(0)?,
                    title: row.get(1)?,
                    workspace: row.get(2)?,
                    store: row.get(3)?,
                    srs: row.get(4)?,
                    abstract_text: row.get(5)?,
                    native_name: row.get(6)?,
                    enabled: row.get::<_, i32>(7)? == 1,
                    minx: row.get(8)?,
                    miny: row.get(9)?,
                    maxx: row.get(10)?,
                    maxy: row.get(11)?,
                    cache_store: None,
                    created: row.get(12)?,
                    modified: row.get(13)?,
                }))
            } else {
                Ok(Some(Layer {
                    name: row.get(0)?,
                    title: row.get(1)?,
                    workspace: row.get(2)?,
                    store: row.get(3)?,
                    srs: row.get(4)?,
                    abstract_text: row.get(5)?,
                    native_name: None,
                    enabled: row.get::<_, i32>(6)? == 1,
                    minx: row.get(7)?,
                    miny: row.get(8)?,
                    maxx: row.get(9)?,
                    maxy: row.get(10)?,
                    cache_store: None,
                    created: row.get(11)?,
                    modified: row.get(12)?,
                }))
            }
        } else {
            Ok(None)
        }
    }

    pub async fn get_all_layers(&self) -> SqlResult<Vec<Layer>> {
        let conn = self.conn.lock().unwrap();

        let has_native_name = column_exists(&conn, "layers", "native_name");
        let has_cache_store = column_exists(&conn, "layers", "cache_store");
        let mut stmt = if has_native_name && has_cache_store {
            conn.prepare(
                "SELECT name, title, workspace, store, srs, abstract_text, native_name, enabled, minx, miny, maxx, maxy, cache_store, created, modified
                 FROM layers ORDER BY name"
            )?
        } else if has_native_name {
            conn.prepare(
                "SELECT name, title, workspace, store, srs, abstract_text, native_name, enabled, minx, miny, maxx, maxy, created, modified
                 FROM layers ORDER BY name"
            )?
        } else {
            conn.prepare(
                "SELECT name, title, workspace, store, srs, abstract_text, enabled, minx, miny, maxx, maxy, created, modified
                 FROM layers ORDER BY name"
            )?
        };

        let rows = stmt.query_map([], move |row| {
            if has_native_name && has_cache_store {
                Ok(Layer {
                    name: row.get(0)?,
                    title: row.get(1)?,
                    workspace: row.get(2)?,
                    store: row.get(3)?,
                    srs: row.get(4)?,
                    abstract_text: row.get(5)?,
                    native_name: row.get(6)?,
                    enabled: row.get::<_, i32>(7)? == 1,
                    minx: row.get(8)?,
                    miny: row.get(9)?,
                    maxx: row.get(10)?,
                    maxy: row.get(11)?,
                    cache_store: row.get(12)?,
                    created: row.get(13)?,
                    modified: row.get(14)?,
                })
            } else if has_native_name {
                Ok(Layer {
                    name: row.get(0)?,
                    title: row.get(1)?,
                    workspace: row.get(2)?,
                    store: row.get(3)?,
                    srs: row.get(4)?,
                    abstract_text: row.get(5)?,
                    native_name: row.get(6)?,
                    enabled: row.get::<_, i32>(7)? == 1,
                    minx: row.get(8)?,
                    miny: row.get(9)?,
                    maxx: row.get(10)?,
                    maxy: row.get(11)?,
                    cache_store: None,
                    created: row.get(12)?,
                    modified: row.get(13)?,
                })
            } else {
                Ok(Layer {
                    name: row.get(0)?,
                    title: row.get(1)?,
                    workspace: row.get(2)?,
                    store: row.get(3)?,
                    srs: row.get(4)?,
                    abstract_text: row.get(5)?,
                    native_name: None,
                    enabled: row.get::<_, i32>(6)? == 1,
                    minx: row.get(7)?,
                    miny: row.get(8)?,
                    maxx: row.get(9)?,
                    maxy: row.get(10)?,
                    cache_store: None,
                    created: row.get(11)?,
                    modified: row.get(12)?,
                })
            }
        })?;

        rows.collect()
    }

    pub async fn create_layer(&self, layer: &Layer) -> SqlResult<Layer> {
        let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let conn = self.conn.lock().unwrap();

        let has_cache_store = column_exists(&conn, "layers", "cache_store");
        if has_cache_store {
            conn.execute(
                "INSERT INTO layers (name, title, workspace, store, srs, abstract_text, native_name, enabled, minx, miny, maxx, maxy, cache_store, created, modified)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    layer.name,
                    layer.title,
                    layer.workspace,
                    layer.store,
                    layer.srs,
                    layer.abstract_text,
                    layer.native_name,
                    layer.enabled as i32,
                    layer.minx,
                    layer.miny,
                    layer.maxx,
                    layer.maxy,
                    layer.cache_store,
                    now,
                    now
                ],
            )?;
        } else {
            conn.execute(
                "INSERT INTO layers (name, title, workspace, store, srs, abstract_text, native_name, enabled, minx, miny, maxx, maxy, created, modified)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    layer.name,
                    layer.title,
                    layer.workspace,
                    layer.store,
                    layer.srs,
                    layer.abstract_text,
                    layer.native_name,
                    layer.enabled as i32,
                    layer.minx,
                    layer.miny,
                    layer.maxx,
                    layer.maxy,
                    now,
                    now
                ],
            )?;
        }

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
            created: now.clone(),
            modified: now,
        })
    }

    pub async fn update_layer(
        &self,
        name: &str,
        title: Option<String>,
        abstract_text: Option<String>,
        native_name: Option<String>,
        enabled: Option<bool>,
        cache_store: Option<Option<String>>,
    ) -> SqlResult<()> {
        let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let conn = self.conn.lock().unwrap();

        let mut updates: Vec<String> = vec!["modified = ?".to_string()];
        let mut values: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(now)];

        if let Some(t) = title {
            updates.push("title = ?".to_string());
            values.push(Box::new(t));
        }
        if let Some(a) = abstract_text {
            updates.push("abstract_text = ?".to_string());
            values.push(Box::new(a));
        }
        if let Some(n) = native_name {
            updates.push("native_name = ?".to_string());
            values.push(Box::new(n));
        }
        if let Some(e) = enabled {
            updates.push("enabled = ?".to_string());
            values.push(Box::new(e as i32));
        }
        // 图层级缓存后端: Some(Some(ds)) = 设置; Some(None) = 清除(回到默认缓存)
        if let Some(cs) = cache_store {
            updates.push("cache_store = ?".to_string());
            values.push(Box::new(cs));
        }

        let query = format!("UPDATE layers SET {} WHERE name = ?", updates.join(", "));
        let mut params: Vec<&dyn rusqlite::types::ToSql> =
            values.iter().map(|v| v.as_ref()).collect();
        params.push(&name);

        conn.execute(&query, params.as_slice())?;
        Ok(())
    }

    pub async fn delete_layer(&self, name: &str) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM layers WHERE name = ?", [name])?;
        Ok(())
    }

    // ========================================================================
    // SQL 视图 (SQL Views)
    // ========================================================================

    pub async fn get_sql_view(
        &self,
        name: &str,
    ) -> SqlResult<Option<crate::models::sql_view::SqlView>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT name, sql, workspace, store, geometry_column, geometry_type, crs, parameters, description, created, modified
             FROM sql_views WHERE name = ?"
        )?;
        let mut rows = stmt.query([name])?;
        if let Some(row) = rows.next()? {
            let params_str: String = row.get(7)?;
            let parameters: Vec<crate::models::sql_view::SqlViewParameter> =
                serde_json::from_str(&params_str).unwrap_or_default();
            Ok(Some(crate::models::sql_view::SqlView {
                name: row.get(0)?,
                sql: row.get(1)?,
                workspace: row.get(2)?,
                store: row.get(3)?,
                geometry_column: row.get(4)?,
                geometry_type: row.get(5)?,
                crs: row.get(6)?,
                parameters,
                description: row.get(8)?,
                created: row.get(9)?,
                modified: row.get(10)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn get_all_sql_views(&self) -> SqlResult<Vec<crate::models::sql_view::SqlView>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT name, sql, workspace, store, geometry_column, geometry_type, crs, parameters, description, created, modified
             FROM sql_views ORDER BY name"
        )?;
        let rows = stmt.query_map([], |row| {
            let params_str: String = row.get(7)?;
            let parameters: Vec<crate::models::sql_view::SqlViewParameter> =
                serde_json::from_str(&params_str).unwrap_or_default();
            Ok(crate::models::sql_view::SqlView {
                name: row.get(0)?,
                sql: row.get(1)?,
                workspace: row.get(2)?,
                store: row.get(3)?,
                geometry_column: row.get(4)?,
                geometry_type: row.get(5)?,
                crs: row.get(6)?,
                parameters,
                description: row.get(8)?,
                created: row.get(9)?,
                modified: row.get(10)?,
            })
        })?;
        rows.collect()
    }

    pub async fn create_sql_view(&self, view: &crate::models::sql_view::SqlView) -> SqlResult<()> {
        let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let params_json =
            serde_json::to_string(&view.parameters).unwrap_or_else(|_| "[]".to_string());
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO sql_views (name, sql, workspace, store, geometry_column, geometry_type, crs, parameters, description, created, modified)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                view.name, view.sql, view.workspace, view.store,
                view.geometry_column, view.geometry_type, view.crs,
                params_json, view.description, now, now
            ],
        )?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)] // matches the Store trait contract for SQL view updates
    pub async fn update_sql_view(
        &self,
        name: &str,
        sql: Option<String>,
        geometry_column: Option<String>,
        geometry_type: Option<String>,
        crs: Option<String>,
        parameters: Option<Vec<crate::models::sql_view::SqlViewParameter>>,
        description: Option<String>,
    ) -> SqlResult<()> {
        let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let conn = self.conn.lock().unwrap();

        let mut updates: Vec<String> = vec!["modified = ?".to_string()];
        let mut values: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(now.clone())];

        if let Some(ref s) = sql {
            updates.push("sql = ?".to_string());
            values.push(Box::new(s.clone()));
        }
        if let Some(ref g) = geometry_column {
            updates.push("geometry_column = ?".to_string());
            values.push(Box::new(g.clone()));
        }
        if let Some(ref g) = geometry_type {
            updates.push("geometry_type = ?".to_string());
            values.push(Box::new(g.clone()));
        }
        if let Some(ref c) = crs {
            updates.push("crs = ?".to_string());
            values.push(Box::new(c.clone()));
        }
        if let Some(ref p) = parameters {
            let params_json = serde_json::to_string(p).unwrap_or_else(|_| "[]".to_string());
            updates.push("parameters = ?".to_string());
            values.push(Box::new(params_json));
        }
        if let Some(ref d) = description {
            updates.push("description = ?".to_string());
            values.push(Box::new(d.clone()));
        }

        let query = format!("UPDATE sql_views SET {} WHERE name = ?", updates.join(", "));
        let mut params: Vec<&dyn rusqlite::types::ToSql> =
            values.iter().map(|v| v.as_ref()).collect();
        params.push(&name);
        conn.execute(&query, params.as_slice())?;
        Ok(())
    }

    pub async fn delete_sql_view(&self, name: &str) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM sql_views WHERE name = ?", [name])?;
        Ok(())
    }

    // ========================================================================
    // 用户管理
    // ========================================================================

    pub async fn get_user(&self, username: &str) -> SqlResult<Option<crate::auth::User>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT username, password_hash, salt, role, enabled, created, modified
             FROM users WHERE username = ?",
        )?;
        let mut rows = stmt.query([username])?;
        if let Some(row) = rows.next()? {
            let role_str: String = row.get(3)?;
            let role = match role_str.as_str() {
                "admin" => crate::auth::UserRole::Admin,
                "manager" => crate::auth::UserRole::Manager,
                "guest" => crate::auth::UserRole::Guest,
                _ => crate::auth::UserRole::User,
            };
            Ok(Some(crate::auth::User {
                username: row.get(0)?,
                password_hash: row.get(1)?,
                salt: row.get(2)?,
                role,
                enabled: row.get::<_, i32>(4)? == 1,
                created: row.get(5)?,
                modified: row.get(6)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn get_all_users(&self) -> SqlResult<Vec<crate::auth::User>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT username, password_hash, salt, role, enabled, created, modified
             FROM users ORDER BY username",
        )?;
        let rows = stmt.query_map([], |row| {
            let role_str: String = row.get(3)?;
            let role = match role_str.as_str() {
                "admin" => crate::auth::UserRole::Admin,
                "manager" => crate::auth::UserRole::Manager,
                "guest" => crate::auth::UserRole::Guest,
                _ => crate::auth::UserRole::User,
            };
            Ok(crate::auth::User {
                username: row.get(0)?,
                password_hash: row.get(1)?,
                salt: row.get(2)?,
                role,
                enabled: row.get::<_, i32>(4)? == 1,
                created: row.get(5)?,
                modified: row.get(6)?,
            })
        })?;
        rows.collect()
    }

    pub async fn create_user(
        &self,
        username: &str,
        password_hash: &str,
        salt: &str,
        role: &crate::auth::UserRole,
        enabled: bool,
    ) -> SqlResult<()> {
        let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO users (username, password_hash, salt, role, enabled, created, modified)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                username,
                password_hash,
                salt,
                role.to_string(),
                enabled as i32,
                now,
                now
            ],
        )?;
        Ok(())
    }

    pub async fn update_user(
        &self,
        username: &str,
        role: Option<&crate::auth::UserRole>,
        enabled: Option<bool>,
    ) -> SqlResult<()> {
        let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let conn = self.conn.lock().unwrap();
        let mut updates = vec!["modified = ?".to_string()];
        let mut values: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(now)];

        if let Some(r) = role {
            updates.push("role = ?".to_string());
            values.push(Box::new(r.to_string()));
        }
        if let Some(e) = enabled {
            updates.push("enabled = ?".to_string());
            values.push(Box::new(e as i32));
        }

        let query = format!("UPDATE users SET {} WHERE username = ?", updates.join(", "));
        let mut params: Vec<&dyn rusqlite::types::ToSql> =
            values.iter().map(|v| v.as_ref()).collect();
        params.push(&username);
        conn.execute(&query, params.as_slice())?;
        Ok(())
    }

    pub async fn delete_user(&self, username: &str) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM users WHERE username = ?", [username])?;
        Ok(())
    }

    /// 记录审计日志
    pub async fn audit_log(
        &self,
        username: &str,
        action: &str,
        resource: Option<&str>,
        detail: Option<&str>,
        ip_address: Option<&str>,
    ) -> SqlResult<()> {
        let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO audit_log (username, action, resource, detail, ip_address, timestamp)
             VALUES (?, ?, ?, ?, ?, ?)",
            rusqlite::params![username, action, resource, detail, ip_address, now],
        )?;
        Ok(())
    }

    /// 查询审计日志
    pub async fn get_audit_logs(
        &self,
        limit: usize,
        offset: usize,
    ) -> SqlResult<Vec<AuditLogRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, username, action, resource, detail, ip_address, timestamp
             FROM audit_log ORDER BY id DESC LIMIT ? OFFSET ?",
        )?;
        let rows = stmt.query_map(rusqlite::params![limit as i64, offset as i64], |row| {
            Ok(AuditLogRecord {
                id: row.get(0)?,
                username: row.get(1)?,
                action: row.get(2)?,
                resource: row.get(3)?,
                detail: row.get(4)?,
                ip_address: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }
        Ok(records)
    }

    // ========================================================================
    // 权限管理
    // ========================================================================

    pub async fn get_permissions(&self) -> SqlResult<Vec<crate::models::permission::Permission>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, username, role, resource_type, resource_name, access_mode, effect, priority
             FROM permissions ORDER BY priority DESC, resource_type, resource_name",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(crate::models::permission::Permission {
                id: Some(row.get(0)?),
                username: row.get(1)?,
                role: row.get(2)?,
                resource_type: row.get(3)?,
                resource_name: row.get(4)?,
                access_mode: row
                    .get::<_, String>(5)?
                    .parse()
                    .unwrap_or(crate::models::permission::AccessMode::Read),
                effect: row
                    .get::<_, String>(6)?
                    .parse()
                    .unwrap_or(crate::models::permission::Effect::Allow),
                priority: row.get(7)?,
            })
        })?;
        rows.collect()
    }

    pub async fn create_permission(
        &self,
        p: &crate::models::permission::Permission,
    ) -> SqlResult<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO permissions (username, role, resource_type, resource_name, access_mode, effect, priority)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![p.username, p.role, p.resource_type, p.resource_name,
                   p.access_mode.to_string(), p.effect.to_string(), p.priority],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub async fn delete_permission(&self, id: i64) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM permissions WHERE id = ?", [id])?;
        Ok(())
    }

    /// 检查用户是否有权访问指定资源
    pub async fn check_permission(
        &self,
        username: &str,
        role: &str,
        resource_type: &str,
        resource_name: &str,
        required_mode: &str,
    ) -> SqlResult<bool> {
        let conn = self.conn.lock().unwrap();

        // 查询匹配的权限规则，按优先级排序
        let sql = "SELECT effect, priority FROM permissions
                   WHERE (username = ?1 OR username = '*')
                     AND (role = ?2 OR role = '*')
                     AND (resource_type = ?3 OR resource_type = '*')
                     AND (resource_name = ?4 OR resource_name = '*')
                     AND (access_mode = ?5 OR access_mode = 'admin')
                   ORDER BY priority DESC LIMIT 1";

        let result = conn.query_row(
            sql,
            rusqlite::params![username, role, resource_type, resource_name, required_mode],
            |row| {
                let effect: String = row.get(0)?;
                Ok(effect == "allow")
            },
        );

        match result {
            Ok(allowed) => Ok(allowed),
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                // 没有匹配规则时，admin 默认有权限，其他人默认无权限
                Ok(role == "admin")
            },
            Err(e) => Err(e),
        }
    }
}

// ========================================================================
// Store trait 实现 (委托给固有方法 + 新增样式/图层组/会话)
// ========================================================================

#[async_trait::async_trait]
impl super::Store for SqliteStore {
    async fn get_workspace(&self, name: &str) -> Result<Option<Workspace>, super::StoreError> {
        SqliteStore::get_workspace(self, name)
            .await
            .map_err(super::StoreError::from)
    }

    async fn get_all_workspaces(&self) -> Result<Vec<Workspace>, super::StoreError> {
        SqliteStore::get_all_workspaces(self)
            .await
            .map_err(super::StoreError::from)
    }

    async fn create_workspace(
        &self,
        request: &CreateWorkspaceRequest,
    ) -> Result<Workspace, super::StoreError> {
        SqliteStore::create_workspace(self, request)
            .await
            .map_err(super::StoreError::from)
    }

    async fn update_workspace(
        &self,
        name: &str,
        title: Option<String>,
        description: Option<String>,
        enabled: Option<bool>,
    ) -> Result<(), super::StoreError> {
        SqliteStore::update_workspace(self, name, title, description, enabled)
            .await
            .map_err(super::StoreError::from)
    }

    async fn delete_workspace(&self, name: &str) -> Result<(), super::StoreError> {
        SqliteStore::delete_workspace(self, name)
            .await
            .map_err(super::StoreError::from)
    }

    async fn get_namespace(
        &self,
        prefix: &str,
    ) -> Result<Option<NamespaceRecord>, super::StoreError> {
        SqliteStore::get_namespace(self, prefix)
            .await
            .map_err(super::StoreError::from)
    }

    async fn get_all_namespaces(&self) -> Result<Vec<NamespaceRecord>, super::StoreError> {
        SqliteStore::get_all_namespaces(self)
            .await
            .map_err(super::StoreError::from)
    }

    async fn create_namespace(
        &self,
        prefix: &str,
        uri: &str,
        workspace: Option<&str>,
        isolated: bool,
    ) -> Result<NamespaceRecord, super::StoreError> {
        SqliteStore::create_namespace(self, prefix, uri, workspace, isolated)
            .await
            .map_err(super::StoreError::from)
    }

    async fn update_namespace(
        &self,
        prefix: &str,
        uri: Option<String>,
        isolated: Option<bool>,
        workspace: Option<String>,
    ) -> Result<(), super::StoreError> {
        SqliteStore::update_namespace(self, prefix, uri, isolated, workspace)
            .await
            .map_err(super::StoreError::from)
    }

    async fn delete_namespace(&self, prefix: &str) -> Result<(), super::StoreError> {
        SqliteStore::delete_namespace(self, prefix)
            .await
            .map_err(super::StoreError::from)
    }

    async fn get_data_source(&self, name: &str) -> Result<Option<DataSource>, super::StoreError> {
        SqliteStore::get_data_source(self, name)
            .await
            .map_err(super::StoreError::from)
    }

    async fn get_all_data_sources(&self) -> Result<Vec<DataSource>, super::StoreError> {
        SqliteStore::get_all_data_sources(self)
            .await
            .map_err(super::StoreError::from)
    }

    async fn create_data_source(
        &self,
        name: &str,
        data_source_type: &DataSourceType,
        workspace: Option<String>,
        enabled: bool,
        connection: &DataSourceConnection,
    ) -> Result<DataSource, super::StoreError> {
        SqliteStore::create_data_source(
            self,
            name,
            data_source_type,
            workspace,
            enabled,
            connection,
        )
        .await
        .map_err(super::StoreError::from)
    }

    async fn update_data_source(
        &self,
        name: &str,
        data_source_type: Option<DataSourceType>,
        workspace: Option<String>,
        enabled: Option<bool>,
        connection: Option<DataSourceConnection>,
    ) -> Result<(), super::StoreError> {
        SqliteStore::update_data_source(
            self,
            name,
            data_source_type,
            workspace,
            enabled,
            connection,
        )
        .await
        .map_err(super::StoreError::from)
    }

    async fn delete_data_source(&self, name: &str) -> Result<(), super::StoreError> {
        SqliteStore::delete_data_source(self, name)
            .await
            .map_err(super::StoreError::from)
    }

    async fn get_layer(&self, name: &str) -> Result<Option<Layer>, super::StoreError> {
        SqliteStore::get_layer(self, name)
            .await
            .map_err(super::StoreError::from)
    }

    async fn get_all_layers(&self) -> Result<Vec<Layer>, super::StoreError> {
        SqliteStore::get_all_layers(self)
            .await
            .map_err(super::StoreError::from)
    }

    async fn create_layer(&self, layer: &Layer) -> Result<Layer, super::StoreError> {
        SqliteStore::create_layer(self, layer)
            .await
            .map_err(super::StoreError::from)
    }

    async fn update_layer(
        &self,
        name: &str,
        title: Option<String>,
        abstract_text: Option<String>,
        native_name: Option<String>,
        enabled: Option<bool>,
        cache_store: Option<Option<String>>,
    ) -> Result<(), super::StoreError> {
        SqliteStore::update_layer(
            self,
            name,
            title,
            abstract_text,
            native_name,
            enabled,
            cache_store,
        )
        .await
        .map_err(super::StoreError::from)
    }

    async fn delete_layer(&self, name: &str) -> Result<(), super::StoreError> {
        SqliteStore::delete_layer(self, name)
            .await
            .map_err(super::StoreError::from)
    }

    async fn get_sql_view(&self, name: &str) -> Result<Option<SqlView>, super::StoreError> {
        SqliteStore::get_sql_view(self, name)
            .await
            .map_err(super::StoreError::from)
    }

    async fn get_all_sql_views(&self) -> Result<Vec<SqlView>, super::StoreError> {
        SqliteStore::get_all_sql_views(self)
            .await
            .map_err(super::StoreError::from)
    }

    async fn create_sql_view(&self, view: &SqlView) -> Result<(), super::StoreError> {
        SqliteStore::create_sql_view(self, view)
            .await
            .map_err(super::StoreError::from)
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
    ) -> Result<(), super::StoreError> {
        SqliteStore::update_sql_view(
            self,
            name,
            sql,
            geometry_column,
            geometry_type,
            crs,
            parameters,
            description,
        )
        .await
        .map_err(super::StoreError::from)
    }

    async fn delete_sql_view(&self, name: &str) -> Result<(), super::StoreError> {
        SqliteStore::delete_sql_view(self, name)
            .await
            .map_err(super::StoreError::from)
    }

    async fn get_user(
        &self,
        username: &str,
    ) -> Result<Option<crate::auth::User>, super::StoreError> {
        SqliteStore::get_user(self, username)
            .await
            .map_err(super::StoreError::from)
    }

    async fn get_all_users(&self) -> Result<Vec<crate::auth::User>, super::StoreError> {
        SqliteStore::get_all_users(self)
            .await
            .map_err(super::StoreError::from)
    }

    async fn create_user(
        &self,
        username: &str,
        password_hash: &str,
        salt: &str,
        role: &crate::auth::UserRole,
        enabled: bool,
    ) -> Result<(), super::StoreError> {
        SqliteStore::create_user(self, username, password_hash, salt, role, enabled)
            .await
            .map_err(super::StoreError::from)
    }

    async fn update_user(
        &self,
        username: &str,
        role: Option<&crate::auth::UserRole>,
        enabled: Option<bool>,
    ) -> Result<(), super::StoreError> {
        SqliteStore::update_user(self, username, role, enabled)
            .await
            .map_err(super::StoreError::from)
    }

    async fn delete_user(&self, username: &str) -> Result<(), super::StoreError> {
        SqliteStore::delete_user(self, username)
            .await
            .map_err(super::StoreError::from)
    }

    async fn audit_log(
        &self,
        username: &str,
        action: &str,
        resource: Option<&str>,
        detail: Option<&str>,
        ip_address: Option<&str>,
    ) -> Result<(), super::StoreError> {
        SqliteStore::audit_log(self, username, action, resource, detail, ip_address)
            .await
            .map_err(super::StoreError::from)
    }

    async fn get_audit_logs(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<AuditLogRecord>, super::StoreError> {
        SqliteStore::get_audit_logs(self, limit, offset)
            .await
            .map_err(super::StoreError::from)
    }

    async fn get_permissions(
        &self,
    ) -> Result<Vec<crate::models::permission::Permission>, super::StoreError> {
        SqliteStore::get_permissions(self)
            .await
            .map_err(super::StoreError::from)
    }

    async fn create_permission(
        &self,
        p: &crate::models::permission::Permission,
    ) -> Result<i64, super::StoreError> {
        SqliteStore::create_permission(self, p)
            .await
            .map_err(super::StoreError::from)
    }

    async fn delete_permission(&self, id: i64) -> Result<(), super::StoreError> {
        SqliteStore::delete_permission(self, id)
            .await
            .map_err(super::StoreError::from)
    }

    async fn check_permission(
        &self,
        username: &str,
        role: &str,
        resource_type: &str,
        resource_name: &str,
        required_mode: &str,
    ) -> Result<bool, super::StoreError> {
        SqliteStore::check_permission(
            self,
            username,
            role,
            resource_type,
            resource_name,
            required_mode,
        )
        .await
        .map_err(super::StoreError::from)
    }

    // ---- 样式 ----

    async fn get_all_styles(&self) -> Result<Vec<StyleRecord>, super::StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT name, title, format, is_builtin, content, created, modified
             FROM styles ORDER BY name",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(StyleRecord {
                name: row.get(0)?,
                title: row.get(1)?,
                format: row.get(2)?,
                is_builtin: row.get::<_, i32>(3)? == 1,
                content: row.get(4)?,
                created: row.get(5)?,
                modified: row.get(6)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<StyleRecord>>>()
            .map_err(super::StoreError::from)
    }

    async fn get_style(&self, name: &str) -> Result<Option<StyleRecord>, super::StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT name, title, format, is_builtin, content, created, modified
             FROM styles WHERE name = ?",
        )?;
        let mut rows = stmt.query([name])?;
        if let Some(row) = rows.next()? {
            Ok(Some(StyleRecord {
                name: row.get(0)?,
                title: row.get(1)?,
                format: row.get(2)?,
                is_builtin: row.get::<_, i32>(3)? == 1,
                content: row.get(4)?,
                created: row.get(5)?,
                modified: row.get(6)?,
            }))
        } else {
            Ok(None)
        }
    }

    async fn create_style(&self, style: &StyleRecord) -> Result<(), super::StoreError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO styles (name, title, format, is_builtin, content, created, modified)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            params![style.name, style.title, style.format, style.is_builtin as i32, style.content, style.created, style.modified],
        )?;
        Ok(())
    }

    async fn update_style(
        &self,
        name: &str,
        title: Option<String>,
        format: Option<String>,
        content: Option<String>,
        is_builtin: Option<bool>,
    ) -> Result<(), super::StoreError> {
        let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let conn = self.conn.lock().unwrap();
        let mut updates: Vec<String> = vec!["modified = ?".to_string()];
        let mut values: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(now)];
        if let Some(t) = title {
            updates.push("title = ?".to_string());
            values.push(Box::new(t));
        }
        if let Some(f) = format {
            updates.push("format = ?".to_string());
            values.push(Box::new(f));
        }
        if let Some(c) = content {
            updates.push("content = ?".to_string());
            values.push(Box::new(c));
        }
        if let Some(b) = is_builtin {
            updates.push("is_builtin = ?".to_string());
            values.push(Box::new(b as i32));
        }
        let query = format!("UPDATE styles SET {} WHERE name = ?", updates.join(", "));
        let mut params: Vec<&dyn rusqlite::types::ToSql> =
            values.iter().map(|v| v.as_ref()).collect();
        params.push(&name);
        conn.execute(&query, params.as_slice())?;
        Ok(())
    }

    async fn delete_style(&self, name: &str) -> Result<(), super::StoreError> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM styles WHERE name = ?", [name])?;
        Ok(())
    }

    // ---- 图层组 ----

    async fn get_all_layer_groups(&self) -> Result<Vec<LayerGroupRecord>, super::StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT name, title, layers, styles, created, modified
             FROM layer_groups ORDER BY name",
        )?;
        let rows = stmt.query_map([], |row| {
            let layers_json: String = row.get(2)?;
            let styles_json: String = row.get(3)?;
            Ok(LayerGroupRecord {
                name: row.get(0)?,
                title: row.get(1)?,
                layers: serde_json::from_str(&layers_json).unwrap_or_default(),
                styles: serde_json::from_str(&styles_json).unwrap_or_default(),
                created: row.get(4)?,
                modified: row.get(5)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<LayerGroupRecord>>>()
            .map_err(super::StoreError::from)
    }

    async fn get_layer_group(
        &self,
        name: &str,
    ) -> Result<Option<LayerGroupRecord>, super::StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT name, title, layers, styles, created, modified
             FROM layer_groups WHERE name = ?",
        )?;
        let mut rows = stmt.query([name])?;
        if let Some(row) = rows.next()? {
            let layers_json: String = row.get(2)?;
            let styles_json: String = row.get(3)?;
            Ok(Some(LayerGroupRecord {
                name: row.get(0)?,
                title: row.get(1)?,
                layers: serde_json::from_str(&layers_json).unwrap_or_default(),
                styles: serde_json::from_str(&styles_json).unwrap_or_default(),
                created: row.get(4)?,
                modified: row.get(5)?,
            }))
        } else {
            Ok(None)
        }
    }

    async fn create_layer_group(&self, group: &LayerGroupRecord) -> Result<(), super::StoreError> {
        let conn = self.conn.lock().unwrap();
        let layers_json = serde_json::to_string(&group.layers).unwrap_or_else(|_| "[]".to_string());
        let styles_json = serde_json::to_string(&group.styles).unwrap_or_else(|_| "[]".to_string());
        conn.execute(
            "INSERT OR REPLACE INTO layer_groups (name, title, layers, styles, created, modified)
             VALUES (?, ?, ?, ?, ?, ?)",
            params![
                group.name,
                group.title,
                layers_json,
                styles_json,
                group.created,
                group.modified
            ],
        )?;
        Ok(())
    }

    async fn delete_layer_group(&self, name: &str) -> Result<(), super::StoreError> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM layer_groups WHERE name = ?", [name])?;
        Ok(())
    }

    // ---- 会话 ----

    async fn create_session(&self, session: &SessionRecord) -> Result<(), super::StoreError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO sessions (jti, username, role, issued_at, expires_at, last_seen_at, revoked, user_agent, ip_address)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                session.jti, session.username, session.role,
                session.issued_at, session.expires_at, session.last_seen_at,
                session.revoked as i32, session.user_agent, session.ip_address
            ],
        )?;
        Ok(())
    }

    async fn get_session(&self, jti: &str) -> Result<Option<SessionRecord>, super::StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT jti, username, role, issued_at, expires_at, last_seen_at, revoked, user_agent, ip_address
             FROM sessions WHERE jti = ?"
        )?;
        let mut rows = stmt.query([jti])?;
        if let Some(row) = rows.next()? {
            Ok(Some(SessionRecord {
                jti: row.get(0)?,
                username: row.get(1)?,
                role: row.get(2)?,
                issued_at: row.get(3)?,
                expires_at: row.get(4)?,
                last_seen_at: row.get(5)?,
                revoked: row.get::<_, i32>(6)? == 1,
                user_agent: row.get(7)?,
                ip_address: row.get(8)?,
            }))
        } else {
            Ok(None)
        }
    }

    async fn delete_session(&self, jti: &str) -> Result<(), super::StoreError> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM sessions WHERE jti = ?", [jti])?;
        Ok(())
    }

    async fn delete_user_sessions(&self, username: &str) -> Result<(), super::StoreError> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM sessions WHERE username = ?", [username])?;
        Ok(())
    }

    async fn cleanup_expired_sessions(&self) -> Result<usize, super::StoreError> {
        let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let conn = self.conn.lock().unwrap();
        let count = conn.execute("DELETE FROM sessions WHERE expires_at < ?", [now])?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handlers::CreateWorkspaceRequest;
    use crate::models::permission::{AccessMode, Effect, Permission};
    use crate::store::Store;

    async fn new_store() -> SqliteStore {
        SqliteStore::new(":memory:").await.expect("in-memory store")
    }

    #[actix_rt::test]
    async fn test_workspace_crud() {
        let store = new_store().await;
        store
            .create_workspace(&CreateWorkspaceRequest {
                name: "ws1".into(),
                title: Some("WS1".into()),
                description: None,
            })
            .await
            .unwrap();

        let got = store.get_workspace("ws1").await.unwrap().unwrap();
        assert_eq!(got.name, "ws1");
        assert_eq!(got.title, "WS1");

        let all = store.get_all_workspaces().await.unwrap();
        assert!(all.iter().any(|w| w.name == "ws1"));

        store.delete_workspace("ws1").await.unwrap();
        assert!(store.get_workspace("ws1").await.unwrap().is_none());
    }

    #[actix_rt::test]
    async fn test_namespace_crud() {
        let store = new_store().await;
        store
            .create_namespace("ns1", "http://example.com/ns1", None, false)
            .await
            .unwrap();
        let got = store.get_namespace("ns1").await.unwrap().unwrap();
        assert_eq!(got.prefix, "ns1");
        assert_eq!(got.uri, "http://example.com/ns1");
        store.delete_namespace("ns1").await.unwrap();
        assert!(store.get_namespace("ns1").await.unwrap().is_none());
    }

    #[actix_rt::test]
    async fn test_user_and_permission() {
        let store = new_store().await;
        store
            .create_user("alice", "hash", "salt", &crate::auth::UserRole::User, true)
            .await
            .unwrap();
        let u = store.get_user("alice").await.unwrap().unwrap();
        assert_eq!(u.username, "alice");

        let perm = Permission {
            id: None,
            username: "alice".into(),
            role: "*".into(),
            resource_type: "layer".into(),
            resource_name: "world".into(),
            access_mode: AccessMode::Read,
            effect: Effect::Allow,
            priority: 0,
        };
        let id = store.create_permission(&perm).await.unwrap();
        assert!(id > 0);
        let perms = store.get_permissions().await.unwrap();
        assert!(perms
            .iter()
            .any(|p| p.id == Some(id) && p.resource_name == "world"));

        let allowed = store
            .check_permission("alice", "user", "layer", "world", "read")
            .await
            .unwrap();
        assert!(allowed, "alice 应对 layer/world 有读权限");

        store.delete_permission(id).await.unwrap();
        store.delete_user("alice").await.unwrap();
        assert!(store.get_user("alice").await.unwrap().is_none());
    }

    #[actix_rt::test]
    async fn test_session_crud() {
        let store = new_store().await;
        let session = SessionRecord {
            jti: "jti-1".into(),
            username: "bob".into(),
            role: "user".into(),
            issued_at: "2026-01-01T00:00:00Z".into(),
            expires_at: "2026-01-02T00:00:00Z".into(),
            last_seen_at: "2026-01-01T00:00:00Z".into(),
            revoked: false,
            user_agent: None,
            ip_address: None,
        };
        store.create_session(&session).await.unwrap();
        let got = store.get_session("jti-1").await.unwrap().unwrap();
        assert_eq!(got.username, "bob");
        store.delete_session("jti-1").await.unwrap();
        assert!(store.get_session("jti-1").await.unwrap().is_none());
    }

    #[actix_rt::test]
    async fn test_layer_cache_store_crud() {
        let store = new_store().await;

        // 创建图层并指定 Redis 缓存数据源
        let layer = Layer {
            name: "redis_layer".into(),
            title: "Redis Layer".into(),
            workspace: "ws1".into(),
            store: "shapes".into(),
            srs: "EPSG:4326".into(),
            abstract_text: None,
            native_name: Some("redis_layer".into()),
            enabled: true,
            minx: -180.0,
            miny: -90.0,
            maxx: 180.0,
            maxy: 90.0,
            cache_store: Some("my_redis".into()),
            created: String::new(),
            modified: String::new(),
        };
        store.create_layer(&layer).await.unwrap();

        let got = store.get_layer("redis_layer").await.unwrap().unwrap();
        assert_eq!(
            got.cache_store.as_deref(),
            Some("my_redis"),
            "cache_store 应持久化"
        );

        let all = store.get_all_layers().await.unwrap();
        assert_eq!(
            all.iter()
                .find(|l| l.name == "redis_layer")
                .and_then(|l| l.cache_store.clone())
                .as_deref(),
            Some("my_redis"),
            "get_all_layers 应返回 cache_store"
        );

        // 更新: 设置新的缓存数据源
        store
            .update_layer(
                "redis_layer",
                None,
                None,
                None,
                None,
                Some(Some("other_redis".into())),
            )
            .await
            .unwrap();
        let updated = store.get_layer("redis_layer").await.unwrap().unwrap();
        assert_eq!(updated.cache_store.as_deref(), Some("other_redis"));

        // 更新: 清除缓存数据源 (回到默认内存/本地缓存)
        store
            .update_layer("redis_layer", None, None, None, None, Some(None))
            .await
            .unwrap();
        let cleared = store.get_layer("redis_layer").await.unwrap().unwrap();
        assert!(cleared.cache_store.is_none(), "cache_store 清除后应为 None");
    }

    #[actix_rt::test]
    async fn test_styles_crud() {
        let store = new_store().await;
        let ts = String::new();
        store
            .create_style(&StyleRecord {
                name: "s1".into(),
                title: "S1".into(),
                format: "SLD".into(),
                is_builtin: false,
                content: "<StyledLayerDescriptor/>".into(),
                created: ts.clone(),
                modified: ts.clone(),
            })
            .await
            .unwrap();

        let got = store.get_style("s1").await.unwrap().unwrap();
        assert_eq!(got.title, "S1");
        assert_eq!(got.format, "SLD");

        store
            .update_style(
                "s1",
                Some("S1 v2".into()),
                None,
                Some("<StyledLayerDescriptor>v2</StyledLayerDescriptor>".into()),
                None,
            )
            .await
            .unwrap();
        let got = store.get_style("s1").await.unwrap().unwrap();
        assert_eq!(got.title, "S1 v2");
        assert!(
            got.content.contains("v2"),
            "内容应已更新, 实际: {}",
            got.content
        );

        let all = store.get_all_styles().await.unwrap();
        assert!(all.iter().any(|s| s.name == "s1"));

        store.delete_style("s1").await.unwrap();
        assert!(store.get_style("s1").await.unwrap().is_none());
    }

    #[actix_rt::test]
    async fn test_layer_group_crud() {
        let store = new_store().await;
        let ts = String::new();
        store
            .create_layer_group(&LayerGroupRecord {
                name: "lg1".into(),
                title: "LG1".into(),
                layers: vec!["world".into()],
                styles: vec![Some("default".into())],
                created: ts.clone(),
                modified: ts.clone(),
            })
            .await
            .unwrap();

        let got = store.get_layer_group("lg1").await.unwrap().unwrap();
        assert_eq!(got.title, "LG1");
        assert_eq!(got.layers, vec!["world".to_string()]);
        assert_eq!(got.styles, vec![Some("default".to_string())]);

        let all = store.get_all_layer_groups().await.unwrap();
        assert!(all.iter().any(|g| g.name == "lg1"));

        store.delete_layer_group("lg1").await.unwrap();
        assert!(store.get_layer_group("lg1").await.unwrap().is_none());
    }

    #[actix_rt::test]
    async fn test_audit_logs() {
        let store = new_store().await;
        store
            .audit_log(
                "alice",
                "login",
                Some("auth"),
                Some("login success"),
                Some("127.0.0.1"),
            )
            .await
            .unwrap();
        store
            .audit_log("alice", "delete", Some("layer/world"), None, None)
            .await
            .unwrap();

        let logs = store.get_audit_logs(10, 0).await.unwrap();
        assert_eq!(logs.len(), 2, "应记录 2 条审计日志, 实际: {}", logs.len());
        assert!(logs.iter().all(|l| l.username == "alice"));

        let login = logs.iter().find(|l| l.action == "login").unwrap();
        assert_eq!(login.resource.as_deref(), Some("auth"));
        assert_eq!(login.detail.as_deref(), Some("login success"));
        assert_eq!(login.ip_address.as_deref(), Some("127.0.0.1"));
        assert!(!login.created_at.is_empty(), "审计日志应带时间戳");

        // 分页
        let paged = store.get_audit_logs(1, 1).await.unwrap();
        assert_eq!(paged.len(), 1, "limit=1 offset=1 应返回 1 条");
    }
}
