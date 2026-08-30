//! WMS-C (Cached WMS) 1.1.1 HTTP Handler
//!
//! Route: `{api_context}/gwc/service/wms`. Behaves like WMS 1.1.1 but supports
//! the `TILED=true` vendor parameter: a grid-aligned GetMap resolves to a
//! single tile through the shared tile engine (mirroring GeoWebCache).

use crate::error::TerraneError;
use crate::services::{wms, wmsc};
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

/// WMS-C entry point: `{api_context}/gwc/service/wms`
pub async fn handle_wmsc_request(
    req: HttpRequest,
    query: web::Query<Vec<(String, String)>>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let params: &[(String, String)] = query.as_ref();
    let request = params
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("REQUEST"))
        .map(|(_, v)| v.to_uppercase())
        .unwrap_or_default();
    let tiled = params
        .iter()
        .any(|(k, v)| k.eq_ignore_ascii_case("TILED") && v.eq_ignore_ascii_case("true"));

    match request.as_str() {
        "GETCAPABILITIES" => {
            let base = base_url(state.get_ref());
            let layers = state.layers.read().await;
            HttpResponse::Ok()
                .content_type("application/vnd.ogc.wms_xml")
                .body(wmsc::build_capabilities(&base, &layers))
        },
        "GETMAP" if tiled => match handle_tiled_get_map(&state, params).await {
            Ok(resp) => resp,
            Err(e) => {
                let (body, content_type) = wms::format_wms_exception(&e, None, 512, 512);
                HttpResponse::Ok().content_type(content_type).body(body)
            },
        },
        "GETMAP" => {
            // Without TILED=true, WMS-C behaves like plain WMS 1.1.1.
            crate::handlers::wms_handler::handle_wms_request(req, query, state).await
        },
        _ => HttpResponse::BadRequest().body("Unsupported WMS-C request"),
    }
}

/// Handle a `TILED=true` GetMap: derive the (gridset, z, col, row) tile from
/// the requested bounding box and render it through the shared tile engine.
async fn handle_tiled_get_map(
    state: &AppState,
    params: &[(String, String)],
) -> Result<HttpResponse, TerraneError> {
    let request = wms::parse_wms_request(params)?;
    let layers = request
        .layers
        .as_ref()
        .ok_or_else(|| TerraneError::BadRequest("LAYERS parameter is required".to_string()))?;
    if layers.len() != 1 {
        return Err(TerraneError::BadRequest(
            "TILED=true currently supports exactly one layer".to_string(),
        ));
    }
    let bbox = request
        .bbox
        .as_ref()
        .ok_or_else(|| TerraneError::BadRequest("BBOX parameter is required".to_string()))?
        .to_bounds();
    let width = request.width.unwrap_or(256) as f64;

    // Map the requested CRS to one of the cached gridsets.
    let crs = request.crs.as_deref().unwrap_or("EPSG:4326");
    let gridset = if crs.contains("3857") || crs.contains("900913") {
        "EPSG:3857"
    } else if crs.contains("4326") {
        "EPSG:4326"
    } else {
        return Err(TerraneError::BadRequest(format!(
            "CRS '{}' is not cached by the tile service",
            crs
        )));
    };

    // Estimate the zoom from the horizontal resolution, then snap to the tile.
    let res = (bbox.maxx - bbox.minx) / width;
    let z = tile_grid::zoom_for_resolution(gridset, res);
    let (col, row) = tile_grid::tile_for_bbox(gridset, z, &bbox)
        .ok_or_else(|| TerraneError::BadRequest("BBOX out of gridset range".to_string()))?;

    let format = crate::handlers::tile_common::TileFormat::from_mime(
        request.format.as_deref().unwrap_or("image/png"),
    );
    let (bytes, _cache_hit) = crate::handlers::tile_common::render_tile_bytes(
        state, &layers[0], gridset, z, col, row, format,
    )
    .await?;

    Ok(HttpResponse::Ok().content_type(format.mime()).body(bytes))
}
