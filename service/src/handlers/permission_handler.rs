//! 权限管理处理器

use super::rest_handler::ApiResponse;
use crate::error::GeoServerError;
use crate::handlers::auth_handler::require_auth;
use crate::models::permission::{AccessMode, Effect, Permission};
use crate::state::AppState;
use actix_web::{web, HttpRequest, HttpResponse};
use serde::Deserialize;
use tracing::info;

#[derive(Debug, Deserialize)]
pub struct CreatePermissionRequest {
    pub username: Option<String>,
    pub role: Option<String>,
    pub resource_type: String,
    pub resource_name: Option<String>,
    pub access_mode: Option<String>,
    pub effect: Option<String>,
    pub priority: Option<i32>,
}

/// 列出所有权限
pub async fn list_permissions(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    require_auth(&req)?;

    if let Some(store) = &state.store {
        match store.get_permissions().await {
            Ok(perms) => {
                let result: Vec<_> = perms
                    .iter()
                    .map(|p| {
                        serde_json::json!({
                            "id": p.id,
                            "username": p.username,
                            "role": p.role,
                            "resourceType": p.resource_type,
                            "resourceName": p.resource_name,
                            "accessMode": p.access_mode.to_string(),
                            "effect": p.effect.to_string(),
                            "priority": p.priority,
                        })
                    })
                    .collect();
                Ok(HttpResponse::Ok().json(ApiResponse::success(result)))
            },
            Err(e) => Err(GeoServerError::InternalError(format!(
                "查询权限失败: {}",
                e
            ))),
        }
    } else {
        Err(GeoServerError::InternalError("数据库不可用".to_string()))
    }
}

/// 创建权限
pub async fn create_permission(
    body: web::Json<CreatePermissionRequest>,
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    require_auth(&req)?;

    let perm = Permission {
        id: None,
        username: body.username.clone().unwrap_or_else(|| "*".to_string()),
        role: body.role.clone().unwrap_or_else(|| "*".to_string()),
        resource_type: body.resource_type.clone(),
        resource_name: body
            .resource_name
            .clone()
            .unwrap_or_else(|| "*".to_string()),
        access_mode: match body.access_mode.as_deref() {
            Some("write") => AccessMode::Write,
            Some("admin") => AccessMode::Admin,
            _ => AccessMode::Read,
        },
        effect: match body.effect.as_deref() {
            Some("deny") => Effect::Deny,
            _ => Effect::Allow,
        },
        priority: body.priority.unwrap_or(0),
    };

    if let Some(store) = &state.store {
        match store.create_permission(&perm).await {
            Ok(id) => {
                info!(
                    "[Permission] 创建权限: id={}, user={}, resource={}/{}",
                    id, perm.username, perm.resource_type, perm.resource_name
                );
                Ok(
                    HttpResponse::Created().json(ApiResponse::success(serde_json::json!({
                        "id": id,
                        "message": "权限创建成功",
                    }))),
                )
            },
            Err(e) => Err(GeoServerError::InternalError(format!(
                "创建权限失败: {}",
                e
            ))),
        }
    } else {
        Err(GeoServerError::InternalError("数据库不可用".to_string()))
    }
}

/// 删除权限
pub async fn delete_permission(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    require_auth(&req)?;
    let id: i64 = req
        .match_info()
        .get("id")
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| GeoServerError::BadRequest("无效的权限 ID".to_string()))?;

    if let Some(store) = &state.store {
        store
            .delete_permission(id)
            .await
            .map_err(|e| GeoServerError::InternalError(format!("删除失败: {}", e)))?;
        Ok(
            HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
                "message": "权限已删除",
            }))),
        )
    } else {
        Err(GeoServerError::InternalError("数据库不可用".to_string()))
    }
}

/// 检查权限
pub async fn check_permission_handler(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let claims = require_auth(&req)?;
    let resource_type = req.match_info().get("type").unwrap_or("");
    let resource_name = req.match_info().get("name").unwrap_or("");
    let required_mode = req
        .query_string()
        .split('&')
        .find_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            if parts.next()? == "mode" {
                parts.next()
            } else {
                None
            }
        })
        .unwrap_or("read");

    if let Some(store) = &state.store {
        match store
            .check_permission(
                &claims.sub,
                &claims.role,
                resource_type,
                resource_name,
                required_mode,
            )
            .await
        {
            Ok(allowed) => Ok(
                HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
                    "allowed": allowed,
                    "username": claims.sub,
                    "resource": format!("{}/{}", resource_type, resource_name),
                    "mode": required_mode,
                }))),
            ),
            Err(e) => Err(GeoServerError::InternalError(format!(
                "权限检查失败: {}",
                e
            ))),
        }
    } else {
        Err(GeoServerError::InternalError("数据库不可用".to_string()))
    }
}
