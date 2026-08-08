//! # GeoPackage 读写器
//!
//! 使用 OGC GeoPackage 标准读写矢量要素。
//! GeoPackage 本质是 SQLite 数据库，包含特定元数据表。
//!
//! 参考标准: https://www.geopackage.org/spec/

use rusqlite::Connection;
use std::collections::HashMap;
use std::path::Path;
use tracing::info;

use crate::models::{Bounds, Feature, GeoJsonGeometry, PropertyValue};

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
    let conn =
        Connection::open(path).map_err(|e| format!("无法打开 GeoPackage '{:?}': {}", path, e))?;

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
    let mut stmt = conn
        .prepare(
            "SELECT c.table_name, c.srs_id, g.column_name, g.geometry_type_name
         FROM gpkg_contents c
         JOIN gpkg_geometry_columns g ON c.table_name = g.table_name",
        )
        .map_err(|e| format!("无法查询图层元数据: {}", e))?;

    let rows = stmt
        .query_map([], |row| {
            let table_name: String = row.get(0)?;
            let srs_id: i32 = row.get(1)?;
            let column_name: String = row.get(2)?;
            let geom_type: String = row.get(3)?;
            Ok((table_name, srs_id, column_name, geom_type))
        })
        .map_err(|e| format!("查询结果错误: {}", e))?;

    for row in rows {
        if let Ok((table_name, srs_id, column_name, geom_type)) = row {
            let crs = srs_map
                .get(&srs_id)
                .cloned()
                .unwrap_or_else(|| format!("EPSG:{}", srs_id));
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
    let conn =
        Connection::open(path).map_err(|e| format!("无法打开 GeoPackage '{:?}': {}", path, e))?;

    // 获取图层元数据
    let layer_info = conn
        .prepare(
            "SELECT g.column_name, g.geometry_type_name, c.srs_id
             FROM gpkg_geometry_columns g
             JOIN gpkg_contents c ON g.table_name = c.table_name
             WHERE g.table_name = ?1",
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
        .map_err(|e| {
            format!(
                "查询图层 '{}' 元数据失败: {}",
                path.display().to_string(),
                e
            )
        })?;

    let srs_map = read_srs(&conn)?;
    let crs = srs_map
        .get(&layer_info.2)
        .cloned()
        .unwrap_or_else(|| format!("EPSG:{}", layer_info.2));

    // 读取所有非几何列名及其声明的类型 (PRAGMA table_info: cid, name, type, ...)
    let geom_col = &layer_info.0;
    let mut attr_columns: Vec<String> = Vec::new();
    let mut col_types: HashMap<String, String> = HashMap::new();
    let pragma_stmt = format!("PRAGMA table_info(\"{}\")", layer_name);
    if let Ok(mut stmt) = conn.prepare(&pragma_stmt) {
        if let Ok(rows) = stmt.query_map([], |row| {
            let col_name: String = row.get(1)?;
            let col_type: String = row.get(2).unwrap_or_default();
            Ok((col_name, col_type))
        }) {
            for row in rows {
                if let Ok((name, ty)) = row {
                    if name != *geom_col {
                        attr_columns.push(name.clone());
                        col_types.insert(name, ty);
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
            // 读取属性 (按 SQLite 实际存储的值类型还原)
            let mut props = HashMap::new();
            for (i, col_name) in attr_columns.iter().enumerate() {
                let v: Option<rusqlite::types::Value> = row.get(i + 1).ok();
                if let Some(v) = v {
                    let prop = match v {
                        rusqlite::types::Value::Integer(n) => {
                            let ty = col_types
                                .get(col_name)
                                .cloned()
                                .unwrap_or_default()
                                .to_uppercase();
                            if ty.contains("BOOL") {
                                PropertyValue::Boolean(n != 0)
                            } else {
                                PropertyValue::Integer(n)
                            }
                        },
                        rusqlite::types::Value::Real(n) => PropertyValue::Number(n),
                        rusqlite::types::Value::Text(s) => PropertyValue::String(s),
                        _ => continue,
                    };
                    props.insert(col_name.clone(), prop);
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

    info!(
        "[GeoPackage] 图层 '{}': 读取 {} 个要素, CRS={}",
        layer_name,
        features.len(),
        crs
    );
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
    if let Ok(mut stmt) = conn
        .prepare("SELECT srs_id, organization, organization_coordsys_id FROM gpkg_spatial_ref_sys")
    {
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

/// 推断属性列的 SQLite 类型:
/// - 全部为 Boolean → `BOOLEAN` (以 INTEGER 0/1 存储)
/// - 全部为 Integer → `INTEGER`
/// - 全部为 Number(浮点) → `REAL`
/// - 其余情况 (含 String / 混合数值 / Null / Array / Object) → `TEXT`
fn infer_attribute_type(values: &[Option<&PropertyValue>]) -> &'static str {
    let mut has_string = false;
    let mut has_bool = false;
    let mut has_int = false;
    let mut has_real = false;
    let mut has_other = false;
    for v in values.iter().flatten() {
        match v {
            PropertyValue::String(_) => has_string = true,
            PropertyValue::Boolean(_) => has_bool = true,
            PropertyValue::Integer(_) => has_int = true,
            PropertyValue::Number(_) => has_real = true,
            _ => has_other = true,
        }
    }
    if has_string || has_other {
        "TEXT"
    } else if has_bool && !has_int && !has_real {
        "BOOLEAN"
    } else if has_real {
        "REAL"
    } else if has_int {
        "INTEGER"
    } else {
        "TEXT"
    }
}

/// 读取 GeoPackage 要素表的列定义 (PRAGMA table_info)。
///
/// 返回 `(列名, SQLite 类型)` 对列表, 包含几何列。供 WFS
/// `DescribeFeatureType` 与 REST `feature-type` 描述真实 schema 使用。
pub fn geopackage_table_columns<P: AsRef<Path>>(
    path: P,
    table_name: &str,
) -> Result<Vec<(String, String)>, String> {
    let conn = Connection::open(path.as_ref())
        .map_err(|e| format!("无法打开 GeoPackage '{:?}': {}", path.as_ref(), e))?;
    let qn = table_name.replace('"', "\"\"");
    let pragma = format!("PRAGMA table_info(\"{}\")", qn);
    let mut stmt = conn
        .prepare(&pragma)
        .map_err(|e| format!("查询 GeoPackage 表 '{}' 列失败: {}", table_name, e))?;
    let rows = stmt
        .query_map([], |row| {
            let name: String = row.get(1)?;
            let ty: String = row.get(2).unwrap_or_default();
            Ok((name, ty))
        })
        .map_err(|e| format!("查询结果错误: {}", e))?;

    let mut cols = Vec::new();
    for row in rows {
        if let Ok((name, ty)) = row {
            cols.push((name, ty));
        }
    }
    Ok(cols)
}

/// 将要素写入一个新的 GeoPackage 文件（写入端）。
///
/// 按 OGC GeoPackage 标准创建核心元数据表
/// (`gpkg_contents` / `gpkg_geometry_columns` / `gpkg_spatial_ref_sys`),
/// 再创建要素表并写入 WKB 几何 + 属性。属性列取所有要素属性键的并集,
/// 并按值类型推断列类型 (INTEGER / REAL / BOOLEAN / TEXT)。`geometry_type`
/// 用 GeoPackage 命名, 如 "POINT"。返回写入的图层信息。
pub fn write_geopackage_features<P: AsRef<Path>>(
    path: P,
    layer_name: &str,
    geometry_type: &str,
    srs_id: i32,
    features: &[Feature],
    bounds: &Bounds,
) -> Result<GeoPackageLayer, String> {
    let path = path.as_ref();
    if path.exists() {
        std::fs::remove_file(path)
            .map_err(|e| format!("无法覆盖已有 GeoPackage '{:?}': {}", path, e))?;
    }

    let conn =
        Connection::open(path).map_err(|e| format!("无法创建 GeoPackage '{:?}': {}", path, e))?;

    // 1. 核心元数据表
    conn.execute_batch(
        "CREATE TABLE gpkg_contents (
            table_name TEXT NOT NULL PRIMARY KEY,
            data_type TEXT NOT NULL,
            identifier TEXT UNIQUE,
            description TEXT DEFAULT '',
            last_change DATETIME NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
            min_x DOUBLE, min_y DOUBLE, max_x DOUBLE, max_y DOUBLE,
            srs_id INTEGER,
            CONSTRAINT fk_gc_r_srs_id FOREIGN KEY (srs_id) REFERENCES gpkg_spatial_ref_sys(srs_id));
         CREATE TABLE gpkg_geometry_columns (
            table_name TEXT NOT NULL,
            column_name TEXT NOT NULL,
            geometry_type_name TEXT NOT NULL,
            srs_id INTEGER NOT NULL,
            z TEXT NOT NULL,
            m TEXT NOT NULL,
            CONSTRAINT pk_geom_cols PRIMARY KEY (table_name, column_name),
            CONSTRAINT fk_gc_tn FOREIGN KEY (table_name) REFERENCES gpkg_contents(table_name),
            CONSTRAINT fk_gc_srs FOREIGN KEY (srs_id) REFERENCES gpkg_spatial_ref_sys(srs_id));
         CREATE TABLE gpkg_spatial_ref_sys (
            srs_name TEXT NOT NULL,
            srs_id INTEGER NOT NULL PRIMARY KEY,
            organization TEXT NOT NULL,
            organization_coordsys_id INTEGER NOT NULL,
            definition TEXT NOT NULL,
            description TEXT);
        ",
    )
    .map_err(|e| format!("创建 GeoPackage 元数据表失败: {}", e))?;

    // 2. 空间参考系 (WGS 84)
    conn.execute(
        "INSERT OR IGNORE INTO gpkg_spatial_ref_sys
            (srs_name, srs_id, organization, organization_coordsys_id, definition, description)
         VALUES ('WGS 84 geodetic', 4326, 'EPSG', 4326,
                 'GEOGCS[\"WGS 84\",DATUM[\"WGS_1984\",...]]',
                 'longitude/latitude coordinates in decimal degrees')",
        [],
    )
    .map_err(|e| format!("写入空间参考系失败: {}", e))?;

    // 3. 属性列 = 所有要素属性键的并集, 并按值类型推断 SQLite 列类型
    let mut attr_columns: Vec<String> = Vec::new();
    for f in features {
        for key in f.properties.keys() {
            if !attr_columns.contains(key) {
                attr_columns.push(key.clone());
            }
        }
    }
    let attr_types: HashMap<String, &'static str> = attr_columns
        .iter()
        .map(|col| {
            let values: Vec<Option<&PropertyValue>> =
                features.iter().map(|f| f.properties.get(col)).collect();
            (col.clone(), infer_attribute_type(&values))
        })
        .collect();

    // 4. 图层元数据
    conn.execute(
        "INSERT INTO gpkg_contents
            (table_name, data_type, identifier, description, min_x, min_y, max_x, max_y, srs_id)
         VALUES (?1, 'features', ?1, '', ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            layer_name,
            bounds.minx,
            bounds.miny,
            bounds.maxx,
            bounds.maxy,
            srs_id
        ],
    )
    .map_err(|e| format!("写入 gpkg_contents 失败: {}", e))?;

    conn.execute(
        "INSERT INTO gpkg_geometry_columns
            (table_name, column_name, geometry_type_name, srs_id, z, m)
         VALUES (?1, 'geom', ?2, ?3, 'XY', '')",
        rusqlite::params![layer_name, geometry_type, srs_id],
    )
    .map_err(|e| format!("写入 gpkg_geometry_columns 失败: {}", e))?;

    // 5. 建要素表
    let qn = |s: &str| s.replace('"', "\"\"");
    let mut create_sql = format!(
        "CREATE TABLE \"{}\" (id INTEGER PRIMARY KEY AUTOINCREMENT, \"geom\" BLOB",
        qn(layer_name)
    );
    for col in &attr_columns {
        create_sql.push_str(&format!(", \"{}\" {}", qn(col), attr_types[col]));
    }
    create_sql.push(')');
    conn.execute_batch(&create_sql)
        .map_err(|e| format!("创建要素表失败: {}", e))?;

    // 6. 写入要素 (WKB 几何 + 类型化属性)
    let mut insert_sql = format!("INSERT INTO \"{}\" (\"geom\"", qn(layer_name));
    for col in &attr_columns {
        insert_sql.push_str(&format!(", \"{}\"", qn(col)));
    }
    insert_sql.push_str(") VALUES (?1");
    for _ in &attr_columns {
        insert_sql.push_str(", ?");
    }
    insert_sql.push(')');

    let mut stmt = conn
        .prepare(&insert_sql)
        .map_err(|e| format!("准备插入语句失败: {}", e))?;
    for f in features {
        let wkb = crate::utils::wkb::geometry_to_wkb(&f.geometry);
        let mut values: Vec<rusqlite::types::Value> = vec![rusqlite::types::Value::Blob(wkb)];
        for col in &attr_columns {
            let val = match f.properties.get(col) {
                Some(PropertyValue::String(s)) => rusqlite::types::Value::Text(s.clone()),
                Some(PropertyValue::Integer(i)) => rusqlite::types::Value::Integer(*i),
                Some(PropertyValue::Number(n)) => rusqlite::types::Value::Real(*n),
                Some(PropertyValue::Boolean(b)) => {
                    rusqlite::types::Value::Integer(if *b { 1 } else { 0 })
                },
                // Array / Object / Null 等复杂值 → 文本或 NULL
                Some(pv) => rusqlite::types::Value::Text(pv.to_string()),
                None => rusqlite::types::Value::Null,
            };
            values.push(val);
        }
        stmt.execute(rusqlite::params_from_iter(values.iter()))
            .map_err(|e| format!("写入要素失败: {}", e))?;
    }
    drop(stmt);

    // 7. 统计并返回
    let count: i64 = conn
        .query_row(
            &format!("SELECT COUNT(*) FROM \"{}\"", qn(layer_name)),
            [],
            |row| row.get(0),
        )
        .map_err(|e| format!("统计要素失败: {}", e))?;

    info!(
        "[GeoPackage] 写入完成: 图层 '{}', {} 个要素, {} 个属性列",
        layer_name,
        count,
        attr_columns.len()
    );

    Ok(GeoPackageLayer {
        table_name: layer_name.to_string(),
        geometry_column: "geom".to_string(),
        geometry_type: geometry_type.to_string(),
        srs_id,
        crs: format!("EPSG:{}", srs_id),
        feature_count: count,
    })
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
        },
        GeoJsonGeometry::MultiPoint { coordinates } => {
            for c in coordinates {
                if c.len() >= 2 {
                    update(c[0], c[1]);
                }
            }
        },
        GeoJsonGeometry::LineString { coordinates } => {
            for c in coordinates {
                if c.len() >= 2 {
                    update(c[0], c[1]);
                }
            }
        },
        GeoJsonGeometry::Polygon { coordinates } => {
            for ring in coordinates {
                for c in ring {
                    if c.len() >= 2 {
                        update(c[0], c[1]);
                    }
                }
            }
        },
        GeoJsonGeometry::MultiPolygon { coordinates } => {
            for poly in coordinates {
                for ring in poly {
                    for c in ring {
                        if c.len() >= 2 {
                            update(c[0], c[1]);
                        }
                    }
                }
            }
        },
        _ => {},
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
        assert!(
            layers[0].crs.contains("4326"),
            "CRS 应包含 4326, 实际: {}",
            layers[0].crs
        );
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
        assert!(
            read_geopackage_layers(&path).is_err(),
            "无 gpkg_contents 应报错"
        );
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
            let wkb_hex: String = wkb_point(*x, *y)
                .iter()
                .map(|b| format!("{:02X}", b))
                .collect();
            conn.execute_batch(&format!(
                "INSERT INTO places (id, geom, name) VALUES ({}, X'{}', '{}');",
                i + 1,
                wkb_hex,
                name
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
        assert!(
            (result.bounds.minx - 10.0).abs() < 1e-6,
            "minx 应为 10.0, 实际: {}",
            result.bounds.minx
        );
        assert!(
            (result.bounds.miny - 20.0).abs() < 1e-6,
            "miny 应为 20.0, 实际: {}",
            result.bounds.miny
        );
        assert!(
            (result.bounds.maxx - 12.0).abs() < 1e-6,
            "maxx 应为 12.0, 实际: {}",
            result.bounds.maxx
        );
        assert!(
            (result.bounds.maxy - 22.0).abs() < 1e-6,
            "maxy 应为 22.0, 实际: {}",
            result.bounds.maxy
        );

        // CRS 来自 gpkg_spatial_ref_sys
        assert!(
            result.crs.contains("4326"),
            "CRS 应包含 4326, 实际: {}",
            result.crs
        );

        // 几何解析 (Point) + 属性 (name)
        if let crate::models::GeoJsonGeometry::Point { coordinates } = &result.features[0].geometry
        {
            assert!(
                (coordinates[0] - 10.0).abs() < 1e-6,
                "x 应为 10.0, 实际: {:?}",
                coordinates
            );
            assert!(
                (coordinates[1] - 20.0).abs() < 1e-6,
                "y 应为 20.0, 实际: {:?}",
                coordinates
            );
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

    // -----------------------------------------------------------------------
    // Batch 7: GeoPackage 写入 (write_geopackage_features) → 读取 往返
    // -----------------------------------------------------------------------

    /// 写入 3 个点 + 属性, 再读回验证几何/属性/边界/图层列表完全一致
    #[test]
    fn test_write_geopackage_roundtrip() {
        let dir = temp_dir("wr1");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("roundtrip.gpkg");

        let mut props = HashMap::new();
        props.insert(
            "name".to_string(),
            PropertyValue::String("pt-a".to_string()),
        );
        let features = vec![
            Feature::with_id(
                "f1".into(),
                GeoJsonGeometry::Point {
                    coordinates: vec![10.0, 20.0],
                },
                props.clone(),
            ),
            Feature::with_id(
                "f2".into(),
                GeoJsonGeometry::Point {
                    coordinates: vec![11.5, 21.5],
                },
                props.clone(),
            ),
            Feature::with_id(
                "f3".into(),
                GeoJsonGeometry::Point {
                    coordinates: vec![12.0, 22.0],
                },
                props.clone(),
            ),
        ];
        let bounds = Bounds::new(10.0, 20.0, 12.0, 22.0);

        let layer =
            write_geopackage_features(&path, "places", "POINT", 4326, &features, &bounds).unwrap();
        assert_eq!(layer.table_name, "places");
        assert_eq!(layer.feature_count, 3);
        assert_eq!(layer.geometry_column, "geom");
        assert!(
            layer.crs.contains("4326"),
            "CRS 应包含 4326, 实际: {}",
            layer.crs
        );

        // 读回验证
        let result = read_geopackage_layer_features(&path, "places", None).unwrap();
        assert_eq!(result.feature_count, 3, "往返后要素数应为 3");
        assert_eq!(result.features.len(), 3);

        assert!(
            (result.bounds.minx - 10.0).abs() < 1e-6,
            "minx 应为 10.0, 实际: {}",
            result.bounds.minx
        );
        assert!(
            (result.bounds.miny - 20.0).abs() < 1e-6,
            "miny 应为 20.0, 实际: {}",
            result.bounds.miny
        );
        assert!(
            (result.bounds.maxx - 12.0).abs() < 1e-6,
            "maxx 应为 12.0, 实际: {}",
            result.bounds.maxx
        );
        assert!(
            (result.bounds.maxy - 22.0).abs() < 1e-6,
            "maxy 应为 22.0, 实际: {}",
            result.bounds.maxy
        );

        if let GeoJsonGeometry::Point { coordinates } = &result.features[0].geometry {
            assert!(
                (coordinates[0] - 10.0).abs() < 1e-6,
                "x 应为 10.0, 实际: {:?}",
                coordinates
            );
            assert!(
                (coordinates[1] - 20.0).abs() < 1e-6,
                "y 应为 20.0, 实际: {:?}",
                coordinates
            );
        } else {
            panic!("第一个要素应为点, 实际: {:?}", result.features[0].geometry);
        }
        match result.features[0].properties.get("name") {
            Some(PropertyValue::String(v)) => assert_eq!(v, "pt-a"),
            other => panic!("name 属性应为 String, 实际: {:?}", other),
        }

        // 图层列表也应可见
        let layers = read_geopackage_layers(&path).unwrap();
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].table_name, "places");
        assert_eq!(layers[0].feature_count, 3);

        std::fs::remove_dir_all(&dir).ok();
    }

    /// LineString 写入 → 读取 往返 (验证 WKB 编码器在多点类型上的正确性)
    #[test]
    fn test_write_geopackage_linestring_roundtrip() {
        let dir = temp_dir("wr2");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("lines.gpkg");

        let line = GeoJsonGeometry::LineString {
            coordinates: vec![vec![0.0, 0.0], vec![1.5, 2.5], vec![3.0, -4.0]],
        };
        let features = vec![Feature::with_id("l1".into(), line, HashMap::new())];
        let bounds = Bounds::new(0.0, -4.0, 3.0, 2.5);

        write_geopackage_features(&path, "roads", "LINESTRING", 4326, &features, &bounds).unwrap();

        let result = read_geopackage_layer_features(&path, "roads", None).unwrap();
        assert_eq!(result.features.len(), 1);
        match &result.features[0].geometry {
            GeoJsonGeometry::LineString { coordinates } => {
                assert_eq!(coordinates.len(), 3);
                assert!(
                    (coordinates[2][0] - 3.0).abs() < 1e-6,
                    "第 3 点 x 应为 3.0, 实际: {:?}",
                    coordinates[2]
                );
                assert!(
                    (coordinates[2][1] + 4.0).abs() < 1e-6,
                    "第 3 点 y 应为 -4.0, 实际: {:?}",
                    coordinates[2]
                );
            },
            other => panic!("应为 LineString, 实际: {:?}", other),
        }

        std::fs::remove_dir_all(&dir).ok();
    }

    // -----------------------------------------------------------------------
    // Batch 10: GeoPackage 类型化属性 (INTEGER / REAL / BOOLEAN / TEXT)
    // -----------------------------------------------------------------------

    #[test]
    fn test_infer_attribute_type() {
        let str1 = PropertyValue::String("a".to_string());
        let int1 = PropertyValue::Integer(1);
        let int2 = PropertyValue::Integer(2);
        let real1 = PropertyValue::Number(1.5);
        let real2 = PropertyValue::Number(2.5);
        let bool1 = PropertyValue::Boolean(true);
        let bool2 = PropertyValue::Boolean(false);
        let none: Option<&PropertyValue> = None;

        assert_eq!(infer_attribute_type(&[Some(&int1), Some(&int2)]), "INTEGER");
        assert_eq!(infer_attribute_type(&[Some(&real1), Some(&real2)]), "REAL");
        assert_eq!(
            infer_attribute_type(&[Some(&bool1), Some(&bool2)]),
            "BOOLEAN"
        );
        assert_eq!(infer_attribute_type(&[Some(&str1)]), "TEXT");
        // 混合数值 → REAL
        assert_eq!(infer_attribute_type(&[Some(&int1), Some(&real1)]), "REAL");
        // 含字符串 → TEXT
        assert_eq!(infer_attribute_type(&[Some(&int1), Some(&str1)]), "TEXT");
        // 全 Null → TEXT
        assert_eq!(infer_attribute_type(&[none, none]), "TEXT");
    }

    /// 写入混合类型属性 (String/Integer/Number/Boolean) → 读回类型保持一致,
    /// 且列类型 (PRAGMA table_info) 正确推断为 TEXT/INTEGER/REAL/BOOLEAN
    #[test]
    fn test_write_geopackage_typed_attributes_roundtrip() {
        let dir = temp_dir("wr3");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("typed.gpkg");

        let mut props1 = HashMap::new();
        props1.insert(
            "name".to_string(),
            PropertyValue::String("alpha".to_string()),
        );
        props1.insert("count".to_string(), PropertyValue::Integer(10));
        props1.insert("price".to_string(), PropertyValue::Number(9.5));
        props1.insert("active".to_string(), PropertyValue::Boolean(true));

        let mut props2 = HashMap::new();
        props2.insert(
            "name".to_string(),
            PropertyValue::String("beta".to_string()),
        );
        props2.insert("count".to_string(), PropertyValue::Integer(20));
        props2.insert("price".to_string(), PropertyValue::Number(19.25));
        props2.insert("active".to_string(), PropertyValue::Boolean(false));

        let features = vec![
            Feature::with_id(
                "f1".into(),
                GeoJsonGeometry::Point {
                    coordinates: vec![1.0, 1.0],
                },
                props1,
            ),
            Feature::with_id(
                "f2".into(),
                GeoJsonGeometry::Point {
                    coordinates: vec![2.0, 2.0],
                },
                props2,
            ),
        ];
        let bounds = Bounds::new(1.0, 1.0, 2.0, 2.0);

        write_geopackage_features(&path, "typed", "POINT", 4326, &features, &bounds).unwrap();

        // 1) 列类型按值推断
        let conn = Connection::open(&path).unwrap();
        let types: Vec<(String, String)> = {
            let mut stmt = conn.prepare("PRAGMA table_info(\"typed\")").unwrap();
            let rows = stmt
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2).unwrap_or_default(),
                    ))
                })
                .unwrap();
            rows.filter_map(|r| r.ok()).collect()
        };
        let get_type = |col: &str| {
            types
                .iter()
                .find(|(n, _)| n == col)
                .map(|(_, t)| t.as_str())
        };
        assert_eq!(get_type("name"), Some("TEXT"));
        assert_eq!(get_type("count"), Some("INTEGER"));
        assert_eq!(get_type("price"), Some("REAL"));
        assert_eq!(get_type("active"), Some("BOOLEAN"));

        // 2) 读回类型保持一致
        let result = read_geopackage_layer_features(&path, "typed", None).unwrap();
        assert_eq!(result.features.len(), 2);

        let p0 = &result.features[0].properties;
        match p0.get("name") {
            Some(PropertyValue::String(v)) => assert_eq!(v, "alpha"),
            other => panic!("name 应为 String, 实际: {:?}", other),
        }
        match p0.get("count") {
            Some(PropertyValue::Integer(v)) => assert_eq!(*v, 10),
            other => panic!("count 应为 Integer, 实际: {:?}", other),
        }
        match p0.get("price") {
            Some(PropertyValue::Number(v)) => assert!((*v - 9.5).abs() < 1e-6),
            other => panic!("price 应为 Number, 实际: {:?}", other),
        }
        match p0.get("active") {
            Some(PropertyValue::Boolean(v)) => assert!(*v),
            other => panic!("active 应为 Boolean, 实际: {:?}", other),
        }

        let p1 = &result.features[1].properties;
        match p1.get("active") {
            Some(PropertyValue::Boolean(v)) => assert!(!*v),
            other => panic!("beta.active 应为 false, 实际: {:?}", other),
        }

        std::fs::remove_dir_all(&dir).ok();
    }
}
