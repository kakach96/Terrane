//! Shared tile-grid math used by the tile protocol surfaces (WMTS / TMS / WMS-C).
//!
//! Grid sets follow the GeoWebCache conventions:
//! - `EPSG:4326` → *global-geodetic*: at level `z` there are `2^(z+1)` columns
//!   and `2^z` rows, the origin is the top-left corner `(-180, 90)`, and a tile
//!   spans `360/2^(z+1)` degrees wide by `180/2^z` degrees tall. The horizontal
//!   resolution is `0.703125 / 2^z` degrees per pixel (a 512 px level 0).
//! - `EPSG:3857` / `EPSG:900913` → *global-mercator*: `2^z × 2^z` tiles over the
//!   Web-Mercator square with horizontal resolution `156543.03 / 2^z` m/px.
//!
//! Bounds are always returned in *degrees* (EPSG:4326) because the shared
//! `MapRenderer` pipeline renders in geographic degrees.

use crate::models::Bounds;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// The maximum zoom level exposed by the tile services.
pub const MAX_ZOOM: u32 = 18;

/// A custom gridset: global geodetic-style coverage (`-180..180 × -90..90`,
/// top-left origin) where every zoom level has its own ground resolution
/// (degrees per pixel). Registered at runtime via `register_custom_gridset`.
#[derive(Debug, Clone)]
pub struct CustomGridset {
    pub tile_width: u32,
    pub tile_height: u32,
    /// Ground resolution in degrees per pixel, indexed by zoom level.
    pub resolutions: Vec<f64>,
}

fn custom_registry() -> &'static Mutex<HashMap<String, CustomGridset>> {
    static REGISTRY: OnceLock<Mutex<HashMap<String, CustomGridset>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Register (or replace) a custom gridset by name.
pub fn register_custom_gridset(name: &str, grid: CustomGridset) {
    custom_registry()
        .lock()
        .unwrap()
        .insert(name.to_string(), grid);
}

/// Look up a registered custom gridset.
pub fn custom_gridset(name: &str) -> Option<CustomGridset> {
    custom_registry().lock().unwrap().get(name).cloned()
}

/// Whether the name refers to a registered custom gridset.
pub fn is_custom_gridset(name: &str) -> bool {
    custom_gridset(name).is_some()
}

/// A tile-grid profile description.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridProfile {
    /// `EPSG:4326` — 2^(z+1) × 2^z tiles, 0.703125/2^z deg/px.
    GlobalGeodetic,
    /// `EPSG:3857` / `EPSG:900913` — 2^z × 2^z tiles, 156543.03/2^z m/px.
    GlobalMercator,
}

/// Resolve a gridset id to a grid profile. `EPSG:900913` is accepted as an
/// alias for the Web-Mercator gridset (the name GeoWebCache uses internally).
pub fn gridset_profile(gridset: &str) -> Option<GridProfile> {
    match gridset {
        "EPSG:4326" => Some(GridProfile::GlobalGeodetic),
        "EPSG:3857" | "EPSG:900913" => Some(GridProfile::GlobalMercator),
        _ => None,
    }
}

/// Canonical gridset id (normalizes `EPSG:900913` → `EPSG:3857`).
pub fn canonical_gridset(gridset: &str) -> String {
    match gridset {
        "EPSG:900913" => "EPSG:3857".to_string(),
        other => other.to_string(),
    }
}

/// The TMS profile label for a gridset (used in capabilities/TileMap docs).
pub fn profile_label(gridset: &str) -> &'static str {
    match gridset_profile(gridset) {
        Some(GridProfile::GlobalGeodetic) => "global-geodetic",
        Some(GridProfile::GlobalMercator) => "global-mercator",
        None => "global-geodetic",
    }
}

/// Matrix width (columns) for a gridset at zoom `z`.
pub fn matrix_width(gridset: &str, z: u32) -> u32 {
    if let Some(g) = custom_gridset(gridset) {
        let res = g.resolutions.get(z as usize).copied().unwrap_or(0.0);
        if res <= 0.0 {
            return 0;
        }
        return (360.0 / (res * g.tile_width as f64)).ceil() as u32;
    }
    let n = 1u32 << z;
    match gridset_profile(gridset) {
        Some(GridProfile::GlobalGeodetic) => n * 2,
        _ => n,
    }
}

