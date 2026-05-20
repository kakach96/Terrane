use actix_web::{HttpResponse, web};
use serde::{Deserialize, Serialize};
use crate::state::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub message: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        ApiResponse {
            success: true,
            data: Some(data),
            message: None,
        }
    }

    pub fn error(message: String) -> ApiResponse<()> {
        ApiResponse {
            success: false,
            data: None,
            message: Some(message),
        }
    }
}

pub async fn health_check() -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
        "status": "healthy",
        "service": "rust-geoserver",
    })))
}

pub async fn get_server_status(state: web::Data<AppState>) -> Result<HttpResponse, crate::error::GeoServerError> {
    let uptime = state.get_uptime();
    let request_count = state.request_count.load(std::sync::atomic::Ordering::Relaxed);
    let error_count = state.error_count.load(std::sync::atomic::Ordering::Relaxed);

    let memory_info = match sysinfo::System::new_all() {
        mut s => {
            s.refresh_memory();
            let total = s.total_memory();
            let used = s.used_memory();
            let percent = if total > 0 { (used as f64 / total as f64) * 100.0 } else { 0.0 };
            serde_json::json!({
                "used": used,
                "total": total,
                "percent": percent,
            })
        }
    };

    let layer_count = state.layers.read().await.len();
    let enabled_count = state.layers.read().await.iter().filter(|l| l.enabled).count();
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

    let response = serde_json::json!({
        "uptime": uptime,
        "memory": memory_info,
        "cpu": 0.0,
        "requests": request_count,
        "errors": error_count,
        "layerCount": layer_count,
        "enabledLayers": enabled_count,
        "workspaceCount": workspace_count,
    });

    Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
}
