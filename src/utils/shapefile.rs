//! Shapefile 解析器
//!
//! 使用 `shapefile` crate (v0.5) 读取 .shp / .dbf / .prj 文件
//!
//! Shape API:
//! - shapefile::Shape::Point(Point)      → { x, y }
//! - shapefile::Shape::Polyline(Poly)    → poly.parts() → &[Vec<Point>]
//! - shapefile::Shape::Polygon(Poly)     → poly.rings() → &[PolygonRing], ring.points()
//! - shapefile::Shape::Multipoint(Mp)    → mp.points()  → &[Point]

use std::collections::HashMap;
use std::path::Path;
use tracing::{debug, info, warn};

use crate::models::{Bounds, CoordinateReferenceSystem, Feature, GeoJsonGeometry, PropertyValue};

/// Shapefile 读取结果
#[derive(Debug, Clone)]
pub struct ShapefileReadResult {
    pub features: Vec<Feature>,
    pub bounds: Bounds,
    pub crs: Option<CoordinateReferenceSystem>,
    pub shape_type: String,
    pub feature_count: usize,
}

/// 读取 Shapefile (.shp) 并返回要素列表
///
/// `shp_path` - .shp 文件的完整路径（函数会自动查找同名的 .dbf / .prj）
pub fn read_shapefile<P: AsRef<Path>>(shp_path: P) -> Result<ShapefileReadResult, String> {
    let shp_path = shp_path.as_ref();
    let stem = shp_path.with_extension("");

    info!("[Shapefile] 开始读取: {:?}", shp_path);

    // --- 只读取几何体 (.shp) 使用 ShapeReader ---
    let mut reader = shapefile::ShapeReader::from_path(shp_path)
        .map_err(|e| format!("无法打开 Shapefile '{:?}': {}", shp_path, e))?;

    let shape_type_name = format!("{:?}", reader.header().shape_type);

    let mut features: Vec<Feature> = Vec::new();
    let mut bounds = Bounds::new(f64::MAX, f64::MAX, f64::MIN, f64::MIN);

    for result in reader.iter_shapes() {
        match result {
            Ok(shape) => {
                if let Some(geometry) = shape_to_geojson(&shape) {
                    update_bounds_from_geometry(&geometry, &mut bounds);
                    let feature = Feature::new(geometry, HashMap::new());
                    features.push(feature);
                }
            },
            Err(e) => {
                warn!("[Shapefile] 跳过一条无效记录: {}", e);
            },
        }
    }

    // 如果边界未更新，使用默认值
    if bounds.minx == f64::MAX {
        bounds = Bounds::new(-180.0, -90.0, 180.0, 90.0);
    }

    // --- 解析属性表 (.dbf) ---
    let dbf_path = stem.with_extension("dbf");
    if dbf_path.exists() {
        match read_dbf_attributes(&dbf_path, &mut features) {
            Ok(count) => debug!("[Shapefile] 成功读取 {} 条属性记录", count),
            Err(e) => warn!("[Shapefile] 读取属性表失败: {}", e),
        }
    } else {
        debug!("[Shapefile] 未找到属性文件: {:?}", dbf_path);
    }

    // --- 解析投影 (.prj) ---
    let prj_path = stem.with_extension("prj");
    let crs = if prj_path.exists() {
        match std::fs::read_to_string(&prj_path) {
            Ok(wkt) => {
                let srs = parse_prj_wkt(&wkt);
                debug!("[Shapefile] 解析 PRJ: {:?}", srs);
                srs
            },
            Err(e) => {
                warn!("[Shapefile] 读取 PRJ 失败: {}", e);
                None
            },
        }
    } else {
        None
    };

    let feature_count = features.len();
    info!(
        "[Shapefile] 读取完成: {} 个要素, shape_type={}, bounds={:?}",
        feature_count, shape_type_name, bounds
    );

    Ok(ShapefileReadResult {
        features,
        bounds,
        crs,
        shape_type: shape_type_name,
        feature_count,
    })
}

