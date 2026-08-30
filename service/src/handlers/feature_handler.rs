use crate::error::TerraneError;
use crate::models::{Bounds, Feature};
use crate::state::AppState;
use actix_web::{web, HttpRequest, HttpResponse};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct FeatureQuery {
    pub bbox: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub format: Option<String>,
}

pub async fn get_layer_features(
    req: HttpRequest,
    query: web::Query<FeatureQuery>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, TerraneError> {
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
    )
    .await?;

    let format = query.format.as_deref().unwrap_or("application/json");

    match format {
        "text/csv" => {
            let csv_content = features_to_csv(&features);
            Ok(HttpResponse::Ok()
                .content_type("text/csv; charset=utf-8")
                .append_header((
                    "Content-Disposition",
                    format!("attachment; filename=\"{}.csv\"", layer_name),
                ))
                .body(csv_content))
        },
        _ => {
            let response = serde_json::json!({
                "type": "FeatureCollection",
                "totalFeatures": features.len(),
                "features": features,
            });
            Ok(HttpResponse::Ok()
                .content_type("application/json")
                .json(response))
        },
    }
}

pub async fn get_feature(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, TerraneError> {
    let layer_name = req.match_info().get("layer").unwrap_or("");
    let feature_id = req.match_info().get("feature").unwrap_or("");

    let features =
        crate::handlers::features::query_layer_features(&state, layer_name, None, None, None)
            .await
            .unwrap_or_default();

    if let Some(feature) = features.iter().find(|f| f.id == feature_id) {
        return Ok(HttpResponse::Ok().json(serde_json::json!({
            "type": "Feature",
            "id": feature.id,
            "geometry": feature.geometry,
            "properties": feature.properties,
        })));
    }

    Err(TerraneError::NotFound(format!(
        "Feature '{}' not found",
        feature_id
    )))
}

/// 将 Feature 列表转换为 CSV 字符串
fn features_to_csv(features: &[Feature]) -> String {
    use crate::models::PropertyValue;

    if features.is_empty() {
        return "id,geometry\n".to_string();
    }

    // 收集所有可能的属性键
    let mut keys: Vec<String> = Vec::new();
    for f in features {
        for key in f.properties.keys() {
            if !keys.contains(key) {
                keys.push(key.clone());
            }
        }
    }

    // 写入 CSV 头
    let mut csv = String::from("id,geometry");
    for key in &keys {
        csv.push(',');
        csv.push_str(&escape_csv_field(key));
    }
    csv.push('\n');

    // 写入每一行
    for f in features {
        csv.push_str(&escape_csv_field(&f.id));
        csv.push(',');
        csv.push_str(&escape_csv_field(&geometry_to_wkt(&f.geometry)));

        for key in &keys {
            csv.push(',');
            match f.properties.get(key) {
                Some(PropertyValue::String(s)) => csv.push_str(&escape_csv_field(s)),
                Some(PropertyValue::Number(n)) => csv.push_str(&n.to_string()),
                Some(PropertyValue::Integer(i)) => csv.push_str(&i.to_string()),
                Some(PropertyValue::Boolean(b)) => csv.push_str(&b.to_string()),
                Some(PropertyValue::Null) => {},
                Some(PropertyValue::Array(arr)) => {
                    let vals: Vec<String> = arr.iter().map(|v| v.to_string()).collect();
                    csv.push_str(&escape_csv_field(&format!("[{}]", vals.join(";"))));
                },
                Some(PropertyValue::Object(obj)) => {
                    let vals: Vec<String> =
                        obj.iter().map(|(k, v)| format!("{}:{}", k, v)).collect();
                    csv.push_str(&escape_csv_field(&format!("{{{}}}", vals.join(";"))));
                },
                None => {},
            }
        }
        csv.push('\n');
    }

    csv
}

fn escape_csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn geometry_to_wkt(geom: &crate::models::GeoJsonGeometry) -> String {
    match geom {
        crate::models::GeoJsonGeometry::Point { coordinates } => {
            if coordinates.len() >= 2 {
                format!("POINT ({} {})", coordinates[0], coordinates[1])
            } else {
                "POINT EMPTY".to_string()
            }
        },
        crate::models::GeoJsonGeometry::MultiPoint { coordinates } => {
            let pts: Vec<String> = coordinates
                .iter()
                .filter(|c| c.len() >= 2)
                .map(|c| format!("{} {}", c[0], c[1]))
                .collect();
            format!("MULTIPOINT ({})", pts.join(", "))
        },
        crate::models::GeoJsonGeometry::LineString { coordinates } => {
            let pts: Vec<String> = coordinates
                .iter()
                .filter(|c| c.len() >= 2)
                .map(|c| format!("{} {}", c[0], c[1]))
                .collect();
            format!("LINESTRING ({})", pts.join(", "))
        },
        crate::models::GeoJsonGeometry::MultiLineString { coordinates } => {
            let lines: Vec<String> = coordinates
                .iter()
                .map(|line| {
                    let pts: Vec<String> = line
                        .iter()
                        .filter(|c| c.len() >= 2)
                        .map(|c| format!("{} {}", c[0], c[1]))
                        .collect();
                    format!("({})", pts.join(", "))
                })
                .collect();
            format!("MULTILINESTRING ({})", lines.join(", "))
        },
        crate::models::GeoJsonGeometry::Polygon { coordinates } => {
            let rings: Vec<String> = coordinates
                .iter()
                .map(|ring| {
                    let pts: Vec<String> = ring
                        .iter()
                        .filter(|c| c.len() >= 2)
                        .map(|c| format!("{} {}", c[0], c[1]))
                        .collect();
                    format!("({})", pts.join(", "))
                })
                .collect();
            format!("POLYGON ({})", rings.join(", "))
        },
        crate::models::GeoJsonGeometry::MultiPolygon { coordinates } => {
            let polys: Vec<String> = coordinates
                .iter()
                .map(|poly| {
                    let rings: Vec<String> = poly
                        .iter()
                        .map(|ring| {
                            let pts: Vec<String> = ring
                                .iter()
                                .filter(|c| c.len() >= 2)
                                .map(|c| format!("{} {}", c[0], c[1]))
                                .collect();
                            format!("({})", pts.join(", "))
                        })
                        .collect();
                    format!("({})", rings.join(", "))
                })
                .collect();
            format!("MULTIPOLYGON ({})", polys.join(", "))
        },
        crate::models::GeoJsonGeometry::GeometryCollection { geometries } => {
            let geoms: Vec<String> = geometries.iter().map(geometry_to_wkt).collect();
            format!("GEOMETRYCOLLECTION ({})", geoms.join(", "))
        },
    }
}
