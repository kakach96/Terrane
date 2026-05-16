use actix_web::{HttpResponse, web, HttpRequest};
use serde::{Deserialize, Serialize};
use crate::state::AppState;
use crate::models::{Feature, FeatureCollection, GeoJsonGeometry, Layer, CoordinateReferenceSystem, BoundingBox, Bounds};
use crate::config::WorkspaceConfig;
use crate::error::GeoServerError;
use crate::utils::rendering;

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub message: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        ApiResponse {
            success: true,
            data: Some(data),
            message: None,
        }
    }
    
    pub fn error(message: String) -> ApiResponse<()> {
        ApiResponse {
            success: false,
            data: None,
            message: Some(message),
        }
    }
}

pub async fn list_layers(state: web::Data<AppState>) -> Result<HttpResponse, GeoServerError> {
    let layers = state.list_layers().await;
    let result: Vec<_> = layers.iter()
        .map(|l| serde_json::json!({
            "name": l.name,
            "title": l.title,
            "workspace": l.workspace,
            "store": l.store,
            "srs": l.srs.to_epsg(),
            "bounds": {
                "minx": l.native_bounds.bounds.minx,
                "miny": l.native_bounds.bounds.miny,
                "maxx": l.native_bounds.bounds.maxx,
                "maxy": l.native_bounds.bounds.maxy,
            },
            "enabled": l.enabled,
        }))
        .collect();
    
    Ok(HttpResponse::Ok().json(ApiResponse::success(result)))
}

pub async fn get_layer(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let layer_name = req.match_info().get("layer").unwrap_or("");
    
    if let Some(layer) = state.get_layer(layer_name).await {
        let response = serde_json::json!({
            "name": layer.name,
            "title": layer.title,
            "abstract": layer.abstract_text,
            "workspace": layer.workspace,
            "store": layer.store,
            "srs": layer.srs.to_epsg(),
            "native_bounds": {
                "crs": layer.native_bounds.crs.to_epsg(),
                "bounds": {
                    "minx": layer.native_bounds.bounds.minx,
                    "miny": layer.native_bounds.bounds.miny,
                    "maxx": layer.native_bounds.bounds.maxx,
                    "maxy": layer.native_bounds.bounds.maxy,
                },
            },
            "lat_lon_bounds": {
                "crs": layer.lat_lon_bounds.crs.to_epsg(),
                "bounds": {
                    "minx": layer.lat_lon_bounds.bounds.minx,
                    "miny": layer.lat_lon_bounds.bounds.miny,
                    "maxx": layer.lat_lon_bounds.bounds.maxx,
                    "maxy": layer.lat_lon_bounds.bounds.maxy,
                },
            },
            "styles": layer.styles,
            "enabled": layer.enabled,
        });
        
        Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
    } else {
        Err(GeoServerError::NotFound(format!("Layer '{}' not found", layer_name)))
    }
}

#[derive(Debug, Deserialize)]
pub struct FeatureQuery {
    pub bbox: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

pub async fn get_layer_features(
    req: HttpRequest,
    query: web::Query<FeatureQuery>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let layer_name = req.match_info().get("layer").unwrap_or("");
    
    let mut features = state.get_layer_features(layer_name).await
        .unwrap_or_else(Vec::new);
    
    if let Some(ref bbox_str) = query.bbox {
        let parts: Vec<f64> = bbox_str.split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        if parts.len() == 4 {
            let (minx, miny, maxx, maxy) = (parts[0], parts[1], parts[2], parts[3]);
            features.retain(|f| {
                if let GeoJsonGeometry::Point { coordinates } = &f.geometry {
                    if coordinates.len() >= 2 {
                        let x = coordinates[0];
                        let y = coordinates[1];
                        return x >= minx && x <= maxx && y >= miny && y <= maxy;
                    }
                }
                true
            });
        }
    }
    
    let total_count = features.len();
    
    if let Some(offset) = query.offset {
        features = features.into_iter().skip(offset as usize).collect();
    }
    
    if let Some(limit) = query.limit {
        features = features.into_iter().take(limit as usize).collect();
    }
    
    let response = serde_json::json!({
        "type": "FeatureCollection",
        "totalFeatures": total_count,
        "features": features,
    });
    
    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .json(response))
}

#[derive(Debug, Deserialize)]
pub struct CreateFeatureRequest {
    pub geometry: GeoJsonGeometry,
    pub properties: std::collections::HashMap<String, serde_json::Value>,
}

