use crate::state::AppState;
use crate::models::{Feature, GeoJsonGeometry, PropertyValue, Bounds, DataSourceType};
use crate::error::GeoServerError;
use std::collections::HashMap;

pub async fn query_layer_features(
    state: &AppState,
    layer_name: &str,
    bbox: Option<&Bounds>,
    limit: Option<u64>,
    offset: Option<u64>,
) -> Result<Vec<Feature>, GeoServerError> {
    let layer = {
        let layers = state.layers.read().await;
        layers.iter().find(|l| l.name == layer_name).cloned()
    };
    let layer = layer.ok_or_else(|| GeoServerError::NotFound(format!("Layer '{}' not found", layer_name)))?;

    let data_source = if let Some(store) = &state.store {
        store.get_data_source(&layer.store).await
            .map_err(|e| GeoServerError::InternalError(format!("DB error: {}", e)))?
    } else {
        None
    };

    if let Some(ref ds) = data_source {
        if ds.data_source_type == DataSourceType::Postgis {
            if let Some(ref conn) = ds.connection {
                if let Some(ref native_name) = layer.native_name {
                    let pool = state.get_pg_pool(&ds.name, conn);
                    return query_postgis_features(&pool, conn, native_name, bbox, limit, offset).await;
                }
            }
        }
    }

    let in_memory = state.features.read().await;
    let all = in_memory.get(layer_name).cloned().unwrap_or_default();
    let mut filtered = filter_features(all, bbox);
    if let Some(o) = offset {
        filtered = filtered.into_iter().skip(o as usize).collect();
    }
    if let Some(l) = limit {
        filtered = filtered.into_iter().take(l as usize).collect();
    }
    Ok(filtered)
}

fn filter_features(features: Vec<Feature>, bbox: Option<&Bounds>) -> Vec<Feature> {
    match bbox {
        Some(b) => features.into_iter().filter(|f| feature_in_bbox(f, b)).collect(),
        None => features,
    }
}

fn feature_in_bbox(feature: &Feature, bounds: &Bounds) -> bool {
    match &feature.geometry {
        GeoJsonGeometry::Point { coordinates } => {
            if coordinates.len() >= 2 {
                let (x, y) = (coordinates[0], coordinates[1]);
                x >= bounds.minx && x <= bounds.maxx && y >= bounds.miny && y <= bounds.maxy
            } else {
                true
            }
        }
        _ => true,
    }
}

async fn query_postgis_features(
    pool: &deadpool_postgres::Pool,
    conn: &crate::models::DataSourceConnection,
    native_name: &str,
    bbox: Option<&Bounds>,
    limit: Option<u64>,
    _offset: Option<u64>,
) -> Result<Vec<Feature>, GeoServerError> {
    let client = pool.get().await
        .map_err(|e| GeoServerError::InternalError(format!("Pool error: {}", e)))?;

    let schema = if conn.schema.is_empty() || conn.schema == "public" {
        "public".to_string()
    } else {
        conn.schema.clone()
    };

    let cols = get_table_columns(&client, &schema, native_name).await;
    let geom_col = get_geometry_column(&client, &schema, native_name).await
        .unwrap_or_else(|| "geom".to_string());

    let non_geom_cols: Vec<String> = cols.iter()
        .filter(|c| *c != &geom_col)
        .cloned()
        .collect();

    let raw_names: Vec<&str> = non_geom_cols.iter().map(|c| c.as_str()).collect();
    let col_list = raw_names.join(", ");

    let mut sql = format!(
        "SELECT {} as _id, ST_AsGeoJSON({}) as _geometry, {} FROM \"{}\".\"{}\"",
        get_id_expr(&client, &schema, native_name).await,
        geom_col,
        if col_list.is_empty() { "1 as _dummy".to_string() } else { col_list },
        schema,
        native_name,
    );

    let limit_val = limit.unwrap_or(10000);

    if let Some(b) = bbox {
        sql.push_str(&format!(
            " WHERE {} && ST_MakeEnvelope({}, {}, {}, {}, 4326) LIMIT {}",
            geom_col, b.minx, b.miny, b.maxx, b.maxy, limit_val
        ));
    } else {
        sql.push_str(&format!(" LIMIT {}", limit_val));
    }

    let rows = client.query(&sql, &[]).await
        .map_err(|e| GeoServerError::InternalError(format!("PostGIS query error: {}", e)))?;

    let mut features = Vec::with_capacity(rows.len());
    for row in &rows {
        let id: String = row.try_get("_id").unwrap_or_else(|_| uuid::Uuid::new_v4().to_string());
        let geojson_str: String = row.try_get("_geometry").unwrap_or_default();
        let geometry = parse_geojson_geometry(&geojson_str);
        let mut properties = HashMap::new();
        for col in &non_geom_cols {
            if let Ok(val) = row.try_get::<_, String>(col.as_str()) {
                properties.insert(col.to_string(), PropertyValue::String(val));
            } else if let Ok(val) = row.try_get::<_, i64>(col.as_str()) {
                properties.insert(col.to_string(), PropertyValue::Integer(val));
            } else if let Ok(val) = row.try_get::<_, f64>(col.as_str()) {
                properties.insert(col.to_string(), PropertyValue::Number(val));
            } else if let Ok(val) = row.try_get::<_, bool>(col.as_str()) {
                properties.insert(col.to_string(), PropertyValue::Boolean(val));
            }
        }
        features.push(Feature::with_id(id, geometry, properties));
    }

    Ok(features)
}

