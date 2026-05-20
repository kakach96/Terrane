use actix_web::{HttpRequest, HttpResponse, web};
use crate::services::wms::{self, WmsRequest, WmsCapabilities};
use crate::error::GeoServerError;
use crate::state::AppState;
use crate::utils::rendering::{MapRenderer, RenderOptions, RenderFormat};
use crate::utils::sld_parser::{self, ParsedRule};
use crate::utils::projection::ProjectionTransformer;
use crate::models::{Bounds, CoordinateReferenceSystem, GeoJsonGeometry};
use quick_xml::se::to_string;
use std::io::Cursor;
use std::collections::HashMap;
use image::ImageFormat;

pub async fn handle_wms_request(
    _req: HttpRequest,
    query: web::Query<Vec<(String, String)>>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let params: Vec<(String, String)> = query.into_inner();
    let wms_request = match wms::parse_wms_request(&params) {
        Ok(r) => r,
        Err(e) => return format_wms_error_response(&e, &params),
    };

    let result = match wms_request.request {
        wms::WmsOperation::GetCapabilities => handle_get_capabilities(&state, &wms_request).await,
        wms::WmsOperation::GetMap => handle_get_map(&state, &wms_request).await,
        wms::WmsOperation::GetFeatureInfo => handle_get_feature_info(&state, &wms_request).await,
        wms::WmsOperation::GetLegendGraphic => handle_get_legend_graphic(&state, &wms_request).await,
        wms::WmsOperation::DescribeLayer => handle_describe_layer(&state, &wms_request).await,
        _ => Err(GeoServerError::BadRequest("Operation not implemented".to_string())),
    };

    match result {
        Ok(resp) => resp,
        Err(e) => format_wms_error_response(&e, &params),
    }
}

fn format_wms_error_response(err: &GeoServerError, params: &[(String, String)]) -> HttpResponse {
    let exceptions = params.iter()
        .find(|(k, _)| k.to_uppercase() == "EXCEPTIONS")
        .map(|(_, v)| v.as_str());
    let width: u32 = params.iter()
        .find(|(k, _)| k.to_uppercase() == "WIDTH")
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(512);
    let height: u32 = params.iter()
        .find(|(k, _)| k.to_uppercase() == "HEIGHT")
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(512);

    let (body, content_type) = wms::format_wms_exception(err, exceptions, width, height);
    HttpResponse::Ok()
        .content_type(content_type)
        .body(body)
}

async fn handle_get_capabilities(state: &AppState, _request: &WmsRequest) -> Result<HttpResponse, GeoServerError> {
    let base_url = format!("http://{}:{}", state.config.server.host, state.config.server.port);
    let mut capabilities = WmsCapabilities::new(&base_url);

    let layers = state.layers.read().await;
    for layer in layers.iter() {
        capabilities.add_layer(layer);
    }

    let xml = to_string(&capabilities)
        .map_err(|e| GeoServerError::ServiceError(format!("Failed to serialize capabilities: {}", e)))?;

    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
{}"#,
        xml
    );

    Ok(HttpResponse::Ok()
        .content_type("application/xml")
        .body(xml))
}

