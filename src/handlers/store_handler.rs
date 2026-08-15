use super::rest_handler::ApiResponse;
use crate::error::GeoServerError;
use crate::models::DataSourceType;
use crate::state::AppState;
use actix_web::{web, HttpRequest, HttpResponse};
use serde::Deserialize;

/// 存储类型：矢量存储 vs 栅格存储
#[derive(Debug, Deserialize)]
pub enum StoreType {
    #[serde(rename = "DataStore")]
    DataStore,
    #[serde(rename = "CoverageStore")]
    CoverageStore,
}

impl std::fmt::Display for StoreType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreType::DataStore => write!(f, "DataStore"),
            StoreType::CoverageStore => write!(f, "CoverageStore"),
        }
    }
}

fn ds_type_to_store_type(ds_type: &DataSourceType) -> &'static str {
    match ds_type {
        DataSourceType::Postgis
        | DataSourceType::Mysql
        | DataSourceType::Mongo
        | DataSourceType::Shapefile
        | DataSourceType::Geopackage
        | DataSourceType::GeoJson => "DataStore",
        DataSourceType::Geotiff
        | DataSourceType::WorldImage
        | DataSourceType::ArcGrid
        | DataSourceType::ImageMosaic
        | DataSourceType::ImagePyramid => "CoverageStore",
        DataSourceType::CascadedWms => "CascadedStore",
        DataSourceType::Redis => "RedisCacheStore",
        DataSourceType::Metadata => "DataStore",
    }
}

fn store_type_to_ds_type(store_type: &StoreType) -> DataSourceType {
    match store_type {
        StoreType::DataStore => DataSourceType::Postgis, // 默认，实际创建时可能需要更精确
        StoreType::CoverageStore => DataSourceType::Geotiff,
    }
}

/// 列出所有存储（按工作空间过滤可选）
pub async fn list_stores(state: web::Data<AppState>) -> Result<HttpResponse, GeoServerError> {
    if let Some(store) = &state.store {
        match store.get_all_data_sources().await {
            Ok(ds_list) => {
                let result: Vec<_> = ds_list
                    .iter()
                    .map(|ds| {
                        let store_type = ds_type_to_store_type(&ds.data_source_type);
                        serde_json::json!({
                            "name": ds.name,
                            "type": store_type,
                            "workspace": ds.workspace,
                            "enabled": ds.enabled,
                            "connection": ds.connection,
                            "created": ds.created,
                            "modified": ds.modified,
                        })
                    })
                    .collect();
                Ok(HttpResponse::Ok().json(ApiResponse::success(result)))
            },
            Err(e) => {
                eprintln!("Failed to list stores: {}", e);
                Err(GeoServerError::InternalError(
                    "Failed to list stores".to_string(),
                ))
            },
        }
    } else {
        let empty: Vec<serde_json::Value> = vec![];
        Ok(HttpResponse::Ok().json(ApiResponse::success(empty)))
    }
}

/// 列出工作空间下的存储
pub async fn list_workspace_stores(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let ws_name = req.match_info().get("workspace").unwrap_or("");

    if let Some(store) = &state.store {
        match store.get_all_data_sources().await {
            Ok(ds_list) => {
                let result: Vec<_> = ds_list
                    .iter()
                    .filter(|ds| ds.workspace.as_deref() == Some(ws_name))
                    .map(|ds| {
                        let store_type = ds_type_to_store_type(&ds.data_source_type);
                        serde_json::json!({
                            "name": ds.name,
                            "type": store_type,
                            "workspace": ds.workspace,
                            "enabled": ds.enabled,
                            "connection": ds.connection,
                            "created": ds.created,
                            "modified": ds.modified,
                        })
                    })
                    .collect();
                Ok(HttpResponse::Ok().json(ApiResponse::success(result)))
            },
            Err(e) => {
                eprintln!("Failed to list stores for workspace '{}': {}", ws_name, e);
                Err(GeoServerError::InternalError(
                    "Failed to list stores".to_string(),
                ))
            },
        }
    } else {
        let empty: Vec<serde_json::Value> = vec![];
        Ok(HttpResponse::Ok().json(ApiResponse::success(empty)))
    }
}

/// 获取单个存储详情
pub async fn get_store(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let name = req.match_info().get("name").unwrap_or("");

    if let Some(store) = &state.store {
        match store.get_data_source(name).await {
            Ok(Some(ds)) => {
                let store_type = ds_type_to_store_type(&ds.data_source_type);
                let response = serde_json::json!({
                    "name": ds.name,
                    "type": store_type,
                    "workspace": ds.workspace,
                    "enabled": ds.enabled,
                    "connection": ds.connection,
                    "created": ds.created,
                    "modified": ds.modified,
                });
                Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
            },
            Ok(None) => Err(GeoServerError::NotFound(format!(
                "Store '{}' not found",
                name
            ))),
            Err(e) => {
                eprintln!("Failed to get store: {}", e);
                Err(GeoServerError::InternalError(
                    "Failed to get store".to_string(),
                ))
            },
        }
    } else {
        Err(GeoServerError::NotFound(format!(
            "Store '{}' not found",
            name
        )))
    }
}

/// 获取特定工作空间下的特定存储
pub async fn get_workspace_store(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let ws_name = req.match_info().get("workspace").unwrap_or("");
    let name = req.match_info().get("name").unwrap_or("");

    if let Some(store) = &state.store {
        match store.get_data_source(name).await {
            Ok(Some(ds)) => {
                if ds.workspace.as_deref() != Some(ws_name) {
                    return Err(GeoServerError::NotFound(format!(
                        "Store '{}' not found in workspace '{}'",
                        name, ws_name
                    )));
                }
                let store_type = ds_type_to_store_type(&ds.data_source_type);
                let response = serde_json::json!({
                    "name": ds.name,
                    "type": store_type,
                    "workspace": ds.workspace,
                    "enabled": ds.enabled,
                    "connection": ds.connection,
                    "created": ds.created,
                    "modified": ds.modified,
                });
                Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
            },
            Ok(None) => Err(GeoServerError::NotFound(format!(
                "Store '{}' not found",
                name
            ))),
            Err(e) => {
                eprintln!("Failed to get store: {}", e);
                Err(GeoServerError::InternalError(
                    "Failed to get store".to_string(),
                ))
            },
        }
    } else {
        Err(GeoServerError::NotFound(format!(
            "Store '{}' not found",
            name
        )))
    }
}
