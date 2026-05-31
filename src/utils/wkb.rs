use crate::models::{GeoJsonGeometry, PropertyValue};
use std::collections::HashMap;

pub fn parse_wkb_geometry(wkb: &[u8]) -> GeoJsonGeometry {
    if wkb.is_empty() {
        return GeoJsonGeometry::Point { coordinates: vec![0.0, 0.0] };
    }

    match wkb[0] {
        0x01 | 0x00 => parse_ewkb_geometry(wkb),
        _ => GeoJsonGeometry::Point { coordinates: vec![0.0, 0.0] },
    }
}

fn parse_ewkb_geometry(wkb: &[u8]) -> GeoJsonGeometry {
    if wkb.len() < 5 {
        return GeoJsonGeometry::Point { coordinates: vec![0.0, 0.0] };
    }

    let is_little_endian = wkb[0] == 0x01;
    let geom_type = if is_little_endian {
        u32::from_le_bytes([wkb[1], wkb[2], wkb[3], wkb[4]])
    } else {
        u32::from_be_bytes([wkb[1], wkb[2], wkb[3], wkb[4]])
    };

    match geom_type {
        1 => parse_wkb_point(wkb, is_little_endian),
        2 => parse_wkb_linestring(wkb, is_little_endian),
        3 => parse_wkb_polygon(wkb, is_little_endian),
        _ => GeoJsonGeometry::Point { coordinates: vec![0.0, 0.0] },
    }
}

fn parse_wkb_point(wkb: &[u8], little: bool) -> GeoJsonGeometry {
    if wkb.len() < 21 {
        return GeoJsonGeometry::Point { coordinates: vec![0.0, 0.0] };
    }

    let x = if little {
        f64::from_le_bytes([wkb[5], wkb[6], wkb[7], wkb[8], wkb[9], wkb[10], wkb[11], wkb[12]])
    } else {
        f64::from_be_bytes([wkb[5], wkb[6], wkb[7], wkb[8], wkb[9], wkb[10], wkb[11], wkb[12]])
    };

    let y = if little {
        f64::from_le_bytes([wkb[13], wkb[14], wkb[15], wkb[16], wkb[17], wkb[18], wkb[19], wkb[20]])
    } else {
        f64::from_be_bytes([wkb[13], wkb[14], wkb[15], wkb[16], wkb[17], wkb[18], wkb[19], wkb[20]])
    };

    GeoJsonGeometry::Point { coordinates: vec![x, y] }
}

fn parse_wkb_linestring(wkb: &[u8], little: bool) -> GeoJsonGeometry {
    if wkb.len() < 9 {
        return GeoJsonGeometry::LineString { coordinates: vec![] };
    }

    let num_points = if little {
        u32::from_le_bytes([wkb[5], wkb[6], wkb[7], wkb[8]]) as usize
    } else {
        u32::from_be_bytes([wkb[5], wkb[6], wkb[7], wkb[8]]) as usize
    };

    let mut coords = Vec::with_capacity(num_points);
    for i in 0..num_points {
        let offset = 9 + i * 16;
        if offset + 16 > wkb.len() {
            break;
        }

        let x = if little {
            f64::from_le_bytes([
                wkb[offset], wkb[offset+1], wkb[offset+2], wkb[offset+3],
                wkb[offset+4], wkb[offset+5], wkb[offset+6], wkb[offset+7]
            ])
        } else {
            f64::from_be_bytes([
                wkb[offset], wkb[offset+1], wkb[offset+2], wkb[offset+3],
                wkb[offset+4], wkb[offset+5], wkb[offset+6], wkb[offset+7]
            ])
        };

        let y = if little {
            f64::from_le_bytes([
                wkb[offset+8], wkb[offset+9], wkb[offset+10], wkb[offset+11],
                wkb[offset+12], wkb[offset+13], wkb[offset+14], wkb[offset+15]
            ])
        } else {
            f64::from_be_bytes([
                wkb[offset+8], wkb[offset+9], wkb[offset+10], wkb[offset+11],
                wkb[offset+12], wkb[offset+13], wkb[offset+14], wkb[offset+15]
            ])
        };

        coords.push(vec![x, y]);
    }

    GeoJsonGeometry::LineString { coordinates: coords }
}

fn parse_wkb_polygon(wkb: &[u8], little: bool) -> GeoJsonGeometry {
    if wkb.len() < 9 {
        return GeoJsonGeometry::Polygon { coordinates: vec![vec![]] };
    }

    let num_rings = if little {
        u32::from_le_bytes([wkb[5], wkb[6], wkb[7], wkb[8]]) as usize
    } else {
        u32::from_be_bytes([wkb[5], wkb[6], wkb[7], wkb[8]]) as usize
    };

    let mut rings = Vec::with_capacity(num_rings);
    let mut offset = 9;

    for _ in 0..num_rings {
        if offset + 4 > wkb.len() {
            break;
        }

        let num_points = if little {
            u32::from_le_bytes([wkb[offset], wkb[offset+1], wkb[offset+2], wkb[offset+3]]) as usize
        } else {
            u32::from_be_bytes([wkb[offset], wkb[offset+1], wkb[offset+2], wkb[offset+3]]) as usize
        };

        offset += 4;

        let mut ring = Vec::with_capacity(num_points);
        for _ in 0..num_points {
            if offset + 16 > wkb.len() {
                break;
            }

            let x = if little {
                f64::from_le_bytes([
                    wkb[offset], wkb[offset+1], wkb[offset+2], wkb[offset+3],
                    wkb[offset+4], wkb[offset+5], wkb[offset+6], wkb[offset+7]
                ])
            } else {
                f64::from_be_bytes([
                    wkb[offset], wkb[offset+1], wkb[offset+2], wkb[offset+3],
                    wkb[offset+4], wkb[offset+5], wkb[offset+6], wkb[offset+7]
                ])
            };

            let y = if little {
                f64::from_le_bytes([
                    wkb[offset+8], wkb[offset+9], wkb[offset+10], wkb[offset+11],
                    wkb[offset+12], wkb[offset+13], wkb[offset+14], wkb[offset+15]
                ])
            } else {
                f64::from_be_bytes([
                    wkb[offset+8], wkb[offset+9], wkb[offset+10], wkb[offset+11],
                    wkb[offset+12], wkb[offset+13], wkb[offset+14], wkb[offset+15]
                ])
            };

            ring.push(vec![x, y]);
            offset += 16;
        }

        rings.push(ring);
    }

    GeoJsonGeometry::Polygon { coordinates: rings }
}

pub fn parse_geojson_geometry(geojson: &str) -> GeoJsonGeometry {
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

pub fn parse_postgres_row(
    row: &tokio_postgres::Row,
    non_geom_cols: &[String],
    _geom_col: &str,
) -> Result<(String, GeoJsonGeometry, HashMap<String, PropertyValue>), tokio_postgres::Error> {
    let id: String = row.try_get("_id").unwrap_or_else(|_| uuid::Uuid::new_v4().to_string());
    let geojson_str: String = row.try_get("_geometry").unwrap_or_default();
    let geometry = parse_geojson_geometry(&geojson_str);
    let mut properties = HashMap::new();

    for col in non_geom_cols {
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

    Ok((id, geometry, properties))
}