/// 将 `shapefile::Shape` 转换为 `GeoJsonGeometry`
fn shape_to_geojson(shape: &shapefile::Shape) -> Option<GeoJsonGeometry> {
    use shapefile::record::{Point, PointZ};
    match shape {
        shapefile::Shape::Point(p) => Some(GeoJsonGeometry::Point {
            coordinates: vec![p.x, p.y],
        }),
        shapefile::Shape::PointZ(p) => Some(GeoJsonGeometry::Point {
            coordinates: vec![p.x, p.y],
        }),
        shapefile::Shape::PointM(p) => Some(GeoJsonGeometry::Point {
            coordinates: vec![p.x, p.y],
        }),
        shapefile::Shape::Polyline(poly) => {
            let parts: &Vec<Vec<Point>> = poly.parts();
            if parts.len() == 1 {
                let coords: Vec<Vec<f64>> = parts[0].iter().map(|p| vec![p.x, p.y]).collect();
                Some(GeoJsonGeometry::LineString {
                    coordinates: coords,
                })
            } else {
                let coords: Vec<Vec<Vec<f64>>> = parts
                    .iter()
                    .map(|part| part.iter().map(|p| vec![p.x, p.y]).collect())
                    .collect();
                Some(GeoJsonGeometry::MultiLineString {
                    coordinates: coords,
                })
            }
        },
        shapefile::Shape::PolylineZ(poly) => {
            let parts: &Vec<Vec<PointZ>> = poly.parts();
            if parts.len() == 1 {
                let coords: Vec<Vec<f64>> = parts[0].iter().map(|p| vec![p.x, p.y]).collect();
                Some(GeoJsonGeometry::LineString {
                    coordinates: coords,
                })
            } else {
                let coords: Vec<Vec<Vec<f64>>> = parts
                    .iter()
                    .map(|part| part.iter().map(|p| vec![p.x, p.y]).collect())
                    .collect();
                Some(GeoJsonGeometry::MultiLineString {
                    coordinates: coords,
                })
            }
        },
        shapefile::Shape::PolylineM(poly) => {
            let parts: &Vec<Vec<shapefile::record::PointM>> = poly.parts();
            if parts.len() == 1 {
                let coords: Vec<Vec<f64>> = parts[0].iter().map(|p| vec![p.x, p.y]).collect();
                Some(GeoJsonGeometry::LineString {
                    coordinates: coords,
                })
            } else {
                let coords: Vec<Vec<Vec<f64>>> = parts
                    .iter()
                    .map(|part| part.iter().map(|p| vec![p.x, p.y]).collect())
                    .collect();
                Some(GeoJsonGeometry::MultiLineString {
                    coordinates: coords,
                })
            }
        },
        shapefile::Shape::Polygon(poly) => {
            let rings = poly.rings();
            if rings.is_empty() {
                return None;
            }
            let coords: Vec<Vec<Vec<f64>>> = rings
                .iter()
                .map(|ring| ring.points().iter().map(|p| vec![p.x, p.y]).collect())
                .collect();
            Some(GeoJsonGeometry::Polygon {
                coordinates: coords,
            })
        },
        shapefile::Shape::PolygonZ(poly) => {
            let rings = poly.rings();
            if rings.is_empty() {
                return None;
            }
            let coords: Vec<Vec<Vec<f64>>> = rings
                .iter()
                .map(|ring| ring.points().iter().map(|p| vec![p.x, p.y]).collect())
                .collect();
            Some(GeoJsonGeometry::Polygon {
                coordinates: coords,
            })
        },
        shapefile::Shape::PolygonM(poly) => {
            let rings = poly.rings();
            if rings.is_empty() {
                return None;
            }
            let coords: Vec<Vec<Vec<f64>>> = rings
                .iter()
                .map(|ring| ring.points().iter().map(|p| vec![p.x, p.y]).collect())
                .collect();
            Some(GeoJsonGeometry::Polygon {
                coordinates: coords,
            })
        },
        shapefile::Shape::Multipoint(mp) => {
            let coords: Vec<Vec<f64>> = mp.points().iter().map(|p| vec![p.x, p.y]).collect();
            Some(GeoJsonGeometry::MultiPoint {
                coordinates: coords,
            })
        },
        shapefile::Shape::MultipointZ(mp) => {
            let coords: Vec<Vec<f64>> = mp.points().iter().map(|p| vec![p.x, p.y]).collect();
            Some(GeoJsonGeometry::MultiPoint {
                coordinates: coords,
            })
        },
        shapefile::Shape::MultipointM(mp) => {
            let coords: Vec<Vec<f64>> = mp.points().iter().map(|p| vec![p.x, p.y]).collect();
            Some(GeoJsonGeometry::MultiPoint {
                coordinates: coords,
            })
        },
        shapefile::Shape::NullShape => None,
        shapefile::Shape::Multipatch(_) => {
            warn!("[Shapefile] Multipatch 类型暂不支持");
            None
        },
    }
}