async fn handle_get_map(state: &AppState, request: &WmsRequest) -> Result<HttpResponse, GeoServerError> {
    wms::validate_wms_getmap_request(request)?;

    let layers_param = request.layers.as_ref().unwrap();
    let width = request.width.unwrap_or(512) as u32;
    let height = request.height.unwrap_or(512) as u32;
    let format = request.format.as_ref().unwrap();
    let bbox = request.bbox.as_ref().unwrap();
    let bounds = Bounds::new(bbox.minx, bbox.miny, bbox.maxx, bbox.maxy);

    let output_crs = request.crs.as_deref().unwrap_or("EPSG:4326");
    let scale_denom = calculate_scale_denom(&bounds, width, height, output_crs);

    let options = RenderOptions {
        width,
        height,
        transparent: request.transparent.unwrap_or(false),
        bg_color: request.bgcolor.as_ref().map(|c| parse_color(c)),
        format: RenderFormat::PNG,
    };

    let renderer = MapRenderer::new(options, bounds.clone());

    let layers_lock = state.layers.read().await;
    let styles_lock = state.styles.read().await;
    let mut render_items = Vec::new();

    for layer_name in layers_param {
        if let Some(layer) = layers_lock.iter().find(|l| l.name == *layer_name) {
            let layer_crs = layer.srs.to_epsg();
            let needs_reproject = layer_crs != output_crs;

            let rules = get_layer_rules(request, &styles_lock, layer);
            if let Some(features) = state.get_layer_features(&layer.name).await {
                for feature in &features {
                    let geom = if needs_reproject {
                        reproject_geometry(&feature.geometry, &layer_crs, output_crs)
                    } else {
                        feature.geometry.clone()
                    };
                    let style = if !rules.is_empty() {
                        sld_parser::resolve_style(&rules, feature, Some(scale_denom))
                    } else {
                        crate::utils::rendering::Style::default()
                    };
                    render_items.push((geom, style));
                }
            }
        }
    }

    if format.to_lowercase().contains("openlayers") {
        let host = state.config.server.host.clone();
        let port = state.config.server.port;
        let base_url = format!("http://{}:{}", host, port);
        let wms_url = format!("{}/wms?", base_url);

        let html = format!(
            r#"<!DOCTYPE html>
<html>
<head>
  <title>Layer Preview - {layer_name}</title>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <link rel="stylesheet" href="https://openlayers.org/en/v4.6.5/css/ol.css" type="text/css">
  <script src="https://openlayers.org/en/v4.6.5/build/ol.js"></script>
  <style>
    html, body, #map {{ height: 100%; width: 100%; margin: 0; padding: 0; }}
  </style>
</head>
<body>
  <div id="map"></div>
  <script>
    var layers = {layers_json};
    var map = new ol.Map({{
      target: 'map',
      layers: layers.map(function(name) {{
        return new ol.layer.Tile({{
          source: new ol.source.TileWMS({{
            url: '{wms_url}',
            params: {{ 'LAYERS': name, 'TILED': true, 'VERSION': '1.3.0' }}
          }})
        }});
      }}),
      view: new ol.View({{
        projection: '{crs}',
        center: [{center_x}, {center_y}],
        zoom: {zoom}
      }})
    }});
  </script>
</body>
</html>"#,
            layer_name = layers_param.join(","),
            layers_json = serde_json::to_string(layers_param).unwrap_or_default(),
            wms_url = wms_url,
            crs = output_crs,
            center_x = (bounds.minx + bounds.maxx) / 2.0,
            center_y = (bounds.miny + bounds.maxy) / 2.0,
            zoom = calculate_openlayers_zoom(&bounds, output_crs),
        );

        return Ok(HttpResponse::Ok()
            .content_type("text/html")
            .body(html));
    }

    let img = renderer.render(render_items);

    let image_format = match format.to_lowercase().as_str() {
        s if s.contains("png") => ImageFormat::Png,
        s if s.contains("jpeg") || s.contains("jpg") => ImageFormat::Jpeg,
        s if s.contains("gif") => ImageFormat::Gif,
        s if s.contains("webp") => ImageFormat::WebP,
        _ => ImageFormat::Png,
    };

    let mut buffer = Cursor::new(Vec::new());
    img.write_to(&mut buffer, image_format)
        .map_err(|e| GeoServerError::RenderingError(format!("Failed to render image: {}", e)))?;

    let content_type = match image_format {
        ImageFormat::Png => "image/png",
        ImageFormat::Jpeg => "image/jpeg",
        ImageFormat::Gif => "image/gif",
        ImageFormat::WebP => "image/webp",
        _ => "image/png",
    };

    Ok(HttpResponse::Ok()
        .content_type(content_type)
        .body(buffer.into_inner()))
}

