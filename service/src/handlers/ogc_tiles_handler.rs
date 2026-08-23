//! OGC API - Tiles HTTP Handler
//!
//! Routes under `/ogc/tiles`: landing page, `/conformance`, `/tileMatrixSets`
//! (+ per-id definitions), `/collections` tileset listings and raster tiles at
//! `/collections/{id}/tiles/{tileMatrixSetId}/{tileMatrix}/{tileRow}/{tileCol}`
//! (PNG / JPEG via `?f=`), reusing the shared tile engine.

use crate::handlers::tile_common::{render_tile_bytes, TileFormat};
use crate::services::ogc_tiles;
use crate::state::AppState;
use actix_web::{web, HttpResponse};
use std::collections::HashMap;

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

/// `GET /ogc/tiles` — landing page.
pub async fn handle_ogc_tiles_landing(state: web::Data<AppState>) -> HttpResponse {
    json_response(ogc_tiles::landing_page(&base_url(state.get_ref())))
}

/// `GET /ogc/tiles/conformance`
pub async fn handle_ogc_tiles_conformance() -> HttpResponse {
    json_response(ogc_tiles::conformance())
}

/// `GET /ogc/tiles/tileMatrixSets`
pub async fn handle_ogc_tiles_tile_matrix_sets(state: web::Data<AppState>) -> HttpResponse {
    json_response(ogc_tiles::tile_matrix_sets(&base_url(state.get_ref())))
}

/// `GET /ogc/tiles/tileMatrixSets/{id}`
pub async fn handle_ogc_tiles_tile_matrix_set(path: web::Path<String>) -> HttpResponse {
    let id = path.into_inner();
    match ogc_tiles::tile_matrix_set(&id) {
        Some(v) => json_response(v),
        None => not_found(&id),
    }
}

/// `GET /ogc/tiles/collections` — tileset overview per layer.
pub async fn handle_ogc_tiles_collections(state: web::Data<AppState>) -> HttpResponse {
    let layers = state.list_layers().await;
    json_response(ogc_tiles::collections(&base_url(state.get_ref()), &layers))
}

/// `GET /ogc/tiles/collections/{collection}/tiles` — tilesets for a layer.
pub async fn handle_ogc_tiles_collection(
    path: web::Path<String>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let name = path.into_inner();
    match state.get_layer(&name).await {
        Some(layer) => json_response(ogc_tiles::collection_tilesets(
            &base_url(state.get_ref()),
            &layer,
        )),
        None => not_found(&name),
    }
}

/// `GET /ogc/tiles/collections/{collection}/tiles/{tileMatrixSetId}/{tileMatrix}/{tileRow}/{tileCol}`
///
/// Returns the raster tile (PNG by default, JPEG via `?f=image/jpeg`).
pub async fn handle_ogc_tile(
    path: web::Path<(String, String, String, u32, u32)>,
    query: web::Query<HashMap<String, String>>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let (collection, tile_matrix_set_id, tile_matrix, tile_row, tile_col) = path.into_inner();

    if state.get_layer(&collection).await.is_none() {
        return not_found(&collection);
    }

    let z = ogc_tiles::parse_zoom(&tile_matrix);
    let format = match query.get("f").map(|s| s.to_lowercase()) {
        Some(f) if f.contains("jpeg") || f == "jpg" => TileFormat::Jpeg,
        _ => TileFormat::Png,
    };

    match render_tile_bytes(
        state.get_ref(),
        &collection,
        &tile_matrix_set_id,
        z,
        tile_col,
        tile_row,
        format,
    )
    .await
    {
        Ok((data, cache_hit)) => HttpResponse::Ok()
            .insert_header(("X-Tile-Cache", if cache_hit { "HIT" } else { "MISS" }))
            .insert_header(("X-OGCAPI-Tiles", "1.0"))
            .content_type(format.mime())
            .body(data),
        Err(e) => {
            // out-of-range tile index / unknown gridset → 404 (tile does not exist)
            HttpResponse::NotFound().json(serde_json::json!({
                "code": "NotFound",
                "description": e.to_string(),
            }))
        },
    }
}
