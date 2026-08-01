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
    pub format: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateStyleRequest {
    pub title: Option<String>,
    pub content: Option<String>,
    pub format: Option<String>,
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
    let meta = state.styles_meta.read().await;
    let content = styles.get(&style_name)
        .ok_or_else(|| GeoServerError::NotFound(format!("Style '{}' not found", style_name)))?;

    let format = meta.get(&style_name).map(|m| &m.format).unwrap_or(&crate::models::style::StyleFormat::SLD);
    let content_type = match format {
        crate::models::style::StyleFormat::SLD => "application/vnd.ogc.sld+xml",
        crate::models::style::StyleFormat::CSS => "text/css",
        crate::models::style::StyleFormat::YSLD => "text/yaml",
        crate::models::style::StyleFormat::MBStyle => "application/json",
    };

    Ok(HttpResponse::Ok()
        .content_type(content_type)
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

    let format = crate::models::style::detect_style_format(&body);

    let mut styles = state.styles.write().await;
    styles.insert(style_name.clone(), body.clone());

    let mut meta = state.styles_meta.write().await;
    meta.entry(style_name.clone()).and_modify(|m| { m.format = format.clone(); });

    // 持久化到存储
    if let Some(store) = &state.store {
        let ts = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let _ = store.create_style(&crate::store::StyleRecord {
            name: style_name.clone(),
            title: style_name.clone(),
            format: format.to_string(),
            is_builtin: meta.get(&style_name).map(|m| m.is_builtin).unwrap_or(false),
            content: body,
            created: ts.clone(),
            modified: ts,
        }).await;
    }

    Ok(HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
        "message": format!("Style '{}' updated", style_name),
    }))))
}

pub async fn list_styles(state: web::Data<AppState>) -> Result<HttpResponse, GeoServerError> {
    let styles = state.styles.read().await;
    let meta = state.styles_meta.read().await;
    let result: Vec<serde_json::Value> = styles.keys().map(|name| {
        let m = meta.get(name);
        let format = m.map(|m| m.format.to_string()).unwrap_or_else(|| "SLD".to_string());
        serde_json::json!({
            "name": name,
            "title": m.map(|m| m.title.as_str()).unwrap_or(name),
            "is_builtin": m.map(|m| m.is_builtin).unwrap_or(false),
            "format": format,
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

    let format = match body.format.as_deref() {
        Some("CSS") => crate::models::style::StyleFormat::CSS,
        Some("YSLD") => crate::models::style::StyleFormat::YSLD,
        Some("MBStyle") => crate::models::style::StyleFormat::MBStyle,
        _ => {
            if body.content.trim().is_empty() {
                crate::models::style::StyleFormat::SLD
            } else {
                crate::models::style::detect_style_format(&body.content)
            }
        }
    };

    state.add_style(&body.name, content.clone()).await;
    let meta_insert = crate::state::StyleMeta {
        title: title.clone(),
        is_builtin: false,
        format: format.clone(),
    };
    {
        let mut meta = state.styles_meta.write().await;
        meta.insert(body.name.clone(), meta_insert);
    }

    // 持久化到存储
    if let Some(store) = &state.store {
        let ts = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let _ = store.create_style(&crate::store::StyleRecord {
            name: body.name.clone(),
            title,
            format: format.to_string(),
            is_builtin: false,
            content,
            created: ts.clone(),
            modified: ts,
        }).await;
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

    let format = m.map(|m| m.format.to_string()).unwrap_or_else(|| "SLD".to_string());

    Ok(HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
        "name": name,
        "title": m.map(|m| m.title.as_str()).unwrap_or(name),
        "content": content,
        "is_builtin": m.map(|m| m.is_builtin).unwrap_or(false),
        "format": format,
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
        let detected = body.format.as_deref().map(|f| match f {
            "CSS" => crate::models::style::StyleFormat::CSS,
            "YSLD" => crate::models::style::StyleFormat::YSLD,
            "MBStyle" => crate::models::style::StyleFormat::MBStyle,
            _ => crate::models::style::detect_style_format(content),
        });
        if let Some(fmt) = detected {
            let mut meta = state.styles_meta.write().await;
            meta.entry(name.to_string()).and_modify(|m| { m.format = fmt; });
        }
    }
    if let Some(title) = &body.title {
        let mut meta = state.styles_meta.write().await;
        meta.entry(name.to_string()).and_modify(|m| { m.title = title.clone(); });
    }

    // 持久化到存储
    if let Some(store) = &state.store {
        let _ = store.update_style(
            name,
            body.title.clone(),
            body.format.clone(),
            body.content.clone(),
            None,
        ).await;
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

    // 从存储删除
    if let Some(store) = &state.store {
        let _ = store.delete_style(name).await;
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
        title: title.clone(),
        layers: layers.clone(),
        styles: vec![],
    };

    {
        let mut groups = state.layer_groups.write().await;
        groups.push(group);
    }

    // 持久化到存储
    if let Some(store) = &state.store {
        let ts = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let _ = store.create_layer_group(&crate::store::LayerGroupRecord {
            name: name.clone(),
            title,
            layers,
            styles: vec![],
            created: ts.clone(),
            modified: ts,
        }).await;
    }

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

    // 从存储删除
    if let Some(store) = &state.store {
        let _ = store.delete_layer_group(name).await;
    }

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

    // 获取 gridset 参数 (默认 EPSG:4326)
    let gridset = req.query_string()
        .split('&')
        .find_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            if parts.next()? == "gridset" { parts.next() } else { None }
        })
        .unwrap_or("EPSG:4326");

    // 1. 尝试从缓存获取
    if let Some(ref cache) = state.tile_cache {
        if let Some(cached) = cache.get(layer_name, gridset, z, x, y).await {
            return Ok(HttpResponse::Ok()
                .insert_header(("X-Tile-Cache", "HIT"))
                .content_type("image/png")
                .body(cached));
        }
    }

    // 2. 计算瓦片边界
    let tile_size = 256u32;
    let n = 2.0_f64.powi(z as i32);
    let minx = (x as f64 / n) * 360.0 - 180.0;
    let maxx = ((x + 1) as f64 / n) * 360.0 - 180.0;
    let sin_lat = |y: f64| -> f64 {
        let v = std::f64::consts::PI * (1.0 - 2.0 * y / n);
        v.cos().recip().ln().atan().to_degrees()
    };
    let miny = sin_lat(y as f64 + 1.0).max(-85.0511);
    let maxy = sin_lat(y as f64).min(85.0511);

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
    let meta_lock = state.styles_meta.read().await;
    let mut render_items = Vec::new();

    if let Some(layer) = layers_lock.iter().find(|l| l.name == layer_name) {
        let layer_crs = layer.srs.to_epsg();
        let needs_reproject = layer_crs != "EPSG:4326";
        let rules = get_style_rules(&styles_lock, &meta_lock, layer);

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
    drop(layers_lock);
    drop(styles_lock);
    drop(meta_lock);

    let img = renderer.render(render_items);

    let mut buffer = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buffer, image::ImageFormat::Png)
        .map_err(|e| GeoServerError::RenderingError(e.to_string()))?;

    let tile_data = buffer.into_inner();

    // 3. 写入缓存
    if let Some(ref cache) = state.tile_cache {
        cache.put(layer_name, gridset, z, x, y, &tile_data).await;
    }

    Ok(HttpResponse::Ok()
        .insert_header(("X-Tile-Cache", "MISS"))
        .content_type("image/png")
        .body(tile_data))
}

/// 清除指定图层的瓦片缓存
pub async fn clear_tile_cache(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let layer_name = req.match_info().get("layer").unwrap_or("");
    if let Some(ref cache) = state.tile_cache {
        let count = cache.clear_layer(layer_name).await
            .map_err(|e| GeoServerError::InternalError(format!("清除缓存失败: {}", e)))?;
        Ok(HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
            "message": format!("已清除图层 '{}' 的 {} 个缓存瓦片", layer_name, count),
            "cleared": count,
        }))))
    } else {
        Err(GeoServerError::InternalError("瓦片缓存未启用".to_string()))
    }
}