async fn get_table_columns(
    client: &tokio_postgres::Client,
    schema: &str,
    table: &str,
) -> Vec<String> {
    let sql = format!(
        "SELECT column_name FROM information_schema.columns
         WHERE table_schema = $1 AND table_name = $2
         ORDER BY ordinal_position"
    );
    match client.query(&sql, &[&schema, &table]).await {
        Ok(rows) => rows.iter().map(|r| r.get::<_, String>(0)).collect(),
        Err(_) => vec![],
    }
}

async fn get_geometry_column(
    client: &tokio_postgres::Client,
    schema: &str,
    table: &str,
) -> Option<String> {
    let sql = "SELECT f_geometry_column FROM geometry_columns WHERE f_table_schema = $1 AND f_table_name = $2";
    if let Ok(rows) = client.query(sql, &[&schema, &table]).await {
        if let Some(row) = rows.first() {
            return row.get::<_, String>(0).into();
        }
    }
    None
}

async fn get_id_expr(
    client: &tokio_postgres::Client,
    schema: &str,
    table: &str,
) -> String {
    let sql = format!(
        "SELECT column_name FROM information_schema.columns
         WHERE table_schema = $1 AND table_name = $2
         AND (column_name = 'id' OR column_name LIKE '%_id' OR column_name = 'gid')
         ORDER BY ordinal_position LIMIT 1"
    );
    match client.query(&sql, &[&schema, &table]).await {
        Ok(rows) if !rows.is_empty() => rows[0].get::<_, String>(0),
        Err(_) | Ok(_) => format!("'{}'", uuid::Uuid::new_v4()),
    }
}

fn parse_geojson_geometry(geojson: &str) -> GeoJsonGeometry {
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(geojson) {
        let typ = val.get("type").and_then(|t| t.as_str()).unwrap_or("Point");
        let coords = val.get("coordinates");
        match typ {
            "Point" => GeoJsonGeometry::Point {
                coordinates: extract_coords_1d(coords),
            },
            "LineString" => GeoJsonGeometry::LineString {
                coordinates: extract_coords_2d(coords),
            },
            "Polygon" => GeoJsonGeometry::Polygon {
                coordinates: extract_coords_3d(coords),
            },
            "MultiPoint" => GeoJsonGeometry::MultiPoint {
                coordinates: extract_coords_2d(coords),
            },
            "MultiLineString" => GeoJsonGeometry::MultiLineString {
                coordinates: extract_coords_3d(coords),
            },
            "MultiPolygon" => GeoJsonGeometry::MultiPolygon {
                coordinates: extract_coords_4d(coords),
            },
            _ => GeoJsonGeometry::Point { coordinates: vec![0.0, 0.0] },
        }
    } else {
        GeoJsonGeometry::Point { coordinates: vec![0.0, 0.0] }
    }
}

fn extract_coords_1d(v: Option<&serde_json::Value>) -> Vec<f64> {
    v.and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|n| n.as_f64()).collect())
        .unwrap_or_default()
}

fn extract_coords_2d(v: Option<&serde_json::Value>) -> Vec<Vec<f64>> {
    v.and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|c| c.as_array().map(|a| a.iter().filter_map(|n| n.as_f64()).collect()))
                .collect()
        })
        .unwrap_or_default()
}

fn extract_coords_3d(v: Option<&serde_json::Value>) -> Vec<Vec<Vec<f64>>> {
    v.and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|ring| {
                    ring.as_array().map(|a| {
                        a.iter()
                            .filter_map(|c| c.as_array().map(|ca| ca.iter().filter_map(|n| n.as_f64()).collect()))
                            .collect()
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn extract_coords_4d(v: Option<&serde_json::Value>) -> Vec<Vec<Vec<Vec<f64>>>> {
    v.and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|poly| {
                    poly.as_array().map(|a| {
                        a.iter()
                            .filter_map(|ring| {
                                ring.as_array().map(|ra| {
                                    ra.iter()
                                        .filter_map(|c| c.as_array().map(|ca| ca.iter().filter_map(|n| n.as_f64()).collect()))
                                        .collect()
                                })
                            })
                            .collect()
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}
