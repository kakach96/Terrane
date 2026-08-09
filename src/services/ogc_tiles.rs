//! # OGC API - Tiles implementation
//!
//! First OGC API - Tiles surface for Terrane (OGC 19-069): landing page,
//! `/conformance`, `/tileMatrixSets` (+ per-id definitions), `/collections`
//! tileset listings and raster tiles at
//! `/ogc/tiles/collections/{id}/tiles/{tileMatrixSetId}/{tileMatrix}/{tileRow}/{tileCol}`.
//!
//! The actual tile rendering reuses the shared tile engine
//! (`utils/tile_grid.rs` grid math + `handlers/tile_common.rs`
//! `render_tile_bytes`), so OGC API - Tiles speaks the same PNG/JPEG tiles as
//! WMTS / TMS / WMS-C. TileMatrixSet definitions mirror the OGC Two Dimensional
//! Tile Matrix Set standard (OGC 17-083r2): EPSG:4326 global-geodetic and
//! EPSG:3857 global-mercator.

use crate::models::Layer;
use crate::utils::tile_grid;
use serde_json::{json, Value};

/// The TileMatrixSets exposed by the tile service.
pub const TILE_MATRIX_SET_IDS: [&str; 2] = ["EPSG:4326", "EPSG:3857"];

fn link(href: &str, rel: &str, type_: &str, title: &str) -> Value {
    json!({
        "href": href,
        "rel": rel,
        "type": type_,
        "title": title,
    })
}

/// Build the OGC API - Tiles landing page.
pub fn landing_page(base_url: &str) -> Value {
    json!({
        "title": "Terrane",
        "description": "Cloud-native spatial data server powered by Rust — OGC API Tiles",
        "links": [
            link(&format!("{}/ogc/tiles", base_url), "self", "application/json", "This document"),
            link(&format!("{}/ogc/tiles/conformance", base_url), "conformance", "application/json", "OGC API conformance classes"),
            link(&format!("{}/ogc/tiles/tileMatrixSets", base_url), "http://www.opengis.net/def/rel/ogc/1.0/tiling-schemes", "application/json", "Tile matrix sets"),
            link(&format!("{}/ogc/tiles/collections", base_url), "data", "application/json", "Collections"),
        ],
    })
}

/// Build the OGC API - Tiles conformance declaration.
pub fn conformance() -> Value {
    json!({
        "conformsTo": [
            "http://www.opengis.net/spec/ogcapi-tiles-1/1.0/conf/core",
            "http://www.opengis.net/spec/ogcapi-tiles-1/1.0/conf/tileset",
            "http://www.opengis.net/spec/ogcapi-tiles-1/1.0/conf/tilesets-list",
            "http://www.opengis.net/spec/ogcapi-tiles-1/1.0/conf/tilematrixset",
            "http://www.opengis.net/spec/ogcapi-tiles-1/1.0/conf/dataset-tileset",
        ]
    })
}

/// Build the `/tileMatrixSets` document listing the supported schemas.
pub fn tile_matrix_sets(base_url: &str) -> Value {
    let sets: Vec<Value> = TILE_MATRIX_SET_IDS
        .iter()
        .map(|id| {
            json!({
                "id": id,
                "links": [link(&format!("{}/ogc/tiles/tileMatrixSets/{}", base_url, id), "self", "application/json", *id)],
            })
        })
        .collect();
    json!({
        "tileMatrixSets": sets,
    })
}

/// Build a single `/tileMatrixSets/{id}` definition. Returns `None` for
/// unknown ids.
pub fn tile_matrix_set(id: &str) -> Option<Value> {
    match id {
        "EPSG:4326" => Some(tile_matrix_set_geodetic()),
        "EPSG:3857" | "EPSG:900913" => Some(tile_matrix_set_mercator()),
        _ => None,
    }
}

fn tile_matrix_json(
    z: u32,
    cell_size: f64,
    origin: [f64; 2],
    matrix_width: u32,
    matrix_height: u32,
) -> Value {
    json!({
        "id": z.to_string(),
        "cellSize": cell_size,
        "cornerOfOrigin": "topLeft",
        "pointOfOrigin": origin,
        "tileWidth": 256,
        "tileHeight": 256,
        "matrixWidth": matrix_width,
        "matrixHeight": matrix_height,
    })
}

