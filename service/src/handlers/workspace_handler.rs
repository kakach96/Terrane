use super::rest_handler::ApiResponse;
use crate::error::TerraneError;
use crate::state::AppState;
use actix_web::{web, HttpRequest, HttpResponse};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CreateWorkspaceRequest {
    pub name: String,
    pub title: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateWorkspaceRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub enabled: Option<bool>,
}

pub async fn list_workspaces(state: web::Data<AppState>) -> Result<HttpResponse, TerraneError> {
    if let Some(store) = &state.store {
        match store.get_all_workspaces().await {
            Ok(ws) => {
                let result: Vec<_> = ws
                    .iter()
                    .map(|w| {
                        serde_json::json!({
                            "name": w.name,
                            "title": w.title,
                            "description": w.description,
                            "enabled": w.enabled,
                            "layerCount": w.layer_count,
                            "created": w.created,
                            "modified": w.modified,
                        })
                    })
                    .collect();
                return Ok(HttpResponse::Ok().json(ApiResponse::success(result)));
            },
            Err(e) => {
                eprintln!("Failed to list workspaces: {}", e);
                return Err(TerraneError::InternalError(
                    "Failed to list workspaces".to_string(),
                ));
            },
        }
    }

    let workspaces: Vec<serde_json::Value> = vec![];
    Ok(HttpResponse::Ok().json(ApiResponse::success(workspaces)))
}

pub async fn get_workspace(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, TerraneError> {
    let name = req.match_info().get("workspace").unwrap_or("");

    if let Some(store) = &state.store {
        match store.get_workspace(name).await {
            Ok(Some(workspace)) => {
                let response = serde_json::json!({
                    "name": workspace.name,
                    "title": workspace.title,
                    "description": workspace.description,
                    "enabled": workspace.enabled,
                    "layerCount": workspace.layer_count,
                    "created": workspace.created,
                    "modified": workspace.modified,
                });
                Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
            },
            Ok(None) => Err(TerraneError::NotFound(format!(
                "Workspace '{}' not found",
                name
            ))),
            Err(e) => {
                eprintln!("Failed to get workspace: {}", e);
                Err(TerraneError::InternalError(
                    "Failed to get workspace".to_string(),
                ))
            },
        }
    } else {
        Err(TerraneError::NotFound(format!(
            "Workspace '{}' not found",
            name
        )))
    }
}

pub async fn create_workspace(
    body: web::Json<CreateWorkspaceRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, TerraneError> {
    if let Some(store) = &state.store {
        match store.create_workspace(&body).await {
            Ok(workspace) => {
                // 自动创建对应的命名空间
                let ns_uri = format!("http://geoserver.org/{}", workspace.name);
                let _ = store
                    .create_namespace(&workspace.name, &ns_uri, Some(&workspace.name), false)
                    .await;

                Ok(
                    HttpResponse::Created().json(ApiResponse::success(serde_json::json!({
                        "name": workspace.name,
                        "title": workspace.title,
                        "description": workspace.description,
                        "enabled": workspace.enabled,
                        "layerCount": workspace.layer_count,
                        "created": workspace.created,
                        "modified": workspace.modified,
                    }))),
                )
            },
            Err(e) => {
                eprintln!("Failed to create workspace: {}", e);
                Err(TerraneError::InternalError(
                    "Failed to create workspace".to_string(),
                ))
            },
        }
    } else {
        Err(TerraneError::InternalError(
            "Database not available".to_string(),
        ))
    }
}

pub async fn update_workspace(
    req: HttpRequest,
    body: web::Json<UpdateWorkspaceRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, TerraneError> {
    let name = req.match_info().get("workspace").unwrap_or("");

    if let Some(store) = &state.store {
        match store
            .update_workspace(
                name,
                body.title.clone(),
                body.description.clone(),
                body.enabled,
            )
            .await
        {
            Ok(_) => Ok(
                HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
                    "message": format!("Workspace '{}' updated", name),
                }))),
            ),
            Err(e) => {
                eprintln!("Failed to update workspace: {}", e);
                Err(TerraneError::InternalError(
                    "Failed to update workspace".to_string(),
                ))
            },
        }
    } else {
        Err(TerraneError::InternalError(
            "Database not available".to_string(),
        ))
    }
}

pub async fn delete_workspace(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, TerraneError> {
    let name = req.match_info().get("workspace").unwrap_or("");

    if let Some(store) = &state.store {
        match store.delete_workspace(name).await {
            Ok(_) => Ok(
                HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
                    "message": format!("Workspace '{}' deleted", name),
                }))),
            ),
            Err(e) => {
                eprintln!("Failed to delete workspace: {}", e);
                Err(TerraneError::InternalError(
                    "Failed to delete workspace".to_string(),
                ))
            },
        }
    } else {
        Err(TerraneError::InternalError(
            "Database not available".to_string(),
        ))
    }
}

// ---------------------------------------------------------------------------
// 工作空间维度端点 (GeoServer 标准路径):
// /workspaces/{ws}/layers | /datastores | /coveragestores
// ---------------------------------------------------------------------------