pub async fn create_feature(
    req: HttpRequest,
    body: web::Json<CreateFeatureRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let layer_name = req.match_info().get("layer").unwrap_or("");
    
    let properties: std::collections::HashMap<String, crate::models::PropertyValue> = body.properties
        .iter()
        .map(|(k, v)| {
            let value = match v {
                serde_json::Value::String(s) => crate::models::PropertyValue::String(s.clone()),
                serde_json::Value::Number(n) => {
                    if let Some(i) = n.as_i64() {
                        crate::models::PropertyValue::Integer(i)
                    } else if let Some(f) = n.as_f64() {
                        crate::models::PropertyValue::Number(f)
                    } else {
                        crate::models::PropertyValue::String(n.to_string())
                    }
                }
                serde_json::Value::Bool(b) => crate::models::PropertyValue::Boolean(*b),
                serde_json::Value::Null => crate::models::PropertyValue::Null,
                _ => crate::models::PropertyValue::String(v.to_string()),
            };
            (k.clone(), value)
        })
        .collect();
    
    let feature = Feature::new(body.geometry.clone(), properties);
    
    state.add_feature(layer_name, feature.clone()).await;
    
    Ok(HttpResponse::Created().json(serde_json::json!({
        "type": "Feature",
        "id": feature.id,
        "geometry": feature.geometry,
        "properties": feature.properties,
    })))
}

pub async fn get_feature(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let layer_name = req.match_info().get("layer").unwrap_or("");
    let feature_id = req.match_info().get("feature").unwrap_or("");
    
    if let Some(features) = state.get_layer_features(layer_name).await {
        if let Some(feature) = features.iter().find(|f| f.id == feature_id) {
            return Ok(HttpResponse::Ok().json(serde_json::json!({
                "type": "Feature",
                "id": feature.id,
                "geometry": feature.geometry,
                "properties": feature.properties,
            })));
        }
    }
    
    Err(GeoServerError::NotFound(format!("Feature '{}' not found", feature_id)))
}

pub async fn delete_feature(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let layer_name = req.match_info().get("layer").unwrap_or("");
    let feature_id = req.match_info().get("feature").unwrap_or("");
    
    let mut features_map = state.features.write().await;
    if let Some(features) = features_map.get_mut(layer_name) {
        if let Some(pos) = features.iter().position(|f| f.id == feature_id) {
            features.remove(pos);
            return Ok(HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
                "deleted": true,
                "feature_id": feature_id,
            }))));
        }
    }
    
    Err(GeoServerError::NotFound(format!("Feature '{}' not found", feature_id)))
}

#[derive(Debug, Deserialize)]
pub struct GeoJsonUploadRequest {
    pub name: String,
    pub title: String,
}

pub async fn upload_geojson(
    body: web::Json<serde_json::Value>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let fc = if let Some(fc) = body.get("FeatureCollection") {
        serde_json::from_value::<FeatureCollection>(fc.clone())
            .map_err(|e| GeoServerError::BadRequest(format!("Invalid GeoJSON: {}", e)))?
    } else {
        serde_json::from_value::<FeatureCollection>(body.clone())
            .map_err(|e| GeoServerError::BadRequest(format!("Invalid GeoJSON: {}", e)))?
    };
    
    let layer_name = fc.features.first()
        .and_then(|f| f.properties.get("layer_name"))
        .and_then(|v| {
            if let crate::models::PropertyValue::String(s) = v {
                Some(s.as_str())
            } else {
                None
            }
        })
        .unwrap_or("uploaded")
        .to_string();
    
    state.add_layer_features(&layer_name, fc.features.clone()).await;
    
    Ok(HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
        "layer": layer_name,
        "features_imported": fc.features.len(),
    }))))
}

pub async fn list_workspaces(state: web::Data<AppState>) -> Result<HttpResponse, GeoServerError> {
    let layers = state.list_layers().await;
    
    let workspace_stats: std::collections::HashMap<String, (usize, bool)> = layers.iter()
        .fold(std::collections::HashMap::new(), |mut acc, layer| {
            let entry = acc.entry(layer.workspace.clone()).or_insert((0, true));
            entry.0 += 1;
            entry.1 = entry.1 && layer.enabled;
            acc
        });
    
    let mut workspaces: Vec<_> = state.config.workspaces.iter()
        .map(|w| {
            let (layer_count, enabled) = workspace_stats.get(&w.name).copied().unwrap_or((0, true));
            serde_json::json!({
                "name": w.name,
                "title": w.name,
                "uri": w.uri,
                "enabled": enabled,
                "layerCount": layer_count,
                "description": format!("Workspace '{}'", w.name),
            })
        })
        .collect();
    
    let default_workspace = serde_json::json!({
        "name": "default",
        "title": "默认工作空间",
        "uri": "http://localhost:8080/geoserver/workspaces/default",
        "enabled": true,
        "layerCount": workspace_stats.get("default").map(|(c, _)| *c).unwrap_or(0),
        "description": "系统默认工作空间",
    });
    
    if !workspaces.iter().any(|w| w["name"] == "default") {
        workspaces.insert(0, default_workspace);
    }
    
    Ok(HttpResponse::Ok().json(ApiResponse::success(workspaces)))
}

