use actix_web::{HttpRequest, HttpResponse, web};
use serde::Deserialize;
use crate::state::AppState;
use crate::models::{Feature, GeoJsonGeometry, Bounds};
use crate::error::GeoServerError;
use super::rest_handler::ApiResponse;

#[derive(Debug, Deserialize)]
pub struct FeatureQuery {
    pub bbox: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct CreateFeatureRequest {
    pub geometry: GeoJsonGeometry,
    pub properties: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateFeatureRequest {
    pub geometry: Option<GeoJsonGeometry>,
    pub properties: Option<std::collections::HashMap<String, serde_json::Value>>,
}

pub async fn get_layer_features(
    req: HttpRequest,
    query: web::Query<FeatureQuery>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let layer_name = req.match_info().get("layer").unwrap_or("");

    let bounds = query.bbox.as_ref().and_then(|b| {
        let parts: Vec<f64> = b.split(',').filter_map(|s| s.trim().parse().ok()).collect();
        if parts.len() == 4 {
            Some(Bounds::new(parts[0], parts[1], parts[2], parts[3]))
        } else {
            None
        }
    });

    let features = crate::handlers::features::query_layer_features(
        &state,
        layer_name,
        bounds.as_ref(),
        query.limit.map(|l| l as u64),
        query.offset.map(|o| o as u64),
    ).await?;

    let total_count = features.len();

    let response = serde_json::json!({
        "type": "FeatureCollection",
        "totalFeatures": total_count,
        "features": features,
    });

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .json(response))
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

    let features = crate::handlers::features::query_layer_features(
        &state, layer_name, None, None, None,
    ).await.unwrap_or_default();

    if let Some(feature) = features.iter().find(|f| f.id == feature_id) {
        return Ok(HttpResponse::Ok().json(serde_json::json!({
            "type": "Feature",
            "id": feature.id,
            "geometry": feature.geometry,
            "properties": feature.properties,
        })));
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
