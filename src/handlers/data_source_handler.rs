use super::rest_handler::ApiResponse;
use crate::error::GeoServerError;
use crate::models::{
    CreateDataSourceRequest, DataSource, DataSourceConnection, DataSourceType,
    UpdateDataSourceRequest, METADATA_DATA_SOURCE,
};
use crate::state::AppState;
use actix_web::{web, HttpRequest, HttpResponse};
use std::time::Instant;

/// 构造内置 metadata 数据源的 JSON 表示 (内置默认选项, 不可编辑/删除)。
///
/// 内置 metadata 数据源被当作普通数据源看待: 它除了存储元数据外, 也可发布其
/// 承载的业务数据。
/// - 元数据存储为 postgres 时 type 显示为 postgis, connection 展示元数据 postgres
///   连接 (复用同一 PG 发布业务表)
/// - 元数据存储为 sqlite 时 type 显示为 metadata (不承载业务表, 要素查询返回空)
fn builtin_metadata_data_source(state: &AppState) -> Option<serde_json::Value> {
    let mc = &state.config.metadata;
    let (ds_type, connection) = if mc.kind == "postgres" {
        let pg = &mc.postgres;
        (
            "postgis".to_string(),
            serde_json::json!({
                "host": pg.host,
                "port": pg.port,
                "database": pg.instance,
                "schema": pg.schema,
                "username": pg.user,
            }),
        )
    } else {
        (
            "metadata".to_string(),
            serde_json::json!({
                "file_path": mc.sqlite_path.to_string_lossy(),
            }),
        )
    };

    Some(serde_json::json!({
        "name": METADATA_DATA_SOURCE,
        "type": ds_type,
        "workspace": serde_json::Value::Null,
        "enabled": true,
        "builtin": true,
        "connection": connection,
        "created": serde_json::Value::Null,
        "modified": serde_json::Value::Null,
    }))
}

/// 判断数据源名称是否为内置 metadata 数据源
fn is_builtin_metadata(name: &str) -> bool {
    name == METADATA_DATA_SOURCE
}

pub async fn list_data_sources(state: web::Data<AppState>) -> Result<HttpResponse, GeoServerError> {
    if let Some(store) = &state.store {
        match store.get_all_data_sources().await {
            Ok(data_sources) => {
                let mut result: Vec<_> = data_sources
                    .iter()
                    .map(|ds| {
                        serde_json::json!({
                            "name": ds.name,
                            "type": format!("{}", ds.data_source_type).to_lowercase(),
                            "workspace": ds.workspace,
                            "enabled": ds.enabled,
                            "connection": ds.connection,
                            "created": ds.created,
                            "modified": ds.modified,
                            "builtin": false,
                        })
                    })
                    .collect();

                // 业务数据复用元数据存储时, 注入内置 metadata 数据源作为默认选项
                if let Some(meta) = builtin_metadata_data_source(&state) {
                    result.insert(0, meta);
                }

                Ok(HttpResponse::Ok().json(ApiResponse::success(result)))
            },
            Err(e) => {
                eprintln!("Failed to list data sources: {}", e);
                Err(GeoServerError::InternalError(
                    "Failed to list data sources".to_string(),
                ))
            },
        }
    } else {
        Err(GeoServerError::InternalError(
            "Database not available".to_string(),
        ))
    }
}

pub async fn get_data_source(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let name = req.match_info().get("name").unwrap_or("");

    // 内置 metadata 数据源
    if is_builtin_metadata(name) {
        if let Some(meta) = builtin_metadata_data_source(&state) {
            return Ok(HttpResponse::Ok().json(ApiResponse::success(meta)));
        }
    }

    if let Some(store) = &state.store {
        match store.get_data_source(name).await {
            Ok(Some(ds)) => Ok(
                HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
                    "name": ds.name,
                    "type": format!("{}", ds.data_source_type).to_lowercase(),
                    "workspace": ds.workspace,
                    "enabled": ds.enabled,
                    "connection": ds.connection,
                    "created": ds.created,
                    "modified": ds.modified,
                    "builtin": false,
                }))),
            ),
            Ok(None) => Err(GeoServerError::NotFound(format!(
                "Data source '{}' not found",
                name
            ))),
            Err(e) => {
                eprintln!("Failed to get data source: {}", e);
                Err(GeoServerError::InternalError(
                    "Failed to get data source".to_string(),
                ))
            },
        }
    } else {
        Err(GeoServerError::InternalError(
            "Database not available".to_string(),
        ))
    }
}