#[derive(Debug, Deserialize)]
pub struct CreateWorkspaceRequest {
    pub name: String,
    pub title: Option<String>,
    pub description: Option<String>,
}

pub async fn create_workspace(
    body: web::Json<CreateWorkspaceRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    if state.config.workspaces.iter().any(|w| w.name == body.name) {
        return Err(GeoServerError::Conflict(format!(
            "Workspace '{}' already exists",
            body.name
        )));
    }
    
    let new_workspace = WorkspaceConfig {
        name: body.name.clone(),
        uri: format!("http://localhost:8080/geoserver/workspaces/{}", body.name),
        stores: vec![],
    };
    
    let mut config = state.config.clone();
    config.workspaces.push(new_workspace);
    
    let response = serde_json::json!({
        "name": body.name,
        "title": body.title.clone().unwrap_or(body.name.clone()),
        "enabled": true,
        "layerCount": 0,
        "description": body.description.clone().unwrap_or_else(|| format!("Workspace '{}'", body.name)),
    });
    
    Ok(HttpResponse::Created().json(ApiResponse::success(response)))
}

pub async fn get_workspace(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let workspace_name = req.match_info().get("workspace").unwrap_or("");
    
    if let Some(workspace) = state.config.workspaces.iter().find(|w| w.name == workspace_name) {
        let layers = state.list_layers().await;
        let layer_count = layers.iter()
            .filter(|l| l.workspace == workspace_name)
            .count();
        let enabled = layers.iter()
            .filter(|l| l.workspace == workspace_name)
            .all(|l| l.enabled);
        
        let response = serde_json::json!({
            "name": workspace.name,
            "title": workspace.name,
            "uri": workspace.uri,
            "enabled": enabled,
            "layerCount": layer_count,
            "description": format!("Workspace '{}'", workspace.name),
        });
        
        Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
    } else {
        Err(GeoServerError::NotFound(format!(
            "Workspace '{}' not found",
            workspace_name
        )))
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateWorkspaceRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub enabled: Option<bool>,
}

pub async fn update_workspace(
    req: HttpRequest,
    body: web::Json<UpdateWorkspaceRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let workspace_name = req.match_info().get("workspace").unwrap_or("");
    
    let exists = state.config.workspaces.iter().any(|w| w.name == workspace_name);
    
    if !exists && workspace_name != "default" {
        return Err(GeoServerError::NotFound(format!(
            "Workspace '{}' not found",
            workspace_name
        )));
    }
    
    if let Some(enabled) = body.enabled {
        let layers = state.list_layers().await;
        for layer in layers {
            if layer.workspace == workspace_name {
                let updates = crate::state::LayerUpdates {
                    title: None,
                    abstract_text: None,
                    enabled: Some(enabled),
                };
                state.update_layer(&layer.name, updates).await;
            }
        }
    }
    
    Ok(HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
        "message": format!("Workspace '{}' updated successfully", workspace_name),
    }))))
}

pub async fn delete_workspace(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let workspace_name = req.match_info().get("workspace").unwrap_or("");
    
    if workspace_name == "default" {
        return Err(GeoServerError::BadRequest(
            "Cannot delete the default workspace".to_string(),
        ));
    }
    
    let layers = state.list_layers().await;
    let layer_count = layers.iter()
        .filter(|l| l.workspace == workspace_name)
        .count();
    
    if layer_count > 0 {
        return Err(GeoServerError::Conflict(format!(
            "Cannot delete workspace '{}' with {} layers",
            workspace_name, layer_count
        )));
    }
    
    let mut config = state.config.clone();
    config.workspaces.retain(|w| w.name != workspace_name);
    
    Ok(HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
        "message": format!("Workspace '{}' deleted successfully", workspace_name),
    }))))
}

pub async fn get_server_status(state: web::Data<AppState>) -> Result<HttpResponse, GeoServerError> {
    use sysinfo::System;
    
    let mut sys = System::new_all();
    sys.refresh_all();
    
    let layers = state.list_layers().await;
    let enabled_layers = layers.iter().filter(|l| l.enabled).count();
    
    let total_memory = sys.total_memory() / 1024 / 1024;
    let used_memory = sys.used_memory() / 1024 / 1024;
    let memory_percent = if total_memory > 0 {
        (used_memory * 100 / total_memory) as u8
    } else {
        0
    };
    
    let cpus = sys.cpus();
    let cpu_usage = if !cpus.is_empty() {
        cpus.iter().map(|c| c.cpu_usage() as u64).sum::<u64>() / cpus.len() as u64
    } else {
        0
    } as u8;
    
    let response = serde_json::json!({
        "uptime": state.get_uptime(),
        "memory": {
            "used": used_memory,
            "total": total_memory,
            "percent": memory_percent,
        },
        "cpu": cpu_usage,
        "requests": state.request_count.load(std::sync::atomic::Ordering::Relaxed),
        "errors": state.error_count.load(std::sync::atomic::Ordering::Relaxed),
        "layerCount": layers.len(),
        "enabledLayers": enabled_layers,
        "workspaceCount": std::collections::HashSet::<&String>::from_iter(
            layers.iter().map(|l| &l.workspace)
        ).len(),
    });
    
    Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
}

