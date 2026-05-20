use actix_web::{HttpRequest, HttpResponse, web};
use serde::Deserialize;
use crate::state::AppState;
use crate::models::{DataSource, DataSourceType, DataSourceConnection};
use crate::error::GeoServerError;
use std::time::Instant;
use super::rest_handler::ApiResponse;

#[derive(Debug, Deserialize)]
pub struct CreateDataSourceRequest {
    pub name: String,
    #[serde(rename = "type")]
    pub data_source_type: DataSourceType,
    pub workspace: Option<String>,
    pub enabled: Option<bool>,
    pub connection: DataSourceConnection,
}

#[derive(Debug, Deserialize)]
pub struct DataSourceConnectionRequest {
    pub name: String,
    #[serde(rename = "type")]
    pub data_source_type: DataSourceType,
    pub connection: DataSourceConnection,
}

#[derive(Debug, Deserialize)]
pub struct UpdateDataSourceRequest {
    #[serde(rename = "type")]
    pub data_source_type: Option<DataSourceType>,
    pub workspace: Option<String>,
    pub enabled: Option<bool>,
    pub connection: Option<DataSourceConnection>,
}

pub async fn list_data_sources(state: web::Data<AppState>) -> Result<HttpResponse, GeoServerError> {
    if let Some(store) = &state.store {
        match store.get_all_data_sources().await {
            Ok(data_sources) => {
                let result: Vec<_> = data_sources.iter()
                    .map(|ds| serde_json::json!({
                        "name": ds.name,
                        "type": format!("{}", ds.data_source_type).to_lowercase(),
                        "workspace": ds.workspace,
                        "enabled": ds.enabled,
                        "connection": ds.connection,
                        "created": ds.created,
                        "modified": ds.modified,
                    }))
                    .collect();
                Ok(HttpResponse::Ok().json(ApiResponse::success(result)))
            }
            Err(e) => {
                eprintln!("Failed to list data sources: {}", e);
                Err(GeoServerError::InternalError("Failed to list data sources".to_string()))
            }
        }
    } else {
        Err(GeoServerError::InternalError("Database not available".to_string()))
    }
}

pub async fn get_data_source(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let name = req.match_info().get("name").unwrap_or("");

    if let Some(store) = &state.store {
        match store.get_data_source(name).await {
            Ok(Some(ds)) => {
                Ok(HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
                    "name": ds.name,
                    "type": format!("{}", ds.data_source_type).to_lowercase(),
                    "workspace": ds.workspace,
                    "enabled": ds.enabled,
                    "connection": ds.connection,
                    "created": ds.created,
                    "modified": ds.modified,
                }))))
            }
            Ok(None) => Err(GeoServerError::NotFound(format!("Data source '{}' not found", name))),
            Err(e) => {
                eprintln!("Failed to get data source: {}", e);
                Err(GeoServerError::InternalError("Failed to get data source".to_string()))
            }
        }
    } else {
        Err(GeoServerError::InternalError("Database not available".to_string()))
    }
}

pub async fn create_data_source(
    body: web::Json<CreateDataSourceRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    if let Some(store) = &state.store {
        match store.get_data_source(&body.name).await {
            Ok(Some(_)) => {
                return Err(GeoServerError::Conflict(format!("Data source '{}' already exists", body.name)));
            }
            _ => {}
        }

        match store.create_data_source(
            &body.name,
            &body.data_source_type,
            body.workspace.clone(),
            body.enabled.unwrap_or(true),
            &body.connection,
        ).await {
            Ok(ds) => {
                Ok(HttpResponse::Created().json(ApiResponse::success(serde_json::json!({
                    "name": ds.name,
                    "type": format!("{}", ds.data_source_type).to_lowercase(),
                    "workspace": ds.workspace,
                    "enabled": ds.enabled,
                    "connection": ds.connection,
                    "created": ds.created,
                    "modified": ds.modified,
                    "message": "Data source created",
                }))))
            }
            Err(e) => {
                eprintln!("Failed to create data source: {}", e);
                Err(GeoServerError::InternalError("Failed to create data source".to_string()))
            }
        }
    } else {
        Err(GeoServerError::InternalError("Database not available".to_string()))
    }
}

