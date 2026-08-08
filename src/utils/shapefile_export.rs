//! Hand-written ESRI Shapefile export (`.shp` / `.shx` / `.dbf` / `.prj`) plus
//! ZIP packaging, used by WFS `GetFeature` `OUTPUTFORMAT=SHAPE-ZIP`.
//!
//! The binary layouts follow the ESRI Shapefile Technical Description and the
//! dBASE III table format. Everything is produced in memory (no temp files),
//! so the result can be zipped straight into an `application/zip` response.
//! The writer mirrors the reader in `utils/shapefile.rs` (a round-trip is
//! verified by unit tests below).

use crate::models::{Bounds, Feature, GeoJsonGeometry, PropertyValue};
use std::io::{Cursor, Write};

// ESRI shape types (2D).
const SHAPE_TYPE_NULL: i32 = 0;
const SHAPE_TYPE_POINT: i32 = 1;
const SHAPE_TYPE_POLYLINE: i32 = 3;
const SHAPE_TYPE_POLYGON: i32 = 5;
const SHAPE_TYPE_MULTIPOINT: i32 = 8;

/// A complete shapefile package: the three mandatory binary files plus the
/// projection file.
#[derive(Debug, Clone)]
pub struct ShapefilePackage {
    pub shp: Vec<u8>,
    pub shx: Vec<u8>,
    pub dbf: Vec<u8>,
    pub prj: Vec<u8>,
}

/// The WGS 84 projection WKT emitted for EPSG:4326 layers.
pub const WGS84_WKT: &str = "GEOGCS[\"WGS 84\",DATUM[\"WGS_1984\",SPHEROID[\"WGS 84\",6378137,298.257223563]],PRIMEM[\"Greenwich\",0],UNIT[\"degree\",0.0174532925199433]]";

/// The Web-Mercator projection WKT emitted for EPSG:3857 layers.
pub const WEB_MERCATOR_WKT: &str = "PROJCS[\"WGS 84 / Pseudo-Mercator\",GEOGCS[\"WGS 84\",DATUM[\"WGS_1984\",SPHEROID[\"WGS 84\",6378137,298.257223563]],PRIMEM[\"Greenwich\",0],UNIT[\"degree\",0.0174532925199433]],PROJECTION[\"Mercator_1SP\"],PARAMETER[\"central_meridian\",0],PARAMETER[\"scale_factor\",1],PARAMETER[\"false_easting\",0],PARAMETER[\"false_northing\",0],UNIT[\"metre\",1]]";

/// Resolve the ESRI shape type for a geometry (used to pick the file-wide type).
fn geometry_shape_type(g: &GeoJsonGeometry) -> Option<i32> {
    match g {
        GeoJsonGeometry::Point { .. } => Some(SHAPE_TYPE_POINT),
        GeoJsonGeometry::MultiPoint { .. } => Some(SHAPE_TYPE_MULTIPOINT),
        GeoJsonGeometry::LineString { .. } | GeoJsonGeometry::MultiLineString { .. } => {
            Some(SHAPE_TYPE_POLYLINE)
        },
        GeoJsonGeometry::Polygon { .. } | GeoJsonGeometry::MultiPolygon { .. } => {
            Some(SHAPE_TYPE_POLYGON)
        },
        GeoJsonGeometry::GeometryCollection { geometries } => {
            geometries.iter().find_map(geometry_shape_type)
        },
    }
}