/// 从几何体更新边界
fn update_bounds_from_geometry(geometry: &GeoJsonGeometry, bounds: &mut Bounds) {
    let coords = extract_coordinates(geometry);
    for coord in coords {
        if coord.len() >= 2 {
            bounds.minx = bounds.minx.min(coord[0]);
            bounds.miny = bounds.miny.min(coord[1]);
            bounds.maxx = bounds.maxx.max(coord[0]);
            bounds.maxy = bounds.maxy.max(coord[1]);
        }
    }
}

/// 从几何体中提取所有坐标点
fn extract_coordinates(geometry: &GeoJsonGeometry) -> Vec<Vec<f64>> {
    match geometry {
        GeoJsonGeometry::Point { coordinates } => vec![coordinates.clone()],
        GeoJsonGeometry::LineString { coordinates } => coordinates.clone(),
        GeoJsonGeometry::Polygon { coordinates } => coordinates
            .iter()
            .flat_map(|ring| ring.iter().cloned())
            .collect(),
        GeoJsonGeometry::MultiPoint { coordinates } => coordinates.clone(),
        GeoJsonGeometry::MultiLineString { coordinates } => coordinates
            .iter()
            .flat_map(|line| line.iter().cloned())
            .collect(),
        GeoJsonGeometry::MultiPolygon { coordinates } => coordinates
            .iter()
            .flat_map(|poly| poly.iter())
            .flat_map(|ring| ring.iter().cloned())
            .collect(),
        GeoJsonGeometry::GeometryCollection { geometries } => geometries
            .iter()
            .flat_map(|g| extract_coordinates(g))
            .collect(),
    }
}

/// 读取 .dbf 属性文件并将属性关联到对应要素
fn read_dbf_attributes(dbf_path: &Path, features: &mut [Feature]) -> Result<usize, String> {
    use shapefile::dbase::{FieldValue, Reader};

    let mut reader =
        Reader::from_path(dbf_path).map_err(|e| format!("无法打开 DBF '{:?}': {}", dbf_path, e))?;

    let field_names: Vec<String> = reader
        .fields()
        .iter()
        .map(|f| f.name().trim().to_string())
        .collect();

    let mut record_count = 0;
    for (i, result) in reader.iter_records().enumerate() {
        match result {
            Ok(record) => {
                if i >= features.len() {
                    break;
                }
                let mut properties = HashMap::new();
                for field_name in &field_names {
                    let value_str = match record.get(field_name) {
                        Some(FieldValue::Character(Some(s))) if !s.trim().is_empty() => {
                            Some(s.trim().to_string())
                        },
                        Some(FieldValue::Numeric(Some(v))) => Some(v.to_string()),
                        Some(FieldValue::Float(Some(v))) => Some(v.to_string()),
                        Some(FieldValue::Logical(Some(v))) => Some(v.to_string()),
                        Some(FieldValue::Integer(v)) => Some(v.to_string()),
                        Some(FieldValue::Double(v)) => Some(v.to_string()),
                        _ => None,
                    };
                    if let Some(value) = value_str {
                        properties.insert(field_name.clone(), PropertyValue::String(value));
                    }
                }
                features[i].properties = properties;
                record_count += 1;
            },
            Err(e) => {
                warn!("[Shapefile] 跳过第 {} 条属性记录: {}", i, e);
            },
        }
    }

    Ok(record_count)
}

