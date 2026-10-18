//! Bulk import handler — `POST /terrane/importer/uploads`.
//!
//! Accepts one multipart file (a `.zip` archive or a single recognized data
//! file), unpacks it and publishes every recognized member as a data source +
//! layer. Returns a per-file report (created / skipped / failed).

use crate::error::TerraneError;
use crate::state::AppState;
use crate::utils::importer::{ensure_workspace, run_import, ImportReport};
use actix_multipart::Multipart;
use actix_web::{web, HttpRequest, HttpResponse};
use futures_util::StreamExt;
use tracing::info;

/// Read all multipart bytes + the first field's filename.
async fn read_upload(mut payload: Multipart) -> Result<(Vec<u8>, String), TerraneError> {
    let mut data: Vec<u8> = Vec::new();
    let mut filename = "upload".to_string();
    while let Some(Ok(mut field)) = payload.next().await {
        if let Some(f) = field.content_disposition().and_then(|cd| cd.get_filename()) {
            filename = f.replace(['\\', '/'], "_").replace("..", "_").to_string();
        }
        while let Some(Ok(chunk)) = field.next().await {
            data.extend_from_slice(&chunk);
        }
    }
    if data.is_empty() {
        return Err(TerraneError::BadRequest("未接收到上传文件".to_string()));
    }
    Ok((data, filename))
}

/// Extract a query parameter value from the raw query string.
fn query_param(req: &HttpRequest, key: &str) -> Option<String> {
    req.query_string().split('&').find_map(|pair| {
        let mut parts = pair.splitn(2, '=');
        if parts.next()? == key {
            parts.next().map(|v| v.to_string())
        } else {
            None
        }
    })
}

/// POST /importer/uploads — bulk import and publish.
pub async fn upload_import(
    req: HttpRequest,
    payload: Multipart,
    state: web::Data<AppState>,
) -> Result<HttpResponse, TerraneError> {
    let (data, filename) = read_upload(payload).await?;
    let workspace = query_param(&req, "workspace")
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "default".to_string());

    if let Some(ref store) = state.store {
        ensure_workspace(store, &workspace).await?;
    }

    let report: ImportReport = run_import(&state, &data, &filename, &workspace).await?;
    info!(
        "[Importer] 完成 '{}': created={} skipped={} failed={}",
        filename, report.created, report.skipped, report.failed
    );
    Ok(
        HttpResponse::Ok().json(crate::handlers::ApiResponse::success(
            serde_json::to_value(report).unwrap_or_default(),
        )),
    )
}