/// The first point of a geometry (for Point-target conversion and bbox).
fn first_point(g: &GeoJsonGeometry) -> Option<[f64; 2]> {
    match g {
        GeoJsonGeometry::Point { coordinates } if coordinates.len() >= 2 => {
            Some([coordinates[0], coordinates[1]])
        },
        GeoJsonGeometry::MultiPoint { coordinates }
        | GeoJsonGeometry::LineString { coordinates } => coordinates
            .iter()
            .find(|c| c.len() >= 2)
            .map(|c| [c[0], c[1]]),
        GeoJsonGeometry::Polygon { coordinates } => coordinates
            .first()
            .and_then(|ring| ring.iter().find(|c| c.len() >= 2))
            .map(|c| [c[0], c[1]]),
        GeoJsonGeometry::MultiLineString { coordinates } => coordinates
            .iter()
            .find_map(|line| line.iter().find(|c| c.len() >= 2).map(|c| [c[0], c[1]])),
        GeoJsonGeometry::MultiPolygon { coordinates } => coordinates.iter().find_map(|poly| {
            poly.first()
                .and_then(|ring| ring.iter().find(|c| c.len() >= 2).map(|c| [c[0], c[1]]))
        }),
        GeoJsonGeometry::GeometryCollection { geometries } => {
            geometries.iter().find_map(first_point)
        },
        _ => None,
    }
}

/// Flatten every coordinate pair of a geometry (for MultiPoint-target output).
fn collect_points(g: &GeoJsonGeometry, out: &mut Vec<(f64, f64)>) {
    match g {
        GeoJsonGeometry::Point { coordinates } if coordinates.len() >= 2 => {
            out.push((coordinates[0], coordinates[1]));
        },
        GeoJsonGeometry::MultiPoint { coordinates }
        | GeoJsonGeometry::LineString { coordinates } => {
            for c in coordinates {
                if c.len() >= 2 {
                    out.push((c[0], c[1]));
                }
            }
        },
        GeoJsonGeometry::Polygon { coordinates }
        | GeoJsonGeometry::MultiLineString { coordinates } => {
            for ring in coordinates {
                for c in ring {
                    if c.len() >= 2 {
                        out.push((c[0], c[1]));
                    }
                }
            }
        },
        GeoJsonGeometry::MultiPolygon { coordinates } => {
            for poly in coordinates {
                for ring in poly {
                    for c in ring {
                        if c.len() >= 2 {
                            out.push((c[0], c[1]));
                        }
                    }
                }
            }
        },
        GeoJsonGeometry::GeometryCollection { geometries } => {
            for sub in geometries {
                collect_points(sub, out);
            }
        },
        _ => {},
    }
}

/// Collect per-part points (for PolyLine/Polygon-target output). A part is one
/// contiguous sequence of coordinates.
fn collect_parts(g: &GeoJsonGeometry, out: &mut Vec<Vec<(f64, f64)>>) {
    match g {
        GeoJsonGeometry::LineString { coordinates } => {
            let pts: Vec<(f64, f64)> = coordinates
                .iter()
                .filter(|c| c.len() >= 2)
                .map(|c| (c[0], c[1]))
                .collect();
            if !pts.is_empty() {
                out.push(pts);
            }
        },
        GeoJsonGeometry::MultiLineString { coordinates } => {
            for line in coordinates {
                let pts: Vec<(f64, f64)> = line
                    .iter()
                    .filter(|c| c.len() >= 2)
                    .map(|c| (c[0], c[1]))
                    .collect();
                if !pts.is_empty() {
                    out.push(pts);
                }
            }
        },
        GeoJsonGeometry::Polygon { coordinates } => {
            for ring in coordinates {
                let pts: Vec<(f64, f64)> = ring
                    .iter()
                    .filter(|c| c.len() >= 2)
                    .map(|c| (c[0], c[1]))
                    .collect();
                if !pts.is_empty() {
                    out.push(pts);
                }
            }
        },
        GeoJsonGeometry::MultiPolygon { coordinates } => {
            for poly in coordinates {
                for ring in poly {
                    let pts: Vec<(f64, f64)> = ring
                        .iter()
                        .filter(|c| c.len() >= 2)
                        .map(|c| (c[0], c[1]))
                        .collect();
                    if !pts.is_empty() {
                        out.push(pts);
                    }
                }
            }
        },
        GeoJsonGeometry::Point { .. } | GeoJsonGeometry::MultiPoint { .. } => {
            // A single-point part is valid for polyline targets too.
            let mut pts = Vec::new();
            collect_points(g, &mut pts);
            if !pts.is_empty() {
                out.push(pts);
            }
        },
        GeoJsonGeometry::GeometryCollection { geometries } => {
            for sub in geometries {
                collect_parts(sub, out);
            }
        },
    }
}

