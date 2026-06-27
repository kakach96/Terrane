use actix_web::{HttpResponse, web, HttpRequest};
use serde::Deserialize;
use crate::state::AppState;
use crate::error::GeoServerError;
use super::rest_handler::ApiResponse;

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

pub async fn list_workspaces(state: web::Data<AppState>) -> Result<HttpResponse, GeoServerError> {
    if let Some(store) = &state.store {
        match store.get_all_workspaces().await {
            Ok(ws) => {
                let result: Vec<_> = ws.iter().map(|w| serde_json::json!({
                    "name": w.name,
                    "title": w.title,
                    "description": w.description,
                    "enabled": w.enabled,
                    "layerCount": w.layer_count,
                    "created": w.created,
                    "modified": w.modified,
                })).collect();
                return Ok(HttpResponse::Ok().json(ApiResponse::success(result)));
            }
            Err(e) => {
                eprintln!("Failed to list workspaces: {}", e);
                return Err(GeoServerError::InternalError("Failed to list workspaces".to_string()));
            }
        }
    }

    let workspaces: Vec<serde_json::Value> = vec![];
    Ok(HttpResponse::Ok().json(ApiResponse::success(workspaces)))
}

pub async fn get_workspace(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
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
            }
            Ok(None) => Err(GeoServerError::NotFound(format!("Workspace '{}' not found", name))),
            Err(e) => {
                eprintln!("Failed to get workspace: {}", e);
                Err(GeoServerError::InternalError("Failed to get workspace".to_string()))
            }
        }
    } else {
        Err(GeoServerError::NotFound(format!("Workspace '{}' not found", name)))
    }
}

pub async fn create_workspace(
    body: web::Json<CreateWorkspaceRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    if let Some(store) = &state.store {
        match store.create_workspace(&body).await {
            Ok(workspace) => {
                // 自动创建对应的命名空间
                let ns_uri = format!("http://geoserver.org/{}", workspace.name);
                let _ = store.create_namespace(&workspace.name, &ns_uri, Some(&workspace.name), false).await;

                Ok(HttpResponse::Created().json(ApiResponse::success(serde_json::json!({
                    "name": workspace.name,
                    "title": workspace.title,
                    "description": workspace.description,
                    "enabled": workspace.enabled,
                    "layerCount": workspace.layer_count,
                    "created": workspace.created,
                    "modified": workspace.modified,
                }))))
            }
            Err(e) => {
                eprintln!("Failed to create workspace: {}", e);
                Err(GeoServerError::InternalError("Failed to create workspace".to_string()))
            }
        }
    } else {
        Err(GeoServerError::InternalError("Database not available".to_string()))
    }
}

pub async fn update_workspace(
    req: HttpRequest,
    body: web::Json<UpdateWorkspaceRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let name = req.match_info().get("workspace").unwrap_or("");

    if let Some(store) = &state.store {
        match store.update_workspace(name, body.title.clone(), body.description.clone(), body.enabled).await {
            Ok(_) => {
                Ok(HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
                    "message": format!("Workspace '{}' updated", name),
                }))))
            }
            Err(e) => {
                eprintln!("Failed to update workspace: {}", e);
                Err(GeoServerError::InternalError("Failed to update workspace".to_string()))
            }
        }
    } else {
        Err(GeoServerError::InternalError("Database not available".to_string()))
    }
}

pub async fn delete_workspace(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let name = req.match_info().get("workspace").unwrap_or("");

    if let Some(store) = &state.store {
        match store.delete_workspace(name).await {
            Ok(_) => {
                Ok(HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
                    "message": format!("Workspace '{}' deleted", name),
                }))))
            }
            Err(e) => {
                eprintln!("Failed to delete workspace: {}", e);
                Err(GeoServerError::InternalError("Failed to delete workspace".to_string()))
            }
        }
    } else {
        Err(GeoServerError::InternalError("Database not available".to_string()))
    }
}
