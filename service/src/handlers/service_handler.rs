//! OGC service settings handlers — `/services/{service}/settings`.
//!
//! Reads/writes the per-service title / abstract / keywords, persisted in the
//! metadata store (`service_settings` table) and mirrored in
//! `AppState.service_settings`. Writes require a valid admin token.

use super::rest_handler::ApiResponse;
use crate::error::GeoServerError;
use crate::handlers::auth_handler::require_auth;
use crate::models::ServiceSettings;
use crate::state::AppState;
use actix_web::{web, HttpRequest, HttpResponse};

/// 支持的 OGC 服务名 (settings 端点白名单)。
fn normalize_service(name: &str) -> Option<String> {
    match name.to_lowercase().as_str() {
        "wms" | "wfs" | "wcs" | "wmts" | "wps" | "csw" => Some(name.to_lowercase()),
        _ => None,
    }
}

/// GET /services/{service}/settings — 返回服务设置 (未设置时返回空设置)。
pub async fn get_service_settings(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let raw = req.match_info().get("service").unwrap_or("");
    let service = normalize_service(raw)
        .ok_or_else(|| GeoServerError::BadRequest(format!("Unknown service '{}'", raw)))?;

    let map = state.service_settings.read().await;
    let settings = map.get(&service).cloned().unwrap_or_default();

    Ok(
        HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
            "service": service,
            "title": settings.title,
            "abstract": settings.abstract_text,
            "keywords": settings.keywords,
        }))),
    )
}

/// PUT /services/{service}/settings — 保存服务设置 (仅 admin)。
pub async fn update_service_settings(
    req: HttpRequest,
    body: web::Json<ServiceSettings>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    require_auth(&req)?;
    let raw = req.match_info().get("service").unwrap_or("");
    let service = normalize_service(raw)
        .ok_or_else(|| GeoServerError::BadRequest(format!("Unknown service '{}'", raw)))?;
    let settings = body.into_inner();

    // 持久化到元数据存储
    if let Some(store) = &state.store {
        store
            .save_service_settings(&service, &settings)
            .await
            .map_err(|e| GeoServerError::InternalError(format!("保存服务设置失败: {}", e)))?;
    }

    // 更新内存缓存 (读路径即刻生效)
    {
        let mut map = state.service_settings.write().await;
        map.insert(service.clone(), settings.clone());
    }

    Ok(
        HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
            "service": service,
            "title": settings.title,
            "abstract": settings.abstract_text,
            "keywords": settings.keywords,
            "message": "Service settings updated",
        }))),
    )
}
