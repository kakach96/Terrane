use actix_web::{HttpRequest, HttpResponse, web};
use crate::services::wms::{self, WmsRequest, WmsCapabilities};
use crate::error::GeoServerError;
use crate::state::AppState;
use quick_xml::se::to_string;
use std::io::Cursor;
use image::ImageFormat;

pub async fn handle_wms_request(
    _req: HttpRequest,
    query: web::Query<Vec<(String, String)>>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let params: Vec<(String, String)> = query.into_inner();

    let wms_request = wms::parse_wms_request(&params)?;
    
    match wms_request.request {
        wms::WmsOperation::GetCapabilities => handle_get_capabilities(&state, &wms_request).await,
        wms::WmsOperation::GetMap => handle_get_map(&state, &wms_request).await,
        wms::WmsOperation::GetFeatureInfo => handle_get_feature_info(&state, &wms_request).await,
        wms::WmsOperation::GetLegendGraphic => handle_get_legend_graphic(&state, &wms_request).await,
        _ => Err(GeoServerError::BadRequest("Operation not implemented".to_string())),
    }
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
    
    let layers = request.layers.as_ref().unwrap();
    let width = request.width.unwrap_or(512) as u32;
    let height = request.height.unwrap_or(512) as u32;
    let format = request.format.as_ref().unwrap();
    
    let bbox = request.bbox.as_ref().unwrap();
    let bounds = crate::models::Bounds::new(bbox.minx, bbox.miny, bbox.maxx, bbox.maxy);
    
    let mut img = image::RgbaImage::new(width, height);
    
    if request.transparent.unwrap_or(false) {
        for pixel in img.pixels_mut() {
            *pixel = image::Rgba([0, 0, 0, 0]);
        }
    } else {
        let bg = request.bgcolor.as_ref().map(|c| parse_color(c)).unwrap_or([255, 255, 255, 255]);
        for pixel in img.pixels_mut() {
            *pixel = image::Rgba(bg);
        }
    }
    
    let layers_lock = state.layers.read().await;
    for layer_name in layers {
        if let Some(layer) = layers_lock.iter().find(|l| &l.name == layer_name) {
            if let Some(features) = state.get_layer_features(&layer.name).await {
                for feature in features {
                    render_feature_to_image(&mut img, &feature.geometry, &bounds, width, height, &layer.styles);
                }
            }
        }
    }
    
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

async fn handle_get_feature_info(state: &AppState, request: &WmsRequest) -> Result<HttpResponse, GeoServerError> {
    let _i = request.i.unwrap_or(0.0) as u32;
    let _j = request.j.unwrap_or(0.0) as u32;
    
    let info_format = request.info_format.as_deref().unwrap_or("text/plain");
    
    let mut features_info = Vec::new();
    
    let layers_lock = state.layers.read().await;
    if let Some(query_layers) = &request.query_layers {
        for layer_name in query_layers {
            if let Some(layer) = layers_lock.iter().find(|l| &l.name == layer_name) {
                if let Some(features) = state.get_layer_features(&layer.name).await {
                    for feature in features {
                        features_info.push(format!("Layer: {}, Feature: {:?}", layer.name, feature.id));
                    }
                }
            }
        }
    }
    
    let response = match info_format {
        "application/json" => {
            serde_json::to_string_pretty(&features_info)
                .map_err(|e| GeoServerError::ServiceError(e.to_string()))?
        }
        "text/html" => {
            format!(
                "<html><body><h1>Feature Information</h1><pre>{:?}</pre></body></html>",
                features_info
            )
        }
        _ => {
            features_info.join("\n")
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

async fn handle_get_legend_graphic(_state: &AppState, _request: &WmsRequest) -> Result<HttpResponse, GeoServerError> {
    let width = 32u32;
    let height = 32u32;
    let mut img = image::RgbaImage::new(width, height);
    
    for pixel in img.pixels_mut() {
        *pixel = image::Rgba([128, 128, 128, 255]);
    }
    
    for y in 0..height {
        for x in 0..width {
            if (x as i32 - 16).abs() <= 12 && (y as i32 - 16).abs() <= 12 {
                img.put_pixel(x, y, image::Rgba([0, 0, 0, 255]));
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

fn render_feature_to_image(
    img: &mut image::RgbaImage,
    geometry: &crate::models::GeoJsonGeometry,
    bounds: &crate::models::Bounds,
    width: u32,
    height: u32,
    _styles: &[crate::models::StyleRef],
) {
    let fill_color = [100, 100, 100, 255];
    let stroke_color = [0, 0, 0, 255];
    
    match geometry {
        crate::models::GeoJsonGeometry::Point { coordinates } => {
            if coordinates.len() >= 2 {
                let (px, py) = world_to_pixel(coordinates[0], coordinates[1], bounds, width, height);
                draw_point(img, px, py, &fill_color, 4);
            }
        }
        crate::models::GeoJsonGeometry::LineString { coordinates } => {
            for window in coordinates.windows(2) {
                if let [c1, c2] = window {
                    if c1.len() >= 2 && c2.len() >= 2 {
                        let (x1, y1) = world_to_pixel(c1[0], c1[1], bounds, width, height);
                        let (x2, y2) = world_to_pixel(c2[0], c2[1], bounds, width, height);
                        draw_line(img, x1, y1, x2, y2, &stroke_color, 1);
                    }
                }
            }
        }
        crate::models::GeoJsonGeometry::Polygon { coordinates } => {
            if let Some(exterior) = coordinates.first() {
                for window in exterior.windows(2) {
                    if let [c1, c2] = window {
                        if c1.len() >= 2 && c2.len() >= 2 {
                            let (x1, y1) = world_to_pixel(c1[0], c1[1], bounds, width, height);
                            let (x2, y2) = world_to_pixel(c2[0], c2[1], bounds, width, height);
                            draw_line(img, x1, y1, x2, y2, &stroke_color, 1);
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

fn world_to_pixel(x: f64, y: f64, bounds: &crate::models::Bounds, width: u32, height: u32) -> (i32, i32) {
    let px = ((x - bounds.minx) / (bounds.maxx - bounds.minx)) * width as f64;
    let py = height as f64 - ((y - bounds.miny) / (bounds.maxy - bounds.miny)) * height as f64;
    (px as i32, py as i32)
}

fn draw_point(img: &mut image::RgbaImage, cx: i32, cy: i32, color: &[u8; 4], radius: i32) {
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            if dx * dx + dy * dy <= radius * radius {
                let x = cx + dx;
                let y = cy + dy;
                if x >= 0 && x < img.width() as i32 && y >= 0 && y < img.height() as i32 {
                    img.put_pixel(x as u32, y as u32, image::Rgba(*color));
                }
            }
        }
    }
}

fn draw_line(img: &mut image::RgbaImage, x0: i32, y0: i32, x1: i32, y1: i32, color: &[u8; 4], width: i32) {
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut x = x0;
    let mut y = y0;
    
    loop {
        for wdx in -width..=width {
            for wdy in -width..=width {
                let px = x + wdx;
                let py = y + wdy;
                if px >= 0 && px < img.width() as i32 && py >= 0 && py < img.height() as i32 {
                    img.put_pixel(px as u32, py as u32, image::Rgba(*color));
                }
            }
        }
        
        if x == x1 && y == y1 {
            break;
        }
        
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
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