fn tile_matrix_set_geodetic() -> Value {
    let mut matrices = Vec::new();
    for z in 0..=tile_grid::MAX_ZOOM {
        let cell = 360.0 / (256.0 * (2u64 << z) as f64); // 0.703125 / 2^z deg/px
        matrices.push(tile_matrix_json(
            z,
            cell,
            [-180.0, 90.0],
            tile_grid::matrix_width("EPSG:4326", z),
            tile_grid::matrix_height("EPSG:4326", z),
        ));
    }
    json!({
        "id": "EPSG:4326",
        "title": "Global Geodetic",
        "crs": "http://www.opengis.net/def/crs/OGC/1.3/CRS84",
        "tileMatrices": matrices,
    })
}

fn tile_matrix_set_mercator() -> Value {
    let mut matrices = Vec::new();
    for z in 0..=tile_grid::MAX_ZOOM {
        let cell = 40075016.68 / (256.0 * (1u64 << z) as f64); // 156543.03 / 2^z m/px
        matrices.push(tile_matrix_json(
            z,
            cell,
            [-20037508.342789244, 20037508.342789244],
            tile_grid::matrix_width("EPSG:3857", z),
            tile_grid::matrix_height("EPSG:3857", z),
        ));
    }
    json!({
        "id": "EPSG:3857",
        "title": "Google Maps Compatible",
        "crs": "http://www.opengis.net/def/crs/EPSG/0/3857",
        "tileMatrices": matrices,
    })
}

fn tile_matrix_set_uri(tms: &str) -> String {
    match tms {
        "EPSG:4326" => {
            "http://www.opengis.net/def/tilematrixset/OGC/1.0/global-geodetic".to_string()
        },
        _ => "http://www.opengis.net/def/tilematrixset/OGC/1.0/global-mercator".to_string(),
    }
}

fn tile_crs(tms: &str) -> &'static str {
    match tms {
        "EPSG:4326" => "http://www.opengis.net/def/crs/OGC/1.3/CRS84",
        _ => "http://www.opengis.net/def/crs/EPSG/0/3857",
    }
}

fn tile_href(base_url: &str, layer: &str, tms: &str, z: &str, row: u32, col: u32) -> String {
    format!(
        "{}/ogc/tiles/collections/{}/tiles/{}/{}/{}/{}?f=image/png",
        base_url, layer, tms, z, row, col
    )
}

/// Build the `/collections` tileset overview (one entry per published layer).
pub fn collections(base_url: &str, layers: &[Layer]) -> Value {
    let colls: Vec<Value> = layers
        .iter()
        .map(|layer| {
            json!({
                "id": layer.name,
                "title": layer.title,
                "links": [
                    link(&format!("{}/ogc/tiles/collections/{}/tiles", base_url, layer.name), "tiles", "application/json", "Tiles"),
                    link(&tile_href(base_url, &layer.name, "EPSG:4326", "0", 0, 0), "item", "image/png", "Tile"),
                ],
            })
        })
        .collect();
    json!({
        "collections": colls,
        "links": [link(&format!("{}/ogc/tiles/collections", base_url), "self", "application/json", "Collections")],
    })
}

/// Build the tileset listing for a single collection
/// (`/collections/{id}/tiles`).
pub fn collection_tilesets(base_url: &str, layer: &Layer) -> Value {
    let tilesets: Vec<Value> = TILE_MATRIX_SET_IDS
        .iter()
        .map(|tms| {
            let title = if *tms == "EPSG:4326" {
                "Global Geodetic"
            } else {
                "Google Maps Compatible"
            };
            json!({
                "title": title,
                "tileMatrixSetURI": tile_matrix_set_uri(tms),
                "crs": tile_crs(tms),
                "links": [
                    link(&tile_href(base_url, &layer.name, tms, "0", 0, 0), "item", "image/png", "Tile"),
                    link(&format!("{}/ogc/tiles/tileMatrixSets/{}", base_url, tms), "http://www.opengis.net/def/rel/ogc/1.0/tiling-schemes", "application/json", tms),
                ],
            })
        })
        .collect();
    json!({
        "tilesets": tilesets,
        "links": [link(&format!("{}/ogc/tiles/collections/{}/tiles", base_url, layer.name), "self", "application/json", "Tiles")],
    })
}

