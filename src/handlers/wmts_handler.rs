//! WMTS (Web Map Tile Service) HTTP Handler
//!
//! 支持操作:
//! - GetCapabilities (XML)
//! - GetTile (重定向到内部 /tiles 端点)
//! - GetFeatureInfo (JSON)

use actix_web::{HttpRequest, HttpResponse, web};
use crate::services::wmts::{self, WmtsOperation};
use crate::state::AppState;
use crate::error::GeoServerError;

/// WMTS 主入口: GET /wmts?SERVICE=WMTS&REQUEST=...
pub async fn handle_wmts_request(
    req: HttpRequest,
    query: web::Query<Vec<(String, String)>>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let params = query.as_ref();
    let wmts_request = wmts::parse_wmts_request(params)?;

    match wmts_request.request {
        WmtsOperation::GetCapabilities => {
            handle_get_capabilities(&state, &req).await
        }
        WmtsOperation::GetTile { layer, style, format, tile_matrix_set, tile_matrix, tile_row, tile_col } => {
            handle_get_tile(&state, &layer, &style, &format, &tile_matrix_set, &tile_matrix, tile_row, tile_col).await
        }
        WmtsOperation::GetFeatureInfo { layer, style, tile_matrix_set, tile_matrix, tile_row, tile_col, i, j, info_format } => {
            handle_get_feature_info(&state, &layer, &style, &tile_matrix_set, &tile_matrix, tile_row, tile_col, i, j, &info_format).await
        }
    }
}

/// GetCapabilities — 返回 WMTS 能力文档 XML
async fn handle_get_capabilities(state: &AppState, req: &HttpRequest) -> Result<HttpResponse, GeoServerError> {
    let _host = req.headers()
        .get("Host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or(&state.config.server.host);

    let scheme = if state.config.server.port == 443 { "https" } else { "http" };
    let base_url = format!("{}://{}:{}", scheme, state.config.server.host, state.config.server.port);

    let layers = state.layers.read().await;
    let xml = wmts::build_capabilities(&base_url, &layers, &state.config.server.api_context)?;
    drop(layers);

    Ok(HttpResponse::Ok()
        .content_type("application/xml")
        .body(xml))
}

/// GetTile — 返回瓦片图像
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

    // 构建内部 /tiles URL 参数，转发到现有的瓦片渲染端点
    let gridset = _tile_matrix_set;

    // 1. 尝试从 GeoWebCache 获取
    if let Some(ref cache) = state.tile_cache {
        if let Some(cached) = cache.get(layer, gridset, z, tile_col, tile_row).await {
            return Ok(HttpResponse::Ok()
                .insert_header(("X-Tile-Cache", "HIT"))
                .insert_header(("X-WMTS", "1.0.0"))
                .content_type("image/png")
                .body(cached));
        }
    }

    // 2. 渲染瓦片（复用现有逻辑）
    use crate::models::Bounds;
    use crate::utils::rendering::{MapRenderer, RenderOptions, RenderFormat};
    use crate::handlers::features;

    let tile_size = 256u32;

    // 根据 gridset 计算瓦片边界
    let bounds = match gridset {
        "EPSG:3857" | "EPSG:900913" => {
            let n = 2.0_f64.powi(z as i32);
            let minx = (tile_col as f64 / n) * 360.0 - 180.0;
            let maxx = ((tile_col + 1) as f64 / n) * 360.0 - 180.0;
            let sin_lat = |y: f64| -> f64 {
                let v = std::f64::consts::PI * (1.0 - 2.0 * y / n);
                v.cos().recip().ln().atan().to_degrees()
            };
            let miny = sin_lat(tile_row as f64 + 1.0).max(-85.0511);
            let maxy = sin_lat(tile_row as f64).min(85.0511);
            Bounds::new(minx, miny, maxx, maxy)
        }
        _ => {
            let n = 2.0_f64.powi(z as i32);
            let minx = (tile_col as f64 / n) * 360.0 - 180.0;
            let maxx = ((tile_col + 1) as f64 / n) * 360.0 - 180.0;
            let miny = (tile_row as f64 / n) * 180.0 - 90.0;
            let maxy = ((tile_row + 1) as f64 / n) * 180.0 - 90.0;
            Bounds::new(minx, miny, maxx, maxy)
        }
    };

    let options = RenderOptions {
        width: tile_size,
        height: tile_size,
        transparent: true,
        bg_color: None,
        format: RenderFormat::PNG,
    };

    let renderer = MapRenderer::new(options, bounds);
    let layers_lock = state.layers.read().await;
    let styles_lock = state.styles.read().await;
    let mut render_items = Vec::new();

    if let Some(layer_obj) = layers_lock.iter().find(|l| l.name == layer) {
        use crate::handlers::style_handler::{get_style_rules, calculate_tile_scale_denom, reproject_geometry_helper};
        use crate::utils::sld_parser;

        let layer_crs = layer_obj.srs.to_epsg();
        let needs_reproject = layer_crs != "EPSG:4326";
        let rules = get_style_rules(&styles_lock, layer_obj);

        let features = features::query_layer_features(
            state, layer, None, None, None,
        ).await.unwrap_or_default();
        let scale_denom = calculate_tile_scale_denom(z);
        for feature in &features {
            let geom = if needs_reproject {
                reproject_geometry_helper(&feature.geometry, &layer_crs, "EPSG:4326")
            } else {
                feature.geometry.clone()
            };
            let style = sld_parser::resolve_style(&rules, feature, Some(scale_denom));
            render_items.push((geom, style));
        }
    }
    drop(layers_lock);
    drop(styles_lock);

    let img = renderer.render(render_items);
    let mut buffer = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buffer, image::ImageFormat::Png)
        .map_err(|e| GeoServerError::RenderingError(e.to_string()))?;
    let tile_data = buffer.into_inner();

    // 3. 写入缓存
    if let Some(ref cache) = state.tile_cache {
        cache.put(layer, gridset, z, tile_col, tile_row, &tile_data).await;
    }

    Ok(HttpResponse::Ok()
        .insert_header(("X-Tile-Cache", "MISS"))
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
    let tile_col: u32 = req.match_info().get("tileCol").and_then(|v| v.parse().ok()).unwrap_or(0);
    let tile_row: u32 = req.match_info().get("tileRow").and_then(|v| v.parse().ok()).unwrap_or(0);

    handle_get_tile(state.get_ref(), layer, "default", "image/png",
                    tile_matrix_set, tile_matrix, tile_row, tile_col).await
}

/// GetFeatureInfo — 返回要素信息
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
    let features = crate::handlers::features::query_layer_features(
        state, layer, None, Some(10), None,
    ).await?;

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .json(serde_json::json!({
            "type": "FeatureCollection",
            "features": features,
            "totalFeatures": features.len(),
        })))
}
