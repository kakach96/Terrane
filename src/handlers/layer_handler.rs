use actix_web::{HttpRequest, HttpResponse, web};
use serde::Deserialize;
use crate::state::AppState;
use crate::models::{Layer, FeatureCollection, CoordinateReferenceSystem};
use crate::error::GeoServerError;
use crate::utils::rendering;
use super::rest_handler::ApiResponse;

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

#[derive(Debug, Deserialize)]
pub struct UpdateLayerRequest {
    pub title: Option<String>,
    #[serde(rename = "abstract")]
    pub abstract_text: Option<String>,
    pub native_name: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct PreviewRequest {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub format: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GeoJsonUploadRequest {
    pub name: String,
    pub title: String,
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
                "styles": l.styles,
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

    let response = if let Some(store) = &state.store {
        match store.get_layer(layer_name).await {
            Ok(Some(layer)) => {
                let mut map = serde_json::Map::new();
                map.insert("name".into(), serde_json::Value::String(layer.name.clone()));
                map.insert("title".into(), serde_json::Value::String(layer.title.clone()));
                map.insert("abstract".into(), layer.abstract_text.clone().map(|v| serde_json::Value::String(v)).unwrap_or(serde_json::Value::Null));
                map.insert("workspace".into(), serde_json::Value::String(layer.workspace.clone()));
                map.insert("store".into(), serde_json::Value::String(layer.store.clone()));
                map.insert("native_name".into(), layer.native_name.clone().map(|v| serde_json::Value::String(v)).unwrap_or(serde_json::Value::Null));
                map.insert("srs".into(), serde_json::Value::String(layer.srs.clone()));
                map.insert("native_bounds".into(), serde_json::json!({
                    "crs": layer.srs, "bounds": {
                        "minx": layer.minx, "miny": layer.miny,
                        "maxx": layer.maxx, "maxy": layer.maxy,
                    }
                }));
                map.insert("lat_lon_bounds".into(), serde_json::json!({
                    "crs": "EPSG:4326", "bounds": {
                        "minx": layer.minx, "miny": layer.miny,
                        "maxx": layer.maxx, "maxy": layer.maxy,
                    }
                }));
                map.insert("enabled".into(), serde_json::Value::Bool(layer.enabled));
                map.insert("styles".into(), serde_json::Value::Array(vec![]));
                serde_json::Value::Object(map)
            }
            Ok(None) => return Err(GeoServerError::NotFound(format!("Layer '{}' not found", layer_name))),
            Err(e) => return Err(GeoServerError::InternalError(format!("Failed to get layer: {}", e))),
        }
    } else {
        if let Some(layer) = state.get_layer(layer_name).await {
            let mut map = serde_json::Map::new();
            map.insert("name".into(), serde_json::Value::String(layer.name.clone()));
            map.insert("title".into(), serde_json::Value::String(layer.title.clone()));
            map.insert("abstract".into(), layer.abstract_text.clone().map(|v| serde_json::Value::String(v)).unwrap_or(serde_json::Value::Null));
            map.insert("workspace".into(), serde_json::Value::String(layer.workspace.clone()));
            map.insert("store".into(), serde_json::Value::String(layer.store.clone()));
            map.insert("native_name".into(), layer.native_name.clone().map(|v| serde_json::Value::String(v)).unwrap_or(serde_json::Value::Null));
            map.insert("srs".into(), serde_json::Value::String(layer.srs.to_epsg()));
            map.insert("native_bounds".into(), serde_json::json!({
                "crs": layer.native_bounds.crs.to_epsg(), "bounds": {
                    "minx": layer.native_bounds.bounds.minx, "miny": layer.native_bounds.bounds.miny,
                    "maxx": layer.native_bounds.bounds.maxx, "maxy": layer.native_bounds.bounds.maxy,
                }
            }));
            map.insert("lat_lon_bounds".into(), serde_json::json!({
                "crs": layer.lat_lon_bounds.crs.to_epsg(), "bounds": {
                    "minx": layer.lat_lon_bounds.bounds.minx, "miny": layer.lat_lon_bounds.bounds.miny,
                    "maxx": layer.lat_lon_bounds.bounds.maxx, "maxy": layer.lat_lon_bounds.bounds.maxy,
                }
            }));
            map.insert("enabled".into(), serde_json::Value::Bool(layer.enabled));
            map.insert("styles".into(), serde_json::to_value(&layer.styles).unwrap_or(serde_json::Value::Array(vec![])));
            serde_json::Value::Object(map)
        } else {
            return Err(GeoServerError::NotFound(format!("Layer '{}' not found", layer_name)));
        }
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
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
                "message": format!("Layer '{}' updated", layer_name),
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
                    "message": format!("Layer '{}' deleted", layer_name),
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
                "deleted": true,
                "layer": layer_name,
            }))))
        } else {
            Err(GeoServerError::NotFound(format!("Layer '{}' not found", layer_name)))
        }
    }
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

    let features = crate::handlers::features::query_layer_features(
        &state, layer_name, None, None, None,
    ).await.unwrap_or_default();

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

    {
        let mut features_map = state.features.write().await;
        features_map.insert(layer_name.clone(), fc.features);
    }

    Ok(HttpResponse::Created().json(ApiResponse::success(serde_json::json!({
        "message": format!("Uploaded to layer '{}'", layer_name),
    }))))
}
