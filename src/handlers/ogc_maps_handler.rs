//! OGC API - Maps HTTP Handler
//!
//! Routes under `/ogc/maps`: landing page, `/conformance`, `/collections`,
//! `/collections/{id}`, `/collections/{id}/styles` and the `map` operation at
//! `/collections/{id}/map` (PNG / JPEG via `?f=`), delegating to the shared
//! WMS GetMap pipeline.

use crate::handlers::wms_handler;
use crate::services::ogc_maps;
use crate::state::AppState;
use actix_web::{web, HttpResponse};
use serde::Deserialize;

/// Service base URL (OGC API is served at the root path, not under the API
/// context).
fn base_url(state: &AppState) -> String {
    format!(
        "http://{}:{}",
        state.config.server.host, state.config.server.port
    )
}

fn json_response(value: serde_json::Value) -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/json")
        .body(serde_json::to_string(&value).unwrap_or_default())
}

fn not_found(what: &str) -> HttpResponse {
    HttpResponse::NotFound().json(serde_json::json!({
        "code": "NotFound",
        "description": format!("Resource not found: {}", what),
    }))
}

/// Query parameters of the `map` operation.
#[derive(Deserialize)]
pub struct MapQuery {
    pub bbox: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub f: Option<String>,
    pub transparent: Option<bool>,
    pub bgcolor: Option<String>,
    pub crs: Option<String>,
    pub datetime: Option<String>,
    pub cql_filter: Option<String>,
}

/// `GET /ogc/maps` — landing page.
pub async fn handle_ogc_maps_landing(state: web::Data<AppState>) -> HttpResponse {
    json_response(ogc_maps::landing_page(&base_url(state.get_ref())))
}

/// `GET /ogc/maps/conformance`
pub async fn handle_ogc_maps_conformance() -> HttpResponse {
    json_response(ogc_maps::conformance())
}

/// `GET /ogc/maps/collections` — map collections (one per published layer).
pub async fn handle_ogc_maps_collections(state: web::Data<AppState>) -> HttpResponse {
    let layers = state.list_layers().await;
    json_response(ogc_maps::collections(&base_url(state.get_ref()), &layers))
}

/// `GET /ogc/maps/collections/{collection}`
pub async fn handle_ogc_maps_collection(
    path: web::Path<String>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let name = path.into_inner();
    match state.get_layer(&name).await {
        Some(layer) => json_response(ogc_maps::collection(&base_url(state.get_ref()), &layer)),
        None => not_found(&name),
    }
}

/// `GET /ogc/maps/collections/{collection}/styles`
pub async fn handle_ogc_maps_styles(
    path: web::Path<String>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let name = path.into_inner();
    match state.get_layer(&name).await {
        Some(layer) => json_response(ogc_maps::styles(&base_url(state.get_ref()), &layer)),
        None => not_found(&name),
    }
}

/// `GET /ogc/maps/collections/{collection}/map`
///
/// Renders a map image (PNG default, JPEG via `?f=image/jpeg`) through the
/// shared WMS GetMap pipeline. Required parameters: `bbox` (minx,miny,maxx,
/// maxy) and `width` / `height` (pixels).
pub async fn handle_ogc_maps_map(
    path: web::Path<String>,
    query: web::Query<MapQuery>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let name = path.into_inner();
    if state.get_layer(&name).await.is_none() {
        return not_found(&name);
    }

    let bounds = match query.bbox.as_deref().and_then(ogc_maps::parse_bbox) {
        Some(b) => b,
        None => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "code": "InvalidParameterValue",
                "description": "bbox parameter is required (minx,miny,maxx,maxy)",
            }))
        },
    };
    let width = query.width.unwrap_or(512);
    let height = query.height.unwrap_or(512);
    let format = match query.f.as_deref() {
        Some(f) if f.to_lowercase().contains("jpeg") || f.to_lowercase() == "jpg" => {
            ogc_maps::MAP_JPEG_MIME
        },
        Some(f) if f.to_lowercase().contains("png") => ogc_maps::MAP_PNG_MIME,
        _ => ogc_maps::MAP_PNG_MIME,
    };
    let crs = query.crs.as_deref().unwrap_or("CRS:84");
    let transparent = query.transparent.unwrap_or(false);
    let bgcolor = query.bgcolor.clone();

    match wms_handler::render_ogc_map(
        state.get_ref(),
        &name,
        &bounds,
        width,
        height,
        format,
        crs,
        transparent,
        bgcolor,
        query.datetime.clone(),
        query.cql_filter.clone(),
    )
    .await
    {
        Ok(resp) => resp,
        Err(e) => HttpResponse::BadRequest().json(serde_json::json!({
            "code": "InvalidParameterValue",
            "description": e.to_string(),
        })),
    }
}
