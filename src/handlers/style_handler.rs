use actix_web::{HttpRequest, HttpResponse, web};
use serde::Deserialize;
use crate::state::AppState;
use crate::models::{Bounds, GeoJsonGeometry, CoordinateReferenceSystem};
use crate::error::GeoServerError;
use super::rest_handler::ApiResponse;

#[derive(Debug, Deserialize)]
pub struct CreateStyleRequest {
    pub name: String,
    pub title: Option<String>,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateStyleRequest {
    pub title: Option<String>,
    pub content: Option<String>,
}

pub async fn get_layer_style(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let layer_name = req.match_info().get("layer").unwrap_or("");

    let style_name = {
        let layers = state.layers.read().await;
        layers.iter()
            .find(|l| l.name == layer_name)
            .and_then(|l| l.styles.first().map(|s| s.name.clone()))
            .ok_or_else(|| GeoServerError::NotFound(format!("Layer '{}' not found", layer_name)))?
    };

    let styles = state.styles.read().await;
    let content = styles.get(&style_name)
        .ok_or_else(|| GeoServerError::NotFound(format!("Style '{}' not found", style_name)))?;

    Ok(HttpResponse::Ok()
        .content_type("application/vnd.ogc.sld+xml")
        .body(content.clone()))
}

pub async fn put_layer_style(
    req: HttpRequest,
    body: String,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let layer_name = req.match_info().get("layer").unwrap_or("");

    let style_name = {
        let layers = state.layers.read().await;
        layers.iter()
            .find(|l| l.name == layer_name)
            .and_then(|l| l.styles.first().map(|s| s.name.clone()))
            .ok_or_else(|| GeoServerError::NotFound(format!("Layer '{}' not found", layer_name)))?
    };

    let mut styles = state.styles.write().await;
    styles.insert(style_name.clone(), body);

    Ok(HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
        "message": format!("Style '{}' updated", style_name),
    }))))
}

pub async fn list_styles(state: web::Data<AppState>) -> Result<HttpResponse, GeoServerError> {
    let styles = state.styles.read().await;
    let meta = state.styles_meta.read().await;
    let result: Vec<serde_json::Value> = styles.keys().map(|name| {
        let m = meta.get(name);
        serde_json::json!({
            "name": name,
            "title": m.map(|m| m.title.as_str()).unwrap_or(name),
            "is_builtin": m.map(|m| m.is_builtin).unwrap_or(false),
        })
    }).collect();
    Ok(HttpResponse::Ok().json(ApiResponse::success(result)))
}

pub async fn create_style(
    body: web::Json<CreateStyleRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    if body.name.is_empty() {
        return Err(GeoServerError::BadRequest("Style name is required".to_string()));
    }
    {
        let styles = state.styles.read().await;
        if styles.contains_key(&body.name) {
            return Err(GeoServerError::Conflict(format!("Style '{}' already exists", body.name)));
        }
    }
    let content = if body.content.trim().is_empty() {
        return Err(GeoServerError::BadRequest("Style content is required".to_string()));
    } else {
        body.content.clone()
    };
    let title = body.title.clone().unwrap_or_else(|| body.name.clone());

    state.add_style(&body.name, content).await;
    {
        let mut meta = state.styles_meta.write().await;
        meta.insert(body.name.clone(), crate::state::StyleMeta {
            title,
            is_builtin: false,
        });
    }

    Ok(HttpResponse::Created().json(ApiResponse::success(serde_json::json!({
        "name": body.name,
        "message": "Style created",
    }))))
}

pub async fn get_style_by_name(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let name = req.match_info().get("name").unwrap_or("");
    let content = state.get_style(name).await
        .ok_or_else(|| GeoServerError::NotFound(format!("Style '{}' not found", name)))?;

    let meta = state.styles_meta.read().await;
    let m = meta.get(name);

    Ok(HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
        "name": name,
        "title": m.map(|m| m.title.as_str()).unwrap_or(name),
        "content": content,
        "is_builtin": m.map(|m| m.is_builtin).unwrap_or(false),
    }))))
}

