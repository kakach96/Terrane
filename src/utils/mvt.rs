//! Mapbox Vector Tile (MVT) 编码器
//!
//! 纯 Rust 实现 MVT 2.1 规范的 Protocol Buffers 编码，
//! 无需 prost/protobuf 编译依赖。

use crate::models::{Bounds, Feature, GeoJsonGeometry, PropertyValue};
use std::collections::HashMap;

// ==================== Protobuf 编码基础 ====================

/// Protocol Buffers wire types
const WIRE_VARINT: u8 = 0;
const WIRE_FIXED64: u8 = 1;
const WIRE_LENGTH_DELIMITED: u8 = 2;
const WIRE_FIXED32: u8 = 5;

/// 编码 varint (无符号)
fn encode_varint(mut value: u64) -> Vec<u8> {
    let mut buf = Vec::new();
    loop {
        if value < 0x80 {
            buf.push(value as u8);
            break;
        } else {
            buf.push((value as u8 & 0x7F) | 0x80);
            value >>= 7;
        }
    }
    buf
}

/// ZigZag 编码 (有符号整数 → 无符号)
fn zigzag(value: i64) -> u64 {
    ((value << 1) ^ (value >> 63)) as u64
}

/// 编码一个 tagged varint 字段
fn field_varint(tag: u32, value: u64) -> Vec<u8> {
    let mut buf = encode_varint(((tag as u64) << 3) | (WIRE_VARINT as u64));
    buf.extend(encode_varint(value));
    buf
}

/// 编码一个 tagged fixed32 字段
fn field_fixed32(tag: u32, value: u32) -> Vec<u8> {
    let mut buf = encode_varint(((tag as u64) << 3) | (WIRE_FIXED32 as u64));
    buf.extend(value.to_le_bytes());
    buf
}

/// 编码一个 tagged 长度分隔字段
fn field_length_delimited(tag: u32, data: &[u8]) -> Vec<u8> {
    let mut buf = encode_varint(((tag as u64) << 3) | (WIRE_LENGTH_DELIMITED as u64));
    buf.extend(encode_varint(data.len() as u64));
    buf.extend(data);
    buf
}

// ==================== MVT 几何编码 ====================

/// MVT 几何类型
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u32)]
pub enum MvtGeomType {
    Point = 1,
    LineString = 2,
    Polygon = 3,
}

/// 几何命令 id
const CMD_MOVE_TO: u32 = 1;
const CMD_LINE_TO: u32 = 2;
const CMD_CLOSE_PATH: u32 = 7;

/// 编码命令整数 (id | (count << 3))
fn cmd_integer(id: u32, count: u32) -> u32 {
    id | (count << 3)
}

/// 编码几何命令参数 (参数序列用 zigzag+varint 编码)
fn encode_geom_params(params: &[i64]) -> Vec<u8> {
    let mut buf = Vec::new();
    for p in params {
        buf.extend(encode_varint(zigzag(*p)));
    }
    buf
}

// ==================== MVT Value 编码 ====================

/// MVT 字段值 (protobuf oneof)
fn encode_mvt_value_string(s: &str) -> Vec<u8> {
    let s_bytes = s.as_bytes();
    field_length_delimited(1, s_bytes) // string_value (field 1)
}

fn encode_mvt_value_float(v: f32) -> Vec<u8> {
    field_fixed32(2, v.to_bits()) // float_value (field 2)
}

fn encode_mvt_value_double(v: f64) -> Vec<u8> {
    let mut buf = encode_varint(((3u64) << 3) | (WIRE_FIXED64 as u64)); // double_value (field 3)
    buf.extend(v.to_le_bytes());
    buf
}

fn encode_mvt_value_int(v: i64) -> Vec<u8> {
    field_varint(4, v as u64) // int_value (field 4, unsigned)
}

fn encode_mvt_value_uint(v: u64) -> Vec<u8> {
    field_varint(5, v) // uint_value (field 5)
}

fn encode_mvt_value_bool(v: bool) -> Vec<u8> {
    field_varint(6, if v { 1 } else { 0 }) // bool_value (field 6)
}