fn points_bbox(points: &[(f64, f64)]) -> Bounds {
    let mut b = Bounds::new(
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    );
    for (x, y) in points {
        b.minx = b.minx.min(*x);
        b.miny = b.miny.min(*y);
        b.maxx = b.maxx.max(*x);
        b.maxy = b.maxy.max(*y);
    }
    b
}

// ---------------------------------------------------------------------------
// Little/big-endian helpers
// ---------------------------------------------------------------------------

fn be_i32(v: &mut Vec<u8>, x: i32) {
    v.extend_from_slice(&x.to_be_bytes());
}

fn le_i32(v: &mut Vec<u8>, x: i32) {
    v.extend_from_slice(&x.to_le_bytes());
}

fn le_f64(v: &mut Vec<u8>, x: f64) {
    v.extend_from_slice(&x.to_le_bytes());
}

// ---------------------------------------------------------------------------
// .shp / .shx
// ---------------------------------------------------------------------------

/// The shared 100-byte file header (`.shp` and `.shx`).
fn file_header(shape_type: i32, bbox: &Bounds, length_words: i32) -> Vec<u8> {
    let mut h = Vec::with_capacity(100);
    be_i32(&mut h, 9994);
    h.extend_from_slice(&[0u8; 20]); // 5 unused 32-bit ints
    be_i32(&mut h, length_words);
    le_i32(&mut h, 1000); // version
    le_i32(&mut h, shape_type);
    le_f64(&mut h, bbox.minx);
    le_f64(&mut h, bbox.miny);
    le_f64(&mut h, bbox.maxx);
    le_f64(&mut h, bbox.maxy);
    le_f64(&mut h, 0.0); // z min
    le_f64(&mut h, 0.0); // z max
    le_f64(&mut h, 0.0); // m min
    le_f64(&mut h, 0.0); // m max
    h
}

/// Encode a single record's content (shape type + geometry) in the file-wide
/// `target` shape type. Returns `(content, record bbox)`.
fn encode_shape_content(geom: &GeoJsonGeometry, target: i32) -> (Vec<u8>, Bounds) {
    match target {
        SHAPE_TYPE_POINT => match first_point(geom) {
            Some([x, y]) => {
                let mut c = Vec::with_capacity(20);
                le_i32(&mut c, SHAPE_TYPE_POINT);
                le_f64(&mut c, x);
                le_f64(&mut c, y);
                (c, Bounds::new(x, y, x, y))
            },
            None => (vec![0u8; 4], empty_bbox()),
        },
        SHAPE_TYPE_MULTIPOINT => {
            let mut pts = Vec::new();
            collect_points(geom, &mut pts);
            if pts.is_empty() {
                return (vec![0u8; 4], empty_bbox());
            }
            let b = points_bbox(&pts);
            let mut c = Vec::new();
            le_i32(&mut c, SHAPE_TYPE_MULTIPOINT);
            le_f64(&mut c, b.minx);
            le_f64(&mut c, b.miny);
            le_f64(&mut c, b.maxx);
            le_f64(&mut c, b.maxy);
            le_i32(&mut c, pts.len() as i32);
            for (x, y) in &pts {
                le_f64(&mut c, *x);
                le_f64(&mut c, *y);
            }
            (c, b)
        },
        SHAPE_TYPE_POLYLINE | SHAPE_TYPE_POLYGON => {
            let mut parts = Vec::new();
            collect_parts(geom, &mut parts);
            // For polygons every ring must be closed (first == last point).
            if target == SHAPE_TYPE_POLYGON {
                for part in parts.iter_mut() {
                    if let (Some(first), Some(last)) = (part.first(), part.last()) {
                        if (first.0 - last.0).abs() > 1e-9 || (first.1 - last.1).abs() > 1e-9 {
                            let first = *first;
                            part.push(first);
                        }
                    }
                }
            }
            let mut pts = Vec::new();
            let mut parts_index = Vec::new();
            for part in &parts {
                parts_index.push(pts.len() as i32);
                pts.extend_from_slice(part);
            }
            if pts.is_empty() {
                return (vec![0u8; 4], empty_bbox());
            }
            let b = points_bbox(&pts);
            let mut c = Vec::new();
            le_i32(&mut c, target);
            le_f64(&mut c, b.minx);
            le_f64(&mut c, b.miny);
            le_f64(&mut c, b.maxx);
            le_f64(&mut c, b.maxy);
            le_i32(&mut c, parts.len() as i32);
            le_i32(&mut c, pts.len() as i32);
            for p in &parts_index {
                le_i32(&mut c, *p);
            }
            for (x, y) in &pts {
                le_f64(&mut c, *x);
                le_f64(&mut c, *y);
            }
            (c, b)
        },
        _ => (vec![0u8; 4], empty_bbox()),
    }
}