pub async fn update_style_by_name(
    req: HttpRequest,
    body: web::Json<UpdateStyleRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let name = req.match_info().get("name").unwrap_or("");

    {
        let styles = state.styles.read().await;
        if !styles.contains_key(name) {
            return Err(GeoServerError::NotFound(format!("Style '{}' not found", name)));
        }
    }

    if let Some(content) = &body.content {
        state.add_style(name, content.clone()).await;
    }
    if let Some(title) = &body.title {
        let mut meta = state.styles_meta.write().await;
        meta.entry(name.to_string()).and_modify(|m| { m.title = title.clone(); });
    }

    Ok(HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
        "message": format!("Style '{}' updated", name),
    }))))
}

pub async fn delete_style_by_name(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let name = req.match_info().get("name").unwrap_or("");

    {
        let styles = state.styles.read().await;
        if !styles.contains_key(name) {
            return Err(GeoServerError::NotFound(format!("Style '{}' not found", name)));
        }
    }

    {
        let mut styles = state.styles.write().await;
        styles.remove(name);
    }
    {
        let mut meta = state.styles_meta.write().await;
        meta.remove(name);
    }

    Ok(HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
        "message": format!("Style '{}' deleted", name),
    }))))
}

pub async fn list_layer_groups(state: web::Data<AppState>) -> Result<HttpResponse, GeoServerError> {
    let groups = state.layer_groups.read().await;
    let result: Vec<_> = groups.iter().map(|g| serde_json::json!({
        "name": g.name,
        "title": g.title,
        "layers": g.layers,
    })).collect();
    Ok(HttpResponse::Ok().json(ApiResponse::success(result)))
}

pub async fn get_layer_group(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let name = req.match_info().get("name").unwrap_or("");
    let groups = state.layer_groups.read().await;
    let group = groups.iter().find(|g| g.name == name)
        .ok_or_else(|| GeoServerError::NotFound(format!("Layer group '{}' not found", name)))?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
        "name": group.name,
        "title": group.title,
        "layers": group.layers,
        "styles": group.styles,
    }))))
}

pub async fn create_layer_group(
    body: web::Json<serde_json::Value>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let name = body.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let title = body.get("title").and_then(|v| v.as_str()).unwrap_or(&name).to_string();
    let layers: Vec<String> = body.get("layers")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();

    if name.is_empty() {
        return Err(GeoServerError::BadRequest("Layer group name is required".to_string()));
    }

    let group = crate::models::layer::LayerGroup {
        name: name.clone(),
        title,
        layers,
        styles: vec![],
    };

    let mut groups = state.layer_groups.write().await;
    groups.push(group);

    Ok(HttpResponse::Created().json(ApiResponse::success(serde_json::json!({
        "name": name,
        "message": "Layer group created",
    }))))
}

pub async fn delete_layer_group(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let name = req.match_info().get("name").unwrap_or("");
    let mut groups = state.layer_groups.write().await;
    let pos = groups.iter().position(|g| g.name == name)
        .ok_or_else(|| GeoServerError::NotFound(format!("Layer group '{}' not found", name)))?;
    groups.remove(pos);

    Ok(HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
        "message": format!("Layer group '{}' deleted", name),
    }))))
}