pub async fn health_check() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy",
        "service": "rust-geoserver",
        "version": "0.1.0",
    }))
}

#[derive(Debug, Deserialize)]
pub struct CreateLayerRequest {
    pub name: String,
    pub title: String,
    pub workspace: String,
    pub store: String,
    pub srs: Option<String>,
    pub minx: Option<f64>,
    pub miny: Option<f64>,
    pub maxx: Option<f64>,
    pub maxy: Option<f64>,
    #[serde(rename = "abstract")]
    pub abstract_text: Option<String>,
}

pub async fn create_layer(
    body: web::Json<CreateLayerRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let srs = body.srs.clone()
        .map(|s| CoordinateReferenceSystem::from_epsg(&s))
        .unwrap_or(CoordinateReferenceSystem::EPSG4326);
    
    let bounds = if let (Some(minx), Some(miny), Some(maxx), Some(maxy)) = 
        (body.minx, body.miny, body.maxx, body.maxy) {
        BoundingBox::new(srs.clone(), Bounds::new(minx, miny, maxx, maxy))
    } else {
        BoundingBox::world(srs.clone())
    };
    
    let mut layer = Layer::new(
        body.name.clone(),
        body.title.clone(),
        body.workspace.clone(),
        body.store.clone(),
        srs,
    ).with_bounds(bounds);
    
    layer.abstract_text = body.abstract_text.clone();
    
    state.add_layer(layer.clone()).await;
    
    let response = serde_json::json!({
        "name": layer.name,
        "title": layer.title,
        "workspace": layer.workspace,
        "store": layer.store,
        "srs": layer.srs.to_epsg(),
        "bounds": {
            "minx": layer.native_bounds.bounds.minx,
            "miny": layer.native_bounds.bounds.miny,
            "maxx": layer.native_bounds.bounds.maxx,
            "maxy": layer.native_bounds.bounds.maxy,
        },
        "enabled": layer.enabled,
        "message": "Layer created successfully",
    });
    
    Ok(HttpResponse::Created().json(ApiResponse::success(response)))
}

#[derive(Debug, Deserialize)]
pub struct UpdateLayerRequest {
    pub title: Option<String>,
    #[serde(rename = "abstract")]
    pub abstract_text: Option<String>,
    pub enabled: Option<bool>,
}

pub async fn update_layer(
    req: HttpRequest,
    body: web::Json<UpdateLayerRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let layer_name = req.match_info().get("layer").unwrap_or("");
    
    let updates = crate::state::LayerUpdates {
        title: body.title.clone(),
        abstract_text: body.abstract_text.clone(),
        enabled: body.enabled,
    };
    
    if state.update_layer(layer_name, updates).await {
        Ok(HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
            "message": format!("Layer '{}' updated successfully", layer_name),
        }))))
    } else {
        Err(GeoServerError::NotFound(format!("Layer '{}' not found", layer_name)))
    }
}

pub async fn delete_layer(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let layer_name = req.match_info().get("layer").unwrap_or("");
    
    if state.delete_layer(layer_name).await {
        Ok(HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
            "message": format!("Layer '{}' deleted successfully", layer_name),
        }))))
    } else {
        Err(GeoServerError::NotFound(format!("Layer '{}' not found", layer_name)))
    }
}

#[derive(Debug, Deserialize)]
pub struct PreviewRequest {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub format: Option<String>,
}

pub async fn preview_layer(
    req: HttpRequest,
    query: web::Query<PreviewRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let layer_name = req.match_info().get("layer").unwrap_or("");
    
    if state.get_layer(layer_name).await.is_none() {
        return Err(GeoServerError::NotFound(format!("Layer '{}' not found", layer_name)));
    }
    
    let width = query.width.unwrap_or(512);
    let height = query.height.unwrap_or(512);
    let format = query.format.clone().unwrap_or_else(|| "png".to_string());
    
    let features = state.get_layer_features(layer_name).await.unwrap_or_default();
    
    let image_data = rendering::render_map(&features, width, height);
    
    let content_type = match format.as_str() {
        "jpeg" | "jpg" => "image/jpeg",
        "gif" => "image/gif",
        _ => "image/png",
    };
    
    Ok(HttpResponse::Ok()
        .content_type(content_type)
        .body(image_data))
}