/// 将 PropertyValue 编码为 MVT Value
fn property_value_to_mvt(value: &PropertyValue) -> Vec<u8> {
    match value {
        PropertyValue::String(s) => encode_mvt_value_string(s),
        PropertyValue::Number(n) => {
            // 判断是否为整数
            if n.fract() == 0.0 {
                encode_mvt_value_int(*n as i64)
            } else {
                encode_mvt_value_double(*n)
            }
        },
        PropertyValue::Boolean(b) => encode_mvt_value_bool(*b),
        PropertyValue::Integer(i) => encode_mvt_value_int(*i),
        PropertyValue::Null => encode_mvt_value_string("null"),
        PropertyValue::Array(_) => encode_mvt_value_string(""),
        PropertyValue::Object(_) => encode_mvt_value_string(""),
    }
}

// ==================== MVT Feature 编码 ====================

/// 将 GeoJSON 坐标转换为 MVT 瓦片坐标 (整数)
///
/// `extent` 默认为 4096
fn project_to_tile(coords: &[f64], bbox: &Bounds, extent: u32) -> (i64, i64) {
    if coords.len() < 2 {
        return (0, 0);
    }
    let x = coords[0];
    let y = coords[1];
    let extent_f = extent as f64;

    let tx = ((x - bbox.minx) / (bbox.maxx - bbox.minx) * extent_f) as i64;
    let ty = ((bbox.maxy - y) / (bbox.maxy - bbox.miny) * extent_f) as i64;
    (tx.clamp(0, extent as i64), ty.clamp(0, extent as i64))
}

/// 编码几何为 MVT 几何命令序列
fn encode_geometry(geom: &GeoJsonGeometry, bbox: &Bounds, extent: u32) -> (MvtGeomType, Vec<u8>) {
    match geom {
        GeoJsonGeometry::Point { coordinates } => {
            let (tx, ty) = project_to_tile(coordinates, bbox, extent);
            let mut buf = Vec::new();
            // MoveTo (1 point)
            buf.extend(encode_varint(cmd_integer(CMD_MOVE_TO, 1) as u64));
            buf.extend(encode_geom_params(&[tx, ty]));
            (MvtGeomType::Point, buf)
        },
        GeoJsonGeometry::MultiPoint { coordinates } => {
            let mut buf = Vec::new();
            let mut params = Vec::new();
            let mut cursor_x = 0i64;
            let mut cursor_y = 0i64;
            for coord in coordinates {
                let (tx, ty) = project_to_tile(coord, bbox, extent);
                params.push(tx - cursor_x);
                params.push(ty - cursor_y);
                cursor_x = tx;
                cursor_y = ty;
            }
            buf.extend(encode_varint(
                cmd_integer(CMD_MOVE_TO, coordinates.len() as u32) as u64,
            ));
            buf.extend(encode_geom_params(&params));
            (MvtGeomType::Point, buf)
        },
        GeoJsonGeometry::LineString { coordinates } => {
            let mut buf = Vec::new();
            if coordinates.is_empty() {
                return (MvtGeomType::LineString, buf);
            }
            let (sx, sy) = project_to_tile(&coordinates[0], bbox, extent);
            buf.extend(encode_varint(cmd_integer(CMD_MOVE_TO, 1) as u64));
            buf.extend(encode_geom_params(&[sx, sy]));

            let mut params = Vec::new();
            let mut cursor_x = sx;
            let mut cursor_y = sy;
            for coord in &coordinates[1..] {
                let (tx, ty) = project_to_tile(coord, bbox, extent);
                params.push(tx - cursor_x);
                params.push(ty - cursor_y);
                cursor_x = tx;
                cursor_y = ty;
            }
            buf.extend(encode_varint(
                cmd_integer(CMD_LINE_TO, params.len() as u32 / 2) as u64,
            ));
            buf.extend(encode_geom_params(&params));
            (MvtGeomType::LineString, buf)
        },
        GeoJsonGeometry::MultiLineString { coordinates } => {
            let mut buf = Vec::new();
            for line in coordinates {
                if line.is_empty() {
                    continue;
                }
                let (sx, sy) = project_to_tile(&line[0], bbox, extent);
                buf.extend(encode_varint(cmd_integer(CMD_MOVE_TO, 1) as u64));
                buf.extend(encode_geom_params(&[sx, sy]));

                let mut params = Vec::new();
                let mut cursor_x = sx;
                let mut cursor_y = sy;
                for coord in &line[1..] {
                    let (tx, ty) = project_to_tile(coord, bbox, extent);
                    params.push(tx - cursor_x);
                    params.push(ty - cursor_y);
                    cursor_x = tx;
                    cursor_y = ty;
                }
                buf.extend(encode_varint(
                    cmd_integer(CMD_LINE_TO, params.len() as u32 / 2) as u64,
                ));
                buf.extend(encode_geom_params(&params));
            }
            (MvtGeomType::LineString, buf)
        },
        GeoJsonGeometry::Polygon { coordinates } => {
            let mut buf = Vec::new();
            for ring in coordinates {
                if ring.len() < 4 {
                    continue;
                }
                let (sx, sy) = project_to_tile(&ring[0], bbox, extent);
                buf.extend(encode_varint(cmd_integer(CMD_MOVE_TO, 1) as u64));
                buf.extend(encode_geom_params(&[sx, sy]));

                let mut params = Vec::new();
                let mut cursor_x = sx;
                let mut cursor_y = sy;
                // 不编码最后一个点(回到起点)
                let end = ring.len() - 1;
                for coord in &ring[1..end] {
                    let (tx, ty) = project_to_tile(coord, bbox, extent);
                    params.push(tx - cursor_x);
                    params.push(ty - cursor_y);
                    cursor_x = tx;
                    cursor_y = ty;
                }
                buf.extend(encode_varint(
                    cmd_integer(CMD_LINE_TO, params.len() as u32 / 2) as u64,
                ));
                buf.extend(encode_geom_params(&params));
                // ClosePath
                buf.extend(encode_varint(cmd_integer(CMD_CLOSE_PATH, 1) as u64));
            }
            (MvtGeomType::Polygon, buf)
        },
        GeoJsonGeometry::MultiPolygon { coordinates } => {
            let mut buf = Vec::new();
            for polygon in coordinates {
                for ring in polygon {
                    if ring.len() < 4 {
                        continue;
                    }
                    let (sx, sy) = project_to_tile(&ring[0], bbox, extent);
                    buf.extend(encode_varint(cmd_integer(CMD_MOVE_TO, 1) as u64));
                    buf.extend(encode_geom_params(&[sx, sy]));

                    let mut params = Vec::new();
                    let mut cursor_x = sx;
                    let mut cursor_y = sy;
                    let end = ring.len() - 1;
                    for coord in &ring[1..end] {
                        let (tx, ty) = project_to_tile(coord, bbox, extent);
                        params.push(tx - cursor_x);
                        params.push(ty - cursor_y);
                        cursor_x = tx;
                        cursor_y = ty;
                    }
                    buf.extend(encode_varint(
                        cmd_integer(CMD_LINE_TO, params.len() as u32 / 2) as u64,
                    ));
                    buf.extend(encode_geom_params(&params));
                    buf.extend(encode_varint(cmd_integer(CMD_CLOSE_PATH, 1) as u64));
                }
            }
            (MvtGeomType::Polygon, buf)
        },
        GeoJsonGeometry::GeometryCollection { geometries } => {
            // 递归编码第一个几何
            for g in geometries {
                return encode_geometry(g, bbox, extent);
            }
            (MvtGeomType::Point, Vec::new())
        },
    }
}