pub async fn create_data_source(
    body: web::Json<CreateDataSourceRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    if is_builtin_metadata(&body.name) {
        return Err(GeoServerError::Conflict(format!(
            "Data source '{}' is a built-in data source",
            body.name
        )));
    }
    if let Some(store) = &state.store {
        match store.get_data_source(&body.name).await {
            Ok(Some(_)) => {
                return Err(GeoServerError::Conflict(format!(
                    "Data source '{}' already exists",
                    body.name
                )));
            },
            _ => {},
        }

        match store
            .create_data_source(
                &body.name,
                &body.data_source_type,
                body.workspace.clone(),
                body.enabled.unwrap_or(true),
                &body.connection,
            )
            .await
        {
            Ok(ds) => Ok(
                HttpResponse::Created().json(ApiResponse::success(serde_json::json!({
                    "name": ds.name,
                    "type": format!("{}", ds.data_source_type).to_lowercase(),
                    "workspace": ds.workspace,
                    "enabled": ds.enabled,
                    "connection": ds.connection,
                    "created": ds.created,
                    "modified": ds.modified,
                    "message": "Data source created",
                }))),
            ),
            Err(e) => {
                eprintln!("Failed to create data source: {}", e);
                Err(GeoServerError::InternalError(
                    "Failed to create data source".to_string(),
                ))
            },
        }
    } else {
        Err(GeoServerError::InternalError(
            "Database not available".to_string(),
        ))
    }
}

pub async fn update_data_source(
    req: HttpRequest,
    body: web::Json<UpdateDataSourceRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let name = req.match_info().get("name").unwrap_or("");

    if is_builtin_metadata(name) {
        return Err(GeoServerError::BadRequest(format!(
            "Data source '{}' is a built-in data source and cannot be modified",
            name
        )));
    }

    if let Some(store) = &state.store {
        match store
            .update_data_source(
                name,
                body.data_source_type.clone(),
                body.workspace.clone(),
                body.enabled,
                body.connection.clone(),
            )
            .await
        {
            Ok(_) => Ok(
                HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
                    "message": format!("Data source '{}' updated", name),
                }))),
            ),
            Err(e) => {
                eprintln!("Failed to update data source: {}", e);
                Err(GeoServerError::InternalError(
                    "Failed to update data source".to_string(),
                ))
            },
        }
    } else {
        Err(GeoServerError::InternalError(
            "Database not available".to_string(),
        ))
    }
}

pub async fn delete_data_source(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let name = req.match_info().get("name").unwrap_or("");

    if is_builtin_metadata(name) {
        return Err(GeoServerError::BadRequest(format!(
            "Data source '{}' is a built-in data source and cannot be deleted",
            name
        )));
    }

    if let Some(store) = &state.store {
        match store.delete_data_source(name).await {
            Ok(_) => Ok(
                HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
                    "message": format!("Data source '{}' deleted", name),
                }))),
            ),
            Err(e) => {
                eprintln!("Failed to delete data source: {}", e);
                Err(GeoServerError::InternalError(
                    "Failed to delete data source".to_string(),
                ))
            },
        }
    } else {
        Err(GeoServerError::InternalError(
            "Database not available".to_string(),
        ))
    }
}