/// 简单解析 .prj WKT 字符串，提取 CRS 名称
fn parse_prj_wkt(wkt: &str) -> Option<CoordinateReferenceSystem> {
    if wkt.contains("WGS_1984") || wkt.contains("GCS_WGS_1984") || wkt.contains("4326") {
        if wkt.contains("900913")
            || wkt.contains("3857")
            || wkt.contains("Mercator")
            || wkt.contains("Web_Mercator")
        {
            Some(CoordinateReferenceSystem::EPSG3857)
        } else {
            Some(CoordinateReferenceSystem::EPSG4326)
        }
    } else if wkt.contains("3857")
        || wkt.contains("900913")
        || wkt.contains("Mercator")
        || wkt.contains("Web_Mercator")
    {
        Some(CoordinateReferenceSystem::EPSG3857)
    } else if wkt.contains("AUTHORITY[\"EPSG\",\"") {
        if let Some(start) = wkt.find("AUTHORITY[\"EPSG\",\"") {
            let rest = &wkt[start + 20..];
            if let Some(end) = rest.find('"') {
                let code = &rest[..end];
                return Some(CoordinateReferenceSystem::from_epsg(&format!(
                    "EPSG:{}",
                    code
                )));
            }
        }
        None
    } else {
        None
    }
}

/// 从 Zip 文件中读取 Shapefile
///
/// 预期 .zip 中包含 .shp / .dbf / .shx / .prj 等文件
pub fn read_shapefile_from_zip<P: AsRef<Path>>(zip_path: P) -> Result<ShapefileReadResult, String> {
    use std::io::Read;

    let zip_path = zip_path.as_ref();
    info!("[Shapefile] 从 ZIP 读取: {:?}", zip_path);

    let file = std::fs::File::open(zip_path)
        .map_err(|e| format!("无法打开 ZIP 文件 '{:?}': {}", zip_path, e))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("无法解析 ZIP 文件: {}", e))?;

    let temp_dir = tempfile::tempdir().map_err(|e| format!("无法创建临时目录: {}", e))?;
    let temp_path = temp_dir.path();

    let mut found_shp = None;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("读取 ZIP 条目失败: {}", e))?;
        let name = entry.name().to_string();
        let lower = name.to_lowercase();

        let target_path = if lower.ends_with(".shp") {
            let path = temp_path.join("extracted.shp");
            found_shp = Some(path.clone());
            path
        } else if lower.ends_with(".dbf") {
            temp_path.join("extracted.dbf")
        } else if lower.ends_with(".prj") {
            temp_path.join("extracted.prj")
        } else if lower.ends_with(".shx") {
            temp_path.join("extracted.shx")
        } else {
            continue;
        };

        let mut out = std::fs::File::create(&target_path)
            .map_err(|e| format!("无法创建临时文件 '{:?}': {}", target_path, e))?;
        let mut buffer = Vec::new();
        entry
            .read_to_end(&mut buffer)
            .map_err(|e| format!("读取 ZIP 条目失败: {}", e))?;
        std::io::copy(&mut buffer.as_slice(), &mut out)
            .map_err(|e| format!("写入临时文件失败: {}", e))?;
    }

    let shp_path = found_shp.ok_or_else(|| "ZIP 中未找到 .shp 文件".to_string())?;
    let result = read_shapefile(&shp_path)?;
    let _ = temp_dir;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_prj_wgs84() {
        let wkt = r#"GEOGCS["GCS_WGS_1984",DATUM["D_WGS_1984",SPHEROID["WGS_1984",6378137,298.257223563]],PRIMEM["Greenwich",0],UNIT["Degree",0.017453292519943295]]"#;
        let crs = parse_prj_wkt(wkt);
        assert!(crs.is_some());
        assert_eq!(crs.unwrap().to_epsg(), "EPSG:4326");
    }

    #[test]
    fn test_parse_prj_mercator() {
        let wkt = r#"PROJCS["WGS_1984_Web_Mercator",GEOGCS["GCS_WGS_1984",DATUM["D_WGS_1984",SPHEROID["WGS_1984",6378137,298.257223563]],PRIMEM["Greenwich",0],UNIT["Degree",0.017453292519943295]],PROJECTION["Mercator"],PARAMETER["false_easting",0],PARAMETER["false_northing",0],PARAMETER["central_meridian",0],PARAMETER["standard_parallel_1",0],UNIT["Meter",1]]"#;
        let crs = parse_prj_wkt(wkt);
        assert!(crs.is_some());
        assert_eq!(crs.unwrap().to_epsg(), "EPSG:3857");
    }

    #[test]
    fn test_extract_coordinates_point() {
        let geom = GeoJsonGeometry::Point {
            coordinates: vec![1.0, 2.0],
        };
        let coords = extract_coordinates(&geom);
        assert_eq!(coords.len(), 1);
        assert_eq!(coords[0], vec![1.0, 2.0]);
    }
}
