//! 备份/恢复处理器

use super::rest_handler::ApiResponse;
use crate::backup::{export_backup, import_backup, GeoServerBackup};
use crate::error::GeoServerError;
use crate::handlers::auth_handler::require_auth;
use crate::state::AppState;
use actix_web::{web, HttpRequest, HttpResponse};
use tracing::info;

/// 导出备份
pub async fn handle_export(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    require_auth(&req)?;

    let backup = export_backup(&state)
        .await
        .map_err(|e| GeoServerError::InternalError(e))?;

    info!("[Backup] 导出完成: {} 个实体", backup.workspaces.len());
    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .insert_header((
            "Content-Disposition",
            "attachment; filename=geoserver-backup.json",
        ))
        .json(ApiResponse::success(backup)))
}

/// 导入备份
pub async fn handle_import(
    req: HttpRequest,
    body: web::Json<GeoServerBackup>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    require_auth(&req)?;

    let backup = body.into_inner();
    let report = import_backup(&state, &backup)
        .await
        .map_err(|e| GeoServerError::InternalError(e))?;

    info!("[Backup] 导入完成: {}", report.summary());
    Ok(
        HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
            "message": "恢复完成",
            "report": report,
        }))),
    )
}
