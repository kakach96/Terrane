//! # OGC API - Styles implementation
//!
//! First OGC API - Styles surface for Terrane (OGC 21-009), served at
//! `/ogc/styles` (JSON). The reference GeoServer at :18080 does not ship the
//! OGC API extension, so this follows the OGC API - Styles schema directly.
//!
//! Resources: landing page, `/conformance`, `/styles` (list / create),
//! `/styles/{styleId}` (get / replace / delete), `/styles/{styleId}/metadata`
//! and the collection linkage `/collections` + `/collections/{id}/styles`.
//! Style content is served in its native format (SLD XML, CSS, YSLD YAML or
//! Mapbox Style JSON).

use crate::models::layer::Layer;
use crate::models::style::StyleFormat;
use serde_json::{json, Value};

/// Native media types per style format.
pub const STYLE_SLD_MIME: &str = "application/vnd.ogc.sld+xml";
pub const STYLE_CSS_MIME: &str = "text/css";
pub const STYLE_YSLD_MIME: &str = "text/yaml";
pub const STYLE_MB_MIME: &str = "application/json";

/// A style summary as exposed by the OGC API - Styles listing / metadata
/// resources.
#[derive(Debug, Clone)]
pub struct StyleSummary {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub format: StyleFormat,
}

/// Map a style format to its native media type.
pub fn mime_for_format(fmt: &StyleFormat) -> &'static str {
    match fmt {
        StyleFormat::SLD => STYLE_SLD_MIME,
        StyleFormat::CSS => STYLE_CSS_MIME,
        StyleFormat::YSLD => STYLE_YSLD_MIME,
        StyleFormat::MBStyle => STYLE_MB_MIME,
    }
}

fn link(href: &str, rel: &str, type_: &str, title: &str) -> Value {
    json!({
        "href": href,
        "rel": rel,
        "type": type_,
        "title": title,
    })
}

/// Build the OGC API - Styles landing page (`GET /ogc/styles`).
pub fn landing_page(base_url: &str) -> Value {
    json!({
        "title": "Terrane",
        "description": "Cloud-native spatial data server powered by Rust — OGC API Styles",
        "links": [
            link(&format!("{}/ogc/styles", base_url), "self", "application/json", "This document"),
            link(&format!("{}/ogc/styles/conformance", base_url), "conformance", "application/json", "OGC API conformance classes"),
            link(&format!("{}/ogc/styles/styles", base_url), "styles", "application/json", "Styles"),
            link(&format!("{}/ogc/styles/collections", base_url), "data", "application/json", "Collections"),
        ],
    })
}

/// Build the OGC API - Styles conformance declaration
/// (`GET /ogc/styles/conformance`).
pub fn conformance() -> Value {
    json!({
        "conformsTo": [
            "http://www.opengis.net/spec/ogcapi-styles-1/1.0/conf/core",
            "http://www.opengis.net/spec/ogcapi-styles-1/1.0/conf/oas30",
            "http://www.opengis.net/spec/ogcapi-styles-1/1.0/conf/html",
            "http://www.opengis.net/spec/ogcapi-styles-1/1.0/conf/styles-list",
            "http://www.opengis.net/spec/ogcapi-styles-1/1.0/conf/style-info",
            "http://www.opengis.net/spec/ogcapi-styles-1/1.0/conf/style-metadata",
            "http://www.opengis.net/spec/ogcapi-styles-1/1.0/conf/style-create-update-delete",
            "http://www.opengis.net/spec/ogcapi-styles-1/1.0/conf/style-search",
            "http://www.opengis.net/spec/ogcapi-styles-1/1.0/conf/collections",
        ]
    })
}

/// The style content href for a style id.
pub fn style_href(base_url: &str, id: &str) -> String {
    format!("{}/ogc/styles/styles/{}", base_url, id)
}

/// The style metadata href for a style id.
pub fn style_meta_href(base_url: &str, id: &str) -> String {
    format!("{}/ogc/styles/styles/{}/metadata", base_url, id)
}

/// Build the `/styles` document listing every style.
pub fn styles_list(base_url: &str, styles: &[StyleSummary]) -> Value {
    let items: Vec<Value> = styles
        .iter()
        .map(|s| {
            json!({
                "id": s.id,
                "title": s.title,
                "links": [
                    link(&style_href(base_url, &s.id), "style", mime_for_format(&s.format), &s.id),
                    link(&style_meta_href(base_url, &s.id), "describedby", "application/json", "Metadata"),
                ],
            })
        })
        .collect();
    json!({
        "styles": items,
        "links": [link(&format!("{}/ogc/styles/styles", base_url), "self", "application/json", "Styles")],
    })
}

/// Build the style metadata document
/// (`GET /ogc/styles/styles/{styleId}/metadata`).
pub fn style_metadata(base_url: &str, s: &StyleSummary) -> Value {
    json!({
        "id": s.id,
        "title": s.title,
        "description": s.description.clone().unwrap_or_default(),
        "links": [
            link(&style_href(base_url, &s.id), "style", mime_for_format(&s.format), &s.id),
            link(&style_meta_href(base_url, &s.id), "self", "application/json", "Metadata"),
        ],
    })
}

