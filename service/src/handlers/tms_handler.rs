//! TMS (Tile Map Service) 1.0.0 HTTP Handler
//!
//! Routes (under `{api_context}`, e.g. `/terrane`):
//! - GET `/gwc/service/tms`                 → KVP GetCapabilities / GetTile
//! - GET `/gwc/service/tms/1.0.0`           → RESTful GetCapabilities
//! - GET `/gwc/service/tms/1.0.0/{tail:.*}` → TileMap document or tile

use crate::error::TerraneError;
use crate::services::tms;
use crate::state::AppState;
use crate::utils::tile_grid;
use actix_web::{web, HttpRequest, HttpResponse};

/// Service base URL including the API context (e.g. `http://host:port/terrane`).
fn base_url(state: &AppState) -> String {
    format!(
        "http://{}:{}{}",
        state.config.server.host, state.config.server.port, state.config.server.api_context
    )
}

/// KVP entry point: `{api_context}/gwc/service/tms`
pub async fn handle_tms_request(
    _req: HttpRequest,
    query: web::Query<Vec<(String, String)>>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, TerraneError> {
    let params: &[(String, String)] = query.as_ref();
    let request = params
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("REQUEST"))
        .map(|(_, v)| v.to_uppercase())
        .unwrap_or_default();

    match request.as_str() {
        "GETCAPABILITIES" => {
            let base = base_url(state.get_ref());
            let layers = state.layers.read().await;
            Ok(HttpResponse::Ok()
                .content_type("text/xml")
                .body(tms::build_tile_map_service(&base, &layers)))
        },
        "GETTILE" => {
            let path = tms::parse_kvp_tile(params).ok_or_else(|| {
                TerraneError::BadRequest("Missing TMS GetTile parameters".to_string())
            })?;
            render_tms_tile(&state, &path).await
        },
        _ => Err(TerraneError::BadRequest(
            "Unsupported TMS request".to_string(),
        )),
    }
}

/// RESTful entry point: `{api_context}/gwc/service/tms/1.0.0[/{tail:.*}]`
pub async fn handle_tms_path(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, TerraneError> {
    let tail = req.match_info().get("tail").unwrap_or("");
    let base = base_url(state.get_ref());
    let layers = state.layers.read().await;

    // Tail empty → capabilities.
    if tail.is_empty() {
        return Ok(HttpResponse::Ok()
            .content_type("text/xml")
            .body(tms::build_tile_map_service(&base, &layers)));
    }

    // `{tilemap}/{z}/{x}/{y}.{ext}` → tile.
    if let Some(path) = tms::parse_tile_path(tail) {
        drop(layers);
        return render_tms_tile(&state, &path).await;
    }

    // `{tilemap}` → TileMap document.
    let (layer_name, gridset, format) = tms::parse_tilemap_id(tail)
        .ok_or_else(|| TerraneError::BadRequest("Invalid TMS path".to_string()))?;
    let layer = layers
        .iter()
        .find(|l| l.name == layer_name)
        .ok_or_else(|| TerraneError::NotFound(format!("Layer '{}' not found", layer_name)))?;
    let doc = tms::build_tile_map(&base, layer, &gridset, &format)
        .ok_or_else(|| TerraneError::BadRequest("Unknown TMS gridset or format".to_string()))?;
    Ok(HttpResponse::Ok().content_type("text/xml").body(doc))
}

/// Render a single TMS tile (flipping the bottom-up TMS row to the top-down
/// row used by the shared tile engine).
async fn render_tms_tile(
    state: &web::Data<AppState>,
    path: &tms::TmsTilePath,
) -> Result<HttpResponse, TerraneError> {
    let row_slippy = tile_grid::tms_row_to_slippy(&path.gridset, path.z, path.y_tms)
        .ok_or_else(|| TerraneError::BadRequest("TMS tile row out of range".to_string()))?;
    let format = crate::handlers::tile_common::TileFormat::from_extension(&path.ext);
    let (bytes, _cache_hit) = crate::handlers::tile_common::render_tile_bytes(
        state.get_ref(),
        &path.layer,
        &path.gridset,
        path.z,
        path.x,
        row_slippy,
        format,
    )
    .await?;
    Ok(HttpResponse::Ok().content_type(format.mime()).body(bytes))
}
