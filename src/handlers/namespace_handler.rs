use actix_web::{HttpResponse, web, HttpRequest};
use serde::Deserialize;
use crate::state::AppState;
use crate::error::GeoServerError;
use super::rest_handler::ApiResponse;

#[derive(Debug, Deserialize)]
pub struct CreateNamespaceRequest {
    pub prefix: String,
    pub uri: String,
    pub workspace: Option<String>,
    pub isolated: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateNamespaceRequest {
    pub uri: Option<String>,
    pub isolated: Option<bool>,
    pub workspace: Option<String>,
}

pub async fn list_namespaces(state: web::Data<AppState>) -> Result<HttpResponse, GeoServerError> {
    if let Some(store) = &state.store {
        match store.get_all_namespaces().await {
            Ok(ns_list) => {
                let result: Vec<_> = ns_list.iter().map(|ns| serde_json::json!({
                    "prefix": ns.prefix,
                    "uri": ns.uri,
                    "isolated": ns.isolated,
                    "workspace": ns.workspace,
                    "created": ns.created,
                    "modified": ns.modified,
                })).collect();
                Ok(HttpResponse::Ok().json(ApiResponse::success(result)))
            }
            Err(e) => {
                eprintln!("Failed to list namespaces: {}", e);
                Err(GeoServerError::InternalError("Failed to list namespaces".to_string()))
            }
        }
    } else {
        let result: Vec<serde_json::Value> = vec![];
        Ok(HttpResponse::Ok().json(ApiResponse::success(result)))
    }
}

pub async fn get_namespace(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let prefix = req.match_info().get("prefix").unwrap_or("");

    if let Some(store) = &state.store {
        match store.get_namespace(prefix).await {
            Ok(Some(ns)) => {
                let response = serde_json::json!({
                    "prefix": ns.prefix,
                    "uri": ns.uri,
                    "isolated": ns.isolated,
                    "workspace": ns.workspace,
                    "created": ns.created,
                    "modified": ns.modified,
                });
                Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
            }
            Ok(None) => Err(GeoServerError::NotFound(format!("Namespace '{}' not found", prefix))),
            Err(e) => {
                eprintln!("Failed to get namespace: {}", e);
                Err(GeoServerError::InternalError("Failed to get namespace".to_string()))
            }
        }
    } else {
        Err(GeoServerError::NotFound(format!("Namespace '{}' not found", prefix)))
    }
}

pub async fn create_namespace(
    body: web::Json<CreateNamespaceRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    if let Some(store) = &state.store {
        // 检查是否已存在
        if let Ok(Some(_)) = store.get_namespace(&body.prefix).await {
            return Err(GeoServerError::Conflict(format!("Namespace '{}' already exists", body.prefix)));
        }

        let isolated = body.isolated.unwrap_or(false);
        let workspace_ref = body.workspace.as_deref();

        match store.create_namespace(&body.prefix, &body.uri, workspace_ref, isolated).await {
            Ok(ns) => {
                Ok(HttpResponse::Created().json(ApiResponse::success(serde_json::json!({
                    "prefix": ns.prefix,
                    "uri": ns.uri,
                    "isolated": ns.isolated,
                    "workspace": ns.workspace,
                    "created": ns.created,
                    "modified": ns.modified,
                }))))
            }
            Err(e) => {
                eprintln!("Failed to create namespace: {}", e);
                Err(GeoServerError::InternalError("Failed to create namespace".to_string()))
            }
        }
    } else {
        Err(GeoServerError::InternalError("Database not available".to_string()))
    }
}

pub async fn update_namespace(
    req: HttpRequest,
    body: web::Json<UpdateNamespaceRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let prefix = req.match_info().get("prefix").unwrap_or("");

    if let Some(store) = &state.store {
        match store.update_namespace(prefix, body.uri.clone(), body.isolated, body.workspace.clone()).await {
            Ok(_) => {
                Ok(HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
                    "message": format!("Namespace '{}' updated", prefix),
                }))))
            }
            Err(e) => {
                eprintln!("Failed to update namespace: {}", e);
                Err(GeoServerError::InternalError("Failed to update namespace".to_string()))
            }
        }
    } else {
        Err(GeoServerError::InternalError("Database not available".to_string()))
    }
}

pub async fn delete_namespace(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let prefix = req.match_info().get("prefix").unwrap_or("");

    if let Some(store) = &state.store {
        match store.delete_namespace(prefix).await {
            Ok(_) => {
                Ok(HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
                    "message": format!("Namespace '{}' deleted", prefix),
                }))))
            }
            Err(e) => {
                eprintln!("Failed to delete namespace: {}", e);
                Err(GeoServerError::InternalError("Failed to delete namespace".to_string()))
            }
        }
    } else {
        Err(GeoServerError::InternalError("Database not available".to_string()))
    }
}
