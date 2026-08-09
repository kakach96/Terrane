//! OGC API - Features HTTP Handler
//!
//! Routes under `/ogc/features`: landing page, `/conformance`, `/collections`,
//! `/collections/{id}`, `/collections/{id}/items` and
//! `/collections/{id}/items/{featureId}`. Responses are JSON / GeoJSON.

use crate::services::ogc_features;
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

fn json_response(value: serde_json::Value, mime: &str) -> HttpResponse {
    HttpResponse::Ok()
        .content_type(mime)
        .body(serde_json::to_string(&value).unwrap_or_default())
}

fn not_found(what: &str) -> HttpResponse {
    HttpResponse::NotFound().json(serde_json::json!({
        "code": "NotFound",
        "description": format!("Resource not found: {}", what),
    }))
}

/// Query parameters of the items endpoint.
#[derive(Deserialize)]
pub struct ItemsQuery {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub bbox: Option<String>,
    #[allow(dead_code)]
    pub f: Option<String>,
}

/// `GET /ogc/features` — landing page.
pub async fn handle_ogc_landing(state: web::Data<AppState>) -> HttpResponse {
    json_response(
        ogc_features::landing_page(&base_url(state.get_ref())),
        "application/json",
    )
}

/// `GET /ogc/features/conformance`
pub async fn handle_ogc_conformance() -> HttpResponse {
    json_response(ogc_features::conformance(), "application/json")
}

/// `GET /ogc/features/collections`
pub async fn handle_ogc_collections(state: web::Data<AppState>) -> HttpResponse {
    let layers = state.list_layers().await;
    json_response(
        ogc_features::collections(&base_url(state.get_ref()), &layers),
        "application/json",
    )
}

/// `GET /ogc/features/collections/{collection}`
pub async fn handle_ogc_collection(
    path: web::Path<String>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let name = path.into_inner();
    match state.get_layer(&name).await {
        Some(layer) => json_response(
            ogc_features::collection(&base_url(state.get_ref()), &layer),
            "application/json",
        ),
        None => not_found(&name),
    }
}

/// `GET /ogc/features/collections/{collection}/items`
pub async fn handle_ogc_items(
    path: web::Path<String>,
    query: web::Query<ItemsQuery>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let name = path.into_inner();
    let layer = match state.get_layer(&name).await {
        Some(l) => l,
        None => return not_found(&name),
    };
    let features = match crate::handlers::features::query_layer_features(
        &state, &name, None, None, None,
    )
    .await
    {
        Ok(f) => f,
        Err(_) => Vec::new(),
    };
    let limit = query.limit.unwrap_or(10).max(1) as usize;
    let offset = query.offset.unwrap_or(0) as usize;
    let bbox = query.bbox.as_deref().and_then(ogc_features::parse_bbox);
    json_response(
        ogc_features::items(
            &base_url(state.get_ref()),
            &layer,
            &features,
            limit,
            offset,
            bbox.as_ref(),
        ),
        ogc_features::GEOJSON_MIME,
    )
}

/// `GET /ogc/features/collections/{collection}/items/{feature}`
pub async fn handle_ogc_item(
    path: web::Path<(String, String)>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let (name, feature_id) = path.into_inner();
    let features = match crate::handlers::features::query_layer_features(
        &state, &name, None, None, None,
    )
    .await
    {
        Ok(f) => f,
        Err(_) => Vec::new(),
    };
    match features.iter().find(|f| f.id == feature_id) {
        Some(f) => json_response(ogc_features::item(f), ogc_features::GEOJSON_MIME),
        None => not_found(&feature_id),
    }
}