fn empty_bbox() -> Bounds {
    Bounds::new(0.0, 0.0, 0.0, 0.0)
}

fn grow_bbox(acc: &mut Bounds, b: &Bounds) {
    if b.minx <= b.maxx && b.miny <= b.maxy {
        acc.minx = acc.minx.min(b.minx);
        acc.miny = acc.miny.min(b.miny);
        acc.maxx = acc.maxx.max(b.maxx);
        acc.maxy = acc.maxy.max(b.maxy);
    }
}

/// Build the `.shp` binary from features using the given file-wide shape type.
fn build_shp(features: &[Feature], shape_type: i32) -> Vec<u8> {
    let mut records: Vec<Vec<u8>> = Vec::new();
    let mut bbox = Bounds::new(
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    );
    for (i, feature) in features.iter().enumerate() {
        let (content, rb) = encode_shape_content(&feature.geometry, shape_type);
        grow_bbox(&mut bbox, &rb);
        let mut rec = Vec::new();
        be_i32(&mut rec, (i + 1) as i32); // record number
        be_i32(&mut rec, (content.len() / 2) as i32); // content length in words
        rec.extend_from_slice(&content);
        records.push(rec);
    }
    if bbox.minx == f64::INFINITY {
        bbox = empty_bbox();
    }
    let content_words: i32 = records.iter().map(|r| (r.len() / 2) as i32).sum();
    let header_words: i32 = 50; // 100 bytes
    let total_words = header_words + content_words;
    let mut shp = file_header(shape_type, &bbox, total_words);
    for r in &records {
        shp.extend_from_slice(r);
    }
    shp
}

/// Build the `.shx` binary (index of the `.shp` records).
fn build_shx(features: &[Feature], shape_type: i32) -> Vec<u8> {
    let mut bbox = Bounds::new(
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    );
    let mut offsets: Vec<(i32, i32)> = Vec::new();
    let mut offset_words: i32 = 50; // records start after the 100-byte header
    for feature in features {
        let (content, rb) = encode_shape_content(&feature.geometry, shape_type);
        grow_bbox(&mut bbox, &rb);
        let content_words = (content.len() / 2) as i32;
        offsets.push((offset_words, content_words));
        offset_words += content_words + 4; // record header is 8 bytes = 4 words
    }
    if bbox.minx == f64::INFINITY {
        bbox = empty_bbox();
    }
    let shx_words: i32 = 50 + (offsets.len() as i32) * 4;
    let mut shx = file_header(shape_type, &bbox, shx_words);
    for (offset, len) in offsets {
        be_i32(&mut shx, offset);
        be_i32(&mut shx, len);
    }
    shx
}

// ---------------------------------------------------------------------------
// .dbf (dBASE III attribute table)
// ---------------------------------------------------------------------------

/// A single dBASE field descriptor.
struct DbfField {
    name: String,
    typ: char, // 'C' | 'N' | 'L'
    length: u8,
    decimals: u8,
}

