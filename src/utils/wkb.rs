use crate::models::{GeoJsonGeometry, PropertyValue};
use std::collections::HashMap;

pub fn parse_wkb_geometry(wkb: &[u8]) -> GeoJsonGeometry {
    if wkb.is_empty() {
        return GeoJsonGeometry::Point {
            coordinates: vec![0.0, 0.0],
        };
    }

    match wkb[0] {
        0x01 | 0x00 => parse_ewkb_geometry(wkb),
        _ => GeoJsonGeometry::Point {
            coordinates: vec![0.0, 0.0],
        },
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

    fn coord_x(c: &[f64]) -> f64 {
        if c.len() >= 2 {
            c[0]
        } else {
            0.0
        }
    }
    fn coord_y(c: &[f64]) -> f64 {
        if c.len() >= 2 {
            c[1]
        } else {
            0.0
        }
    }

    match geom {
        GeoJsonGeometry::Point { coordinates } => {
            point_wkb(coord_x(coordinates), coord_y(coordinates))
        },
        GeoJsonGeometry::MultiPoint { coordinates } => {
            let mut v = Vec::new();
            v.push(0x01);
            v.extend_from_slice(&4u32.to_le_bytes());
            v.extend_from_slice(&(coordinates.len() as u32).to_le_bytes());
            for c in coordinates {
                v.extend_from_slice(&point_wkb(coord_x(c), coord_y(c)));
            }
            v
        },
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
        },
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
        },
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
        },
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
        },
        GeoJsonGeometry::GeometryCollection { geometries } => {
            let mut v = Vec::new();
            v.push(0x01);
            v.extend_from_slice(&7u32.to_le_bytes());
            v.extend_from_slice(&(geometries.len() as u32).to_le_bytes());
            for g in geometries {
                v.extend_from_slice(&geometry_to_wkb(g));
            }
            v
        },
    }
}

/// 游标式 WKB/EWKB 解析器。
///
/// 支持 WKB 类型 1-7: Point / LineString / Polygon / MultiPoint /
/// MultiLineString / MultiPolygon / GeometryCollection。解析结果与
/// `geometry_to_wkb` 编码器往返一致 (仅 2D 坐标)。
struct WkbReader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> WkbReader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        WkbReader { buf, pos: 0 }
    }

    fn read_byte(&mut self) -> Option<u8> {
        let b = *self.buf.get(self.pos)?;
        self.pos += 1;
        Some(b)
    }

    fn read_u32(&mut self, little: bool) -> Option<u32> {
        let slice = self.buf.get(self.pos..self.pos + 4)?;
        self.pos += 4;
        Some(if little {
            u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]])
        } else {
            u32::from_be_bytes([slice[0], slice[1], slice[2], slice[3]])
        })
    }

    fn read_f64(&mut self, little: bool) -> Option<f64> {
        let slice = self.buf.get(self.pos..self.pos + 8)?;
        self.pos += 8;
        let arr = [
            slice[0], slice[1], slice[2], slice[3], slice[4], slice[5], slice[6], slice[7],
        ];
        Some(if little {
            f64::from_le_bytes(arr)
        } else {
            f64::from_be_bytes(arr)
        })
    }

    /// 读取一个完整几何体（含字节序头 + 类型头），成功时推进游标
    fn read_geometry(&mut self) -> Option<GeoJsonGeometry> {
        let order = self.read_byte()?;
        let little = order == 0x01;
        // 低 16 位为 WKB 类型, 高位为 Z/M/SRID 标志 (此处仅解析 2D)
        let geom_type = (self.read_u32(little)? & 0xFFFF) as u32;
        match geom_type {
            1 => self.read_point(little),
            2 => self.read_linestring(little),
            3 => self.read_polygon(little),
            4 => self.read_multipoint(little),
            5 => self.read_multilinestring(little),
            6 => self.read_multipolygon(little),
            7 => self.read_geometrycollection(little),
            _ => None,
        }
    }

    fn read_point(&mut self, little: bool) -> Option<GeoJsonGeometry> {
        let x = self.read_f64(little)?;
        let y = self.read_f64(little)?;
        Some(GeoJsonGeometry::Point {
            coordinates: vec![x, y],
        })
    }

    fn read_linestring(&mut self, little: bool) -> Option<GeoJsonGeometry> {
        let n = self.read_u32(little)? as usize;
        let mut coords = Vec::with_capacity(n);
        for _ in 0..n {
            let x = self.read_f64(little)?;
            let y = self.read_f64(little)?;
            coords.push(vec![x, y]);
        }
        Some(GeoJsonGeometry::LineString {
            coordinates: coords,
        })
    }

    fn read_polygon(&mut self, little: bool) -> Option<GeoJsonGeometry> {
        let n = self.read_u32(little)? as usize;
        let mut rings = Vec::with_capacity(n);
        for _ in 0..n {
            let m = self.read_u32(little)? as usize;
            let mut ring = Vec::with_capacity(m);
            for _ in 0..m {
                let x = self.read_f64(little)?;
                let y = self.read_f64(little)?;
                ring.push(vec![x, y]);
            }
            rings.push(ring);
        }
        Some(GeoJsonGeometry::Polygon { coordinates: rings })
    }

    fn read_multipoint(&mut self, little: bool) -> Option<GeoJsonGeometry> {
        let n = self.read_u32(little)? as usize;
        let mut coords = Vec::with_capacity(n);
        for _ in 0..n {
            let g = self.read_geometry()?;
            if let GeoJsonGeometry::Point { coordinates } = g {
                coords.push(coordinates);
            } else {
                return None;
            }
        }
        Some(GeoJsonGeometry::MultiPoint {
            coordinates: coords,
        })
    }

    fn read_multilinestring(&mut self, little: bool) -> Option<GeoJsonGeometry> {
        let n = self.read_u32(little)? as usize;
        let mut lines = Vec::with_capacity(n);
        for _ in 0..n {
            let g = self.read_geometry()?;
            if let GeoJsonGeometry::LineString { coordinates } = g {
                lines.push(coordinates);
            } else {
                return None;
            }
        }
        Some(GeoJsonGeometry::MultiLineString { coordinates: lines })
    }

    fn read_multipolygon(&mut self, little: bool) -> Option<GeoJsonGeometry> {
        let n = self.read_u32(little)? as usize;
        let mut polys = Vec::with_capacity(n);
        for _ in 0..n {
            let g = self.read_geometry()?;
            if let GeoJsonGeometry::Polygon { coordinates } = g {
                polys.push(coordinates);
            } else {
                return None;
            }
        }
        Some(GeoJsonGeometry::MultiPolygon { coordinates: polys })
    }

    fn read_geometrycollection(&mut self, little: bool) -> Option<GeoJsonGeometry> {
        let n = self.read_u32(little)? as usize;
        let mut geoms = Vec::with_capacity(n);
        for _ in 0..n {
            geoms.push(self.read_geometry()?);
        }
        Some(GeoJsonGeometry::GeometryCollection { geometries: geoms })
    }
}