/// 校验工作空间存在 (供维度端点复用)。
async fn ensure_workspace(
    store: Option<&std::sync::Arc<dyn crate::store::Store>>,
    ws: &str,
) -> Result<(), TerraneError> {
    if let Some(store) = store {
        match store.get_workspace(ws).await {
            Ok(Some(_)) => Ok(()),
            Ok(None) => Err(TerraneError::NotFound(format!(
                "Workspace '{}' not found",
                ws
            ))),
            Err(e) => {
                eprintln!("Failed to get workspace: {}", e);
                Err(TerraneError::InternalError(
                    "Failed to get workspace".to_string(),
                ))
            },
        }
    } else {
        Err(TerraneError::InternalError(
            "Database not available".to_string(),
        ))
    }
}

/// 列出工作空间下的图层 (GET /workspaces/{workspace}/layers)。
pub async fn list_workspace_layers(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, TerraneError> {
    let ws = req.match_info().get("workspace").unwrap_or("");
    ensure_workspace(state.store.as_ref(), ws).await?;

    if let Some(store) = &state.store {
        match store.get_all_layers().await {
            Ok(layers) => {
                let result: Vec<_> = layers
                    .iter()
                    .filter(|l| l.workspace == ws)
                    .map(|l| {
                        serde_json::json!({
                            "name": l.name,
                            "title": l.title,
                            "workspace": l.workspace,
                            "store": l.store,
                            "native_name": l.native_name,
                            "srs": l.srs,
                            "bounds": {
                                "minx": l.minx,
                                "miny": l.miny,
                                "maxx": l.maxx,
                                "maxy": l.maxy,
                            },
                            "enabled": l.enabled,
                            "cache_store": l.cache_store,
                        })
                    })
                    .collect();
                return Ok(HttpResponse::Ok().json(ApiResponse::success(result)));
            },
            Err(e) => {
                eprintln!("Failed to list layers: {}", e);
                return Err(TerraneError::InternalError(
                    "Failed to list layers".to_string(),
                ));
            },
        }
    }

    // 无 store: 从内存目录过滤
    let layers = state.list_layers().await;
    let result: Vec<_> = layers
        .iter()
        .filter(|l| l.workspace == ws)
        .map(|l| {
            serde_json::json!({
                "name": l.name,
                "title": l.title,
                "workspace": l.workspace,
                "store": l.store,
                "native_name": l.native_name,
                "srs": l.srs.to_epsg(),
                "bounds": {
                    "minx": l.native_bounds.bounds.minx,
                    "miny": l.native_bounds.bounds.miny,
                    "maxx": l.native_bounds.bounds.maxx,
                    "maxy": l.native_bounds.bounds.maxy,
                },
                "styles": l.styles,
                "enabled": l.enabled,
                "cache_store": l.cache_store,
            })
        })
        .collect();
    Ok(HttpResponse::Ok().json(ApiResponse::success(result)))
}

/// 列出工作空间下的矢量数据存储 (GET /workspaces/{workspace}/datastores)。
pub async fn list_workspace_datastores(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, TerraneError> {
    let ws = req.match_info().get("workspace").unwrap_or("");
    ensure_workspace(state.store.as_ref(), ws).await?;
    list_workspace_stores_filtered(&state, ws, "DataStore").await
}

/// 列出工作空间下的栅格数据存储 (GET /workspaces/{workspace}/coveragestores)。
pub async fn list_workspace_coveragestores(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, TerraneError> {
    let ws = req.match_info().get("workspace").unwrap_or("");
    ensure_workspace(state.store.as_ref(), ws).await?;
    list_workspace_stores_filtered(&state, ws, "CoverageStore").await
}

/// 按 store 类型过滤工作空间下的数据源。
async fn list_workspace_stores_filtered(
    state: &web::Data<AppState>,
    ws: &str,
    store_type: &str,
) -> Result<HttpResponse, TerraneError> {
    if let Some(store) = &state.store {
        match store.get_all_data_sources().await {
            Ok(ds_list) => {
                let result: Vec<_> = ds_list
                    .iter()
                    .filter(|ds| {
                        ds.workspace.as_deref() == Some(ws)
                            && crate::handlers::store_handler::ds_type_to_store_type(
                                &ds.data_source_type,
                            ) == store_type
                    })
                    .map(|ds| {
                        serde_json::json!({
                            "name": ds.name,
                            "type": crate::handlers::store_handler::ds_type_to_store_type(
                                &ds.data_source_type,
                            ),
                            "workspace": ds.workspace,
                            "enabled": ds.enabled,
                            "connection": ds.connection,
                            "created": ds.created,
                            "modified": ds.modified,
                        })
                    })
                    .collect();
                return Ok(HttpResponse::Ok().json(ApiResponse::success(result)));
            },
            Err(e) => {
                eprintln!("Failed to list data sources: {}", e);
                return Err(TerraneError::InternalError(
                    "Failed to list data sources".to_string(),
                ));
            },
        }
    }
    let empty: Vec<serde_json::Value> = vec![];
    Ok(HttpResponse::Ok().json(ApiResponse::success(empty)))
}