pub async fn test_data_source_connection(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let name = req.match_info().get("name").unwrap_or("");

    // 内置 metadata 数据源: 测试元数据存储连接
    if is_builtin_metadata(name) {
        let mc = &state.config.metadata;
        if mc.kind == "postgres" {
            let pg = &mc.postgres;
            let ds = DataSource {
                name: name.to_string(),
                data_source_type: DataSourceType::Postgis,
                workspace: None,
                enabled: true,
                connection: Some(DataSourceConnection {
                    host: Some(pg.host.clone()),
                    port: Some(pg.port),
                    database: Some(pg.instance.clone()),
                    schema: Some(pg.schema.clone()),
                    username: Some(pg.user.clone()),
                    password: Some(pg.password.clone()),
                    ..Default::default()
                }),
                created: None,
                modified: None,
            };
            let result = test_postgis_connection(&ds).await;
            return Ok(HttpResponse::Ok().json(result));
        } else {
            eprintln!(
                "[test_data_source_connection] metadata 数据源连接测试仅支持 postgres 元数据存储"
            );
            return Err(GeoServerError::BadRequest(
                "Metadata data source connection test only supports postgres metadata store"
                    .to_string(),
            ));
        }
    }

    if let Some(store) = &state.store {
        match store.get_data_source(name).await {
            Ok(Some(ds)) => {
                let result = test_postgis_connection(&ds).await;
                Ok(HttpResponse::Ok().json(result))
            },
            Ok(None) => Err(GeoServerError::NotFound(format!(
                "Data source '{}' not found",
                name
            ))),
            Err(e) => {
                eprintln!("Failed to get data source: {}", e);
                Err(GeoServerError::InternalError(
                    "Failed to get data source".to_string(),
                ))
            },
        }
    } else {
        Err(GeoServerError::InternalError(
            "Database not available".to_string(),
        ))
    }
}

pub async fn test_connection(
    body: web::Json<CreateDataSourceRequest>,
    _state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let ds = DataSource {
        name: body.name.clone(),
        data_source_type: body.data_source_type.clone(),
        workspace: body.workspace.clone(),
        enabled: body.enabled.unwrap_or(true),
        connection: Some(body.connection.clone()),
        created: None,
        modified: None,
    };

    let result = test_postgis_connection(&ds).await;
    Ok(HttpResponse::Ok().json(result))
}

async fn test_postgis_connection(ds: &DataSource) -> serde_json::Value {
    let conn_info = match &ds.connection {
        Some(c) => c,
        None => {
            return serde_json::json!({
                "success": false,
                "message": "No connection configuration",
            })
        },
    };

    let pg_host = conn_info.host.as_deref().unwrap_or("localhost");
    let pg_port = conn_info.port.unwrap_or(5432);
    let pg_db = conn_info.database.as_deref().unwrap_or("geoserver");
    let pg_user = conn_info.username.as_deref().unwrap_or("postgres");

    match tokio_postgres::connect(
        &format!(
            "host={} port={} dbname={} user={} password={}",
            pg_host,
            pg_port,
            pg_db,
            pg_user,
            conn_info.password.as_deref().unwrap_or("")
        ),
        tokio_postgres::NoTls,
    )
    .await
    {
        Ok((client, connection)) => {
            tokio::spawn(connection);
            match client.query_one("SELECT 1", &[]).await {
                Ok(_) => serde_json::json!({
                    "success": true,
                    "message": "Connection successful",
                }),
                Err(e) => serde_json::json!({
                    "success": false,
                    "message": format!("Query failed: {}", e),
                }),
            }
        },
        Err(e) => serde_json::json!({
            "success": false,
            "message": format!("Connection failed: {}", e),
        }),
    }
}