// ==================== MVT Tile 构建 ====================

/// 构建 MVT tile 的 protobuf 字节
///
/// # 参数
/// * `features` - 要素列表
/// * `layer_name` - 图层名称
/// * `bbox` - 瓦片对应的地理范围
/// * `extent` - 瓦片坐标范围 (默认 4096)
pub fn encode_tile(features: &[Feature], layer_name: &str, bbox: &Bounds, extent: u32) -> Vec<u8> {
    // ========== 构建 Layer ==========
    // 收集 keys 和 values (去重字典)
    let mut keys: Vec<String> = Vec::new();
    let mut values: Vec<Vec<u8>> = Vec::new();
    let mut key_index: HashMap<String, u32> = HashMap::new();
    let mut value_index: HashMap<u64, u32> = HashMap::new(); // 用 hash 去重

    // 预计算所有 tags
    let mut feature_tags: Vec<Vec<u32>> = Vec::new();

    for feature in features {
        let mut tags = Vec::new();
        for (k, v) in &feature.properties {
            let kidx = match key_index.get(k) {
                Some(idx) => *idx,
                None => {
                    let idx = keys.len() as u32;
                    keys.push(k.clone());
                    key_index.insert(k.clone(), idx);
                    idx
                },
            };

            let value_bytes = property_value_to_mvt(v);
            // 用字节 hash 做简单去重
            let value_hash = simple_hash(&value_bytes);
            let vidx = match value_index.get(&value_hash) {
                Some(idx) => *idx,
                None => {
                    let idx = values.len() as u32;
                    values.push(value_bytes);
                    value_index.insert(value_hash, idx);
                    idx
                },
            };

            tags.push(kidx);
            tags.push(vidx);
        }
        feature_tags.push(tags);
    }

    // 构建 Layer protobuf
    let mut layer_buf = Vec::new();

    // field 1 (name): string
    layer_buf.extend(field_length_delimited(1, layer_name.as_bytes()));

    // field 2 (features): repeated Feature
    for (fi, feature) in features.iter().enumerate() {
        let (geom_type, geom_data) = encode_geometry(&feature.geometry, bbox, extent);
        let mut feature_buf = Vec::new();

        // field 1 (id): uint64 - 使用 feature.id 的 hash
        let id = feature
            .id
            .parse::<u64>()
            .unwrap_or_else(|_| simple_hash(feature.id.as_bytes()));
        feature_buf.extend(field_varint(1, id));

        // field 2 (tags): repeated uint32
        for tag in &feature_tags[fi] {
            feature_buf.extend(field_varint(2, *tag as u64));
        }

        // field 3 (type): GeomType enum
        feature_buf.extend(field_varint(3, geom_type as u64));

        // field 4 (geometry): bytes
        feature_buf.extend(field_length_delimited(4, &geom_data));

        layer_buf.extend(field_length_delimited(2, &feature_buf));
    }

    // field 3 (keys): repeated string
    for key in &keys {
        layer_buf.extend(field_length_delimited(3, key.as_bytes()));
    }

    // field 4 (values): repeated Value
    for value in &values {
        layer_buf.extend(field_length_delimited(4, value));
    }

    // field 5 (extent): uint32 (optional, 默认 4096)
    layer_buf.extend(field_varint(5, extent as u64));

    // ========== 构建 Tile ==========
    let mut tile_buf = Vec::new();
    // field 3 (layers): repeated Layer
    tile_buf.extend(field_length_delimited(3, &layer_buf));

    tile_buf
}

