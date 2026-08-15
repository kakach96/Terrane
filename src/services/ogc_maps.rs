//! # OGC API - Maps implementation
//!
//! First OGC API - Maps surface for Terrane (OGC 20-058), served at
//! `/ogc/maps` (JSON). The reference GeoServer at :18080 does not ship the
//! OGC API extension, so this follows the OGC API - Maps schema directly.
//!
//! Resources: landing page, `/conformance`, `/collections`,
//! `/collections/{id}`, `/collections/{id}/styles` and the `map` operation at
//! `/collections/{id}/map` (rendered through the shared WMS GetMap pipeline,
//! so OGC API - Maps serves the same PNG/JPEG maps as the WMS 1.1.1/1.3.0
//! interface).

use crate::models::{Bounds, Layer};
use serde_json::{json, Value};

/// Default map media types.
pub const MAP_PNG_MIME: &str = "image/png";
pub const MAP_JPEG_MIME: &str = "image/jpeg";

fn link(href: &str, rel: &str, type_: &str, title: &str) -> Value {
    json!({
        "href": href,
        "rel": rel,
        "type": type_,
        "title": title,
    })
}

/// Build the OGC API - Maps landing page (`GET /ogc/maps`).
pub fn landing_page(base_url: &str) -> Value {
    json!({
        "title": "Terrane",
        "description": "Cloud-native spatial data server powered by Rust — OGC API Maps",
        "links": [
            link(&format!("{}/ogc/maps", base_url), "self", "application/json", "This document"),
            link(&format!("{}/ogc/maps/conformance", base_url), "conformance", "application/json", "OGC API conformance classes"),
            link(&format!("{}/ogc/maps/collections", base_url), "data", "application/json", "Collections"),
        ],
    })
}

/// Build the OGC API - Maps conformance declaration
/// (`GET /ogc/maps/conformance`).
pub fn conformance() -> Value {
    json!({
        "conformsTo": [
            "http://www.opengis.net/spec/ogcapi-maps-1/1.0/conf/core",
            "http://www.opengis.net/spec/ogcapi-maps-1/1.0/conf/oas30",
            "http://www.opengis.net/spec/ogcapi-maps-1/1.0/conf/html",
            "http://www.opengis.net/spec/ogcapi-maps-1/1.0/conf/map",
            "http://www.opengis.net/spec/ogcapi-maps-1/1.0/conf/collections",
            "http://www.opengis.net/spec/ogcapi-maps-1/1.0/conf/collection-map",
        ]
    })
}

/// Build the map operation href for a collection.
fn map_href(base_url: &str, layer: &str, f: &str) -> String {
    format!("{}/ogc/maps/collections/{}/map?f={}", base_url, layer, f)
}

/// Build the `/collections` document listing every published layer as a map
/// collection.
pub fn collections(base_url: &str, layers: &[Layer]) -> Value {
    let colls: Vec<Value> = layers
        .iter()
        .map(|layer| {
            let coll = format!("{}/ogc/maps/collections/{}", base_url, layer.name);
            json!({
                "id": layer.name,
                "title": layer.title,
                "links": [
                    link(&coll, "self", "application/json", &layer.name),
                    link(&map_href(base_url, &layer.name, MAP_PNG_MIME), "map", MAP_PNG_MIME, "Map (PNG)"),
                ],
            })
        })
        .collect();
    json!({
        "collections": colls,
        "links": [link(&format!("{}/ogc/maps/collections", base_url), "self", "application/json", "Collections")],
    })
}

fn extent_of(layer: &Layer) -> Value {
    let b = &layer.lat_lon_bounds.bounds;
    json!({
        "spatial": {
            "bbox": [[b.minx, b.miny, b.maxx, b.maxy]],
            "crs": "http://www.opengis.net/def/crs/OGC/1.3/CRS84"
        }
    })
}

/// Build a single collection document (`GET /ogc/maps/collections/{id}`).
pub fn collection(base_url: &str, layer: &Layer) -> Value {
    let coll = format!("{}/ogc/maps/collections/{}", base_url, layer.name);
    json!({
        "id": layer.name,
        "title": layer.title,
        "description": layer.abstract_text.clone().unwrap_or_default(),
        "extent": extent_of(layer),
        "links": [
            link(&coll, "self", "application/json", &layer.name),
            link(&format!("{}/styles", coll), "styles", "application/json", "Styles"),
            link(&map_href(base_url, &layer.name, MAP_PNG_MIME), "map", MAP_PNG_MIME, "Map (PNG)"),
            link(&map_href(base_url, &layer.name, MAP_JPEG_MIME), "map", MAP_JPEG_MIME, "Map (JPEG)"),
        ],
    })
}