pub async fn get_data_source_tables(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let start_total = Instant::now();
    let name = req.match_info().get("name").unwrap_or("");
    tracing::debug!("[get_data_source_tables] 开始处理, name={}", name);

    // 内置 metadata 数据源: 元数据为 postgres 时复用同一 PG 列出业务表 (postgis 语义);
    // sqlite 元数据不承载业务表, 返回空表列表
    if is_builtin_metadata(name) {
        if state.config.metadata.kind == "postgres" {
            let mc = &state.config.metadata;
            let conn = DataSourceConnection {
                host: Some(mc.postgres.host.clone()),
                port: Some(mc.postgres.port),
                database: Some(mc.postgres.instance.clone()),
                schema: Some(mc.postgres.schema.clone()),
                username: Some(mc.postgres.user.clone()),
                password: Some(mc.postgres.password.clone()),
                ..Default::default()
            };
            let pool = state.get_pg_pool(METADATA_DATA_SOURCE, &conn);
            let tables = list_postgis_tables_from_pool(&pool, &conn).await;
            return Ok(HttpResponse::Ok().json(ApiResponse::success(tables)));
        }
        return Ok(HttpResponse::Ok().json(ApiResponse::success(Vec::<String>::new())));
    }

    if let Some(store) = &state.store {
        let t1 = Instant::now();
        match store.get_data_source(name).await {
            Ok(Some(data_source)) => {
                let elapsed_get_ds = t1.elapsed();
                tracing::debug!(
                    "[get_data_source_tables] get_data_source 耗时: {:?}",
                    elapsed_get_ds
                );

                if data_source.data_source_type == DataSourceType::Geopackage {
                    // GeoPackage: 列出文件中的要素表 (local / s3)
                    let conn = data_source.connection.as_ref().ok_or_else(|| {
                        GeoServerError::BadRequest(
                            "GeoPackage data source has no connection".to_string(),
                        )
                    })?;
                    let materialized = crate::store::materialize_file(conn).await?.ok_or_else(
                        || {
                            GeoServerError::BadRequest(
                                "GeoPackage data source has no file path".to_string(),
                            )
                        },
                    )?;
                    let local_path = materialized.path.to_string_lossy().to_string();
                    let tables = crate::utils::geopackage::read_geopackage_layers(&local_path)
                        .map(|layers| {
                            layers
                                .iter()
                                .map(|l| l.table_name.clone())
                                .collect::<Vec<String>>()
                        })
                        .unwrap_or_default();
                    return Ok(HttpResponse::Ok().json(ApiResponse::success(tables)));
                }

                if data_source.data_source_type != DataSourceType::Postgis {
                    tracing::debug!("[get_data_source_tables] 非 PostGIS/GeoPackage 数据源, 跳过");
                    return Err(GeoServerError::BadRequest(
                        "Only PostGIS and GeoPackage data sources support table listing"
                            .to_string(),
                    ));
                }

                if let Some(conn_info) = &data_source.connection {
                    let t2 = Instant::now();
                    let pool = state.get_pg_pool(&data_source.name, conn_info);
                    let elapsed_get_pool = t2.elapsed();
                    tracing::debug!(
                        "[get_data_source_tables] get_pg_pool 耗时: {:?}",
                        elapsed_get_pool
                    );

                    let t3 = Instant::now();
                    let tables = list_postgis_tables_from_pool(&pool, conn_info).await;
                    let elapsed_query = t3.elapsed();
                    tracing::debug!("[get_data_source_tables] list_postgis_tables_from_pool 查询耗时: {:?}, 返回 {} 个表", elapsed_query, tables.len());

                    let total = start_total.elapsed();
                    tracing::debug!("[get_data_source_tables] 总耗时: {:?}", total);

                    Ok(HttpResponse::Ok().json(ApiResponse::success(tables)))
                } else {
                    tracing::debug!("[get_data_source_tables] 数据源无连接配置");
                    Err(GeoServerError::BadRequest(
                        "Data source has no connection configuration".to_string(),
                    ))
                }
            },
            Ok(None) => {
                tracing::debug!("[get_data_source_tables] 数据源 '{}' 未找到", name);
                Err(GeoServerError::NotFound(format!(
                    "Data source '{}' not found",
                    name
                )))
            },
            Err(e) => {
                tracing::debug!("[get_data_source_tables] 获取数据源失败: {}", e);
                eprintln!("Failed to get data source: {}", e);
                Err(GeoServerError::InternalError(
                    "Failed to get data source".to_string(),
                ))
            },
        }
    } else {
        tracing::debug!("[get_data_source_tables] 数据库不可用");
        Err(GeoServerError::InternalError(
            "Database not available".to_string(),
        ))
    }
}