/// Infer the union of attributes across features (insertion order) and their
/// field types / widths.
fn infer_fields(features: &[Feature]) -> Vec<DbfField> {
    let mut order: Vec<String> = Vec::new();
    for f in features {
        for k in f.properties.keys() {
            if !order.iter().any(|o| o == k) {
                order.push(k.clone());
            }
        }
    }

    let mut fields = Vec::new();
    for name in order {
        // Type from the first non-null value.
        let mut typ = 'C';
        for f in features {
            if let Some(v) = f.properties.get(&name) {
                if !matches!(v, PropertyValue::Null) {
                    typ = match v {
                        PropertyValue::String(_) => 'C',
                        PropertyValue::Integer(_) | PropertyValue::Number(_) => 'N',
                        PropertyValue::Boolean(_) => 'L',
                        _ => 'C',
                    };
                    break;
                }
            }
        }
        // Width from the widest value.
        let mut width = 1usize;
        for f in features {
            if let Some(v) = f.properties.get(&name) {
                let w = match v {
                    PropertyValue::String(s) => s.chars().count(),
                    PropertyValue::Integer(i) => i.to_string().len(),
                    PropertyValue::Number(n) => {
                        let s = format!("{:.8}", n);
                        s.trim_end_matches('0').trim_end_matches('.').len()
                    },
                    PropertyValue::Boolean(_) => 1,
                    _ => 1,
                };
                width = width.max(w);
            }
        }
        if typ == 'N' {
            width = (width + 1).max(4); // space for a sign / decimal point
        }
        let length = width.clamp(1, 254) as u8;
        let decimals = if typ == 'N' { 8 } else { 0 };
        fields.push(DbfField {
            name,
            typ,
            length,
            decimals,
        });
    }
    fields
}

/// Encode a single dBASE value right/left-padded to the field width.
fn encode_dbf_value(value: Option<&PropertyValue>, field: &DbfField) -> Vec<u8> {
    let width = field.length as usize;
    let mut buf = vec![b' '; width];
    match (value, field.typ) {
        (Some(PropertyValue::String(s)), _) => {
            let bytes = s.as_bytes();
            let n = bytes.len().min(width);
            buf[..n].copy_from_slice(&bytes[..n]);
        },
        (Some(PropertyValue::Integer(i)), 'N') => {
            let s = i.to_string();
            let n = s.len().min(width);
            let start = width - n;
            buf[start..].copy_from_slice(&s.as_bytes()[..n]);
        },
        (Some(PropertyValue::Number(num)), 'N') => {
            let s = format!("{:.8}", num);
            let s = s.trim_end_matches('0').trim_end_matches('.');
            let n = s.len().min(width);
            let start = width - n;
            buf[start..].copy_from_slice(&s.as_bytes()[..n]);
        },
        (Some(PropertyValue::Boolean(b)), 'L') => buf[0] = if *b { b'T' } else { b'F' },
        (Some(other), _) => {
            let s = other.to_string();
            let bytes = s.as_bytes();
            let n = bytes.len().min(width);
            buf[..n].copy_from_slice(&bytes[..n]);
        },
        (None, 'L') => buf[0] = b'?',
        (None, _) => {},
    }
    buf
}

/// Build the `.dbf` binary (dBASE III).
fn build_dbf(features: &[Feature]) -> Vec<u8> {
    let fields = infer_fields(features);
    let num_fields = fields.len() as u16;
    let header_size = 32 + 32 * num_fields + 1;
    let record_size: u16 = 1 + fields.iter().map(|f| f.length as u16).sum::<u16>();
    let num_records = features.len() as u32;

    let mut out = Vec::new();
    out.push(0x03); // version
    let now = now_ymd();
    out.push(now.0);
    out.push(now.1);
    out.push(now.2);
    out.extend_from_slice(&num_records.to_le_bytes());
    out.extend_from_slice(&header_size.to_le_bytes());
    out.extend_from_slice(&record_size.to_le_bytes());
    out.extend_from_slice(&[0u8; 20]);

    for field in &fields {
        let mut desc = [0u8; 32];
        let name = field.name.as_bytes();
        let n = name.len().min(11);
        desc[..n].copy_from_slice(&name[..n]);
        desc[11] = field.typ as u8;
        desc[16] = field.length;
        desc[17] = field.decimals;
        out.extend_from_slice(&desc);
    }
    out.push(0x0D); // header terminator

    for feature in features {
        out.push(b' '); // not-deleted flag
        for field in &fields {
            out.extend_from_slice(&encode_dbf_value(
                feature.properties.get(&field.name),
                field,
            ));
        }
    }
    out.push(0x1A); // EOF marker
    out
}

