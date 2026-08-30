//! `/about` endpoints (GeoServer-compatible system information).
//!
//! - `GET /about/version` — build / version metadata.
//! - `GET /about/system-status` — runtime status (uptime, memory, counters).

use super::rest_handler::ApiResponse;
use crate::error::TerraneError;
use crate::state::AppState;
use actix_web::{web, HttpResponse};

/// GET /about/version — Terrane 版本与构建元数据。
pub async fn about_version() -> Result<HttpResponse, TerraneError> {
    Ok(
        HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
            "name": env!("CARGO_PKG_NAME"),
            "version": env!("CARGO_PKG_VERSION"),
            "edition": "2021",
            "rustc": option_env!("RUSTC_VERSION").unwrap_or("unknown"),
        }))),
    )
}

/// GET /about/system-status — 运行时状态 (与 /server/status 同源数据)。
pub async fn about_system_status(state: web::Data<AppState>) -> Result<HttpResponse, TerraneError> {
    let uptime = state.get_uptime();
    let request_count = state
        .request_count
        .load(std::sync::atomic::Ordering::Relaxed);
    let error_count = state.error_count.load(std::sync::atomic::Ordering::Relaxed);

    let mut s = sysinfo::System::new_all();
    s.refresh_memory();
    let memory_info = {
        let total = s.total_memory();
        let used = s.used_memory();
        let percent = if total > 0 {
            (used as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        serde_json::json!({ "used": used, "total": total, "percent": percent })
    };

    let layer_count = state.layers.read().await.len();
    let workspace_count = {
        if let Some(store) = &state.store {
            match store.get_all_workspaces().await {
                Ok(w) => w.len(),
                Err(_) => 0,
            }
        } else {
            0
        }
    };

    Ok(
        HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
            "uptime": uptime,
            "memory": memory_info,
            "requests": request_count,
            "errors": error_count,
            "layerCount": layer_count,
            "workspaceCount": workspace_count,
            "dataDir": state.config.data_dir.to_string_lossy(),
        }))),
    )
}