async fn list_postgis_tables_from_pool(
    pool: &deadpool_postgres::Pool,
    conn: &DataSourceConnection,
) -> Vec<String> {
    let start = Instant::now();

    let t1 = Instant::now();
    let client = match pool.get().await {
        Ok(c) => c,
        Err(e) => {
            tracing::debug!(
                "[list_postgis_tables_from_pool] pool.get() 失败: {}, 耗时: {:?}",
                e,
                t1.elapsed()
            );
            eprintln!("Failed to get PG client from pool: {}", e);
            return vec![];
        },
    };
    tracing::debug!(
        "[list_postgis_tables_from_pool] pool.get() 耗时: {:?}",
        t1.elapsed()
    );

    let schema_val = conn
        .schema
        .as_deref()
        .map(|s| {
            if s.is_empty() || s == "public" {
                "public"
            } else {
                s
            }
        })
        .unwrap_or("public");
    let schema_filter = format!("AND n.nspname = '{}'", schema_val);

    let query = format!(
        "SELECT c.relname
         FROM pg_class c
         JOIN pg_namespace n ON n.oid = c.relnamespace
         WHERE c.relkind IN ('r', 'v')
         AND n.nspname NOT IN ('pg_catalog', 'information_schema')
         {}
         ORDER BY c.relname",
        schema_filter
    );
    tracing::debug!("[list_postgis_tables_from_pool] 查询SQL: {}", query);

    let t2 = Instant::now();
    match client.query(&query, &[]).await {
        Ok(rows) => {
            let elapsed_query = t2.elapsed();
            let tables: Vec<String> = rows.iter().map(|row| row.get::<_, String>(0)).collect();
            tracing::debug!(
                "[list_postgis_tables_from_pool] client.query 执行耗时: {:?}, 返回 {} 行",
                elapsed_query,
                tables.len()
            );
            tracing::debug!(
                "[list_postgis_tables_from_pool] 总耗时: {:?}",
                start.elapsed()
            );
            tables
        },
        Err(e) => {
            tracing::debug!(
                "[list_postgis_tables_from_pool] 查询失败: {}, 耗时: {:?}",
                e,
                t2.elapsed()
            );
            eprintln!("Failed to query tables: {}", e);
            vec![]
        },
    }
}

