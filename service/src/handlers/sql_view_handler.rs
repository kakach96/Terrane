//! SQL 视图处理器
//!
//! SQL View 允许将参数化 SQL 查询发布为虚拟图层。
//! 支持 CRUD 操作以及查询预览。

use super::rest_handler::ApiResponse;
use crate::error::GeoServerError;
use crate::models::sql_view::{SqlView, SqlViewParameter};
use crate::state::AppState;
use actix_web::{web, HttpRequest, HttpResponse};
use serde::Deserialize;
use tracing::info;

#[derive(Debug, Deserialize)]
pub struct CreateSqlViewRequest {
    pub name: String,
    pub sql: String,
    pub workspace: String,
    pub store: String,
    pub geometry_column: Option<String>,
    pub geometry_type: Option<String>,
    pub crs: Option<String>,
    pub parameters: Option<Vec<SqlViewParameter>>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSqlViewRequest {
    pub sql: Option<String>,
    pub geometry_column: Option<String>,
    pub geometry_type: Option<String>,
    pub crs: Option<String>,
    pub parameters: Option<Vec<SqlViewParameter>>,
    pub description: Option<String>,
}

/// SQL 视图预览请求
#[derive(Debug, Deserialize)]
pub struct PreviewSqlViewRequest {
    pub sql: String,
    pub workspace: String,
    pub store: String,
    pub parameters: Option<Vec<SqlViewParameter>>,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

pub async fn list_sql_views(state: web::Data<AppState>) -> Result<HttpResponse, GeoServerError> {
    if let Some(store) = &state.store {
        match store.get_all_sql_views().await {
            Ok(views) => {
                let result: Vec<_> = views
                    .iter()
                    .map(|v| {
                        serde_json::json!({
                            "name": v.name,
                            "sql": v.sql,
                            "workspace": v.workspace,
                            "store": v.store,
                            "geometryColumn": v.geometry_column,
                            "geometryType": v.geometry_type,
                            "crs": v.crs,
                            "parameters": v.parameters,
                            "description": v.description,
                            "created": v.created,
                            "modified": v.modified,
                        })
                    })
                    .collect();
                Ok(HttpResponse::Ok().json(ApiResponse::success(result)))
            },
            Err(e) => {
                eprintln!("Failed to list SQL views: {}", e);
                Err(GeoServerError::InternalError(
                    "Failed to list SQL views".to_string(),
                ))
            },
        }
    } else {
        let empty: Vec<serde_json::Value> = vec![];
        Ok(HttpResponse::Ok().json(ApiResponse::success(empty)))
    }
}

pub async fn get_sql_view(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let name = req.match_info().get("name").unwrap_or("");

    if let Some(store) = &state.store {
        match store.get_sql_view(name).await {
            Ok(Some(view)) => {
                let response = serde_json::json!({
                    "name": view.name,
                    "sql": view.sql,
                    "workspace": view.workspace,
                    "store": view.store,
                    "geometryColumn": view.geometry_column,
                    "geometryType": view.geometry_type,
                    "crs": view.crs,
                    "parameters": view.parameters,
                    "description": view.description,
                    "created": view.created,
                    "modified": view.modified,
                });
                Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
            },
            Ok(None) => Err(GeoServerError::NotFound(format!(
                "SQL view '{}' not found",
                name
            ))),
            Err(e) => {
                eprintln!("Failed to get SQL view: {}", e);
                Err(GeoServerError::InternalError(
                    "Failed to get SQL view".to_string(),
                ))
            },
        }
    } else {
        Err(GeoServerError::NotFound(format!(
            "SQL view '{}' not found",
            name
        )))
    }
}

pub async fn create_sql_view(
    body: web::Json<CreateSqlViewRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    if let Some(store) = &state.store {
        // 检查是否已存在
        if let Ok(Some(_)) = store.get_sql_view(&body.name).await {
            return Err(GeoServerError::Conflict(format!(
                "SQL view '{}' already exists",
                body.name
            )));
        }

        let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let view = SqlView {
            name: body.name.clone(),
            sql: body.sql.clone(),
            workspace: body.workspace.clone(),
            store: body.store.clone(),
            geometry_column: body
                .geometry_column
                .clone()
                .unwrap_or_else(|| "geom".to_string()),
            geometry_type: body
                .geometry_type
                .clone()
                .unwrap_or_else(|| "Geometry".to_string()),
            crs: body.crs.clone().unwrap_or_else(|| "EPSG:4326".to_string()),
            parameters: body.parameters.clone().unwrap_or_default(),
            description: body.description.clone(),
            created: now.clone(),
            modified: now,
        };

        match store.create_sql_view(&view).await {
            Ok(_) => {
                // 同时创建一个虚拟图层
                info!(
                    "[SQL View] Creating virtual layer '{}' from SQL view",
                    view.name
                );
                let layer = crate::models::Layer::new(
                    view.name.clone(),
                    view.description
                        .clone()
                        .unwrap_or_else(|| view.name.clone()),
                    view.workspace.clone(),
                    view.store.clone(),
                    crate::models::CoordinateReferenceSystem::from_epsg(&view.crs),
                );
                state.layers.write().await.push(layer);

                Ok(
                    HttpResponse::Created().json(ApiResponse::success(serde_json::json!({
                        "name": view.name,
                        "sql": view.sql,
                        "workspace": view.workspace,
                        "store": view.store,
                        "message": "SQL view created and virtual layer published",
                    }))),
                )
            },
            Err(e) => {
                eprintln!("Failed to create SQL view: {}", e);
                Err(GeoServerError::InternalError(
                    "Failed to create SQL view".to_string(),
                ))
            },
        }
    } else {
        Err(GeoServerError::InternalError(
            "Database not available".to_string(),
        ))
    }
}

pub async fn update_sql_view(
    req: HttpRequest,
    body: web::Json<UpdateSqlViewRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let name = req.match_info().get("name").unwrap_or("");

    if let Some(store) = &state.store {
        match store
            .update_sql_view(
                name,
                body.sql.clone(),
                body.geometry_column.clone(),
                body.geometry_type.clone(),
                body.crs.clone(),
                body.parameters.clone(),
                body.description.clone(),
            )
            .await
        {
            Ok(_) => Ok(
                HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
                    "message": format!("SQL view '{}' updated", name),
                }))),
            ),
            Err(e) => {
                eprintln!("Failed to update SQL view: {}", e);
                Err(GeoServerError::InternalError(
                    "Failed to update SQL view".to_string(),
                ))
            },
        }
    } else {
        Err(GeoServerError::InternalError(
            "Database not available".to_string(),
        ))
    }
}

