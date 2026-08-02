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

/// Liveness 探针 — 进程存活即返回 200, 不依赖任何外部资源。
/// 用于 K8s livenessProbe / 容器 HEALTHCHECK 判断是否需要重启实例。
pub async fn health_live() -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
        "status": "alive",
        "service": "rust-geoserver",
    })))
}

/// Readiness 探针 — 检查元数据存储 / 业务存储等关键依赖是否就绪。
/// 就绪返回 200, 未就绪返回 503。用于 K8s readinessProbe 与滚动发布流量摘除。
pub async fn health_ready(state: web::Data<AppState>) -> HttpResponse {
    let mut checks: Vec<serde_json::Value> = Vec::new();
    let mut ready = true;

    // 1. 元数据存储 (SQLite / PostgreSQL) — 必须初始化且可查询
    if let Some(ref store) = state.store {
        match store.get_all_workspaces().await {
            Ok(_) => checks.push(serde_json::json!({"name": "metadata_store", "status": "ok"})),
            Err(e) => {
                ready = false;
                checks.push(serde_json::json!({
                    "name": "metadata_store",
                    "status": "error",
                    "detail": format!("query failed: {}", e)
                }));
            }
        }
    } else {
        ready = false;
        checks.push(serde_json::json!({
            "name": "metadata_store",
            "status": "error",
            "detail": "store not initialized"
        }));
    }

    // 2. 业务数据存储 (local / metadata / postgres) — 未配置时视为就绪
    if let Some(ref bs) = state.business_store {
        match bs.list_tables().await {
            Ok(_) => checks.push(serde_json::json!({"name": "business_store", "status": "ok"})),
            Err(e) => {
                ready = false;
                checks.push(serde_json::json!({
                    "name": "business_store",
                    "status": "error",
                    "detail": format!("query failed: {}", e)
                }));
            }
        }
    } else {
        checks.push(serde_json::json!({"name": "business_store", "status": "ok", "detail": "not configured"}));
    }

    // 3. 瓦片缓存目录 (可选) — 已配置但不可用时不阻塞就绪, 仅记录
    if let Some(ref cache) = state.tile_cache {
        checks.push(serde_json::json!({
            "name": "tile_cache",
            "status": "ok",
            "cache_dir": cache.config.cache_dir.to_string_lossy(),
            "hits": cache.stats().hits,
            "misses": cache.stats().misses,
        }));
    }

    if ready {
        HttpResponse::Ok().json(serde_json::json!({
            "status": "ready",
            "service": "rust-geoserver",
            "checks": checks,
        }))
    } else {
        HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "status": "not_ready",
            "service": "rust-geoserver",
            "checks": checks,
        }))
    }
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