/// Current UTC date as `(year - 2000, month, day)` for the dBASE header.
fn now_ymd() -> (u8, u8, u8) {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = secs / 86400;
    // Days from civil date algorithm (Howard Hinnant).
    let z = days as i64 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u8;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
    let y = if m <= 2 { y + 1 } else { y };
    ((y - 2000) as u8, m, d)
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Build a shapefile package from features. The file-wide shape type is derived
/// from the first feature's geometry; features that cannot be converted to it
/// become null shapes.
pub fn features_to_shapefile(features: &[Feature]) -> Result<ShapefilePackage, String> {
    let shape_type = features
        .iter()
        .find_map(|f| geometry_shape_type(&f.geometry))
        .unwrap_or(SHAPE_TYPE_POINT);

    let shp = build_shp(features, shape_type);
    let shx = build_shx(features, shape_type);
    let dbf = build_dbf(features);
    let prj = WGS84_WKT.to_string().into_bytes();

    Ok(ShapefilePackage { shp, shx, dbf, prj })
}

/// Package a shapefile into a ZIP archive (STORE compression) with the given
/// base file name (e.g. `archsites` → `archsites.shp` … `archsites.prj`).
pub fn zip_shapefile_package(pkg: &ShapefilePackage, base: &str) -> Result<Vec<u8>, String> {
    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    let cursor = Cursor::new(Vec::new());
    let mut writer = ZipWriter::new(cursor);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    let files: [(&str, &[u8]); 4] = [
        ("shp", &pkg.shp),
        ("shx", &pkg.shx),
        ("dbf", &pkg.dbf),
        ("prj", &pkg.prj),
    ];
    for (ext, data) in files {
        writer
            .start_file(format!("{}.{}", base, ext), options)
            .map_err(|e| format!("zip start_file failed: {}", e))?;
        writer
            .write_all(data)
            .map_err(|e| format!("zip write failed: {}", e))?;
    }
    let cursor = writer
        .finish()
        .map_err(|e| format!("zip finish failed: {}", e))?;
    Ok(cursor.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn point(x: f64, y: f64, props: HashMap<String, PropertyValue>) -> Feature {
        Feature::with_id(
            format!("f{}", y),
            GeoJsonGeometry::Point {
                coordinates: vec![x, y],
            },
            props,
        )
    }

    fn props(name: &str, cat: i64) -> HashMap<String, PropertyValue> {
        let mut m = HashMap::new();
        m.insert("name".to_string(), PropertyValue::String(name.to_string()));
        m.insert("cat".to_string(), PropertyValue::Integer(cat));
        m
    }

    fn temp_path(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("terrane-shp-{}-{}", tag, std::process::id()))
    }

    #[test]
    fn test_package_structure_point() {
        let f1 = point(-103.8, 44.38, props("A", 1));
        let f2 = point(-103.7, 44.4, props("B", 2));
        let pkg = features_to_shapefile(&[f1, f2]).unwrap();

        // .shp: 100-byte header (magic 9994 BE) + 2 records × (8 + 20) bytes.
        assert_eq!(&pkg.shp[0..4], &[0, 0, 0x27, 0x0A]);
        assert_eq!(pkg.shp.len(), 100 + 2 * 28);
        // Shape type at offset 32 (LE int).
        assert_eq!(
            i32::from_le_bytes(pkg.shp[32..36].try_into().unwrap()),
            SHAPE_TYPE_POINT
        );
        // Record 1: number 1 (BE), content length 10 words (20 bytes).
        assert_eq!(&pkg.shp[100..104], &[0, 0, 0, 1]);
        assert_eq!(&pkg.shp[104..108], &[0, 0, 0, 10]);

        // .shx: 100-byte header + 2 × 8-byte index records.
        assert_eq!(pkg.shx.len(), 100 + 16);
        // First index record offset = 50 words.
        assert_eq!(&pkg.shx[100..104], &[0, 0, 0, 50]);

        // .dbf is non-empty and contains field names + values.
        let dbf = String::from_utf8_lossy(&pkg.dbf);
        assert!(dbf.contains("name"));
        assert!(dbf.contains("cat"));
        assert!(pkg.dbf.ends_with(&[0x1A]));

        // .prj is WGS84.
        assert!(pkg.prj.starts_with(b"GEOGCS"));
    }

    #[test]
    fn test_roundtrip_read_shapefile() {
        let f1 = point(-103.8, 44.38, props("A", 1));
        let f2 = point(-103.7, 44.4, props("B", 2));
        let pkg = features_to_shapefile(&[f1, f2]).unwrap();

        let dir = temp_path("roundtrip");
        std::fs::create_dir_all(&dir).unwrap();
        let base = dir.join("points");
        std::fs::write(base.with_extension("shp"), &pkg.shp).unwrap();
        std::fs::write(base.with_extension("dbf"), &pkg.dbf).unwrap();
        std::fs::write(base.with_extension("prj"), &pkg.prj).unwrap();

        let result = crate::utils::shapefile::read_shapefile(base.with_extension("shp")).unwrap();
        assert_eq!(result.features.len(), 2);
        let f0 = &result.features[0];
        match &f0.geometry {
            GeoJsonGeometry::Point { coordinates } => {
                assert!((coordinates[0] - -103.8).abs() < 1e-9);
                assert!((coordinates[1] - 44.38).abs() < 1e-9);
            },
            _ => panic!("expected a point"),
        }
        // The existing reader stringifies dBASE values (all PropertyValue::String).
        assert!(matches!(
            f0.properties.get("cat"),
            Some(PropertyValue::String(s)) if s == "1"
        ));
        assert!(matches!(
            f0.properties.get("name"),
            Some(PropertyValue::String(s)) if s == "A"
        ));
        // CRS parsed from the .prj.
        assert_eq!(
            result.crs,
            Some(crate::models::CoordinateReferenceSystem::EPSG4326)
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_polyline_and_polygon_types() {
        let line = Feature::with_id(
            "l1".to_string(),
            GeoJsonGeometry::LineString {
                coordinates: vec![vec![0.0, 0.0], vec![1.0, 1.0], vec![2.0, 0.0]],
            },
            HashMap::new(),
        );
        let pkg = features_to_shapefile(&[line]).unwrap();
        assert_eq!(
            i32::from_le_bytes(pkg.shp[32..36].try_into().unwrap()),
            SHAPE_TYPE_POLYLINE
        );

        let poly = Feature::with_id(
            "p1".to_string(),
            GeoJsonGeometry::Polygon {
                coordinates: vec![vec![
                    vec![0.0, 0.0],
                    vec![0.0, 4.0],
                    vec![4.0, 4.0],
                    vec![4.0, 0.0],
                    vec![0.0, 0.0],
                ]],
            },
            HashMap::new(),
        );
        let pkg2 = features_to_shapefile(&[poly]).unwrap();
        assert_eq!(
            i32::from_le_bytes(pkg2.shp[32..36].try_into().unwrap()),
            SHAPE_TYPE_POLYGON
        );
    }

    #[test]
    fn test_zip_package_roundtrip() {
        let f = point(-103.8, 44.38, props("A", 1));
        let pkg = features_to_shapefile(&[f]).unwrap();
        let zip_bytes = zip_shapefile_package(&pkg, "archsites").unwrap();

        let mut archive = zip::ZipArchive::new(Cursor::new(zip_bytes)).unwrap();
        assert_eq!(archive.len(), 4);
        let mut names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();
        names.sort();
        assert_eq!(
            names,
            vec![
                "archsites.dbf".to_string(),
                "archsites.prj".to_string(),
                "archsites.shp".to_string(),
                "archsites.shx".to_string()
            ]
        );
    }
}
