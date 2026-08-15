//! WMTS (Web Map Tile Service) HTTP Handler
//!
//! 支持操作:
//! - GetCapabilities (XML)
//! - GetTile (重定向到内部 /tiles 端点)
//! - GetFeatureInfo (JSON)

use crate::error::GeoServerError;
use crate::services::wmts::{self, WmtsOperation};
use crate::state::AppState;
use actix_web::{web, HttpRequest, HttpResponse};

/// WMTS 主入口: GET /wmts?SERVICE=WMTS&REQUEST=...
pub async fn handle_wmts_request(
    req: HttpRequest,
    query: web::Query<Vec<(String, String)>>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let params = query.as_ref();
    let wmts_request = wmts::parse_wmts_request(params)?;

    match wmts_request.request {
        WmtsOperation::Capabilities => handle_get_capabilities(&state, &req).await,
        WmtsOperation::Tile {
            layer,
            style,
            format,
            tile_matrix_set,
            tile_matrix,
            tile_row,
            tile_col,
        } => {
            handle_get_tile(
                &state,
                &layer,
                &style,
                &format,
                &tile_matrix_set,
                &tile_matrix,
                tile_row,
                tile_col,
            )
            .await
        },
        WmtsOperation::FeatureInfo {
            layer,
            style,
            tile_matrix_set,
            tile_matrix,
            tile_row,
            tile_col,
            i,
            j,
            info_format,
        } => {
            handle_get_feature_info(
                &state,
                &layer,
                &style,
                &tile_matrix_set,
                &tile_matrix,
                tile_row,
                tile_col,
                i,
                j,
                &info_format,
            )
            .await
        },
    }
}

/// GetCapabilities — 返回 WMTS 能力文档 XML
async fn handle_get_capabilities(
    state: &AppState,
    req: &HttpRequest,
) -> Result<HttpResponse, GeoServerError> {
    let _host = req
        .headers()
        .get("Host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or(&state.config.server.host);

    let scheme = if state.config.server.port == 443 {
        "https"
    } else {
        "http"
    };
    let base_url = format!(
        "{}://{}:{}",
        scheme, state.config.server.host, state.config.server.port
    );

    let layers = state.layers.read().await;
    let xml = wmts::build_capabilities(&base_url, &layers, &state.config.server.api_context)?;
    drop(layers);

    Ok(HttpResponse::Ok().content_type("application/xml").body(xml))
}

/// GetTile — 返回瓦片图像
#[allow(clippy::too_many_arguments)] // signature mirrors the WMTS GetTile query parameters
async fn handle_get_tile(
    state: &AppState,
    layer: &str,
    _style: &str,
    _format: &str,
    _tile_matrix_set: &str,
    tile_matrix: &str,
    tile_row: u32,
    tile_col: u32,
) -> Result<HttpResponse, GeoServerError> {
    // 从 TileMatrix 标识符提取 zoom level: "EPSG:4326:5" -> 5
    let z: u32 = if let Some(idx) = tile_matrix.rfind(':') {
        tile_matrix[idx + 1..].parse().unwrap_or(0)
    } else {
        tile_matrix.parse().unwrap_or(0)
    };

    let gridset = _tile_matrix_set;

    // 通过共享瓦片渲染管线取瓦片 (含缓存)
    let (tile_data, cache_hit) = crate::handlers::tile_common::render_tile_bytes(
        state,
        layer,
        gridset,
        z,
        tile_col,
        tile_row,
        crate::handlers::tile_common::TileFormat::Png,
    )
    .await?;

    let cache_state = if cache_hit { "HIT" } else { "MISS" };
    Ok(HttpResponse::Ok()
        .insert_header(("X-Tile-Cache", cache_state))
        .insert_header(("X-WMTS", "1.0.0"))
        .content_type("image/png")
        .body(tile_data))
}

/// WMTS RESTful 瓦片端点: GET /wmts/{layer}/{tileMatrixSet}/{tileMatrix}/{tileCol}/{tileRow}
pub async fn handle_wmts_rest_tile(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let layer = req.match_info().get("layer").unwrap_or("");
    let tile_matrix_set = req.match_info().get("tileMatrixSet").unwrap_or("EPSG:4326");
    let tile_matrix = req.match_info().get("tileMatrix").unwrap_or("0");
    let tile_col: u32 = req
        .match_info()
        .get("tileCol")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let tile_row: u32 = req
        .match_info()
        .get("tileRow")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    handle_get_tile(
        state.get_ref(),
        layer,
        "default",
        "image/png",
        tile_matrix_set,
        tile_matrix,
        tile_row,
        tile_col,
    )
    .await
}

/// GetFeatureInfo — 返回要素信息
#[allow(clippy::too_many_arguments)] // signature mirrors the WMTS GetFeatureInfo query parameters
async fn handle_get_feature_info(
    state: &AppState,
    layer: &str,
    _style: &str,
    _tile_matrix_set: &str,
    _tile_matrix: &str,
    _tile_row: u32,
    _tile_col: u32,
    _i: u32,
    _j: u32,
    _info_format: &str,
) -> Result<HttpResponse, GeoServerError> {
    // 简化实现: 返回图层要素列表的 JSON
    let features =
        crate::handlers::features::query_layer_features(state, layer, None, Some(10), None).await?;

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .json(serde_json::json!({
            "type": "FeatureCollection",
            "features": features,
            "totalFeatures": features.len(),
        })))
}
