use actix_web::{HttpResponse, web, HttpRequest};
use serde::{Deserialize, Serialize};
use crate::state::AppState;
use crate::models::{Feature, FeatureCollection, GeoJsonGeometry, Layer, CoordinateReferenceSystem, Bounds};
use crate::config::WorkspaceConfig;
use crate::error::GeoServerError;
use crate::utils::rendering;
use std::time::Instant;

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
    if let Some(store) = &state.store {
        match store.get_all_layers().await {
            Ok(layers) => {
                let result: Vec<_> = layers.iter()
                    .map(|l| serde_json::json!({
                        "name": l.name,
                        "title": l.title,
                        "workspace": l.workspace,
                        "store": l.store,
                        "native_name": l.native_name,
                        "srs": l.srs,
                        "bounds": {
                            "minx": l.minx,
                            "miny": l.miny,
                            "maxx": l.maxx,
                            "maxy": l.maxy,
                        },
                        "enabled": l.enabled,
                    }))
                    .collect();

                Ok(HttpResponse::Ok().json(ApiResponse::success(result)))
            }
            Err(e) => {
                eprintln!("Failed to list layers: {}", e);
                Err(GeoServerError::InternalError("Failed to list layers".to_string()))
            }
        }
    } else {
        let layers = state.list_layers().await;
        let result: Vec<_> = layers.iter()
            .map(|l| serde_json::json!({
                "name": l.name,
                "title": l.title,
                "workspace": l.workspace,
                "store": l.store,
                "native_name": l.native_name,
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
}

pub async fn get_layer(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let layer_name = req.match_info().get("layer").unwrap_or("");

    if let Some(store) = &state.store {
        match store.get_layer(layer_name).await {
            Ok(Some(layer)) => {
                let response = serde_json::json!({
                    "name": layer.name,
                    "title": layer.title,
                    "abstract": layer.abstract_text,
                    "workspace": layer.workspace,
                    "store": layer.store,
                    "native_name": layer.native_name,
                    "srs": layer.srs,
                    "native_bounds": {
                        "crs": layer.srs,
                        "bounds": {
                            "minx": layer.minx,
                            "miny": layer.miny,
                            "maxx": layer.maxx,
                            "maxy": layer.maxy,
                        },
                    },
                    "lat_lon_bounds": {
                        "crs": "EPSG:4326",
                        "bounds": {
                            "minx": layer.minx,
                            "miny": layer.miny,
                            "maxx": layer.maxx,
                            "maxy": layer.maxy,
                        },
                    },
                    "styles": [],
                    "enabled": layer.enabled,
                });

                Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
            }
            Ok(None) => {
                Err(GeoServerError::NotFound(format!("Layer '{}' not found", layer_name)))
            }
            Err(e) => {
                eprintln!("Failed to get layer: {}", e);
                Err(GeoServerError::InternalError("Failed to get layer".to_string()))
            }
        }
    } else {
        if let Some(layer) = state.get_layer(layer_name).await {
            let response = serde_json::json!({
                "name": layer.name,
                "title": layer.title,
                "abstract": layer.abstract_text,
                "workspace": layer.workspace,
                "store": layer.store,
                "native_name": layer.native_name,
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
pub struct UpdateFeatureRequest {
    pub geometry: Option<GeoJsonGeometry>,
    pub properties: Option<std::collections::HashMap<String, serde_json::Value>>,
}

pub async fn update_feature(
    req: HttpRequest,
    body: web::Json<UpdateFeatureRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let layer_name = req.match_info().get("layer").unwrap_or("");
    let feature_id = req.match_info().get("feature").unwrap_or("");
    
    let mut features_map = state.features.write().await;
    if let Some(features) = features_map.get_mut(layer_name) {
        if let Some(feature) = features.iter_mut().find(|f| f.id == feature_id) {
            if let Some(new_geometry) = &body.geometry {
                feature.geometry = new_geometry.clone();
            }
            
            if let Some(new_properties) = &body.properties {
                for (key, value) in new_properties {
                    let prop_value = match value {
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
                        _ => crate::models::PropertyValue::String(value.to_string()),
                    };
                    feature.properties.insert(key.clone(), prop_value);
                }
            }
            
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

    let base_url = format!("http://{}:{}{}", state.config.server.host, state.config.server.port, state.config.server.api_context);
    let default_workspace = serde_json::json!({
        "name": "default",
        "title": "默认工作空间",
        "uri": format!("{}/workspaces/default", base_url),
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
    
    let base_url = format!("http://{}:{}{}", state.config.server.host, state.config.server.port, state.config.server.api_context);
    let new_workspace = WorkspaceConfig {
        name: body.name.clone(),
        uri: format!("{}/workspaces/{}", base_url, body.name),
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
    pub native_name: Option<String>,
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
        .unwrap_or_else(|| "EPSG:4326".to_string());

    let (minx, miny, maxx, maxy) = if let (Some(x1), Some(y1), Some(x2), Some(y2)) =
        (body.minx, body.miny, body.maxx, body.maxy) {
        (x1, y1, x2, y2)
    } else {
        (-180.0, -90.0, 180.0, 90.0)
    };

    if let Some(store) = &state.store {
        let layer = crate::store::sqlite_store::Layer {
            name: body.name.clone(),
            title: body.title.clone(),
            workspace: body.workspace.clone(),
            store: body.store.clone(),
            srs: srs.clone(),
            abstract_text: body.abstract_text.clone(),
            native_name: body.native_name.clone(),
            enabled: true,
            minx,
            miny,
            maxx,
            maxy,
            created: String::new(),
            modified: String::new(),
        };

        match store.create_layer(&layer).await {
            Ok(created_layer) => {
                state.add_layer(Layer::new(
                    created_layer.name.clone(),
                    created_layer.title.clone(),
                    created_layer.workspace.clone(),
                    created_layer.store.clone(),
                    CoordinateReferenceSystem::from_epsg(&created_layer.srs),
                )).await;

                let response = serde_json::json!({
                    "name": created_layer.name,
                    "title": created_layer.title,
                    "workspace": created_layer.workspace,
                    "store": created_layer.store,
                    "native_name": created_layer.native_name,
                    "srs": created_layer.srs,
                    "bounds": {
                        "minx": created_layer.minx,
                        "miny": created_layer.miny,
                        "maxx": created_layer.maxx,
                        "maxy": created_layer.maxy,
                    },
                    "enabled": created_layer.enabled,
                    "message": "Layer created successfully",
                });

                Ok(HttpResponse::Created().json(ApiResponse::success(response)))
            }
            Err(e) => {
                eprintln!("Failed to create layer: {}", e);
                Err(GeoServerError::InternalError("Failed to create layer".to_string()))
            }
        }
    } else {
        Err(GeoServerError::InternalError("Database not available".to_string()))
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateLayerRequest {
    pub title: Option<String>,
    #[serde(rename = "abstract")]
    pub abstract_text: Option<String>,
    pub native_name: Option<String>,
    pub enabled: Option<bool>,
}

pub async fn update_layer(
    req: HttpRequest,
    body: web::Json<UpdateLayerRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let layer_name = req.match_info().get("layer").unwrap_or("");

    if let Some(store) = &state.store {
        match store.update_layer(
            layer_name,
            body.title.clone(),
            body.abstract_text.clone(),
            body.native_name.clone(),
            body.enabled,
        ).await {
            Ok(_) => {
                Ok(HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
                    "message": format!("Layer '{}' updated successfully", layer_name),
                }))))
            }
            Err(e) => {
                eprintln!("Failed to update layer: {}", e);
                Err(GeoServerError::InternalError("Failed to update layer".to_string()))
            }
        }
    } else {
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
}

pub async fn delete_layer(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let layer_name = req.match_info().get("layer").unwrap_or("");

    if let Some(store) = &state.store {
        match store.delete_layer(layer_name).await {
            Ok(_) => {
                Ok(HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
                    "message": format!("Layer '{}' deleted successfully", layer_name),
                }))))
            }
            Err(e) => {
                eprintln!("Failed to delete layer: {}", e);
                Err(GeoServerError::InternalError("Failed to delete layer".to_string()))
            }
        }
    } else {
        if state.delete_layer(layer_name).await {
            Ok(HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
                "message": format!("Layer '{}' deleted successfully", layer_name),
            }))))
        } else {
            Err(GeoServerError::NotFound(format!("Layer '{}' not found", layer_name)))
        }
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

pub async fn list_data_sources(state: web::Data<AppState>) -> Result<HttpResponse, GeoServerError> {
    if let Some(store) = &state.store {
        match store.get_all_data_sources().await {
            Ok(data_sources) => {
                let result: Vec<_> = data_sources.iter()
                    .map(|ds| serde_json::json!({
                        "name": ds.name,
                        "type": format!("{}", ds.data_source_type).to_lowercase(),
                        "workspace": ds.workspace,
                        "enabled": ds.enabled,
                        "connection": ds.connection.as_ref().map(|c| serde_json::json!({
                            "host": c.host,
                            "port": c.port,
                            "database": c.database,
                            "username": c.username,
                        })),
                        "created": ds.created,
                        "modified": ds.modified,
                    }))
                    .collect();
                Ok(HttpResponse::Ok().json(ApiResponse::success(result)))
            }
            Err(e) => {
                eprintln!("Failed to get data sources: {}", e);
                Ok(HttpResponse::Ok().json(ApiResponse::<Vec<serde_json::Value>>::success(vec![])))
            }
        }
    } else {
        Ok(HttpResponse::Ok().json(ApiResponse::<Vec<serde_json::Value>>::success(vec![])))
    }
}

pub async fn get_data_source(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let name = req.match_info().get("name").unwrap_or("");
    
    if let Some(store) = &state.store {
        match store.get_data_source(name).await {
            Ok(Some(ds)) => {
                let response = serde_json::json!({
                    "name": ds.name,
                    "type": format!("{}", ds.data_source_type).to_lowercase(),
                    "workspace": ds.workspace,
                    "enabled": ds.enabled,
                    "connection": ds.connection.as_ref().map(|c| serde_json::json!({
                        "host": c.host,
                        "port": c.port,
                        "database": c.database,
                        "username": c.username,
                    })),
                    "created": ds.created,
                    "modified": ds.modified,
                });
                Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
            }
            Ok(None) => {
                return Err(GeoServerError::NotFound(format!("Data source '{}' not found", name)));
            }
            Err(e) => {
                eprintln!("Failed to get data source: {}", e);
                Err(GeoServerError::InternalError("Failed to get data source".to_string()))
            }
        }
    } else {
        Err(GeoServerError::InternalError("Database not available".to_string()))
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateDataSourceRequest {
    pub name: String,
    #[serde(rename = "type")]
    pub data_source_type: String,
    pub workspace: Option<String>,
    pub enabled: Option<bool>,
    pub connection: DataSourceConnectionRequest,
}

#[derive(Debug, Deserialize)]
pub struct DataSourceConnectionRequest {
    pub host: String,
    pub port: u16,
    pub database: String,
    #[serde(default)]
    pub schema: Option<String>,
    pub username: String,
    pub password: Option<String>,
}

pub async fn create_data_source(
    body: web::Json<CreateDataSourceRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let data_source_type = match body.data_source_type.as_str() {
        "postgis" => crate::models::DataSourceType::Postgis,
        "shapefile" => crate::models::DataSourceType::Shapefile,
        "geotiff" => crate::models::DataSourceType::Geotiff,
        _ => return Err(GeoServerError::BadRequest("Invalid data source type".to_string())),
    };

    if let Some(store) = &state.store {
        match store.get_data_source(&body.name).await {
            Ok(Some(_)) => {
                return Err(GeoServerError::Conflict(format!(
                    "Data source '{}' already exists",
                    body.name
                )));
            }
            Err(e) => {
                eprintln!("Failed to check data source: {}", e);
                return Err(GeoServerError::InternalError("Failed to create data source".to_string()));
            }
            _ => {}
        }

        let connection = crate::models::DataSourceConnection {
            host: body.connection.host.clone(),
            port: body.connection.port,
            database: body.connection.database.clone(),
            schema: body.connection.schema.clone().unwrap_or("public".to_string()),
            username: body.connection.username.clone(),
            password: body.connection.password.clone(),
        };

        match store.create_data_source(
            &body.name,
            &data_source_type,
            body.workspace.clone(),
            body.enabled.unwrap_or(true),
            &connection,
        ).await {
            Ok(ds) => {
                let response = serde_json::json!({
                    "name": ds.name,
                    "type": format!("{}", ds.data_source_type).to_lowercase(),
                    "workspace": ds.workspace,
                    "enabled": ds.enabled,
                    "created": ds.created,
                });
                Ok(HttpResponse::Created().json(ApiResponse::success(response)))
            }
            Err(e) => {
                eprintln!("Failed to create data source: {}", e);
                Err(GeoServerError::InternalError("Failed to create data source".to_string()))
            }
        }
    } else {
        Err(GeoServerError::InternalError("Database not available".to_string()))
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateDataSourceRequest {
    #[serde(rename = "type")]
    pub data_source_type: Option<String>,
    pub workspace: Option<String>,
    pub enabled: Option<bool>,
    pub connection: Option<DataSourceConnectionRequest>,
}

pub async fn update_data_source(
    req: HttpRequest,
    body: web::Json<UpdateDataSourceRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let name = req.match_info().get("name").unwrap_or("");

    let data_source_type = if let Some(ref t) = body.data_source_type {
        match t.as_str() {
            "postgis" => Some(crate::models::DataSourceType::Postgis),
            "shapefile" => Some(crate::models::DataSourceType::Shapefile),
            "geotiff" => Some(crate::models::DataSourceType::Geotiff),
            _ => return Err(GeoServerError::BadRequest("Invalid data source type".to_string())),
        }
    } else {
        None
    };

    let connection = body.connection.as_ref().map(|c| crate::models::DataSourceConnection {
        host: c.host.clone(),
        port: c.port,
        database: c.database.clone(),
        schema: c.schema.clone().unwrap_or("public".to_string()),
        username: c.username.clone(),
        password: c.password.clone(),
    });

    if let Some(store) = &state.store {
        match store.update_data_source(
            name,
            data_source_type,
            body.workspace.clone(),
            body.enabled,
            connection,
        ).await {
            Ok(_) => {
                Ok(HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
                    "message": format!("Data source '{}' updated successfully", name),
                }))))
            }
            Err(e) => {
                eprintln!("Failed to update data source: {}", e);
                Err(GeoServerError::InternalError("Failed to update data source".to_string()))
            }
        }
    } else {
        Err(GeoServerError::InternalError("Database not available".to_string()))
    }
}

pub async fn delete_data_source(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let name = req.match_info().get("name").unwrap_or("");

    if let Some(store) = &state.store {
        match store.get_data_source(name).await {
            Ok(None) => {
                return Err(GeoServerError::NotFound(format!("Data source '{}' not found", name)));
            }
            Err(e) => {
                eprintln!("Failed to check data source: {}", e);
                return Err(GeoServerError::InternalError("Failed to delete data source".to_string()));
            }
            _ => {}
        }

        match store.delete_data_source(name).await {
            Ok(_) => {
                Ok(HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
                    "message": format!("Data source '{}' deleted successfully", name),
                }))))
            }
            Err(e) => {
                eprintln!("Failed to delete data source: {}", e);
                Err(GeoServerError::InternalError("Failed to delete data source".to_string()))
            }
        }
    } else {
        Err(GeoServerError::InternalError("Database not available".to_string()))
    }
}

pub async fn test_data_source_connection(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let name = req.match_info().get("name").unwrap_or("");

    if let Some(store) = &state.store {
        match store.get_data_source(name).await {
            Ok(Some(ds)) => {
                if let Some(conn) = &ds.connection {
                    let result = test_postgis_connection(conn).await;
                    Ok(HttpResponse::Ok().json(result))
                } else {
                    Ok(HttpResponse::Ok().json(serde_json::json!({
                        "success": false,
                        "message": "No connection configuration found",
                    })))
                }
            }
            Ok(None) => {
                Ok(HttpResponse::Ok().json(serde_json::json!({
                    "success": false,
                    "message": format!("Data source '{}' not found", name),
                })))
            }
            Err(e) => {
                eprintln!("Failed to get data source: {}", e);
                Ok(HttpResponse::Ok().json(serde_json::json!({
                    "success": false,
                    "message": "Failed to get data source",
                })))
            }
        }
    } else {
        Ok(HttpResponse::Ok().json(serde_json::json!({
            "success": false,
            "message": "Database not available",
        })))
    }
}

pub async fn test_connection(
    body: web::Json<CreateDataSourceRequest>,
    _state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let connection = crate::models::DataSourceConnection {
        host: body.connection.host.clone(),
        port: body.connection.port,
        database: body.connection.database.clone(),
        schema: body.connection.schema.clone().unwrap_or("public".to_string()),
        username: body.connection.username.clone(),
        password: body.connection.password.clone(),
    };

    let result = test_postgis_connection(&connection).await;
    Ok(HttpResponse::Ok().json(result))
}

async fn test_postgis_connection(conn: &crate::models::DataSourceConnection) -> serde_json::Value {
    let conn_str = format!(
        "host={} port={} dbname={} user={} {}",
        conn.host,
        conn.port,
        conn.database,
        conn.username,
        if let Some(pwd) = &conn.password {
            format!("password={}", pwd)
        } else {
            "".to_string()
        }
    );

    match tokio_postgres::connect(&conn_str, tokio_postgres::NoTls).await {
        Ok((client, connection)) => {
            tokio::spawn(async move {
                if let Err(e) = connection.await {
                    eprintln!("Postgres connection error: {}", e);
                }
            });

            if let Ok(_) = client.query_one("SELECT version();", &[]).await {
                serde_json::json!({
                    "success": true,
                    "message": "Connection successful",
                })
            } else {
                serde_json::json!({
                    "success": false,
                    "message": "Failed to execute query",
                })
            }
        }
        Err(e) => {
            serde_json::json!({
                "success": false,
                "message": format!("Connection failed: {}", e),
            })
        }
    }
}

pub async fn get_data_source_tables(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let start_total = Instant::now();
    let name = req.match_info().get("name").unwrap_or("");
    tracing::debug!("[get_data_source_tables] 开始处理, name={}", name);

    if let Some(store) = &state.store {
        let t1 = Instant::now();
        match store.get_data_source(name).await {
            Ok(Some(data_source)) => {
                let elapsed_get_ds = t1.elapsed();
                tracing::debug!("[get_data_source_tables] get_data_source 耗时: {:?}", elapsed_get_ds);

                if data_source.data_source_type != crate::models::DataSourceType::Postgis {
                    tracing::debug!("[get_data_source_tables] 非 PostGIS 数据源, 跳过");
                    return Err(GeoServerError::BadRequest("Only PostGIS data sources support table listing".to_string()));
                }

                if let Some(conn_info) = &data_source.connection {
                    let t2 = Instant::now();
                    let pool = state.get_pg_pool(&data_source.name, conn_info);
                    let elapsed_get_pool = t2.elapsed();
                    tracing::debug!("[get_data_source_tables] get_pg_pool 耗时: {:?}", elapsed_get_pool);

                    let t3 = Instant::now();
                    let tables = list_postgis_tables_from_pool(&pool, conn_info).await;
                    let elapsed_query = t3.elapsed();
                    tracing::debug!("[get_data_source_tables] list_postgis_tables_from_pool 查询耗时: {:?}, 返回 {} 个表", elapsed_query, tables.len());

                    let total = start_total.elapsed();
                    tracing::debug!("[get_data_source_tables] 总耗时: {:?}", total);

                    Ok(HttpResponse::Ok().json(ApiResponse::success(tables)))
                } else {
                    tracing::debug!("[get_data_source_tables] 数据源无连接配置");
                    Err(GeoServerError::BadRequest("Data source has no connection configuration".to_string()))
                }
            }
            Ok(None) => {
                tracing::debug!("[get_data_source_tables] 数据源 '{}' 未找到", name);
                Err(GeoServerError::NotFound(format!("Data source '{}' not found", name)))
            }
            Err(e) => {
                tracing::debug!("[get_data_source_tables] 获取数据源失败: {}", e);
                eprintln!("Failed to get data source: {}", e);
                Err(GeoServerError::InternalError("Failed to get data source".to_string()))
            }
        }
    } else {
        tracing::debug!("[get_data_source_tables] 数据库不可用");
        Err(GeoServerError::InternalError("Database not available".to_string()))
    }
}

async fn list_postgis_tables_from_pool(
    pool: &deadpool_postgres::Pool,
    conn: &crate::models::DataSourceConnection,
) -> Vec<String> {
    let start = Instant::now();

    let t1 = Instant::now();
    let client = match pool.get().await {
        Ok(c) => c,
        Err(e) => {
            tracing::debug!("[list_postgis_tables_from_pool] pool.get() 失败: {}, 耗时: {:?}", e, t1.elapsed());
            eprintln!("Failed to get PG client from pool: {}", e);
            return vec![];
        }
    };
    tracing::debug!("[list_postgis_tables_from_pool] pool.get() 耗时: {:?}", t1.elapsed());

    let schema_filter = if conn.schema.is_empty() || conn.schema == "public" {
        "AND n.nspname = 'public'".to_string()
    } else {
        format!("AND n.nspname = '{}'", conn.schema)
    };

    let query = format!(
        "SELECT c.relname
         FROM pg_class c
         JOIN pg_namespace n ON n.oid = c.relnamespace
         WHERE c.relkind IN ('r', 'v')
         AND n.nspname NOT IN ('pg_catalog', 'information_schema')
         {}
         ORDER BY c.relname",
        schema_filter
    );
    tracing::debug!("[list_postgis_tables_from_pool] 查询SQL: {}", query);

    let t2 = Instant::now();
    match client.query(&query, &[]).await {
        Ok(rows) => {
            let elapsed_query = t2.elapsed();
            let tables: Vec<String> = rows.iter()
                .map(|row| row.get::<_, String>(0))
                .collect();
            tracing::debug!("[list_postgis_tables_from_pool] client.query 执行耗时: {:?}, 返回 {} 行", elapsed_query, tables.len());
            tracing::debug!("[list_postgis_tables_from_pool] 总耗时: {:?}", start.elapsed());
            tables
        }
        Err(e) => {
            tracing::debug!("[list_postgis_tables_from_pool] 查询失败: {}, 耗时: {:?}", e, t2.elapsed());
            eprintln!("Failed to query tables: {}", e);
            vec![]
        }
    }
}

pub async fn get_layer_feature_type(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let layer_name = req.match_info().get("layer").unwrap_or("");

    if let Some(store) = &state.store {
        let layer = store.get_layer(layer_name).await
            .map_err(|e| {
                eprintln!("Failed to get layer: {}", e);
                GeoServerError::InternalError("Failed to get layer".to_string())
            })?
            .ok_or_else(|| GeoServerError::NotFound(format!("Layer '{}' not found", layer_name)))?;

        let data_source = store.get_data_source(&layer.store).await
            .map_err(|e| {
                eprintln!("Failed to get data source: {}", e);
                GeoServerError::InternalError("Failed to get data source".to_string())
            })?
            .ok_or_else(|| GeoServerError::NotFound(format!("Data source '{}' not found", layer.store)))?;

        if data_source.data_source_type != crate::models::DataSourceType::Postgis {
            return Err(GeoServerError::BadRequest(
                "Feature type information is only available for PostGIS data sources".to_string()
            ));
        }

        let table_name = layer.native_name
            .as_ref()
            .ok_or_else(|| GeoServerError::BadRequest(
                "Layer has no native table name configured".to_string()
            ))?;

        if let Some(conn_info) = &data_source.connection {
            let pool = state.get_pg_pool(&data_source.name, conn_info);
            let columns = get_postgis_table_columns(&pool, conn_info, table_name).await;
            Ok(HttpResponse::Ok().json(ApiResponse::success(columns)))
        } else {
            Err(GeoServerError::BadRequest("Data source has no connection configuration".to_string()))
        }
    } else {
        Err(GeoServerError::InternalError("Database not available".to_string()))
    }
}

async fn get_postgis_table_columns(
    pool: &deadpool_postgres::Pool,
    conn: &crate::models::DataSourceConnection,
    table_name: &str,
) -> Vec<serde_json::Value> {
    let client = match pool.get().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to get PG client from pool: {}", e);
            return vec![];
        }
    };

    let schema_filter = if conn.schema.is_empty() || conn.schema == "public" {
        "'public'".to_string()
    } else {
        format!("'{}'", conn.schema.replace('\'', "''"))
    };

    let query = format!(
        "SELECT column_name, data_type, character_maximum_length, is_nullable
         FROM information_schema.columns
         WHERE table_schema = {} AND table_name = $1
         ORDER BY ordinal_position",
        schema_filter
    );

    match client.query(&query, &[&table_name]).await {
        Ok(rows) => {
            rows.iter().map(|row| {
                let col_name: String = row.get(0);
                let data_type: String = row.get(1);
                let max_length: Option<i32> = row.get(2);
                let is_nullable: String = row.get(3);
                serde_json::json!({
                    "name": col_name,
                    "type": data_type,
                    "length": max_length,
                    "nullable": is_nullable == "YES",
                })
            }).collect()
        }
        Err(e) => {
            eprintln!("Failed to query table columns: {}", e);
            vec![]
        }
    }
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
    let names: Vec<String> = styles.keys().cloned().collect();
    Ok(HttpResponse::Ok().json(ApiResponse::success(names)))
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
        let rules = get_layer_style_rules(&styles_lock, layer);

        if let Some(features) = state.get_layer_features(&layer.name).await {
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
    }

    let img = renderer.render(render_items);

    let mut buffer = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buffer, image::ImageFormat::Png)
        .map_err(|e| GeoServerError::RenderingError(e.to_string()))?;

    Ok(HttpResponse::Ok()
        .content_type("image/png")
        .body(buffer.into_inner()))
}