/// Build the `/collections` document listing every published layer as a style
/// collection.
pub fn collections(base_url: &str, layers: &[Layer]) -> Value {
    let colls: Vec<Value> = layers
        .iter()
        .map(|layer| {
            let coll = format!("{}/ogc/styles/collections/{}", base_url, layer.name);
            json!({
                "id": layer.name,
                "title": layer.title,
                "links": [
                    link(&coll, "self", "application/json", &layer.name),
                    link(&format!("{}/styles", coll), "styles", "application/json", "Styles"),
                ],
            })
        })
        .collect();
    json!({
        "collections": colls,
        "links": [link(&format!("{}/ogc/styles/collections", base_url), "self", "application/json", "Collections")],
    })
}

/// Build the style list of a layer collection
/// (`GET /ogc/styles/collections/{collectionId}/styles`). Only styles that
/// still exist in the catalog are listed.
pub fn collection_styles(base_url: &str, layer: &Layer, available: &[StyleSummary]) -> Value {
    let coll = format!("{}/ogc/styles/collections/{}", base_url, layer.name);
    let styles: Vec<Value> = layer
        .styles
        .iter()
        .filter_map(|r| available.iter().find(|s| s.id == r.name))
        .map(|s| {
            json!({
                "id": s.id,
                "title": s.title,
                "links": [
                    link(&style_href(base_url, &s.id), "style", mime_for_format(&s.format), &s.id),
                    link(&style_meta_href(base_url, &s.id), "describedby", "application/json", "Metadata"),
                ],
            })
        })
        .collect();
    json!({
        "styles": styles,
        "links": [link(&format!("{}/styles", coll), "self", "application/json", "Styles")],
    })
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_style() -> StyleSummary {
        StyleSummary {
            id: "default".to_string(),
            title: "Default".to_string(),
            description: Some("Default style".to_string()),
            format: StyleFormat::SLD,
        }
    }

    fn sample_layer() -> Layer {
        Layer {
            name: "world".to_string(),
            title: "World".to_string(),
            abstract_text: Some("World layer".to_string()),
            workspace: "default".to_string(),
            store: "shapes".to_string(),
            native_name: None,
            native_bounds: crate::models::BoundingBox::world(
                crate::models::CoordinateReferenceSystem::EPSG4326,
            ),
            lat_lon_bounds: crate::models::BoundingBox::world(
                crate::models::CoordinateReferenceSystem::EPSG4326,
            ),
            srs: crate::models::CoordinateReferenceSystem::EPSG4326,
            styles: vec![crate::models::layer::StyleRef {
                name: "default".to_string(),
                href: None,
            }],
            resource: crate::models::layer::LayerResource {
                resource_type: crate::models::layer::ResourceType::FeatureType,
                path: None,
            },
            enabled: true,
            cache_store: None,
        }
    }

    #[test]
    fn test_mime_for_format() {
        assert_eq!(mime_for_format(&StyleFormat::SLD), STYLE_SLD_MIME);
        assert_eq!(mime_for_format(&StyleFormat::CSS), STYLE_CSS_MIME);
        assert_eq!(mime_for_format(&StyleFormat::YSLD), STYLE_YSLD_MIME);
        assert_eq!(mime_for_format(&StyleFormat::MBStyle), STYLE_MB_MIME);
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
        assert!(rels.contains(&"styles"));
        assert!(rels.contains(&"data"));
    }

    #[test]
    fn test_conformance_classes() {
        let v = conformance();
        let conforms = v["conformsTo"].as_array().unwrap();
        assert!(conforms
            .iter()
            .any(|x| x.as_str().unwrap().contains("core")));
        assert!(conforms
            .iter()
            .any(|x| x.as_str().unwrap().contains("styles-list")));
        assert!(conforms
            .iter()
            .any(|x| x.as_str().unwrap().contains("style-metadata")));
        assert!(conforms
            .iter()
            .any(|x| x.as_str().unwrap().contains("style-create-update-delete")));
    }

    #[test]
    fn test_styles_list() {
        let v = styles_list("http://localhost:8080", &[sample_style()]);
        let items = v["styles"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["id"], "default");
        let rels: Vec<&str> = items[0]["links"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l["rel"].as_str().unwrap())
            .collect();
        assert!(rels.contains(&"style"));
        assert!(rels.contains(&"describedby"));
    }

    #[test]
    fn test_style_metadata() {
        let v = style_metadata("http://localhost:8080", &sample_style());
        assert_eq!(v["id"], "default");
        assert_eq!(v["title"], "Default");
        assert_eq!(v["description"], "Default style");
    }

    #[test]
    fn test_collections_lists_layers() {
        let v = collections("http://localhost:8080", &[sample_layer()]);
        let colls = v["collections"].as_array().unwrap();
        assert_eq!(colls.len(), 1);
        assert_eq!(colls[0]["id"], "world");
    }

    #[test]
    fn test_collection_styles_resolves_existing() {
        let v = collection_styles("http://localhost:8080", &sample_layer(), &[sample_style()]);
        let styles = v["styles"].as_array().unwrap();
        assert_eq!(styles.len(), 1);
        assert_eq!(styles[0]["id"], "default");

        // A style the layer references but that no longer exists is skipped.
        let v = collection_styles("http://localhost:8080", &sample_layer(), &[]);
        assert_eq!(v["styles"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_style_href_formats() {
        assert!(
            style_href("http://localhost:8080", "default").contains("/ogc/styles/styles/default")
        );
        assert!(style_meta_href("http://localhost:8080", "default")
            .contains("/ogc/styles/styles/default/metadata"));
    }
}
