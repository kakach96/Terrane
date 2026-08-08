//! # OGC API - Features implementation
//!
//! First OGC API surface for Terrane: the OGC API - Features Part 1 Core
//! (OGC 17-069r3) resources served at `/ogc/features` (JSON). The reference
//! GeoServer at :18080 does not ship the OGC API extension, so this follows
//! the OGC API - Features Core schema directly.
//!
//! Resources: landing page, `/conformance`, `/collections`,
//! `/collections/{id}`, `/collections/{id}/items` and
//! `/collections/{id}/items/{featureId}`. Collections map to the Terrane layer
//! catalog; items are GeoJSON feature collections with `bbox` / `limit` /
//! `offset` query support.

use crate::models::{Bounds, Feature, Layer};
use crate::utils::geometry::calculate_bounds;
use serde_json::{json, Value};

/// Feature collection media type.
pub const GEOJSON_MIME: &str = "application/geo+json";

fn link(href: &str, rel: &str, type_: &str, title: &str) -> Value {
    json!({
        "href": href,
        "rel": rel,
        "type": type_,
        "title": title,
    })
}

/// Build the landing page document (`GET /ogc/features`).
pub fn landing_page(base_url: &str) -> Value {
    json!({
        "title": "Terrane",
        "description": "Cloud-native spatial data server powered by Rust — OGC API Features",
        "links": [
            link(&format!("{}/ogc/features", base_url), "self", "application/json", "This document"),
            link(&format!("{}/ogc/features/conformance", base_url), "conformance", "application/json", "OGC API conformance classes"),
            link(&format!("{}/ogc/features/collections", base_url), "data", "application/json", "Collections"),
        ],
    })
}