/// Matrix height (rows) for a gridset at zoom `z`.
pub fn matrix_height(gridset: &str, z: u32) -> u32 {
    if let Some(g) = custom_gridset(gridset) {
        let res = g.resolutions.get(z as usize).copied().unwrap_or(0.0);
        if res <= 0.0 {
            return 0;
        }
        return (180.0 / (res * g.tile_height as f64)).ceil() as u32;
    }
    1u32 << z
}

/// Horizontal resolution (map units per pixel) for a gridset at zoom `z`.
pub fn units_per_pixel(gridset: &str, z: u32) -> f64 {
    if let Some(g) = custom_gridset(gridset) {
        return g.resolutions.get(z as usize).copied().unwrap_or(0.0);
    }
    match gridset_profile(gridset) {
        // 360 deg across 2^(z+1) * 256 px → 0.703125 / 2^z
        Some(GridProfile::GlobalGeodetic) => 360.0 / (256.0 * (2u64 << z) as f64),
        // 40075016.68 m across 2^z * 256 px → 156543.03 / 2^z
        _ => 40075016.68 / (256.0 * (1u64 << z) as f64),
    }
}

/// Web-Mercator latitude for a normalized top-down y in `[0, 1]`.
fn mercator_lat(t: f64) -> f64 {
    let v = std::f64::consts::PI * (1.0 - 2.0 * t);
    v.cos().recip().ln().atan().to_degrees()
}

/// Bounds (in degrees) for the tile at `(col, row)` where `row` is top-down
/// (WMTS / slippy convention). Returns `None` when the gridset is unknown or
/// the tile index is outside the matrix.
pub fn tile_bounds(gridset: &str, z: u32, col: u32, row: u32) -> Option<Bounds> {
    if z > MAX_ZOOM {
        return None;
    }
    let n = 1u32 << z;
    let (mw, mh) = (matrix_width(gridset, z), matrix_height(gridset, z));
    if col >= mw || row >= mh {
        return None;
    }
    match gridset_profile(gridset) {
        Some(GridProfile::GlobalGeodetic) => {
            let n2 = (n * 2) as f64;
            let minx = (col as f64 / n2) * 360.0 - 180.0;
            let maxx = ((col + 1) as f64 / n2) * 360.0 - 180.0;
            let miny = (row as f64 / n as f64) * 180.0 - 90.0;
            let maxy = ((row + 1) as f64 / n as f64) * 180.0 - 90.0;
            Some(Bounds::new(minx, miny, maxx, maxy))
        },
        Some(GridProfile::GlobalMercator) => {
            let nf = n as f64;
            let minx = (col as f64 / nf) * 360.0 - 180.0;
            let maxx = ((col + 1) as f64 / nf) * 360.0 - 180.0;
            let miny = mercator_lat((row + 1) as f64 / nf).max(-85.0511);
            let maxy = mercator_lat(row as f64 / nf).min(85.0511);
            Some(Bounds::new(minx, miny, maxx, maxy))
        },
        None => {
            if let Some(g) = custom_gridset(gridset) {
                let res = g.resolutions.get(z as usize).copied()?;
                if res <= 0.0 {
                    return None;
                }
                let tw = res * g.tile_width as f64;
                let th = res * g.tile_height as f64;
                let minx = col as f64 * tw - 180.0;
                let maxx = (col + 1) as f64 * tw - 180.0;
                let maxy = 90.0 - row as f64 * th;
                let miny = 90.0 - (row + 1) as f64 * th;
                Some(Bounds::new(minx, miny, maxx, maxy))
            } else {
                None
            }
        },
    }
}

/// Convert a TMS (bottom-up) row to the top-down (WMTS / slippy) row used by
/// the shared tile engine. Returns `None` when the row is outside the matrix.
pub fn tms_row_to_slippy(gridset: &str, z: u32, y_tms: u32) -> Option<u32> {
    let mh = matrix_height(gridset, z);
    if y_tms >= mh {
        return None;
    }
    Some(mh - 1 - y_tms)
}

