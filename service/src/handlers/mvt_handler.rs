//! 矢量瓦片 (Mapbox Vector Tile) 处理器
//!
//! 提供 MVT 格式的矢量瓦片服务。
//! 端点: GET /tiles/{layer}/{z}/{x}/{y}.pbf 或 GET /tiles/{layer}/{z}/{x}/{y}?format=mvt

use crate::error::GeoServerError;
use crate::handlers::features;
use crate::state::AppState;
use crate::utils::mvt;
use actix_web::{web, HttpRequest, HttpResponse};

/// 处理 MVT 矢量瓦片请求
pub async fn handle_mvt_tile(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let layer_name = req.match_info().get("layer").unwrap_or("");
    let z_str = req.match_info().get("z").unwrap_or("0");
    let x_str = req.match_info().get("x").unwrap_or("0");
    let y_str = req.match_info().get("y").unwrap_or("0");

    let z: u32 = z_str
        .parse()
        .map_err(|_| GeoServerError::BadRequest(format!("无效的 zoom 级别: {}", z_str)))?;
    let x: u32 = x_str
        .parse()
        .map_err(|_| GeoServerError::BadRequest(format!("无效的 x: {}", x_str)))?;
    let y: u32 = y_str
        .parse()
        .map_err(|_| GeoServerError::BadRequest(format!("无效的 y: {}", y_str)))?;

    // 确保图层存在 (支持 `workspace:layer` 限定名, 与 WMS 一致)
    let layer = {
        let layers = state.layers.read().await;
        crate::handlers::features::resolve_layer(&layers, layer_name).cloned()
    };
    let layer = layer
        .ok_or_else(|| GeoServerError::NotFound(format!("Layer '{}' not found", layer_name)))?;
    // 统一使用短名 (Layer.name 不带 workspace 前缀) 查询要素与编码
    let layer_name = layer.name.as_str();

    // 计算瓦片地理边界
    let bbox = mvt::tile_bounds(z, x, y);

    // 查询该范围内的要素
    let features = features::query_layer_features(
        &state,
        layer_name,
        Some(&bbox),
        Some(100000), // 最大 10 万个要素
        None,
    )
    .await?;

    // 编码为 MVT 格式
    let tile_data = mvt::encode_tile(&features, layer_name, &bbox, 4096);

    Ok(HttpResponse::Ok()
        .content_type("application/vnd.mapbox-vector-tile")
        .append_header(("Access-Control-Allow-Origin", "*"))
        .body(tile_data))
}
