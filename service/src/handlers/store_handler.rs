use super::rest_handler::ApiResponse;
use crate::error::TerraneError;
use crate::models::{
    CreateDataSourceRequest, DataSourceType, UpdateDataSourceRequest, METADATA_DATA_SOURCE,
};
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

pub(crate) fn ds_type_to_store_type(ds_type: &DataSourceType) -> &'static str {
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
pub async fn list_stores(state: web::Data<AppState>) -> Result<HttpResponse, TerraneError> {
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
                Err(TerraneError::InternalError(
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
) -> Result<HttpResponse, TerraneError> {
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
                Err(TerraneError::InternalError(
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
) -> Result<HttpResponse, TerraneError> {
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
            Ok(None) => Err(TerraneError::NotFound(format!(
                "Store '{}' not found",
                name
            ))),
            Err(e) => {
                eprintln!("Failed to get store: {}", e);
                Err(TerraneError::InternalError(
                    "Failed to get store".to_string(),
                ))
            },
        }
    } else {
        Err(TerraneError::NotFound(format!(
            "Store '{}' not found",
            name
        )))
    }
}

/// 获取特定工作空间下的特定存储
pub async fn get_workspace_store(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, TerraneError> {
    let ws_name = req.match_info().get("workspace").unwrap_or("");
    let name = req.match_info().get("name").unwrap_or("");

    if let Some(store) = &state.store {
        match store.get_data_source(name).await {
            Ok(Some(ds)) => {
                if ds.workspace.as_deref() != Some(ws_name) {
                    return Err(TerraneError::NotFound(format!(
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
            Ok(None) => Err(TerraneError::NotFound(format!(
                "Store '{}' not found",
                name
            ))),
            Err(e) => {
                eprintln!("Failed to get store: {}", e);
                Err(TerraneError::InternalError(
                    "Failed to get store".to_string(),
                ))
            },
        }
    } else {
        Err(TerraneError::NotFound(format!(
            "Store '{}' not found",
            name
        )))
    }
}

// ---------------------------------------------------------------------------
// Store CRUD — store 是数据源的 GeoServer 兼容别名视图, 与 /data-sources
// 共享同一元数据 (data_sources 表)。创建/更新/删除复用数据源存储逻辑。
// ---------------------------------------------------------------------------

/// 创建存储 (POST /stores)。请求体与 /data-sources 一致。
pub async fn create_store(
    body: web::Json<CreateDataSourceRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, TerraneError> {
    if body.name == METADATA_DATA_SOURCE {
        return Err(TerraneError::Conflict(format!(
            "Store '{}' is a built-in store",
            body.name
        )));
    }
    if let Some(store) = &state.store {
        if let Ok(Some(_)) = store.get_data_source(&body.name).await {
            return Err(TerraneError::Conflict(format!(
                "Store '{}' already exists",
                body.name
            )));
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
                    "type": ds_type_to_store_type(&ds.data_source_type),
                    "workspace": ds.workspace,
                    "enabled": ds.enabled,
                    "connection": ds.connection,
                    "created": ds.created,
                    "modified": ds.modified,
                    "message": "Store created",
                }))),
            ),
            Err(e) => {
                eprintln!("Failed to create store: {}", e);
                Err(TerraneError::InternalError(
                    "Failed to create store".to_string(),
                ))
            },
        }
    } else {
        Err(TerraneError::InternalError(
            "Database not available".to_string(),
        ))
    }
}

/// 按工作空间创建存储 (POST /workspaces/{workspace}/stores)。
/// 工作空间来自路径, 请求体中的 workspace 被忽略/校验一致。
pub async fn create_workspace_store(
    req: HttpRequest,
    body: web::Json<CreateDataSourceRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, TerraneError> {
    let ws_name = req.match_info().get("workspace").unwrap_or("").to_string();
    if let Some(store) = &state.store {
        // 工作空间必须存在
        match store.get_workspace(&ws_name).await {
            Ok(Some(_)) => {},
            Ok(None) => {
                return Err(TerraneError::NotFound(format!(
                    "Workspace '{}' not found",
                    ws_name
                )))
            },
            Err(e) => {
                eprintln!("Failed to get workspace: {}", e);
                return Err(TerraneError::InternalError(
                    "Failed to get workspace".to_string(),
                ));
            },
        }
        if body.name == METADATA_DATA_SOURCE {
            return Err(TerraneError::Conflict(format!(
                "Store '{}' is a built-in store",
                body.name
            )));
        }
        if let Ok(Some(_)) = store.get_data_source(&body.name).await {
            return Err(TerraneError::Conflict(format!(
                "Store '{}' already exists",
                body.name
            )));
        }
        match store
            .create_data_source(
                &body.name,
                &body.data_source_type,
                Some(ws_name.clone()),
                body.enabled.unwrap_or(true),
                &body.connection,
            )
            .await
        {
            Ok(ds) => Ok(
                HttpResponse::Created().json(ApiResponse::success(serde_json::json!({
                    "name": ds.name,
                    "type": ds_type_to_store_type(&ds.data_source_type),
                    "workspace": ds.workspace,
                    "enabled": ds.enabled,
                    "connection": ds.connection,
                    "created": ds.created,
                    "modified": ds.modified,
                    "message": "Store created",
                }))),
            ),
            Err(e) => {
                eprintln!("Failed to create store in workspace '{}': {}", ws_name, e);
                Err(TerraneError::InternalError(
                    "Failed to create store".to_string(),
                ))
            },
        }
    } else {
        Err(TerraneError::InternalError(
            "Database not available".to_string(),
        ))
    }
}

/// 更新存储 (PUT /stores/{name})。
pub async fn update_store(
    req: HttpRequest,
    body: web::Json<UpdateDataSourceRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, TerraneError> {
    let name = req.match_info().get("name").unwrap_or("");
    if name == METADATA_DATA_SOURCE {
        return Err(TerraneError::BadRequest(format!(
            "Store '{}' is a built-in store and cannot be modified",
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
                    "message": format!("Store '{}' updated", name),
                }))),
            ),
            Err(e) => {
                eprintln!("Failed to update store: {}", e);
                Err(TerraneError::InternalError(
                    "Failed to update store".to_string(),
                ))
            },
        }
    } else {
        Err(TerraneError::InternalError(
            "Database not available".to_string(),
        ))
    }
}

/// 删除存储 (DELETE /stores/{name})。行为与 /data-sources/{name} 一致。
pub async fn delete_store(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, TerraneError> {
    let name = req.match_info().get("name").unwrap_or("");
    if name == METADATA_DATA_SOURCE {
        return Err(TerraneError::BadRequest(format!(
            "Store '{}' is a built-in store and cannot be deleted",
            name
        )));
    }
    if let Some(store) = &state.store {
        match store.delete_data_source(name).await {
            Ok(_) => Ok(
                HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
                    "message": format!("Store '{}' deleted", name),
                }))),
            ),
            Err(e) => {
                eprintln!("Failed to delete store: {}", e);
                Err(TerraneError::InternalError(
                    "Failed to delete store".to_string(),
                ))
            },
        }
    } else {
        Err(TerraneError::InternalError(
            "Database not available".to_string(),
        ))
    }
}