/// 获取缓存统计
pub async fn get_tile_cache_stats(
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    if let Some(ref cache) = state.tile_cache {
        let disk_stats = cache.calculate_disk_stats().await;
        Ok(HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
            "enabled": true,
            "hits": disk_stats.hits,
            "misses": disk_stats.misses,
            "hitRate": cache.hit_rate(),
            "totalTiles": disk_stats.total_tiles,
            "cacheSizeBytes": disk_stats.cache_size_bytes,
            "cacheSizeMb": format!("{:.2} MB", disk_stats.cache_size_bytes as f64 / 1_048_576.0),
        }))))
    } else {
        Ok(HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
            "enabled": false,
            "hits": 0,
            "misses": 0,
            "hitRate": 0.0,
            "totalTiles": 0,
            "cacheSizeBytes": 0,
        }))))
    }
}

pub fn get_style_rules(
    styles: &std::collections::HashMap<String, String>,
    meta: &std::collections::HashMap<String, crate::state::StyleMeta>,
    layer: &crate::models::Layer,
) -> Vec<crate::utils::sld_parser::ParsedRule> {
    let style_name = layer.styles.first().map(|s| &s.name).cloned().unwrap_or_default();
    let content = styles.get(&style_name);
    let format = meta.get(&style_name).map(|m| &m.format);

    match (content, format) {
        (Some(c), Some(fmt)) => parse_style_content(c.as_str(), fmt),
        (Some(c), None) => {
            let detected = crate::models::style::detect_style_format(c.as_str());
            parse_style_content(c.as_str(), &detected)
        }
        _ => crate::utils::sld_parser::parse_sld(&crate::utils::sld_parser::default_sld(&layer.name)),
    }
}

pub fn parse_style_content(
    content: &str,
    format: &crate::models::style::StyleFormat,
) -> Vec<crate::utils::sld_parser::ParsedRule> {
    match format {
        crate::models::style::StyleFormat::CSS => crate::utils::css_parser::parse_css(content),
        crate::models::style::StyleFormat::YSLD => crate::utils::ysld_parser::parse_ysld(content),
        crate::models::style::StyleFormat::MBStyle => crate::utils::mbstyle_parser::parse_mbstyle(content),
        crate::models::style::StyleFormat::SLD => crate::utils::sld_parser::parse_sld(content),
    }
}

pub fn calculate_tile_scale_denom(z: u32) -> f64 {
    let resolution = 156543.03 / 2.0_f64.powi(z as i32);
    resolution / 0.00028
}

pub fn reproject_geometry_helper(
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