/// Estimate the zoom level for a horizontal resolution (degrees per pixel for
/// `EPSG:4326`, meters per pixel for the mercator gridset).
pub fn zoom_for_resolution(gridset: &str, res: f64) -> u32 {
    if let Some(g) = custom_gridset(gridset) {
        return g
            .resolutions
            .iter()
            .enumerate()
            .min_by(|a, b| {
                (a.1 - res)
                    .abs()
                    .partial_cmp(&(b.1 - res).abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i as u32)
            .unwrap_or(0);
    }
    let z = match gridset_profile(gridset) {
        // res(z) = 0.703125 / 2^z  → z = log2(0.703125 / res)
        Some(GridProfile::GlobalGeodetic) => (0.703125 / res).log2().round(),
        // res(z) = 156543.03 / 2^z → z = log2(156543.03 / res)
        _ => (156543.03 / res).log2().round(),
    };
    z.max(0.0).min(MAX_ZOOM as f64) as u32
}

/// Derive the `(col, row_slippy)` tile covering a geographic bbox on a gridset
/// at zoom `z`. The bbox is snapped to the grid (WMS-C `TILED=true` semantics).
pub fn tile_for_bbox(gridset: &str, z: u32, bbox: &Bounds) -> Option<(u32, u32)> {
    let n = 1u32 << z;
    let (mw, mh) = (matrix_width(gridset, z), matrix_height(gridset, z));
    match gridset_profile(gridset) {
        Some(GridProfile::GlobalGeodetic) => {
            let n2 = (n * 2) as f64;
            let col = (((bbox.minx + 180.0) / 360.0) * n2).floor() as u32;
            let row = (((90.0 - bbox.maxy) / 180.0) * n as f64).floor() as u32;
            if col < mw && row < mh {
                Some((col, row))
            } else {
                None
            }
        },
        Some(GridProfile::GlobalMercator) => {
            // The bbox is in Web-Mercator meters: col/row derive directly from
            // the meter extents of the 2^z × 2^z matrix over [-20037508.34, 20037508.34].
            let nf = n as f64;
            let span = 40075016.68;
            let col = (((bbox.minx + span / 2.0) / span) * nf).floor() as u32;
            let y_norm = 1.0 - ((bbox.maxy + span / 2.0) / span);
            let row = (y_norm * nf).floor() as u32;
            if col < mw && row < mh {
                Some((col, row))
            } else {
                None
            }
        },
        None => {
            // Custom gridsets cover the globe uniformly at every zoom level, so
            // the geodetic-style col/row derivation applies.
            if custom_gridset(gridset).is_some() {
                let col = (((bbox.minx + 180.0) / 360.0) * mw as f64).floor() as u32;
                let row = (((90.0 - bbox.maxy) / 180.0) * mh as f64).floor() as u32;
                if col < mw && row < mh {
                    Some((col, row))
                } else {
                    None
                }
            } else {
                None
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_custom_gridset_matrix_and_bounds() {
        // z0: 1 deg/px → 256 px 宽 → 360/256 → 2 列; 180/256 → 1 行
        // z1: 0.5 deg/px → 128 deg/瓦片 → 3 列 × 2 行
        register_custom_gridset(
            "CUSTOM:TEST",
            CustomGridset {
                tile_width: 256,
                tile_height: 256,
                resolutions: vec![1.0, 0.5],
            },
        );
        assert!(is_custom_gridset("CUSTOM:TEST"));
        assert_eq!(matrix_width("CUSTOM:TEST", 0), 2);
        assert_eq!(matrix_height("CUSTOM:TEST", 0), 1);
        assert_eq!(matrix_width("CUSTOM:TEST", 1), 3);
        assert_eq!(matrix_height("CUSTOM:TEST", 1), 2);

        // 左上角原点 (-180, 90), 瓦片 (0,0) 覆盖 [-180, 76] × [-166, 90]
        let b = tile_bounds("CUSTOM:TEST", 0, 0, 0).unwrap();
        assert!((b.minx - -180.0).abs() < 1e-9);
        assert!((b.maxx - 76.0).abs() < 1e-9);
        assert!((b.maxy - 90.0).abs() < 1e-9);
        assert!((b.miny - -166.0).abs() < 1e-9);

        // units_per_pixel / zoom_for_resolution
        assert!((units_per_pixel("CUSTOM:TEST", 1) - 0.5).abs() < 1e-12);
        assert_eq!(zoom_for_resolution("CUSTOM:TEST", 0.55), 1);

        // 越界瓦片 → None
        assert!(tile_bounds("CUSTOM:TEST", 0, 5, 0).is_none());
    }

    #[test]
    fn test_unknown_custom_gridset_falls_back() {
        // 未注册名称 → 走标准 profile 分支 (None)
        assert!(!is_custom_gridset("EPSG:9999"));
        assert!(tile_bounds("EPSG:9999", 0, 0, 0).is_none());
    }

    #[test]
    fn test_geodetic_matrix_and_bounds() {
        // Level 0: 2x1 matrix; tile (0,0) is the western half.
        assert_eq!(matrix_width("EPSG:4326", 0), 2);
        assert_eq!(matrix_height("EPSG:4326", 0), 1);
        let b = tile_bounds("EPSG:4326", 0, 0, 0).unwrap();
        assert!((b.minx - -180.0).abs() < 1e-9);
        assert!((b.miny - -90.0).abs() < 1e-9);
        assert!((b.maxx - 0.0).abs() < 1e-9);
        assert!((b.maxy - 90.0).abs() < 1e-9);

        // Level 1: 4x2 matrix; tile (1,1) is the north-west quadrant.
        assert_eq!(matrix_width("EPSG:4326", 1), 4);
        assert_eq!(matrix_height("EPSG:4326", 1), 2);
        let b1 = tile_bounds("EPSG:4326", 1, 1, 1).unwrap();
        assert!((b1.minx - -90.0).abs() < 1e-9);
        assert!((b1.maxx - 0.0).abs() < 1e-9);
        assert!((b1.miny - 0.0).abs() < 1e-9);
        assert!((b1.maxy - 90.0).abs() < 1e-9);

        // units-per-pixel matches GeoWebCache global-geodetic.
        assert!((units_per_pixel("EPSG:4326", 0) - 0.703125).abs() < 1e-9);
        assert!((units_per_pixel("EPSG:4326", 1) - 0.3515625).abs() < 1e-9);
    }

    #[test]
    fn test_mercator_matrix_and_bounds() {
        // Level 0: 1x1 matrix over the whole world.
        assert_eq!(matrix_width("EPSG:3857", 0), 1);
        assert_eq!(matrix_height("EPSG:3857", 0), 1);
        let b = tile_bounds("EPSG:3857", 0, 0, 0).unwrap();
        assert!((b.minx - -180.0).abs() < 1e-9);
        assert!((b.maxx - 180.0).abs() < 1e-9);
        assert!((b.miny - -85.0511).abs() < 1e-4);
        assert!((b.maxy - 85.0511).abs() < 1e-4);

        // 900913 is an alias for the mercator gridset.
        let b2 = tile_bounds("EPSG:900913", 0, 0, 0).unwrap();
        assert!((b2.maxy - b.maxy).abs() < 1e-9);

        // Level 2: 4x4 matrix; units-per-pixel 156543.03/4.
        assert_eq!(matrix_width("EPSG:3857", 2), 4);
        assert!((units_per_pixel("EPSG:3857", 0) - 156543.03).abs() < 1e-2);
    }

    #[test]
    fn test_tms_y_flip() {
        // geodetic level 2 has 4 rows; TMS y=0 is the bottom (southernmost) row.
        assert_eq!(tms_row_to_slippy("EPSG:4326", 2, 0).unwrap(), 3);
        assert_eq!(tms_row_to_slippy("EPSG:4326", 2, 3).unwrap(), 0);
        assert_eq!(tms_row_to_slippy("EPSG:4326", 2, 4), None);
        assert_eq!(tms_row_to_slippy("EPSG:3857", 0, 0).unwrap(), 0);
    }

    #[test]
    fn test_zoom_for_resolution() {
        assert_eq!(zoom_for_resolution("EPSG:4326", 0.703125), 0);
        assert_eq!(zoom_for_resolution("EPSG:4326", 0.3515625), 1);
        assert_eq!(zoom_for_resolution("EPSG:3857", 156543.03), 0);
        assert_eq!(zoom_for_resolution("EPSG:3857", 39135.7575), 2);
    }

    #[test]
    fn test_tile_for_bbox() {
        // geodetic z=0: the western half maps to tile (0,0).
        let bbox = Bounds::new(-180.0, -90.0, 0.0, 90.0);
        let t = tile_for_bbox("EPSG:4326", 0, &bbox).unwrap();
        assert_eq!(t, (0, 0));

        // geodetic z=1: the 0..90 lon block is column 2 of the 4-col grid;
        // the 0..90 lat block is the top row (row 0, top-down).
        let bbox2 = Bounds::new(0.0, 0.0, 90.0, 90.0);
        let t2 = tile_for_bbox("EPSG:4326", 1, &bbox2).unwrap();
        assert_eq!(t2, (2, 0));

        // mercator z=0: whole world in Web-Mercator meters → (0,0).
        let world = Bounds::new(-20037508.34, -20037508.34, 20037508.34, 20037508.34);
        let t3 = tile_for_bbox("EPSG:3857", 0, &world).unwrap();
        assert_eq!(t3, (0, 0));
    }
}