/// 简单字符串哈希
fn simple_hash(data: &[u8]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    data.hash(&mut hasher);
    hasher.finish()
}

/// 计算瓦片的地理边界
///
/// 使用 Web Mercator (EPSG:3857) 瓦片方案
pub fn tile_bounds(z: u32, x: u32, y: u32) -> Bounds {
    let n = (1u64 << z) as f64;
    let minx = (x as f64 / n) * 360.0 - 180.0;
    let maxx = ((x + 1) as f64 / n) * 360.0 - 180.0;
    let miny_rad = (std::f64::consts::PI * (1.0 - 2.0 * (y + 1) as f64 / n))
        .sin()
        .asinh();
    let maxy_rad = (std::f64::consts::PI * (1.0 - 2.0 * y as f64 / n))
        .sin()
        .asinh();
    let miny = miny_rad.to_degrees();
    let maxy = maxy_rad.to_degrees();
    Bounds::new(minx, miny, maxx, maxy)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_varint() {
        assert_eq!(encode_varint(0), vec![0]);
        assert_eq!(encode_varint(1), vec![1]);
        assert_eq!(encode_varint(300), vec![0xAC, 0x02]);
    }

    #[test]
    fn test_zigzag() {
        assert_eq!(zigzag(0), 0);
        assert_eq!(zigzag(-1), 1);
        assert_eq!(zigzag(1), 2);
        assert_eq!(zigzag(-2), 3);
    }

    #[test]
    fn test_tile_bounds() {
        let b = tile_bounds(0, 0, 0);
        assert!(b.minx < -179.0);
        assert!(b.maxx > 179.0);
    }

    #[test]
    fn test_encode_empty_tile() {
        let b = tile_bounds(0, 0, 0);
        let result = encode_tile(&[], "test", &b, 4096);
        assert!(!result.is_empty());
        // Tile protobuf 应该包含 layers 字段
        assert!(result.len() > 2);
    }

    #[test]
    fn test_encode_point_feature() {
        let b = tile_bounds(0, 0, 0);
        let mut props = std::collections::HashMap::new();
        props.insert(
            "name".to_string(),
            PropertyValue::String("test".to_string()),
        );
        let feature = Feature::new(
            GeoJsonGeometry::Point {
                coordinates: vec![0.0, 0.0],
            },
            props,
        );
        let result = encode_tile(&[feature], "test", &b, 4096);
        assert!(!result.is_empty());
    }
}
