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

/// 将 GeoJSON 几何编码为小端序 WKB 字节。
///
/// 支持 Point / LineString / Polygon / MultiPoint / MultiLineString /
/// MultiPolygon / GeometryCollection。缺失坐标时以 0.0 兜底。
pub fn geometry_to_wkb(geom: &GeoJsonGeometry) -> Vec<u8> {
    fn point_wkb(x: f64, y: f64) -> Vec<u8> {
        let mut v = Vec::with_capacity(21);
        v.push(0x01); // little-endian
        v.extend_from_slice(&1u32.to_le_bytes()); // type = Point
        v.extend_from_slice(&x.to_le_bytes());
        v.extend_from_slice(&y.to_le_bytes());
        v
    }

    fn coord_x(c: &[f64]) -> f64 { if c.len() >= 2 { c[0] } else { 0.0 } }
    fn coord_y(c: &[f64]) -> f64 { if c.len() >= 2 { c[1] } else { 0.0 } }

    match geom {
        GeoJsonGeometry::Point { coordinates } => point_wkb(coord_x(coordinates), coord_y(coordinates)),
        GeoJsonGeometry::MultiPoint { coordinates } => {
            let mut v = Vec::new();
            v.push(0x01);
            v.extend_from_slice(&4u32.to_le_bytes());
            v.extend_from_slice(&(coordinates.len() as u32).to_le_bytes());
            for c in coordinates {
                v.extend_from_slice(&point_wkb(coord_x(c), coord_y(c)));
            }
            v
        }
        GeoJsonGeometry::LineString { coordinates } => {
            let mut v = Vec::new();
            v.push(0x01);
            v.extend_from_slice(&2u32.to_le_bytes());
            v.extend_from_slice(&(coordinates.len() as u32).to_le_bytes());
            for c in coordinates {
                v.extend_from_slice(&coord_x(c).to_le_bytes());
                v.extend_from_slice(&coord_y(c).to_le_bytes());
            }
            v
        }
        GeoJsonGeometry::MultiLineString { coordinates } => {
            let mut v = Vec::new();
            v.push(0x01);
            v.extend_from_slice(&5u32.to_le_bytes());
            v.extend_from_slice(&(coordinates.len() as u32).to_le_bytes());
            for line in coordinates {
                v.extend_from_slice(&geometry_to_wkb(&GeoJsonGeometry::LineString {
                    coordinates: line.clone(),
                }));
            }
            v
        }
        GeoJsonGeometry::Polygon { coordinates } => {
            let mut v = Vec::new();
            v.push(0x01);
            v.extend_from_slice(&3u32.to_le_bytes());
            v.extend_from_slice(&(coordinates.len() as u32).to_le_bytes());
            for ring in coordinates {
                v.extend_from_slice(&(ring.len() as u32).to_le_bytes());
                for c in ring {
                    v.extend_from_slice(&coord_x(c).to_le_bytes());
                    v.extend_from_slice(&coord_y(c).to_le_bytes());
                }
            }
            v
        }
        GeoJsonGeometry::MultiPolygon { coordinates } => {
            let mut v = Vec::new();
            v.push(0x01);
            v.extend_from_slice(&6u32.to_le_bytes());
            v.extend_from_slice(&(coordinates.len() as u32).to_le_bytes());
            for poly in coordinates {
                v.extend_from_slice(&geometry_to_wkb(&GeoJsonGeometry::Polygon {
                    coordinates: poly.clone(),
                }));
            }
            v
        }
        GeoJsonGeometry::GeometryCollection { geometries } => {
            let mut v = Vec::new();
            v.push(0x01);
            v.extend_from_slice(&7u32.to_le_bytes());
            v.extend_from_slice(&(geometries.len() as u32).to_le_bytes());
            for g in geometries {
                v.extend_from_slice(&geometry_to_wkb(g));
            }
            v
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 编码 → 解析 往返: Point / LineString / Polygon 必须一致 (解析端支持这三类)
    #[test]
    fn test_geometry_to_wkb_roundtrip_basic() {
        let cases = vec![
            GeoJsonGeometry::Point { coordinates: vec![10.5, -20.25] },
            GeoJsonGeometry::LineString {
                coordinates: vec![vec![0.0, 0.0], vec![1.5, 2.5], vec![3.0, -4.0]],
            },
            GeoJsonGeometry::Polygon {
                coordinates: vec![vec![
                    vec![0.0, 0.0], vec![4.0, 0.0], vec![4.0, 4.0], vec![0.0, 4.0], vec![0.0, 0.0],
                ]],
            },
        ];
        for geom in cases {
            let wkb = geometry_to_wkb(&geom);
            let parsed = parse_wkb_geometry(&wkb);
            assert_eq!(
                format!("{:?}", parsed), format!("{:?}", geom),
                "WKB 往返应保持几何不变, wkb={:?}", wkb
            );
        }
    }

    /// Multi* / GeometryCollection 编码应生成结构正确的 WKB (解析端暂不支持, 仅校验字节长度)
    #[test]
    fn test_geometry_to_wkb_multipart_lengths() {
        // MultiPoint: 1 + 4 + 4 + 2 * 21
        let mp = GeoJsonGeometry::MultiPoint {
            coordinates: vec![vec![1.0, 2.0], vec![3.0, 4.0]],
        };
        assert_eq!(geometry_to_wkb(&mp).len(), 1 + 4 + 4 + 2 * 21);

        // MultiLineString: 1 + 4 + 4 + 2 条线的长度 (每条 1+4+4+16*n)
        let mls = GeoJsonGeometry::MultiLineString {
            coordinates: vec![vec![vec![0.0, 0.0], vec![1.0, 1.0]], vec![vec![2.0, 2.0]]],
        };
        let wkb_mls = geometry_to_wkb(&mls);
        assert_eq!(wkb_mls.len(), 1 + 4 + 4 + (1 + 4 + 4 + 2 * 16) + (1 + 4 + 4 + 1 * 16));
    }
}