/// Build the style list of a collection
/// (`GET /ogc/maps/collections/{id}/styles`).
pub fn styles(base_url: &str, layer: &Layer) -> Value {
    let coll = format!("{}/ogc/maps/collections/{}", base_url, layer.name);
    let styles: Vec<Value> = layer
        .styles
        .iter()
        .map(|s| {
            json!({
                "id": s.name,
                "href": s.href.clone().unwrap_or_else(|| format!("{}/styles/{}", coll, s.name)),
            })
        })
        .collect();
    json!({
        "styles": styles,
        "links": [link(&format!("{}/styles", coll), "self", "application/json", "Styles")],
    })
}

/// Parse an OGC API - Maps `bbox` query value (`minx,miny,maxx,maxy`).
/// Returns `None` when the value is missing or malformed.
pub fn parse_bbox(s: &str) -> Option<Bounds> {
    let parts: Vec<f64> = s
        .split(',')
        .map(|p| p.trim().parse::<f64>().ok())
        .collect::<Option<Vec<_>>>()?;
    if parts.len() != 4 {
        return None;
    }
    Some(Bounds::new(parts[0], parts[1], parts[2], parts[3]))
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{BoundingBox, CoordinateReferenceSystem};

    fn sample_layer() -> Layer {
        Layer {
            name: "world".to_string(),
            title: "World".to_string(),
            abstract_text: Some("World layer".to_string()),
            workspace: "default".to_string(),
            store: "shapes".to_string(),
            native_name: None,
            native_bounds: BoundingBox::world(CoordinateReferenceSystem::EPSG4326),
            lat_lon_bounds: BoundingBox::world(CoordinateReferenceSystem::EPSG4326),
            srs: CoordinateReferenceSystem::EPSG4326,
            styles: vec![],
            resource: crate::models::layer::LayerResource {
                resource_type: crate::models::layer::ResourceType::FeatureType,
                path: None,
            },
            enabled: true,
            cache_store: None,
        }
    }

    #[test]
    fn test_landing_structure() {
        let v = landing_page("http://localhost:8080");
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
    fn test_conformance_classes() {
        let v = conformance();
        let conforms = v["conformsTo"].as_array().unwrap();
        assert!(conforms
            .iter()
            .any(|x| x.as_str().unwrap().contains("core")));
        assert!(conforms.iter().any(|x| x.as_str().unwrap().contains("map")));
        assert!(conforms
            .iter()
            .any(|x| x.as_str().unwrap().contains("collections")));
    }

    #[test]
    fn test_collections_lists_layers() {
        let v = collections("http://localhost:8080", &[sample_layer()]);
        let colls = v["collections"].as_array().unwrap();
        assert_eq!(colls.len(), 1);
        assert_eq!(colls[0]["id"], "world");
        let href = colls[0]["links"][0]["href"].as_str().unwrap();
        assert!(href.contains("/ogc/maps/collections/world"));
    }

    #[test]
    fn test_collection_links_map_and_styles() {
        let v = collection("http://localhost:8080", &sample_layer());
        assert_eq!(v["id"], "world");
        let rels: Vec<&str> = v["links"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l["rel"].as_str().unwrap())
            .collect();
        assert!(rels.contains(&"map"));
        assert!(rels.contains(&"styles"));
        assert!(rels.contains(&"self"));
        // spatial extent in CRS84
        let bbox = v["extent"]["spatial"]["bbox"][0].as_array().unwrap();
        assert_eq!(bbox[0], -180.0);
    }

    #[test]
    fn test_styles_empty_and_with_style() {
        let v = styles("http://localhost:8080", &sample_layer());
        assert_eq!(v["styles"].as_array().unwrap().len(), 0);

        let mut layer = sample_layer();
        layer.styles = vec![crate::models::layer::StyleRef {
            name: "default".to_string(),
            href: None,
        }];
        let v = styles("http://localhost:8080", &layer);
        let arr = v["styles"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["id"], "default");
    }

    #[test]
    fn test_parse_bbox_valid() {
        let b = parse_bbox("-10,20,30,40").unwrap();
        assert_eq!(b.minx, -10.0);
        assert_eq!(b.miny, 20.0);
        assert_eq!(b.maxx, 30.0);
        assert_eq!(b.maxy, 40.0);
    }

    #[test]
    fn test_parse_bbox_invalid() {
        assert!(parse_bbox("-10,20,30").is_none());
        assert!(parse_bbox("a,b,c,d").is_none());
        assert!(parse_bbox("").is_none());
    }

    #[test]
    fn test_map_href_formats() {
        let png = map_href("http://localhost:8080", "world", MAP_PNG_MIME);
        assert!(png.contains("/ogc/maps/collections/world/map?f=image/png"));
        let jpeg = map_href("http://localhost:8080", "world", MAP_JPEG_MIME);
        assert!(jpeg.contains("?f=image/jpeg"));
    }
}