pub async fn delete_sql_view(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let name = req.match_info().get("name").unwrap_or("");

    if let Some(store) = &state.store {
        match store.delete_sql_view(name).await {
            Ok(_) => {
                // 移除对应的虚拟图层
                state.layers.write().await.retain(|l| l.name != name);
                Ok(
                    HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
                        "message": format!("SQL view '{}' deleted", name),
                    }))),
                )
            },
            Err(e) => {
                eprintln!("Failed to delete SQL view: {}", e);
                Err(GeoServerError::InternalError(
                    "Failed to delete SQL view".to_string(),
                ))
            },
        }
    } else {
        Err(GeoServerError::InternalError(
            "Database not available".to_string(),
        ))
    }
}

/// 预览 SQL 视图（执行查询并返回前 N 条结果）
pub async fn preview_sql_view(
    body: web::Json<PreviewSqlViewRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let data_source = {
        let store_lock = &state.store;
        if let Some(ref store) = store_lock {
            store
                .get_data_source(&body.store)
                .await
                .map_err(|e| GeoServerError::InternalError(format!("DB error: {}", e)))?
        } else {
            None
        }
    };

    let ds = data_source.ok_or_else(|| {
        GeoServerError::NotFound(format!("Data source '{}' not found", body.store))
    })?;

    if ds.data_source_type != crate::models::DataSourceType::Postgis {
        return Err(GeoServerError::BadRequest(
            "SQL View 仅支持 PostGIS 数据源".to_string(),
        ));
    }

    let conn = ds
        .connection
        .as_ref()
        .ok_or_else(|| GeoServerError::BadRequest("数据源缺少连接配置".to_string()))?;

    // 替换参数
    let mut sql = body.sql.clone();
    if let Some(ref params) = body.parameters {
        for param in params {
            sql = sql.replace(&format!("%{}%", param.name), &param.default_value);
        }
    }

    // 构建 PostGIS 连接并执行查询
    let pool = state.get_pg_pool(&body.store, conn);
    let client = pool
        .get()
        .await
        .map_err(|e| GeoServerError::ServiceError(format!("数据库连接失败: {}", e)))?;

    // 限制结果行数，仅返回预览
    let limited_sql = format!("SELECT * FROM ({}) AS _sv LIMIT 100", sql);

    let rows = client
        .query(&limited_sql, &[])
        .await
        .map_err(|e| GeoServerError::ServiceError(format!("SQL 执行失败: {}", e)))?;

    // 解析列信息（从第一行获取）
    let mut columns_info: Vec<serde_json::Value> = Vec::new();

    // 构建结果行（简化版，仅返回 JSON 字符串值）
    let mut features = Vec::new();
    let col_count = if rows.is_empty() { 0 } else { rows[0].len() };

    if !rows.is_empty() {
        let names = client
            .prepare(&format!("SELECT * FROM ({}) AS _sv LIMIT 0", body.sql))
            .await
            .map(|stmt| {
                stmt.columns()
                    .iter()
                    .map(|c| {
                        serde_json::json!({
                            "name": c.name(),
                            "type": format!("{:?}", c.type_()),
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        columns_info = names;
    }

    for row in &rows {
        let mut props = serde_json::Map::new();
        for i in 0..col_count {
            let col_name = format!("col_{}", i);
            let value: serde_json::Value = match row.try_get::<_, String>(i) {
                Ok(v) => serde_json::Value::String(v),
                Err(_) => serde_json::Value::Null,
            };
            props.insert(col_name, value);
        }
        features.push(props);
    }

    Ok(
        HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
            "columns": columns_info,
            "rows": features,
            "total": features.len(),
        }))),
    )
}
