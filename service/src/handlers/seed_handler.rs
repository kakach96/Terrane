//! Tile seed / truncate REST handlers — `/tiles/seed`.
//!
//! - `POST /tiles/seed` — create a background seed job.
//! - `GET /tiles/seed` — list all jobs.
//! - `GET /tiles/seed/{id}` — job progress.
//! - `DELETE /tiles/seed/{id}` — cancel a running job (cooperative).
//! - `POST /tiles/seed/truncate` — remove cached tiles for a layer.
//!
//! Create / cancel / truncate require admin auth (they mutate the tile cache).

use crate::error::TerraneError;
use crate::handlers::auth_handler::require_auth;
use crate::handlers::rest_handler::ApiResponse;
use crate::state::AppState;
use crate::utils::tile_seed::{SeedJob, SeedRequest, SeedStatus};
use actix_web::{web, HttpRequest, HttpResponse};
use serde::Deserialize;
use std::sync::Arc;

/// Request body for truncate.
#[derive(Debug, Deserialize)]
pub struct TruncateRequest {
    pub layer: String,
    /// Optional gridset; when omitted the whole layer cache is cleared.
    pub gridset: Option<String>,
}

/// POST /tiles/seed — 创建并启动种子任务。
pub async fn create_seed_job(
    req: HttpRequest,
    body: web::Json<SeedRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, TerraneError> {
    require_auth(&req)?;

    if body.z_min > body.z_max {
        return Err(TerraneError::BadRequest("z_min 不能大于 z_max".to_string()));
    }
    if body.z_max > 22 {
        return Err(TerraneError::BadRequest("z_max 不能超过 22".to_string()));
    }

    // 图层必须存在
    let layer = {
        let layers = state.layers.read().await;
        layers.iter().find(|l| l.name == body.layer).cloned()
    }
    .ok_or_else(|| TerraneError::NotFound(format!("Layer '{}' not found", body.layer)))?;

    // 校验 gridset
    let gridset =
        crate::utils::tile_grid::canonical_gridset(body.gridset.as_deref().unwrap_or("EPSG:4326"));
    if crate::utils::tile_grid::gridset_profile(&gridset).is_none() {
        return Err(TerraneError::BadRequest(format!(
            "Unsupported gridset '{}'",
            gridset
        )));
    }

    let format = body
        .format
        .clone()
        .unwrap_or_else(|| "png".to_string())
        .to_lowercase();
    if format != "png" && format != "jpeg" && format != "jpg" {
        return Err(TerraneError::BadRequest(format!(
            "Unsupported format '{}' (png/jpeg)",
            format
        )));
    }

    let ts = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let job = SeedJob {
        id: uuid::Uuid::new_v4().to_string(),
        layer: layer.name.clone(),
        gridset: gridset.clone(),
        z_min: body.z_min,
        z_max: body.z_max,
        format: if format == "jpeg" || format == "jpg" {
            "jpeg".to_string()
        } else {
            "png".to_string()
        },
        status: SeedStatus::Pending,
        total: 0,
        done: 0,
        error: None,
        created_at: ts.clone(),
        updated_at: ts,
    };

    // 后台执行 (Arc<AppState> 供 tokio::spawn)
    let state_arc: Arc<AppState> = state.into_inner();
    let started = crate::utils::tile_seed::start_seed_job(
        state_arc.seed_jobs.clone(),
        state_arc,
        job.clone(),
    );

    Ok(
        HttpResponse::Created().json(ApiResponse::success(serde_json::json!({
            "job": started,
            "message": format!(
                "Seed job started: {} z{}-{} ({})",
                layer.name, body.z_min, body.z_max, gridset
            ),
        }))),
    )
}

/// GET /tiles/seed — 任务列表。
pub async fn list_seed_jobs(state: web::Data<AppState>) -> Result<HttpResponse, TerraneError> {
    let map = state.seed_jobs.lock().unwrap();
    let mut jobs: Vec<&SeedJob> = map.values().collect();
    jobs.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(HttpResponse::Ok().json(ApiResponse::success(jobs)))
}

/// GET /tiles/seed/{id} — 任务进度。
pub async fn get_seed_job(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, TerraneError> {
    let id = req.match_info().get("id").unwrap_or("");
    let map = state.seed_jobs.lock().unwrap();
    let job = map
        .get(id)
        .ok_or_else(|| TerraneError::NotFound(format!("Seed job '{}' not found", id)))?;
    Ok(HttpResponse::Ok().json(ApiResponse::success(job)))
}

/// DELETE /tiles/seed/{id} — 取消任务 (协作式; 已写瓦片保留)。
pub async fn cancel_seed_job(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, TerraneError> {
    require_auth(&req)?;
    let id = req.match_info().get("id").unwrap_or("").to_string();
    let mut map = state.seed_jobs.lock().unwrap();
    let job = map
        .get_mut(&id)
        .ok_or_else(|| TerraneError::NotFound(format!("Seed job '{}' not found", id)))?;
    match job.status {
        SeedStatus::Pending | SeedStatus::Running => {
            job.status = SeedStatus::Cancelled;
            job.updated_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
            Ok(
                HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
                    "job": id,
                    "message": "Seed job cancellation requested",
                }))),
            )
        },
        _ => Err(TerraneError::BadRequest(format!(
            "Seed job '{}' is not running (status: {:?})",
            id, job.status
        ))),
    }
}

/// POST /tiles/seed/truncate — 清除图层的缓存瓦片 (可按 gridset)。
pub async fn truncate_tiles(
    req: HttpRequest,
    body: web::Json<TruncateRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, TerraneError> {
    require_auth(&req)?;

    let cache = {
        let layers = state.layers.read().await;
        layers.iter().find(|l| l.name == body.layer).cloned()
    };
    let removed = match cache {
        Some(l) => {
            let tile_cache = state.tile_cache_for(&l).await;
            match tile_cache {
                Some(c) => c.clear_layer(&body.layer).await.unwrap_or(0),
                None => 0,
            }
        },
        None => {
            return Err(TerraneError::NotFound(format!(
                "Layer '{}' not found",
                body.layer
            )))
        },
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
        "layer": body.layer,
        "gridset": body.gridset,
        "removed": removed,
        "message": format!("Tile cache truncated for '{}' ({} tile(s) removed)", body.layer, removed),
    }))))
}