/// Build the conformance declaration (`GET /ogc/features/conformance`).
pub fn conformance() -> Value {
    json!({
        "conformsTo": [
            "http://www.opengis.net/spec/ogcapi-features-1/1.0/conf/core",
            "http://www.opengis.net/spec/ogcapi-features-1/1.0/conf/oas30",
            "http://www.opengis.net/spec/ogcapi-features-1/1.0/conf/geojson",
        ]
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

fn collection_json(base_url: &str, layer: &Layer) -> Value {
    let coll = format!("{}/ogc/features/collections/{}", base_url, layer.name);
    let items = format!("{}/items", coll);
    json!({
        "id": layer.name,
        "title": layer.title,
        "description": layer.abstract_text.clone().unwrap_or_default(),
        "extent": extent_of(layer),
        "links": [
            link(&coll, "self", "application/json", &layer.name),
            link(&items, "items", GEOJSON_MIME, "Items"),
        ],
    })
}

/// Build the `/collections` document.
pub fn collections(base_url: &str, layers: &[Layer]) -> Value {
    let colls: Vec<Value> = layers.iter().map(|l| collection_json(base_url, l)).collect();
    json!({
        "collections": colls,
        "links": [
            link(&format!("{}/ogc/features/collections", base_url), "self", "application/json", "Collections"),
        ],
    })
}

/// Build a single `/collections/{id}` document.
pub fn collection(base_url: &str, layer: &Layer) -> Value {
    collection_json(base_url, layer)
}

/// Parse a `bbox` query value: four comma-separated WGS84 numbers
/// (`minx,miny,maxx,maxy`). Returns `None` for malformed values.
pub fn parse_bbox(s: &str) -> Option<Bounds> {
    let parts: Vec<f64> = s
        .split(',')
        .filter_map(|p| p.trim().parse().ok())
        .collect();
    if parts.len() >= 4 {
        Some(Bounds::new(parts[0], parts[1], parts[2], parts[3]))
    } else {
        None
    }
}

fn feature_json(feature: &Feature) -> Value {
    json!({
        "type": "Feature",
        "id": feature.id,
        "geometry": feature.geometry,
        "properties": feature.properties,
    })
}

fn intersects_bbox(feature: &Feature, bbox: &Bounds) -> bool {
    match calculate_bounds(std::iter::once(feature.geometry.to_geo())) {
        Some(b) => b.intersects(bbox),
        None => false,
    }
}

/// Build the `/collections/{id}/items` document (GeoJSON FeatureCollection)
/// with `bbox` filtering and `limit` / `offset` paging.
pub fn items(
    base_url: &str,
    layer: &Layer,
    features: &[Feature],
    limit: usize,
    offset: usize,
    bbox: Option<&Bounds>,
) -> Value {
    let filtered: Vec<Feature> = match bbox {
        Some(b) => features
            .iter()
            .filter(|f| intersects_bbox(f, b))
            .cloned()
            .collect(),
        None => features.to_vec(),
    };
    let matched = filtered.len();
    let page: Vec<&Feature> = filtered.iter().skip(offset).take(limit).collect();
    let returned = page.len();
    let features_json: Vec<Value> = page.iter().map(|f| feature_json(f)).collect();

    let items_href = format!(
        "{}/ogc/features/collections/{}/items",
        base_url, layer.name
    );
    let mut links = vec![link(&items_href, "self", GEOJSON_MIME, "Items")];
    if offset + returned < matched {
        links.push(link(
            &format!("{}?offset={}&limit={}", items_href, offset + returned, limit),
            "next",
            GEOJSON_MIME,
            "Next page",
        ));
    }

    json!({
        "type": "FeatureCollection",
        "features": features_json,
        "numberMatched": matched,
        "numberReturned": returned,
        "links": links,
    })
}

/// Build a single `/collections/{id}/items/{featureId}` document.
pub fn item(feature: &Feature) -> Value {
    feature_json(feature)
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{BoundingBox, CoordinateReferenceSystem, GeoJsonGeometry, PropertyValue};
    use std::collections::HashMap;

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
            Bounds::new(-180.0, -90.0, 180.0, 90.0),
        );
        layer
    }

    fn point_feature(id: &str, x: f64, y: f64) -> Feature {
        Feature::with_id(
            id.to_string(),
            GeoJsonGeometry::Point {
                coordinates: vec![x, y],
            },
            HashMap::new(),
        )
    }

    #[test]
    fn test_landing_page_links() {
        let v = landing_page("http://127.0.0.1:8080");
        assert_eq!(v["title"], "Terrane");
        let links = v["links"].as_array().unwrap();
        assert_eq!(links.len(), 3);
        let rels: Vec<&str> = links
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
        assert!(c.iter().any(|x| x.as_str().unwrap().contains("geojson")));
    }

    #[test]
    fn test_parse_bbox() {
        assert!(parse_bbox("-180,-90,180,90").is_some());
        let b = parse_bbox("1,2,3,4").unwrap();
        assert_eq!(b.minx, 1.0);
        assert_eq!(b.maxy, 4.0);
        // malformed → None
        assert!(parse_bbox("a,b").is_none());
        assert!(parse_bbox("").is_none());
    }

    #[test]
    fn test_collections_structure() {
        let layers = vec![test_layer("world", "World"), test_layer("usa", "USA")];
        let v = collections("http://x", &layers);
        let colls = v["collections"].as_array().unwrap();
        assert_eq!(colls.len(), 2);
        assert_eq!(colls[0]["id"], "world");
        assert_eq!(colls[0]["title"], "World");
        assert_eq!(
            colls[0]["extent"]["spatial"]["bbox"][0],
            json!([-180.0, -90.0, 180.0, 90.0])
        );
        let rels: Vec<&str> = colls[0]["links"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l["rel"].as_str().unwrap())
            .collect();
        assert!(rels.contains(&"items"));
        // single collection
        let single = collection("http://x", &layers[0]);
        assert_eq!(single["id"], "world");
        assert!(single["links"][1]["type"] == GEOJSON_MIME);
    }

    #[test]
    fn test_items_paging_and_number() {
        let layer = test_layer("world", "World");
        let features = vec![
            point_feature("a", 1.0, 1.0),
            point_feature("b", 2.0, 2.0),
            point_feature("c", 3.0, 3.0),
        ];
        // page 2, size 2 → one feature + next link
        let v = items("http://x", &layer, &features, 2, 2, None);
        assert_eq!(v["numberMatched"], 3);
        assert_eq!(v["numberReturned"], 1);
        assert_eq!(v["features"].as_array().unwrap()[0]["id"], "c");
        let rels: Vec<&str> = v["links"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l["rel"].as_str().unwrap())
            .collect();
        assert!(rels.contains(&"self"));
        assert!(!rels.contains(&"next")); // last page
        // first page has next link
        let v = items("http://x", &layer, &features, 2, 0, None);
        let rels: Vec<&str> = v["links"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l["rel"].as_str().unwrap())
            .collect();
        assert!(rels.contains(&"next"));
    }

    #[test]
    fn test_items_bbox_filter() {
        let layer = test_layer("world", "World");
        let features = vec![
            point_feature("in", 5.0, 5.0),
            point_feature("out", 100.0, 100.0),
        ];
        let bbox = parse_bbox("0,0,10,10").unwrap();
        let v = items("http://x", &layer, &features, 10, 0, Some(&bbox));
        assert_eq!(v["numberMatched"], 1);
        assert_eq!(v["features"].as_array().unwrap()[0]["id"], "in");
    }

    #[test]
    fn test_item() {
        let f = point_feature("abc", 1.0, 2.0);
        let v = item(&f);
        assert_eq!(v["type"], "Feature");
        assert_eq!(v["id"], "abc");
        assert_eq!(v["geometry"]["type"], "Point");
    }
}