fn get_layer_style_rules(
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
    geom: &crate::models::GeoJsonGeometry,
    from_crs: &str,
    to_crs: &str,
) -> crate::models::GeoJsonGeometry {
    use crate::utils::projection::ProjectionTransformer;
    use crate::models::CoordinateReferenceSystem;
    let transformer = ProjectionTransformer::new(
        CoordinateReferenceSystem::from_epsg(from_crs),
        CoordinateReferenceSystem::from_epsg(to_crs),
    );
    match geom {
        crate::models::GeoJsonGeometry::Point { coordinates } => {
            if coordinates.len() >= 2 {
                if let Ok((x, y)) = transformer.transform_point(coordinates[0], coordinates[1]) {
                    return crate::models::GeoJsonGeometry::Point { coordinates: vec![x, y] };
                }
            }
            geom.clone()
        }
        crate::models::GeoJsonGeometry::LineString { coordinates } => {
            let projected: Vec<Vec<f64>> = coordinates.iter()
                .filter_map(|c| {
                    if c.len() >= 2 {
                        transformer.transform_point(c[0], c[1]).ok().map(|(x, y)| vec![x, y])
                    } else { None }
                })
                .collect();
            if projected.len() == coordinates.len() {
                crate::models::GeoJsonGeometry::LineString { coordinates: projected }
            } else { geom.clone() }
        }
        crate::models::GeoJsonGeometry::Polygon { coordinates } => {
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
                crate::models::GeoJsonGeometry::Polygon { coordinates: projected }
            } else { geom.clone() }
        }
        _ => geom.clone(),
    }
}