pub async fn get_layer_feature_type(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let layer_name = req.match_info().get("layer").unwrap_or("");

    if let Some(store) = &state.store {
        let layer = store
            .get_layer(layer_name)
            .await
            .map_err(|e| {
                eprintln!("Failed to get layer: {}", e);
                GeoServerError::InternalError("Failed to get layer".to_string())
            })?
            .ok_or_else(|| GeoServerError::NotFound(format!("Layer '{}' not found", layer_name)))?;

        let data_source = store
            .get_data_source(&layer.store)
            .await
            .map_err(|e| {
                eprintln!("Failed to get data source: {}", e);
                GeoServerError::InternalError("Failed to get data source".to_string())
            })?
            .ok_or_else(|| {
                GeoServerError::NotFound(format!("Data source '{}' not found", layer.store))
            })?;

        if data_source.data_source_type == DataSourceType::Postgis {
            let table_name = layer.native_name.as_ref().ok_or_else(|| {
                GeoServerError::BadRequest("Layer has no native table name configured".to_string())
            })?;

            if let Some(conn_info) = &data_source.connection {
                let pool = state.get_pg_pool(&data_source.name, conn_info);
                let columns = get_postgis_table_columns(&pool, conn_info, table_name).await;
                Ok(HttpResponse::Ok().json(ApiResponse::success(columns)))
            } else {
                Err(GeoServerError::BadRequest(
                    "Data source has no connection configuration".to_string(),
                ))
            }
        } else if data_source.data_source_type == DataSourceType::Geopackage {
            // GeoPackage: 从 .gpkg 文件的要素表读取列定义 (local / s3)
            let table_name = layer.native_name.as_ref().ok_or_else(|| {
                GeoServerError::BadRequest("Layer has no native table name configured".to_string())
            })?;
            let conn = data_source.connection.as_ref().ok_or_else(|| {
                GeoServerError::BadRequest(
                    "GeoPackage data source has no connection".to_string(),
                )
            })?;
            let materialized = crate::store::materialize_file(conn).await?.ok_or_else(|| {
                GeoServerError::BadRequest("GeoPackage data source has no file path".to_string())
            })?;
            let local_path = materialized.path.to_string_lossy().to_string();
            let columns = get_geopackage_table_columns(&local_path, table_name)
                .await
                .map_err(|e| {
                    GeoServerError::BadRequest(format!("Failed to read GeoPackage: {}", e))
                })?;
            Ok(HttpResponse::Ok().json(ApiResponse::success(columns)))
        } else {
            Err(GeoServerError::BadRequest(
                "Feature type information is only available for PostGIS and GeoPackage data sources".to_string()
            ))
        }
    } else {
        Err(GeoServerError::InternalError(
            "Database not available".to_string(),
        ))
    }
}

async fn get_postgis_table_columns(
    pool: &deadpool_postgres::Pool,
    conn: &DataSourceConnection,
    table_name: &str,
) -> Vec<serde_json::Value> {
    let client = match pool.get().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to get PG client from pool: {}", e);
            return vec![];
        },
    };

    let schema_val = conn
        .schema
        .as_deref()
        .map(|s| {
            if s.is_empty() || s == "public" {
                "public"
            } else {
                s
            }
        })
        .unwrap_or("public");
    let schema_filter = format!("'{}'", schema_val.replace('\'', "''"));

    let query = format!(
        "SELECT column_name, data_type, character_maximum_length, is_nullable
         FROM information_schema.columns
         WHERE table_schema = {} AND table_name = $1
         ORDER BY ordinal_position",
        schema_filter
    );

    match client.query(&query, &[&table_name]).await {
        Ok(rows) => rows
            .iter()
            .map(|row| {
                let col_name: String = row.get(0);
                let data_type: String = row.get(1);
                let max_length: Option<i32> = row.get(2);
                let is_nullable: String = row.get(3);
                serde_json::json!({
                    "name": col_name,
                    "type": data_type,
                    "length": max_length,
                    "nullable": is_nullable == "YES",
                })
            })
            .collect(),
        Err(e) => {
            eprintln!("Failed to query table columns: {}", e);
            vec![]
        },
    }
}

/// 读取 GeoPackage 要素表的列定义 (PRAGMA table_info)
async fn get_geopackage_table_columns(
    file_path: &str,
    table_name: &str,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = rusqlite::Connection::open(file_path)
        .map_err(|e| format!("无法打开 GeoPackage '{}': {}", file_path, e))?;
    let qn = table_name.replace('"', "\"\"");
    let pragma = format!("PRAGMA table_info(\"{}\")", qn);
    let mut stmt = conn
        .prepare(&pragma)
        .map_err(|e| format!("查询 GeoPackage 表 '{}' 列失败: {}", table_name, e))?;
    let rows = stmt
        .query_map([], |row| {
            let name: String = row.get(1)?;
            let ty: String = row.get(2).unwrap_or_default();
            Ok((name, ty))
        })
        .map_err(|e| format!("查询结果错误: {}", e))?;

    let mut cols = Vec::new();
    for row in rows {
        if let Ok((name, ty)) = row {
            cols.push(serde_json::json!({
                "name": name,
                "type": ty,
                "length": serde_json::Value::Null,
                "nullable": true,
            }));
        }
    }
    Ok(cols)
}