fn get_layer_rules(
    request: &WmsRequest,
    styles: &std::collections::HashMap<String, String>,
    layer: &crate::models::Layer,
) -> Vec<ParsedRule> {
    let sld_xml = request.sld_body.clone().or_else(|| {
        let style_name = layer.styles.first().map(|s| &s.name).cloned().unwrap_or_default();
        styles.get(&style_name).cloned()
    });
    match sld_xml {
        Some(xml) => sld_parser::parse_sld(&xml),
        None => sld_parser::parse_sld(&sld_parser::default_sld(&layer.name)),
    }
}

async fn handle_get_feature_info(state: &AppState, request: &WmsRequest) -> Result<HttpResponse, GeoServerError> {
    let i = request.i.unwrap_or(0.0);
    let j = request.j.unwrap_or(0.0);
    let width = request.width.unwrap_or(512) as f64;
    let height = request.height.unwrap_or(512) as f64;
    let feature_count = request.feature_count.unwrap_or(10) as usize;

    let info_format = request.info_format.as_deref().unwrap_or("text/plain");

    let click_point = request.bbox.as_ref().map(|bbox| {
        let wx = bbox.minx + (i / width) * (bbox.maxx - bbox.minx);
        let wy = bbox.maxy - (j / height) * (bbox.maxy - bbox.miny);
        (wx, wy)
    });

    let layers_lock = state.layers.read().await;
    let mut found_features: Vec<(String, String, HashMap<String, String>)> = Vec::new();

    if let Some(query_layers) = &request.query_layers {
        for layer_name in query_layers {
            if let Some(layer) = layers_lock.iter().find(|l| l.name == *layer_name) {
                if let Some(features) = state.get_layer_features(&layer.name).await {
                    for feature in &features {
                        let hit = if let Some((cx, cy)) = click_point {
                            feature_hit_test(&feature.geometry, cx, cy, &bbox_to_bounds(request))
                        } else {
                            true
                        };
                        if hit {
                            let mut props = HashMap::new();
                            for (k, v) in &feature.properties {
                                props.insert(k.clone(), v.to_string());
                            }
                            found_features.push((layer.name.clone(), feature.id.clone(), props));
                            if found_features.len() >= feature_count {
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    let response = match info_format {
        "application/json" => {
            let json_features: Vec<serde_json::Value> = found_features.iter().map(|(layer, fid, props)| {
                serde_json::json!({
                    "layer": layer,
                    "feature_id": fid,
                    "properties": props,
                })
            }).collect();
            serde_json::to_string_pretty(&json_features)
                .map_err(|e| GeoServerError::ServiceError(e.to_string()))?
        }
        "text/html" => {
            let rows: String = found_features.iter().map(|(layer, fid, props)| {
                let prop_rows: String = props.iter()
                    .map(|(k, v)| format!("<tr><td>{}</td><td>{}</td></tr>", k, v))
                    .collect();
                format!("<h3>Layer: {} (ID: {})</h3><table border='1'>{}</table>", layer, fid, prop_rows)
            }).collect();
            format!("<html><body><h1>Feature Information</h1>{}</body></html>", rows)
        }
        _ => {
            found_features.iter().map(|(layer, fid, props)| {
                let prop_str: String = props.iter()
                    .map(|(k, v)| format!("  {} = {}", k, v))
                    .collect::<Vec<_>>()
                    .join("\n");
                format!("Layer: {}\nFeature ID: {}\n{}\n", layer, fid, prop_str)
            }).collect::<Vec<_>>().join("---\n")
        }
    };

    let content_type = match info_format {
        "application/json" => "application/json",
        "text/html" => "text/html",
        _ => "text/plain",
    };

    Ok(HttpResponse::Ok()
        .content_type(content_type)
        .body(response))
}

fn bbox_to_bounds(request: &WmsRequest) -> Bounds {
    request.bbox.as_ref().map(|b| Bounds::new(b.minx, b.miny, b.maxx, b.maxy))
        .unwrap_or_default()
}

fn feature_hit_test(geom: &GeoJsonGeometry, cx: f64, cy: f64, bounds: &Bounds) -> bool {
    const TOLERANCE: f64 = 0.05;
    let range = (bounds.maxx - bounds.minx).max(bounds.maxy - bounds.miny);
    let tolerance = (range / 200.0).max(TOLERANCE);

    match geom {
        GeoJsonGeometry::Point { coordinates } => {
            if coordinates.len() >= 2 {
                let dx = coordinates[0] - cx;
                let dy = coordinates[1] - cy;
                (dx * dx + dy * dy).sqrt() <= tolerance
            } else {
                false
            }
        }
        GeoJsonGeometry::LineString { coordinates } => {
            coordinates.windows(2).any(|seg| {
                if seg.len() < 2 || seg[0].len() < 2 || seg[1].len() < 2 {
                    return false;
                }
                point_to_segment_distance(cx, cy, seg[0][0], seg[0][1], seg[1][0], seg[1][1]) <= tolerance
            })
        }
        GeoJsonGeometry::Polygon { coordinates } => {
            coordinates.first().map(|ring| point_in_ring(cx, cy, ring)).unwrap_or(false)
        }
        _ => false,
    }
}

fn point_to_segment_distance(px: f64, py: f64, ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    let abx = bx - ax;
    let aby = by - ay;
    let apx = px - ax;
    let apy = py - ay;
    let ab2 = abx * abx + aby * aby;
    if ab2 == 0.0 {
        return ((px - ax).powi(2) + (py - ay).powi(2)).sqrt();
    }
    let t = ((apx * abx + apy * aby) / ab2).clamp(0.0, 1.0);
    let projx = ax + t * abx;
    let projy = ay + t * aby;
    ((px - projx).powi(2) + (py - projy).powi(2)).sqrt()
}

fn point_in_ring(px: f64, py: f64, ring: &[Vec<f64>]) -> bool {
    let mut inside = false;
    let mut j = ring.len() - 1;
    for i in 0..ring.len() {
        if ring[i].len() < 2 || ring[j].len() < 2 {
            j = i;
            continue;
        }
        let (xi, yi) = (ring[i][0], ring[i][1]);
        let (xj, yj) = (ring[j][0], ring[j][1]);
        if ((yi > py) != (yj > py)) && (px < (xj - xi) * (py - yi) / (yj - yi) + xi) {
            inside = !inside;
        }
        j = i;
    }
    inside
}

async fn handle_describe_layer(state: &AppState, request: &WmsRequest) -> Result<HttpResponse, GeoServerError> {
    let layers_param = request.layers.as_ref()
        .ok_or_else(|| GeoServerError::BadRequest("LAYERS parameter required".to_string()))?;

    let layers_lock = state.layers.read().await;
    let mut layer_descriptions = Vec::new();

    for layer_name in layers_param {
        if let Some(layer) = layers_lock.iter().find(|l| l.name == *layer_name) {
            layer_descriptions.push(serde_json::json!({
                "name": layer.name,
                "title": layer.title,
                "crs": layer.srs.to_epsg(),
                "native_bounds": {
                    "crs": layer.native_bounds.crs.to_epsg(),
                    "minx": layer.native_bounds.bounds.minx,
                    "miny": layer.native_bounds.bounds.miny,
                    "maxx": layer.native_bounds.bounds.maxx,
                    "maxy": layer.native_bounds.bounds.maxy,
                },
                "lat_lon_bounds": {
                    "crs": layer.lat_lon_bounds.crs.to_epsg(),
                    "minx": layer.lat_lon_bounds.bounds.minx,
                    "miny": layer.lat_lon_bounds.bounds.miny,
                    "maxx": layer.lat_lon_bounds.bounds.maxx,
                    "maxy": layer.lat_lon_bounds.bounds.maxy,
                },
                "styles": layer.styles.iter().map(|s| {
                    serde_json::json!({ "name": s.name })
                }).collect::<Vec<_>>(),
            }));
        }
    }

    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<DescribeLayerResponse version="1.1.1" xmlns="http://www.opengis.net/wms">
{}
</DescribeLayerResponse>"#,
        layer_descriptions.iter().map(|desc| {
            format!(
                r#"  <LayerDescription name="{name}" crs="{crs}">
    <Bounds CRS="{crs}" minx="{minx}" miny="{miny}" maxx="{maxx}" maxy="{maxy}"/>
  </LayerDescription>"#,
                name = desc["name"].as_str().unwrap_or(""),
                crs = desc["crs"].as_str().unwrap_or(""),
                minx = desc["native_bounds"]["minx"].as_f64().unwrap_or(0.0),
                miny = desc["native_bounds"]["miny"].as_f64().unwrap_or(0.0),
                maxx = desc["native_bounds"]["maxx"].as_f64().unwrap_or(0.0),
                maxy = desc["native_bounds"]["maxy"].as_f64().unwrap_or(0.0),
            )
        }).collect::<Vec<_>>().join("\n")
    );

    Ok(HttpResponse::Ok()
        .content_type("text/xml")
        .body(xml))
}

async fn handle_get_legend_graphic(state: &AppState, request: &WmsRequest) -> Result<HttpResponse, GeoServerError> {
    let layer_name = request.layers.as_ref()
        .and_then(|l| l.first())
        .ok_or_else(|| GeoServerError::BadRequest("LAYER parameter required for GetLegendGraphic".to_string()))?;

    let layers_lock = state.layers.read().await;
    let styles_lock = state.styles.read().await;

    let layer = layers_lock.iter().find(|l| l.name == *layer_name)
        .ok_or_else(|| GeoServerError::NotFound(format!("Layer '{}' not found", layer_name)))?;

    let rules = get_layer_rules(request, &styles_lock, layer);
    let padding = 5u32;
    let icon_size = 20u32;
    let row_height = icon_size + 4;
    let total_height = if rules.is_empty() { row_height } else { (rules.len() as u32) * row_height + padding * 2 };
    let total_width = 40u32;

    let mut img = image::RgbaImage::new(total_width, total_height);
    for pixel in img.pixels_mut() {
        *pixel = image::Rgba([255, 255, 255, 255]);
    }

    for (idx, rule) in rules.iter().enumerate() {
        let y = padding + idx as u32 * row_height;
        let style = &rule.style;

        let swatch_x = (total_width - icon_size) / 2;
        let swatch_y = y + 2;

        if let Some(fill) = &style.fill {
            if let Some(color) = parse_color_opt(&fill.color) {
                for dy in 0..icon_size {
                    for dx in 0..icon_size {
                        let px = swatch_x + dx;
                        let py = swatch_y + dy;
                        if px < total_width && py < total_height {
                            img.put_pixel(px, py, image::Rgba(color));
                        }
                    }
                }
            }
        }
        if let Some(stroke) = &style.stroke {
            if let Some(color) = parse_color_opt(&stroke.color) {
                for dx in 0..icon_size {
                    let px = swatch_x + dx;
                    for py in [swatch_y, swatch_y + icon_size - 1] {
                        if px < total_width && py < total_height {
                            img.put_pixel(px, py, image::Rgba(color));
                        }
                    }
                }
                for dy in 0..icon_size {
                    let py = swatch_y + dy;
                    for px in [swatch_x, swatch_x + icon_size - 1] {
                        if px < total_width && py < total_height {
                            img.put_pixel(px, py, image::Rgba(color));
                        }
                    }
                }
            }
        }
    }

    let mut buffer = Cursor::new(Vec::new());
    img.write_to(&mut buffer, ImageFormat::Png)
        .map_err(|e| GeoServerError::RenderingError(e.to_string()))?;

    Ok(HttpResponse::Ok()
        .content_type("image/png")
        .body(buffer.into_inner()))
}

fn parse_color_opt(color: &str) -> Option<[u8; 4]> {
    if color.starts_with('#') {
        let hex = &color[1..];
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            return Some([r, g, b, 255]);
        }
    }
    None
}

fn reproject_geometry(geom: &GeoJsonGeometry, from_crs: &str, to_crs: &str) -> GeoJsonGeometry {
    let transformer = ProjectionTransformer::new(
        CoordinateReferenceSystem::from_epsg(from_crs),
        CoordinateReferenceSystem::from_epsg(to_crs),
    );
    match geom {
        GeoJsonGeometry::Point { coordinates } => {
            if coordinates.len() >= 2 {
                if let Ok((x, y)) = transformer.transform_point(coordinates[0], coordinates[1]) {
                    return GeoJsonGeometry::Point { coordinates: vec![x, y] };
                }
            }
            geom.clone()
        }
        GeoJsonGeometry::LineString { coordinates } => {
            let projected: Vec<Vec<f64>> = coordinates.iter()
                .filter_map(|c| {
                    if c.len() >= 2 {
                        transformer.transform_point(c[0], c[1]).ok()
                            .map(|(x, y)| vec![x, y])
                    } else {
                        None
                    }
                })
                .collect();
            if projected.len() == coordinates.len() {
                GeoJsonGeometry::LineString { coordinates: projected }
            } else {
                geom.clone()
            }
        }
        GeoJsonGeometry::Polygon { coordinates } => {
            let projected: Vec<Vec<Vec<f64>>> = coordinates.iter()
                .map(|ring| {
                    ring.iter()
                        .filter_map(|c| {
                            if c.len() >= 2 {
                                transformer.transform_point(c[0], c[1]).ok()
                                    .map(|(x, y)| vec![x, y])
                            } else {
                                None
                            }
                        })
                        .collect()
                })
                .collect();
            if projected.len() == coordinates.len()
                && projected.iter().zip(coordinates.iter()).all(|(p, o)| p.len() == o.len())
            {
                GeoJsonGeometry::Polygon { coordinates: projected }
            } else {
                geom.clone()
            }
        }
        _ => geom.clone(),
    }
}

fn calculate_openlayers_zoom(bounds: &Bounds, crs: &str) -> f64 {
    let world_width = match crs {
        "EPSG:3857" | "3857" | "EPSG:900913" | "900913" => 20037508.34 * 2.0,
        _ => 360.0,
    };
    let range = (bounds.maxx - bounds.minx).max(bounds.maxy - bounds.miny);
    if range <= 0.0 { return 1.0; }
    let zoom = (world_width / range).log2().max(0.0).min(20.0);
    zoom - 1.0
}

fn calculate_scale_denom(bounds: &Bounds, width: u32, height: u32, crs: &str) -> f64 {
    let res_x = (bounds.maxx - bounds.minx) / width as f64;
    let res_y = (bounds.maxy - bounds.miny) / height as f64;
    let ground_res = res_x.max(res_y);
    const PIXEL_SIZE: f64 = 0.00028;
    match crs {
        "EPSG:3857" | "3857" | "EPSG:900913" | "900913" => {
            ground_res / PIXEL_SIZE
        }
        _ => {
            let center_lat = (bounds.miny + bounds.maxy) / 2.0;
            let meters_per_degree = 111319.5 * center_lat.to_radians().cos().abs().max(0.01);
            ground_res * meters_per_degree / PIXEL_SIZE
        }
    }
}

fn parse_color(color: &str) -> [u8; 4] {
    if color.starts_with('#') && color.len() >= 7 {
        let r = u8::from_str_radix(&color[1..3], 16).unwrap_or(255);
        let g = u8::from_str_radix(&color[3..5], 16).unwrap_or(255);
        let b = u8::from_str_radix(&color[5..7], 16).unwrap_or(255);
        [r, g, b, 255]
    } else {
        [255, 255, 255, 255]
    }
}
