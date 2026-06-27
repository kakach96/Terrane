use std::sync::{Arc, Mutex};
use rusqlite::{Connection, Result as SqlResult, params};
use chrono::Utc;
use crate::models::{DataSource, DataSourceType, DataSourceConnection};
use crate::handlers::CreateWorkspaceRequest;

/// 数据库层命名空间记录
#[derive(Debug, Clone)]
pub struct NamespaceRecord {
    pub prefix: String,
    pub uri: String,
    pub isolated: bool,
    pub workspace: Option<String>,
    pub created: String,
    pub modified: String,
}

fn column_exists(conn: &Connection, table: &str, column: &str) -> bool {
    conn.query_row(
        &format!("SELECT COUNT(*) FROM pragma_table_info('{}') WHERE name='{}'", table, column),
        [],
        |row| row.get::<_, i32>(0).map(|c| c > 0),
    ).unwrap_or(false)
}

fn parse_ds_type(type_str: &str) -> DataSourceType {
    match type_str {
        "postgis" => DataSourceType::Postgis,
        "shapefile" => DataSourceType::Shapefile,
        "geotiff" => DataSourceType::Geotiff,
        "geopackage" => DataSourceType::Geopackage,
        "worldimage" => DataSourceType::WorldImage,
        _ => DataSourceType::Postgis,
    }
}

#[derive(Debug, Clone)]
pub struct Workspace {
    pub name: String,
    pub title: String,
    pub enabled: bool,
    pub layer_count: i32,
    pub description: String,
    pub created: String,
    pub modified: String,
}