pub async fn get_tile(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let layer_name = req.match_info().get("layer").unwrap_or("");
    let z: u32 = req.match_info().get("z").and_then(|v| v.parse().ok()).unwrap_or(0);
    let x: u32 = req.match_info().get("x").and_then(|v| v.parse().ok()).unwrap_or(0);
    let y: u32 = req.match_info().get("y").and_then(|v| v.parse().ok()).unwrap_or(0);

    let tile_size = 256u32;
    let n = 2.0_f64.powi(z as i32);
    let minx = (x as f64 / n) * 360.0 - 180.0;
    let maxx = ((x + 1) as f64 / n) * 360.0 - 180.0;
    let miny = (y as f64 / n * std::f64::consts::PI).tan().asin().to_degrees();
    let minx = minx.min(maxx);
    let maxx = minx.max(maxx);

    let miny = if miny.is_finite() { miny } else { -85.0511 };
    let maxy = ((y + 1) as f64 / n * std::f64::consts::PI).tan().asin().to_degrees();
    let maxy = if maxy.is_finite() { maxy } else { 85.0511 };

    let bounds = Bounds::new(minx, miny, maxx, maxy);

    let options = crate::utils::rendering::RenderOptions {
        width: tile_size,
        height: tile_size,
        transparent: true,
        bg_color: None,
        format: crate::utils::rendering::RenderFormat::PNG,
    };

    let renderer = crate::utils::rendering::MapRenderer::new(options, bounds);

    let layers_lock = state.layers.read().await;
    let styles_lock = state.styles.read().await;
    let mut render_items = Vec::new();

    if let Some(layer) = layers_lock.iter().find(|l| l.name == layer_name) {
        let layer_crs = layer.srs.to_epsg();
        let needs_reproject = layer_crs != "EPSG:4326";
        let rules = get_style_rules(&styles_lock, layer);

        let features = crate::handlers::features::query_layer_features(
            state.get_ref(), &layer.name, None, None, None,
        ).await.unwrap_or_default();
        let scale_denom = calculate_tile_scale_denom(z);
        for feature in &features {
            let geom = if needs_reproject {
                reproject_geometry_helper(&feature.geometry, &layer_crs, "EPSG:4326")
            } else {
                feature.geometry.clone()
            };
            let style = crate::utils::sld_parser::resolve_style(&rules, feature, Some(scale_denom));
            render_items.push((geom, style));
        }
    }

    let img = renderer.render(render_items);

    let mut buffer = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buffer, image::ImageFormat::Png)
        .map_err(|e| GeoServerError::RenderingError(e.to_string()))?;

    Ok(HttpResponse::Ok()
        .content_type("image/png")
        .body(buffer.into_inner()))
}

fn get_style_rules(
    styles: &std::collections::HashMap<String, String>,
    layer: &crate::models::Layer,
) -> Vec<crate::utils::sld_parser::ParsedRule> {
    let style_name = layer.styles.first().map(|s| &s.name).cloned().unwrap_or_default();
    let sld_content = styles.get(&style_name).cloned();
    match sld_content {
        Some(xml) => crate::utils::sld_parser::parse_sld(&xml),
        None => crate::utils::sld_parser::parse_sld(&crate::utils::sld_parser::default_sld(&layer.name)),
    }
}

fn calculate_tile_scale_denom(z: u32) -> f64 {
    let resolution = 156543.03 / 2.0_f64.powi(z as i32);
    resolution / 0.00028
}

fn reproject_geometry_helper(
    geom: &GeoJsonGeometry,
    from_crs: &str,
    to_crs: &str,
) -> GeoJsonGeometry {
    use crate::utils::projection::ProjectionTransformer;
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
                        transformer.transform_point(c[0], c[1]).ok().map(|(x, y)| vec![x, y])
                    } else { None }
                })
                .collect();
            if projected.len() == coordinates.len() {
                GeoJsonGeometry::LineString { coordinates: projected }
            } else { geom.clone() }
        }
        GeoJsonGeometry::Polygon { coordinates } => {
            let projected: Vec<Vec<Vec<f64>>> = coordinates.iter()
                .map(|ring| ring.iter().filter_map(|c| {
                    if c.len() >= 2 {
                        transformer.transform_point(c[0], c[1]).ok().map(|(x, y)| vec![x, y])
                    } else { None }
                }).collect())
                .collect();
            if projected.len() == coordinates.len()
                && projected.iter().zip(coordinates.iter()).all(|(p, o)| p.len() == o.len())
            {
                GeoJsonGeometry::Polygon { coordinates: projected }
            } else { geom.clone() }
        }
        _ => geom.clone(),
    }
}