/// Parse a `tileMatrix` identifier to a zoom level: `"5"` or `"EPSG:4326:5"`.
pub fn parse_zoom(tile_matrix: &str) -> u32 {
    if let Some(idx) = tile_matrix.rfind(':') {
        tile_matrix[idx + 1..].parse().unwrap_or(0)
    } else {
        tile_matrix.parse().unwrap_or(0)
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{BoundingBox, CoordinateReferenceSystem};

    fn test_layer(name: &str, title: &str) -> Layer {
        let mut layer = Layer::new(
            name.to_string(),
            title.to_string(),
            "default".to_string(),
            "shapes".to_string(),
            CoordinateReferenceSystem::EPSG4326,
        );
        layer.lat_lon_bounds = BoundingBox::new(
            CoordinateReferenceSystem::EPSG4326,
            crate::models::Bounds::new(-180.0, -90.0, 180.0, 90.0),
        );
        layer
    }

    #[test]
    fn test_landing_page_links() {
        let v = landing_page("http://x");
        assert_eq!(v["title"], "Terrane");
        let rels: Vec<&str> = v["links"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l["rel"].as_str().unwrap())
            .collect();
        assert!(rels.contains(&"self"));
        assert!(rels.contains(&"conformance"));
        assert!(rels.contains(&"data"));
    }

    #[test]
    fn test_conformance() {
        let v = conformance();
        let c = v["conformsTo"].as_array().unwrap();
        assert!(c.iter().any(|x| x.as_str().unwrap().contains("core")));
        assert!(c.iter().any(|x| x.as_str().unwrap().contains("tileset")));
    }

    #[test]
    fn test_tile_matrix_sets_list() {
        let v = tile_matrix_sets("http://x");
        let sets = v["tileMatrixSets"].as_array().unwrap();
        assert_eq!(sets.len(), 2);
        assert_eq!(sets[0]["id"], "EPSG:4326");
        assert_eq!(sets[1]["id"], "EPSG:3857");
    }

    #[test]
    fn test_tile_matrix_set_geodetic() {
        let v = tile_matrix_set("EPSG:4326").unwrap();
        assert_eq!(v["id"], "EPSG:4326");
        assert_eq!(v["crs"], "http://www.opengis.net/def/crs/OGC/1.3/CRS84");
        let matrices = v["tileMatrices"].as_array().unwrap();
        // zoom 0: 2×1 matrix, cellSize 0.703125
        assert_eq!(matrices[0]["id"], "0");
        assert_eq!(matrices[0]["matrixWidth"], 2);
        assert_eq!(matrices[0]["matrixHeight"], 1);
        assert_eq!(matrices[0]["cellSize"], 0.703125);
        assert_eq!(matrices[0]["pointOfOrigin"][0], -180.0);
        // zoom 1: 4×2
        assert_eq!(matrices[1]["matrixWidth"], 4);
        assert_eq!(matrices[1]["matrixHeight"], 2);
        // unknown → None
        assert!(tile_matrix_set("bogus").is_none());
    }

    #[test]
    fn test_tile_matrix_set_mercator() {
        let v = tile_matrix_set("EPSG:3857").unwrap();
        assert_eq!(v["id"], "EPSG:3857");
        let matrices = v["tileMatrices"].as_array().unwrap();
        assert_eq!(matrices[0]["matrixWidth"], 1);
        assert_eq!(matrices[0]["matrixHeight"], 1);
        assert!((matrices[0]["cellSize"].as_f64().unwrap() - 156543.03392804097).abs() < 1.0);
        // EPSG:900913 alias
        assert!(tile_matrix_set("EPSG:900913").is_some());
    }

    #[test]
    fn test_collection_tilesets() {
        let layer = test_layer("world", "World");
        let v = collection_tilesets("http://x", &layer);
        let tilesets = v["tilesets"].as_array().unwrap();
        assert_eq!(tilesets.len(), 2);
        assert_eq!(
            tilesets[0]["tileMatrixSetURI"],
            "http://www.opengis.net/def/tilematrixset/OGC/1.0/global-geodetic"
        );
        assert_eq!(
            tilesets[1]["crs"],
            "http://www.opengis.net/def/crs/EPSG/0/3857"
        );
        let item_rel = tilesets[0]["links"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l["rel"] == "item" && l["type"] == "image/png");
        assert!(item_rel);
        // collections overview
        let layers = vec![layer];
        let v = collections("http://x", &layers);
        let colls = v["collections"].as_array().unwrap();
        assert_eq!(colls.len(), 1);
        assert_eq!(colls[0]["id"], "world");
    }

    #[test]
    fn test_parse_zoom() {
        assert_eq!(parse_zoom("5"), 5);
        assert_eq!(parse_zoom("EPSG:4326:7"), 7);
        assert_eq!(parse_zoom("abc"), 0);
    }
}