pub async fn update_data_source(
    req: HttpRequest,
    body: web::Json<UpdateDataSourceRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let name = req.match_info().get("name").unwrap_or("");

    if let Some(store) = &state.store {
        match store.update_data_source(
            name,
            body.data_source_type.clone(),
            body.workspace.clone(),
            body.enabled,
            body.connection.clone(),
        ).await {
            Ok(_) => {
                Ok(HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
                    "message": format!("Data source '{}' updated", name),
                }))))
            }
            Err(e) => {
                eprintln!("Failed to update data source: {}", e);
                Err(GeoServerError::InternalError("Failed to update data source".to_string()))
            }
        }
    } else {
        Err(GeoServerError::InternalError("Database not available".to_string()))
    }
}

pub async fn delete_data_source(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let name = req.match_info().get("name").unwrap_or("");

    if let Some(store) = &state.store {
        match store.delete_data_source(name).await {
            Ok(_) => {
                Ok(HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
                    "message": format!("Data source '{}' deleted", name),
                }))))
            }
            Err(e) => {
                eprintln!("Failed to delete data source: {}", e);
                Err(GeoServerError::InternalError("Failed to delete data source".to_string()))
            }
        }
    } else {
        Err(GeoServerError::InternalError("Database not available".to_string()))
    }
}

pub async fn test_data_source_connection(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let name = req.match_info().get("name").unwrap_or("");

    if let Some(store) = &state.store {
        match store.get_data_source(name).await {
            Ok(Some(ds)) => {
                let result = test_postgis_connection(&ds).await;
                Ok(HttpResponse::Ok().json(result))
            }
            Ok(None) => Err(GeoServerError::NotFound(format!("Data source '{}' not found", name))),
            Err(e) => {
                eprintln!("Failed to get data source: {}", e);
                Err(GeoServerError::InternalError("Failed to get data source".to_string()))
            }
        }
    } else {
        Err(GeoServerError::InternalError("Database not available".to_string()))
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
        None => return serde_json::json!({
            "success": false,
            "message": "No connection configuration",
        }),
    };

    match tokio_postgres::connect(
        &format!("host={} port={} dbname={} user={} password={}",
            conn_info.host, conn_info.port, conn_info.database,
            conn_info.username,
            conn_info.password.as_deref().unwrap_or("")),
        tokio_postgres::NoTls,
    ).await {
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
        }
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

    if let Some(store) = &state.store {
        let t1 = Instant::now();
        match store.get_data_source(name).await {
            Ok(Some(data_source)) => {
                let elapsed_get_ds = t1.elapsed();
                tracing::debug!("[get_data_source_tables] get_data_source 耗时: {:?}", elapsed_get_ds);

                if data_source.data_source_type != DataSourceType::Postgis {
                    tracing::debug!("[get_data_source_tables] 非 PostGIS 数据源, 跳过");
                    return Err(GeoServerError::BadRequest("Only PostGIS data sources support table listing".to_string()));
                }

                if let Some(conn_info) = &data_source.connection {
                    let t2 = Instant::now();
                    let pool = state.get_pg_pool(&data_source.name, conn_info);
                    let elapsed_get_pool = t2.elapsed();
                    tracing::debug!("[get_data_source_tables] get_pg_pool 耗时: {:?}", elapsed_get_pool);

                    let t3 = Instant::now();
                    let tables = list_postgis_tables_from_pool(&pool, conn_info).await;
                    let elapsed_query = t3.elapsed();
                    tracing::debug!("[get_data_source_tables] list_postgis_tables_from_pool 查询耗时: {:?}, 返回 {} 个表", elapsed_query, tables.len());

                    let total = start_total.elapsed();
                    tracing::debug!("[get_data_source_tables] 总耗时: {:?}", total);

                    Ok(HttpResponse::Ok().json(ApiResponse::success(tables)))
                } else {
                    tracing::debug!("[get_data_source_tables] 数据源无连接配置");
                    Err(GeoServerError::BadRequest("Data source has no connection configuration".to_string()))
                }
            }
            Ok(None) => {
                tracing::debug!("[get_data_source_tables] 数据源 '{}' 未找到", name);
                Err(GeoServerError::NotFound(format!("Data source '{}' not found", name)))
            }
            Err(e) => {
                tracing::debug!("[get_data_source_tables] 获取数据源失败: {}", e);
                eprintln!("Failed to get data source: {}", e);
                Err(GeoServerError::InternalError("Failed to get data source".to_string()))
            }
        }
    } else {
        tracing::debug!("[get_data_source_tables] 数据库不可用");
        Err(GeoServerError::InternalError("Database not available".to_string()))
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
            tracing::debug!("[list_postgis_tables_from_pool] pool.get() 失败: {}, 耗时: {:?}", e, t1.elapsed());
            eprintln!("Failed to get PG client from pool: {}", e);
            return vec![];
        }
    };
    tracing::debug!("[list_postgis_tables_from_pool] pool.get() 耗时: {:?}", t1.elapsed());

    let schema_filter = if conn.schema.is_empty() || conn.schema == "public" {
        "AND n.nspname = 'public'".to_string()
    } else {
        format!("AND n.nspname = '{}'", conn.schema)
    };

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
            let tables: Vec<String> = rows.iter()
                .map(|row| row.get::<_, String>(0))
                .collect();
            tracing::debug!("[list_postgis_tables_from_pool] client.query 执行耗时: {:?}, 返回 {} 行", elapsed_query, tables.len());
            tracing::debug!("[list_postgis_tables_from_pool] 总耗时: {:?}", start.elapsed());
            tables
        }
        Err(e) => {
            tracing::debug!("[list_postgis_tables_from_pool] 查询失败: {}, 耗时: {:?}", e, t2.elapsed());
            eprintln!("Failed to query tables: {}", e);
            vec![]
        }
    }
}