fn parse_ewkb_geometry(wkb: &[u8]) -> GeoJsonGeometry {
    let mut reader = WkbReader::new(wkb);
    reader
        .read_geometry()
        .unwrap_or_else(|| GeoJsonGeometry::Point {
            coordinates: vec![0.0, 0.0],
        })
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
            _ => GeoJsonGeometry::Point {
                coordinates: vec![0.0, 0.0],
            },
        }
    } else {
        GeoJsonGeometry::Point {
            coordinates: vec![0.0, 0.0],
        }
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
                .filter_map(|c| {
                    c.as_array()
                        .map(|a| a.iter().filter_map(|n| n.as_f64()).collect())
                })
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
                            .filter_map(|c| {
                                c.as_array()
                                    .map(|ca| ca.iter().filter_map(|n| n.as_f64()).collect())
                            })
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
                                        .filter_map(|c| {
                                            c.as_array().map(|ca| {
                                                ca.iter().filter_map(|n| n.as_f64()).collect()
                                            })
                                        })
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
    let id: String = row
        .try_get("_id")
        .unwrap_or_else(|_| uuid::Uuid::new_v4().to_string());
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
            GeoJsonGeometry::Point {
                coordinates: vec![10.5, -20.25],
            },
            GeoJsonGeometry::LineString {
                coordinates: vec![vec![0.0, 0.0], vec![1.5, 2.5], vec![3.0, -4.0]],
            },
            GeoJsonGeometry::Polygon {
                coordinates: vec![vec![
                    vec![0.0, 0.0],
                    vec![4.0, 0.0],
                    vec![4.0, 4.0],
                    vec![0.0, 4.0],
                    vec![0.0, 0.0],
                ]],
            },
        ];
        for geom in cases {
            let wkb = geometry_to_wkb(&geom);
            let parsed = parse_wkb_geometry(&wkb);
            assert_eq!(
                format!("{:?}", parsed),
                format!("{:?}", geom),
                "WKB 往返应保持几何不变, wkb={:?}",
                wkb
            );
        }
    }

    /// Multi* / GeometryCollection 编码 → 解析 往返 (解析端现支持全部类型)
    #[test]
    fn test_geometry_to_wkb_roundtrip_multipart() {
        let cases = vec![
            GeoJsonGeometry::MultiPoint {
                coordinates: vec![vec![1.0, 2.0], vec![3.0, 4.0], vec![-1.5, 0.5]],
            },
            GeoJsonGeometry::MultiLineString {
                coordinates: vec![
                    vec![vec![0.0, 0.0], vec![1.0, 1.0]],
                    vec![vec![2.0, 2.0], vec![3.0, 3.0], vec![4.0, 4.0]],
                ],
            },
            GeoJsonGeometry::MultiPolygon {
                coordinates: vec![
                    vec![vec![
                        vec![0.0, 0.0],
                        vec![4.0, 0.0],
                        vec![4.0, 4.0],
                        vec![0.0, 4.0],
                        vec![0.0, 0.0],
                    ]],
                    vec![vec![
                        vec![10.0, 10.0],
                        vec![12.0, 10.0],
                        vec![12.0, 12.0],
                        vec![10.0, 12.0],
                        vec![10.0, 10.0],
                    ]],
                ],
            },
            GeoJsonGeometry::GeometryCollection {
                geometries: vec![
                    GeoJsonGeometry::Point {
                        coordinates: vec![1.0, 1.0],
                    },
                    GeoJsonGeometry::LineString {
                        coordinates: vec![vec![0.0, 0.0], vec![5.0, 5.0]],
                    },
                    GeoJsonGeometry::MultiPoint {
                        coordinates: vec![vec![7.0, 8.0], vec![9.0, 10.0]],
                    },
                ],
            },
        ];
        for geom in cases {
            let wkb = geometry_to_wkb(&geom);
            let parsed = parse_wkb_geometry(&wkb);
            assert_eq!(
                format!("{:?}", parsed),
                format!("{:?}", geom),
                "WKB 往返应保持 Multi*/GeometryCollection 不变, wkb 长度={}",
                wkb.len()
            );
        }
    }

    /// Multi* / GeometryCollection 编码的字节长度校验 (与结构解析一致)
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
        assert_eq!(
            wkb_mls.len(),
            1 + 4 + 4 + (1 + 4 + 4 + 2 * 16) + (1 + 4 + 4 + 1 * 16)
        );

        // MultiPolygon: 1 + 4 + 4 + 2 个多边形 (每个 1+4+4+ring 数+点数)
        let mpoly = GeoJsonGeometry::MultiPolygon {
            coordinates: vec![
                vec![vec![vec![0.0, 0.0], vec![1.0, 0.0], vec![0.0, 0.0]]],
                vec![vec![vec![2.0, 2.0], vec![3.0, 2.0], vec![2.0, 2.0]]],
            ],
        };
        let wkb_mpoly = geometry_to_wkb(&mpoly);
        assert_eq!(wkb_mpoly.len(), 1 + 4 + 4 + (1 + 4 + 4 + 4 + 3 * 16) * 2);

        // GeometryCollection: 1 + 4 + 4 + 各子几何长度
        let gc = GeoJsonGeometry::GeometryCollection {
            geometries: vec![
                GeoJsonGeometry::Point {
                    coordinates: vec![0.0, 0.0],
                },
                GeoJsonGeometry::LineString {
                    coordinates: vec![vec![0.0, 0.0], vec![1.0, 1.0]],
                },
            ],
        };
        let wkb_gc = geometry_to_wkb(&gc);
        assert_eq!(wkb_gc.len(), 1 + 4 + 4 + 21 + (1 + 4 + 4 + 2 * 16));
    }

    /// 大端序 WKB 解码 (Point + MultiPoint 子几何均为大端序)
    #[test]
    fn test_parse_wkb_big_endian() {
        // 大端序 Point(10.5, -20.25)
        let mut be_point = vec![0x00u8];
        be_point.extend_from_slice(&1u32.to_be_bytes());
        be_point.extend_from_slice(&10.5f64.to_be_bytes());
        be_point.extend_from_slice(&(-20.25f64).to_be_bytes());
        let parsed = parse_wkb_geometry(&be_point);
        assert_eq!(
            format!("{:?}", parsed),
            format!(
                "{:?}",
                GeoJsonGeometry::Point {
                    coordinates: vec![10.5, -20.25]
                }
            )
        );

        // 大端序 MultiPoint(1,2) (3,4)
        let mut be_mp = vec![0x00u8];
        be_mp.extend_from_slice(&4u32.to_be_bytes());
        be_mp.extend_from_slice(&2u32.to_be_bytes());
        for (x, y) in [(1.0f64, 2.0f64), (3.0f64, 4.0f64)] {
            be_mp.push(0x00);
            be_mp.extend_from_slice(&1u32.to_be_bytes());
            be_mp.extend_from_slice(&x.to_be_bytes());
            be_mp.extend_from_slice(&y.to_be_bytes());
        }
        let parsed = parse_wkb_geometry(&be_mp);
        assert_eq!(
            format!("{:?}", parsed),
            format!(
                "{:?}",
                GeoJsonGeometry::MultiPoint {
                    coordinates: vec![vec![1.0, 2.0], vec![3.0, 4.0]]
                }
            )
        );
    }
}
