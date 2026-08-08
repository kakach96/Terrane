//! # GeoPackage 读写器
//!
//! 使用 OGC GeoPackage 标准读写矢量要素。
//! GeoPackage 本质是 SQLite 数据库，包含特定元数据表。
//!
//! 参考标准: https://www.geopackage.org/spec/

use std::path::Path;
use std::collections::HashMap;
use rusqlite::Connection;
use tracing::info;

use crate::models::{Feature, GeoJsonGeometry, PropertyValue, Bounds};

/// GeoPackage 读取结果
#[derive(Debug, Clone)]
pub struct GeoPackageReadResult {
    pub features: Vec<Feature>,
    pub bounds: Bounds,
    pub crs: String,
    pub feature_count: usize,
    pub layers: Vec<GeoPackageLayer>,
}

/// GeoPackage 中的图层信息
#[derive(Debug, Clone)]
pub struct GeoPackageLayer {
    pub table_name: String,
    pub geometry_column: String,
    pub geometry_type: String,
    pub srs_id: i32,
    pub crs: String,
    pub feature_count: i64,
}

/// 读取 GeoPackage 文件中的所有图层信息
pub fn read_geopackage_layers<P: AsRef<Path>>(path: P) -> Result<Vec<GeoPackageLayer>, String> {
    let path = path.as_ref();
    let conn = Connection::open(path)
        .map_err(|e| format!("无法打开 GeoPackage '{:?}': {}", path, e))?;

    // 验证是否为有效的 GeoPackage
    let is_gpkg: bool = conn
        .prepare("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='gpkg_contents'")
        .and_then(|mut stmt| stmt.query_row([], |row| row.get::<_, i64>(0)))
        .map(|count| count > 0)
        .unwrap_or(false);

    if !is_gpkg {
        return Err(format!("'{:?}' 不是有效的 GeoPackage 文件", path));
    }

    // 读取空间参考系
    let srs_map = read_srs(&conn)?;

    // 读取图层列表
    let mut layers = Vec::new();
    let mut stmt = conn.prepare(
        "SELECT c.table_name, c.srs_id, g.column_name, g.geometry_type_name
         FROM gpkg_contents c
         JOIN gpkg_geometry_columns g ON c.table_name = g.table_name"
    ).map_err(|e| format!("无法查询图层元数据: {}", e))?;

    let rows = stmt.query_map([], |row| {
        let table_name: String = row.get(0)?;
        let srs_id: i32 = row.get(1)?;
        let column_name: String = row.get(2)?;
        let geom_type: String = row.get(3)?;
        Ok((table_name, srs_id, column_name, geom_type))
    }).map_err(|e| format!("查询结果错误: {}", e))?;

    for row in rows {
        if let Ok((table_name, srs_id, column_name, geom_type)) = row {
            let crs = srs_map.get(&srs_id).cloned().unwrap_or_else(|| format!("EPSG:{}", srs_id));
            let count = conn
                .prepare(&format!("SELECT COUNT(*) FROM \"{}\"", table_name))
                .and_then(|mut stmt| stmt.query_row([], |row| row.get::<_, i64>(0)))
                .unwrap_or(0);

            layers.push(GeoPackageLayer {
                table_name,
                geometry_column: column_name,
                geometry_type: geom_type,
                srs_id,
                crs,
                feature_count: count,
            });
        }
    }

    info!("[GeoPackage] 读取完成: {} 个图层", layers.len());
    Ok(layers)
}

