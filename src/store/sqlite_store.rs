use std::sync::{Arc, Mutex};
use rusqlite::{Connection, Result as SqlResult, params};
use chrono::Utc;
use crate::models::{DataSource, DataSourceType, DataSourceConnection};
use crate::handlers::CreateWorkspaceRequest;

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
        let schema_exists: Result<bool, _> = conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('data_sources') WHERE name='schema_name'",
            [],
            |row| row.get::<_, i32>(0).map(|c| c > 0),
        );

        if let Ok(false) = schema_exists {
            conn.execute(
                "ALTER TABLE data_sources ADD COLUMN schema_name TEXT DEFAULT 'public'",
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
        let native_name_exists: Result<bool, _> = conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('layers') WHERE name='native_name'",
            [],
            |row| row.get::<_, i32>(0).map(|c| c > 0),
        );

        if let Ok(false) = native_name_exists {
            conn.execute(
                "ALTER TABLE layers ADD COLUMN native_name TEXT",
                [],
            )?;
        }

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

    pub async fn get_data_source(&self, name: &str) -> SqlResult<Option<DataSource>> {
        let conn = self.conn.lock().unwrap();
        
        // 检查 schema_name 列是否存在
        let schema_exists: bool = conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('data_sources') WHERE name='schema_name'",
            [],
            |row| row.get::<_, i32>(0).map(|c| c > 0),
        )?;

        let (stmt, num_columns) = if schema_exists {
            let stmt = conn.prepare(
                "SELECT name, type, workspace, enabled, host, port, database_name, schema_name, username, password, created, modified 
                 FROM data_sources WHERE name = ?"
            )?;
            (stmt, 12)
        } else {
            let stmt = conn.prepare(
                "SELECT name, type, workspace, enabled, host, port, database_name, username, password, created, modified 
                 FROM data_sources WHERE name = ?"
            )?;
            (stmt, 11)
        };

        let mut stmt = stmt;
        let mut rows = stmt.query([name])?;
        if let Some(row) = rows.next()? {
            let (schema_idx, username_idx, password_idx, created_idx, modified_idx) = if num_columns == 12 {
                (7, 8, 9, 10, 11)
            } else {
                (7, 7, 8, 9, 10)
            };

            Ok(Some(DataSource {
                name: row.get(0)?,
                data_source_type: match row.get::<_, String>(1)?.as_str() {
                    "postgis" => DataSourceType::Postgis,
                    "shapefile" => DataSourceType::Shapefile,
                    "geotiff" => DataSourceType::Geotiff,
                    _ => DataSourceType::Postgis,
                },
                workspace: row.get(2)?,
                enabled: row.get::<_, i32>(3)? == 1,
                connection: Some(DataSourceConnection {
                    host: row.get(4)?,
                    port: row.get(5)?,
                    database: row.get(6)?,
                    schema: if num_columns == 12 {
                        row.get::<_, Option<String>>(schema_idx)?.unwrap_or_else(|| "public".to_string())
                    } else {
                        "public".to_string()
                    },
                    username: row.get(username_idx)?,
                    password: row.get(password_idx)?,
                }),
                created: row.get(created_idx)?,
                modified: row.get(modified_idx)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn get_all_data_sources(&self) -> SqlResult<Vec<DataSource>> {
        let conn = self.conn.lock().unwrap();
        
        // 检查 schema_name 列是否存在
        let schema_exists: bool = conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('data_sources') WHERE name='schema_name'",
            [],
            |row| row.get::<_, i32>(0).map(|c| c > 0),
        )?;

        let (stmt, num_columns) = if schema_exists {
            let stmt = conn.prepare(
                "SELECT name, type, workspace, enabled, host, port, database_name, schema_name, username, password, created, modified 
                 FROM data_sources ORDER BY name"
            )?;
            (stmt, 12)
        } else {
            let stmt = conn.prepare(
                "SELECT name, type, workspace, enabled, host, port, database_name, username, password, created, modified 
                 FROM data_sources ORDER BY name"
            )?;
            (stmt, 11)
        };

        let mut stmt = stmt;
        let rows = stmt.query_map([], move |row| {
            let (schema_idx, username_idx, password_idx, created_idx, modified_idx) = if num_columns == 12 {
                (7, 8, 9, 10, 11)
            } else {
                (7, 7, 8, 9, 10)
            };

            Ok(DataSource {
                name: row.get(0)?,
                data_source_type: match row.get::<_, String>(1)?.as_str() {
                    "postgis" => DataSourceType::Postgis,
                    "shapefile" => DataSourceType::Shapefile,
                    "geotiff" => DataSourceType::Geotiff,
                    _ => DataSourceType::Postgis,
                },
                workspace: row.get(2)?,
                enabled: row.get::<_, i32>(3)? == 1,
                connection: Some(DataSourceConnection {
                    host: row.get(4)?,
                    port: row.get(5)?,
                    database: row.get(6)?,
                    schema: if num_columns == 12 {
                        row.get::<_, Option<String>>(schema_idx)?.unwrap_or_else(|| "public".to_string())
                    } else {
                        "public".to_string()
                    },
                    username: row.get(username_idx)?,
                    password: row.get(password_idx)?,
                }),
                created: row.get(created_idx)?,
                modified: row.get(modified_idx)?,
            })
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

        // 检查 native_name 列是否存在
        let native_name_exists: bool = conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('layers') WHERE name='native_name'",
            [],
            |row| row.get::<_, i32>(0).map(|c| c > 0),
        )?;

        let mut stmt = if native_name_exists {
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
            if native_name_exists {
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

        // 检查 native_name 列是否存在
        let native_name_exists: bool = conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('layers') WHERE name='native_name'",
            [],
            |row| row.get::<_, i32>(0).map(|c| c > 0),
        )?;

        let mut stmt = if native_name_exists {
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
            if native_name_exists {
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
}