pub async fn get_layer_feature_type(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let layer_name = req.match_info().get("layer").unwrap_or("");

    if let Some(store) = &state.store {
        let layer = store.get_layer(layer_name).await
            .map_err(|e| {
                eprintln!("Failed to get layer: {}", e);
                GeoServerError::InternalError("Failed to get layer".to_string())
            })?
            .ok_or_else(|| GeoServerError::NotFound(format!("Layer '{}' not found", layer_name)))?;

        let data_source = store.get_data_source(&layer.store).await
            .map_err(|e| {
                eprintln!("Failed to get data source: {}", e);
                GeoServerError::InternalError("Failed to get data source".to_string())
            })?
            .ok_or_else(|| GeoServerError::NotFound(format!("Data source '{}' not found", layer.store)))?;

        if data_source.data_source_type != DataSourceType::Postgis {
            return Err(GeoServerError::BadRequest(
                "Feature type information is only available for PostGIS data sources".to_string()
            ));
        }

        let table_name = layer.native_name
            .as_ref()
            .ok_or_else(|| GeoServerError::BadRequest(
                "Layer has no native table name configured".to_string()
            ))?;

        if let Some(conn_info) = &data_source.connection {
            let pool = state.get_pg_pool(&data_source.name, conn_info);
            let columns = get_postgis_table_columns(&pool, conn_info, table_name).await;
            Ok(HttpResponse::Ok().json(ApiResponse::success(columns)))
        } else {
            Err(GeoServerError::BadRequest("Data source has no connection configuration".to_string()))
        }
    } else {
        Err(GeoServerError::InternalError("Database not available".to_string()))
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
        }
    };

    let schema_filter = if conn.schema.is_empty() || conn.schema == "public" {
        "'public'".to_string()
    } else {
        format!("'{}'", conn.schema.replace('\'', "''"))
    };

    let query = format!(
        "SELECT column_name, data_type, character_maximum_length, is_nullable
         FROM information_schema.columns
         WHERE table_schema = {} AND table_name = $1
         ORDER BY ordinal_position",
        schema_filter
    );

    match client.query(&query, &[&table_name]).await {
        Ok(rows) => {
            rows.iter().map(|row| {
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
            }).collect()
        }
        Err(e) => {
            eprintln!("Failed to query table columns: {}", e);
            vec![]
        }
    }
}