/// 读取 GeoPackage 中指定图层的所有要素
pub fn read_geopackage_layer_features<P: AsRef<Path>>(
    path: P,
    layer_name: &str,
    limit: Option<u64>,
) -> Result<GeoPackageReadResult, String> {
    let path = path.as_ref();
    let conn = Connection::open(path)
        .map_err(|e| format!("无法打开 GeoPackage '{:?}': {}", path, e))?;

    // 获取图层元数据
    let layer_info = conn
        .prepare(
            "SELECT g.column_name, g.geometry_type_name, c.srs_id
             FROM gpkg_geometry_columns g
             JOIN gpkg_contents c ON g.table_name = c.table_name
             WHERE g.table_name = ?1"
        )
        .and_then(|mut stmt| {
            stmt.query_row([layer_name], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i32>(2)?,
                ))
            })
        })
        .map_err(|e| format!("查询图层 '{}' 元数据失败: {}", path.display().to_string(), e))?;

    let srs_map = read_srs(&conn)?;
    let crs = srs_map.get(&layer_info.2).cloned()
        .unwrap_or_else(|| format!("EPSG:{}", layer_info.2));

    // 读取所有非几何列名
    let geom_col = &layer_info.0;
    let mut attr_columns: Vec<String> = Vec::new();
    let pragma_stmt = format!("PRAGMA table_info(\"{}\")", layer_name);
    if let Ok(mut stmt) = conn.prepare(&pragma_stmt) {
        if let Ok(rows) = stmt.query_map([], |row| {
            let col_name: String = row.get(1)?;
            Ok(col_name)
        }) {
            for row in rows {
                if let Ok(name) = row {
                    if name != *geom_col {
                        attr_columns.push(name);
                    }
                }
            }
        }
    }

    // 构建查询
    let cols = if attr_columns.is_empty() {
        format!("*")
    } else {
        format!("{}, {}", geom_col, attr_columns.join(", "))
    };

    let query = match limit {
        Some(l) => format!("SELECT {} FROM \"{}\" LIMIT {}", cols, layer_name, l),
        None => format!("SELECT {} FROM \"{}\"", cols, layer_name),
    };

    // 执行查询并解析 WKB 几何
    let mut features = Vec::new();
    let mut bounds = Bounds::new(f64::MAX, f64::MAX, f64::MIN, f64::MIN);

    if let Ok(mut stmt) = conn.prepare(&query) {
        if let Ok(rows) = stmt.query_map([], |row| {
            // 读取几何 (WKB)
            let geom_blob: Option<Vec<u8>> = row.get(0).ok();
            // 读取属性
            let mut props = HashMap::new();
            for (i, col_name) in attr_columns.iter().enumerate() {
                let val: Option<String> = row.get(i + 1).ok();
                if let Some(v) = val {
                    props.insert(col_name.clone(), PropertyValue::String(v));
                }
            }
            Ok((geom_blob, props))
        }) {
            for row in rows {
                if let Ok((geom_blob, props)) = row {
                    if let Some(wkb) = geom_blob {
                        let geometry = crate::utils::wkb::parse_wkb_geometry(&wkb);
                        update_bounds_from_geometry(&geometry, &mut bounds);
                        features.push(Feature::new(geometry, props));
                    }
                }
            }
        }
    }

    info!("[GeoPackage] 图层 '{}': 读取 {} 个要素, CRS={}", layer_name, features.len(), crs);
    Ok(GeoPackageReadResult {
        feature_count: features.len(),
        features,
        bounds,
        crs,
        layers: vec![],
    })
}

/// 读取空间参考系映射
fn read_srs(conn: &Connection) -> Result<HashMap<i32, String>, String> {
    let mut map = HashMap::new();
    if let Ok(mut stmt) = conn.prepare(
        "SELECT srs_id, organization, organization_coordsys_id FROM gpkg_spatial_ref_sys"
    ) {
        if let Ok(rows) = stmt.query_map([], |row| {
            let id: i32 = row.get(0)?;
            let org: String = row.get(1)?;
            let cs_id: i32 = row.get(2)?;
            Ok((id, org, cs_id))
        }) {
            for row in rows {
                if let Ok((id, org, cs_id)) = row {
                    map.insert(id, format!("{}:{}", org, cs_id));
                }
            }
        }
    };
    // 确保标准 CRS 存在
    map.entry(4326).or_insert_with(|| "EPSG:4326".to_string());
    map.entry(3857).or_insert_with(|| "EPSG:3857".to_string());
    Ok(map)
}