#[derive(Debug, Clone)]
pub struct Layer {
    pub name: String,
    pub title: String,
    pub workspace: String,
    pub store: String,
    pub srs: String,
    pub abstract_text: Option<String>,
    pub native_name: Option<String>,
    pub enabled: bool,
    pub minx: f64,
    pub miny: f64,
    pub maxx: f64,
    pub maxy: f64,
    pub created: String,
    pub modified: String,
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
            conn.execute(
                "ALTER TABLE data_sources ADD COLUMN file_path TEXT",
                [],
            )?;
        }

        // 检查并添加 file_storage_type 列
        if !column_exists(conn, "data_sources", "file_storage_type") {
            conn.execute(
                "ALTER TABLE data_sources ADD COLUMN file_storage_type TEXT DEFAULT 'local'",
                [],
            )?;
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
                created TEXT,
                modified TEXT
            )",
            [],
        )?;

        // 检查并添加 native_name 列（向后兼容）
        if !column_exists(conn, "layers", "native_name") {
            conn.execute(
                "ALTER TABLE layers ADD COLUMN native_name TEXT",
                [],
            )?;
        }

        // 要素存储表（用于 GeoJSON 上传持久化）
        conn.execute(
            "CREATE TABLE IF NOT EXISTS features (
                layer_name TEXT NOT NULL,
                feature_id TEXT NOT NULL,
                geometry TEXT NOT NULL,
                properties TEXT,
                PRIMARY KEY (layer_name, feature_id)
            )",
            [],
        )?;

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

        Ok(())
    }

    pub async fn get_workspace(&self, name: &str) -> SqlResult<Option<Workspace>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT name, title, description, enabled, layer_count, created, modified 
             FROM workspaces WHERE name = ?"
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
             FROM workspaces ORDER BY name"
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
        let title = request.title.clone().unwrap_or_else(|| request.name.clone());
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

    pub async fn update_workspace(&self, name: &str, title: Option<String>, description: Option<String>, enabled: Option<bool>) -> SqlResult<()> {
        let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let conn = self.conn.lock().unwrap();
        
        let mut updates: Vec<String> = vec!["modified = ?".to_string()];
        let mut values: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(now.clone())];
        
        if let Some(t) = title {
            updates.push(format!("title = ?"));
            values.push(Box::new(t));
        }
        if let Some(d) = description {
            updates.push(format!("description = ?"));
            values.push(Box::new(d));
        }
        if let Some(e) = enabled {
            updates.push(format!("enabled = ?"));
            values.push(Box::new(e as i32));
        }
        
        let query = format!("UPDATE workspaces SET {} WHERE name = ?", updates.join(", "));
        let mut params: Vec<&dyn rusqlite::types::ToSql> = values.iter().map(|v| v.as_ref()).collect();
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
             FROM namespaces WHERE prefix = ?"
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
             FROM namespaces ORDER BY prefix"
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

    pub async fn create_namespace(&self, prefix: &str, uri: &str, workspace: Option<&str>, isolated: bool) -> SqlResult<NamespaceRecord> {
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

    pub async fn update_namespace(&self, prefix: &str, uri: Option<String>, isolated: Option<bool>, workspace: Option<String>) -> SqlResult<()> {
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

        let query = format!("UPDATE namespaces SET {} WHERE prefix = ?", updates.join(", "));
        let mut params: Vec<&dyn rusqlite::types::ToSql> = values.iter().map(|v| v.as_ref()).collect();
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
    fn row_to_data_source(row: &rusqlite::Row, has_schema: bool, has_file: bool) -> rusqlite::Result<DataSource> {
        let host: Option<String> = row.get(4)?;
        let port: Option<u16> = row.get(5)?;
        let db: Option<String> = row.get(6)?;

        let schema = if has_schema {
            row.get::<_, Option<String>>(7)?.unwrap_or_else(|| "public".to_string())
        } else {
            "public".to_string()
        };

        let (user_idx, pass_idx, file_path_idx, file_storage_idx, created_idx, modified_idx) = if has_file {
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
            }),
            created,
            modified,
        })
    }

    pub async fn get_data_source(&self, name: &str) -> SqlResult<Option<DataSource>> {
        let conn = self.conn.lock().unwrap();

        let has_schema = column_exists(&conn, "data_sources", "schema_name");
        let has_file = column_exists(&conn, "data_sources", "file_path");

        let sql = if has_file {
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
            Ok(Some(Self::row_to_data_source(row, has_schema, has_file)?))
        } else {
            Ok(None)
        }
    }

    pub async fn get_all_data_sources(&self) -> SqlResult<Vec<DataSource>> {
        let conn = self.conn.lock().unwrap();

        let has_schema = column_exists(&conn, "data_sources", "schema_name");
        let has_file = column_exists(&conn, "data_sources", "file_path");

        let sql = if has_file {
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
        let rows = stmt.query_map([], move |row| {
            Self::row_to_data_source(row, has_schema_ref, has_file_ref)
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

        if has_file {
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
        }

        let query = format!("UPDATE data_sources SET {} WHERE name = ?", updates.join(", "));
        let mut params: Vec<&dyn rusqlite::types::ToSql> = values.iter().map(|v| v.as_ref()).collect();
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
        let mut stmt = if has_native_name {
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
            if has_native_name {
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
        let mut stmt = if has_native_name {
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
            if has_native_name {
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
            created: now.clone(),
            modified: now,
        })
    }

    pub async fn update_layer(&self, name: &str, title: Option<String>, abstract_text: Option<String>, native_name: Option<String>, enabled: Option<bool>) -> SqlResult<()> {
        let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let conn = self.conn.lock().unwrap();

        let mut updates: Vec<String> = vec!["modified = ?".to_string()];
        let mut values: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(now)];

        if let Some(t) = title {
            updates.push(format!("title = ?"));
            values.push(Box::new(t));
        }
        if let Some(a) = abstract_text {
            updates.push(format!("abstract_text = ?"));
            values.push(Box::new(a));
        }
        if let Some(n) = native_name {
            updates.push(format!("native_name = ?"));
            values.push(Box::new(n));
        }
        if let Some(e) = enabled {
            updates.push(format!("enabled = ?"));
            values.push(Box::new(e as i32));
        }

        let query = format!("UPDATE layers SET {} WHERE name = ?", updates.join(", "));
        let mut params: Vec<&dyn rusqlite::types::ToSql> = values.iter().map(|v| v.as_ref()).collect();
        params.push(&name);

        conn.execute(&query, params.as_slice())?;
        Ok(())
    }

    pub async fn delete_layer(&self, name: &str) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM layers WHERE name = ?", [name])?;
        Ok(())
    }

    // ---- 要素持久化 ----

    /// 保存要素到 features 表（GeoJSON 上传持久化）
    pub async fn save_features(
        &self,
        layer_name: &str,
        features: &[crate::models::Feature],
    ) -> SqlResult<usize> {
        let conn = self.conn.lock().unwrap();

        // 清空该图层的旧数据
        conn.execute("DELETE FROM features WHERE layer_name = ?", [layer_name])?;

        let mut count = 0;
        for feature in features {
            let geometry = serde_json::to_string(&feature.geometry)
                .unwrap_or_else(|_| "{}".to_string());
            let properties = serde_json::to_string(&feature.properties)
                .unwrap_or_else(|_| "{}".to_string());

            conn.execute(
                "INSERT OR REPLACE INTO features (layer_name, feature_id, geometry, properties)
                 VALUES (?, ?, ?, ?)",
                rusqlite::params![layer_name, feature.id, geometry, properties],
            )?;
            count += 1;
        }

        Ok(count)
    }

    /// 加载指定图层的所有要素
    pub async fn load_features(
        &self,
        layer_name: &str,
    ) -> SqlResult<Vec<crate::models::Feature>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT feature_id, geometry, properties FROM features WHERE layer_name = ? ORDER BY feature_id"
        )?;

        let rows = stmt.query_map([layer_name], |row| {
            let id: String = row.get(0)?;
            let geometry_str: String = row.get(1)?;
            let properties_str: Option<String> = row.get(2)?;

            let geometry = serde_json::from_str(&geometry_str)
                .unwrap_or(crate::models::GeoJsonGeometry::Point { coordinates: vec![0.0, 0.0] });
            let properties = properties_str
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();

            Ok(crate::models::Feature::with_id(id, geometry, properties))
        })?;

        rows.collect()
    }

    /// 删除指定图层的所有要素
    pub async fn delete_features(&self, layer_name: &str) -> SqlResult<usize> {
        let conn = self.conn.lock().unwrap();
        let count = conn.execute("DELETE FROM features WHERE layer_name = ?", [layer_name])?;
        Ok(count)
    }

    // ========================================================================
    // SQL 视图 (SQL Views)
    // ========================================================================

    pub async fn get_sql_view(&self, name: &str) -> SqlResult<Option<crate::models::sql_view::SqlView>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT name, sql, workspace, store, geometry_column, geometry_type, crs, parameters, description, created, modified
             FROM sql_views WHERE name = ?"
        )?;
        let mut rows = stmt.query([name])?;
        if let Some(row) = rows.next()? {
            let params_str: String = row.get(7)?;
            let parameters: Vec<crate::models::sql_view::SqlViewParameter> = serde_json::from_str(&params_str).unwrap_or_default();
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
            let parameters: Vec<crate::models::sql_view::SqlViewParameter> = serde_json::from_str(&params_str).unwrap_or_default();
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
        let params_json = serde_json::to_string(&view.parameters).unwrap_or_else(|_| "[]".to_string());
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

    pub async fn update_sql_view(&self, name: &str, sql: Option<String>, geometry_column: Option<String>,
                                  geometry_type: Option<String>, crs: Option<String>,
                                  parameters: Option<Vec<crate::models::sql_view::SqlViewParameter>>,
                                  description: Option<String>) -> SqlResult<()> {
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
        let mut params: Vec<&dyn rusqlite::types::ToSql> = values.iter().map(|v| v.as_ref()).collect();
        params.push(&name);
        conn.execute(&query, params.as_slice())?;
        Ok(())
    }

    pub async fn delete_sql_view(&self, name: &str) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM sql_views WHERE name = ?", [name])?;
        Ok(())
    }
}