/// 从几何更新边界
fn update_bounds_from_geometry(geometry: &GeoJsonGeometry, bounds: &mut Bounds) {
    let mut update = |x: f64, y: f64| {
        let p = Bounds::new(x, y, x, y);
        bounds.expand_to_include(&p);
    };
    match geometry {
        GeoJsonGeometry::Point { coordinates } if coordinates.len() >= 2 => {
            update(coordinates[0], coordinates[1]);
        }
        GeoJsonGeometry::MultiPoint { coordinates } => {
            for c in coordinates { if c.len() >= 2 { update(c[0], c[1]); } }
        }
        GeoJsonGeometry::LineString { coordinates } => {
            for c in coordinates { if c.len() >= 2 { update(c[0], c[1]); } }
        }
        GeoJsonGeometry::Polygon { coordinates } => {
            for ring in coordinates { for c in ring { if c.len() >= 2 { update(c[0], c[1]); } } }
        }
        GeoJsonGeometry::MultiPolygon { coordinates } => {
            for poly in coordinates { for ring in poly { for c in ring { if c.len() >= 2 { update(c[0], c[1]); } } } }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_dir(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("terrane-gpkg-{}-{}", tag, std::process::id()))
    }

    fn create_minimal_gpkg() -> PathBuf {
        let dir = temp_dir("fixture");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.gpkg");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE gpkg_contents (
                table_name TEXT PRIMARY KEY, data_type TEXT, identifier TEXT, description TEXT,
                last_change TEXT, min_x REAL, min_y REAL, max_x REAL, max_y REAL, srs_id INTEGER);
             CREATE TABLE gpkg_geometry_columns (
                table_name TEXT, column_name TEXT, geometry_type_name TEXT, srs_id INTEGER, z TEXT, m TEXT);
             CREATE TABLE gpkg_spatial_ref_sys (
                srs_name TEXT, srs_id INTEGER PRIMARY KEY, organization TEXT,
                organization_coordsys_id INTEGER, definition TEXT, description TEXT);
             CREATE TABLE points (id INTEGER PRIMARY KEY, geom BLOB, name TEXT);
             INSERT INTO gpkg_spatial_ref_sys VALUES ('WGS 84', 4326, 'EPSG', 4326, 'GEOGCS[...]', '');
             INSERT INTO gpkg_contents VALUES ('points', 'features', 'points', '', '2026-01-01', 0.0, 0.0, 1.0, 1.0, 4326);
             INSERT INTO gpkg_geometry_columns VALUES ('points', 'geom', 'POINT', 4326, 'XY', '');
             INSERT INTO points VALUES (1, X'0101000000', 'p1');
            ",
        )
        .unwrap();
        path
    }

    #[test]
    fn test_read_geopackage_layers() {
        let path = create_minimal_gpkg();
        let layers = read_geopackage_layers(&path).unwrap();

        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].table_name, "points");
        assert_eq!(layers[0].geometry_column, "geom");
        assert_eq!(layers[0].geometry_type, "POINT");
        assert_eq!(layers[0].srs_id, 4326);
        assert!(layers[0].crs.contains("4326"), "CRS 应包含 4326, 实际: {}", layers[0].crs);
        assert_eq!(layers[0].feature_count, 1);

        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    #[test]
    fn test_invalid_gpkg() {
        let dir = temp_dir("bad");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bad.gpkg");
        Connection::open(&path)
            .unwrap()
            .execute_batch("CREATE TABLE foo (id INTEGER);")
            .unwrap();
        assert!(read_geopackage_layers(&path).is_err(), "无 gpkg_contents 应报错");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 构造一个 WKB 点 (小端序)
    fn wkb_point(x: f64, y: f64) -> Vec<u8> {
        let mut v = Vec::with_capacity(21);
        v.push(0x01); // little-endian
        v.extend_from_slice(&1u32.to_le_bytes()); // type = Point
        v.extend_from_slice(&x.to_le_bytes());
        v.extend_from_slice(&y.to_le_bytes());
        v
    }

    /// 构造带真实要素数据 (3 个点 + 属性) 的 GeoPackage
    /// tag: 每测试独立子目录, 避免并行测试 remove_dir_all 互相破坏
    fn create_features_gpkg(tag: &str) -> PathBuf {
        let dir = temp_dir(tag);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("features.gpkg");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE gpkg_contents (
                table_name TEXT PRIMARY KEY, data_type TEXT, identifier TEXT, description TEXT,
                last_change TEXT, min_x REAL, min_y REAL, max_x REAL, max_y REAL, srs_id INTEGER);
             CREATE TABLE gpkg_geometry_columns (
                table_name TEXT, column_name TEXT, geometry_type_name TEXT, srs_id INTEGER, z TEXT, m TEXT);
             CREATE TABLE gpkg_spatial_ref_sys (
                srs_name TEXT, srs_id INTEGER PRIMARY KEY, organization TEXT,
                organization_coordsys_id INTEGER, definition TEXT, description TEXT);
             CREATE TABLE places (id INTEGER PRIMARY KEY, geom BLOB, name TEXT);
             INSERT INTO gpkg_spatial_ref_sys VALUES ('WGS 84', 4326, 'EPSG', 4326, 'GEOGCS[...]', '');
             INSERT INTO gpkg_contents VALUES ('places', 'features', 'places', '', '2026-01-01', 10.0, 20.0, 12.0, 22.0, 4326);
             INSERT INTO gpkg_geometry_columns VALUES ('places', 'geom', 'POINT', 4326, 'XY', '');
            ",
        )
        .unwrap();
        let rows = [(10.0, 20.0, "p1"), (11.5, 21.5, "p2"), (12.0, 22.0, "p3")];
        for (i, (x, y, name)) in rows.iter().enumerate() {
            let wkb_hex: String = wkb_point(*x, *y).iter().map(|b| format!("{:02X}", b)).collect();
            conn.execute_batch(&format!(
                "INSERT INTO places (id, geom, name) VALUES ({}, X'{}', '{}');",
                i + 1, wkb_hex, name
            ))
            .unwrap();
        }
        path
    }

    #[test]
    fn test_read_geopackage_features() {
        let path = create_features_gpkg("feat1");
        let result = read_geopackage_layer_features(&path, "places", None).unwrap();

        assert_eq!(result.feature_count, 3);
        assert_eq!(result.features.len(), 3);

        // 边界应覆盖全部 3 个点
        assert!((result.bounds.minx - 10.0).abs() < 1e-6, "minx 应为 10.0, 实际: {}", result.bounds.minx);
        assert!((result.bounds.miny - 20.0).abs() < 1e-6, "miny 应为 20.0, 实际: {}", result.bounds.miny);
        assert!((result.bounds.maxx - 12.0).abs() < 1e-6, "maxx 应为 12.0, 实际: {}", result.bounds.maxx);
        assert!((result.bounds.maxy - 22.0).abs() < 1e-6, "maxy 应为 22.0, 实际: {}", result.bounds.maxy);

        // CRS 来自 gpkg_spatial_ref_sys
        assert!(result.crs.contains("4326"), "CRS 应包含 4326, 实际: {}", result.crs);

        // 几何解析 (Point) + 属性 (name)
        if let crate::models::GeoJsonGeometry::Point { coordinates } = &result.features[0].geometry {
            assert!((coordinates[0] - 10.0).abs() < 1e-6, "x 应为 10.0, 实际: {:?}", coordinates);
            assert!((coordinates[1] - 20.0).abs() < 1e-6, "y 应为 20.0, 实际: {:?}", coordinates);
        } else {
            panic!("第一个要素应为点, 实际: {:?}", result.features[0].geometry);
        }
        match result.features[0].properties.get("name") {
            Some(crate::models::PropertyValue::String(v)) => assert_eq!(v, "p1"),
            other => panic!("name 属性应为 String, 实际: {:?}", other),
        }

        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    #[test]
    fn test_read_geopackage_features_limit() {
        let path = create_features_gpkg("feat2");
        let result = read_geopackage_layer_features(&path, "places", Some(2)).unwrap();
        assert_eq!(result.feature_count, 2, "limit=2 应只读取 2 个要素");
        assert_eq!(result.features.len(), 2);
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }
}
