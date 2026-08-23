//! # OGC API - Coverages implementation
//!
//! First OGC API - Coverages surface for Terrane (OGC 19-088), served at
//! `/ogc/coverages` (JSON). The reference GeoServer at :18080 does not ship
//! the OGC API extension, so this follows the OGC API - Coverages schema
//! directly.
//!
//! Resources: landing page, `/conformance`, `/collections`,
//! `/collections/{id}` and the `coverage` operation at
//! `/collections/{id}/coverage` (GeoTIFF default, PNG / JPEG via `?f=`), which
//! reuses the raster readers (GeoTIFF / ArcGrid / WorldImage) behind the WCS
//! 2.0 GetCoverage pipeline — so OGC API - Coverages serves the same coverages
//! as the WCS 2.0 interface.

use crate::models::Bounds;
use serde_json::{json, Value};

/// Default coverage media types.
pub const COVERAGE_TIFF_MIME: &str = "image/tiff";
pub const COVERAGE_PNG_MIME: &str = "image/png";
pub const COVERAGE_JPEG_MIME: &str = "image/jpeg";

/// A coverage collection — one per raster data source (GeoTIFF / WorldImage /
/// ArcGrid), mirroring how WCS 2.0 exposes coverages.
#[derive(Debug, Clone)]
pub struct CoverageCollection {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    /// Geographic (CRS84) bounds of the coverage.
    pub bbox: Bounds,
    /// Native SRS of the raster file, when known.
    pub srs: String,
    /// Raster size in pixels.
    pub width: u32,
    pub height: u32,
    /// Number of bands.
    pub band_count: usize,
    /// Raster file type (GeoTIFF / WorldImage / ArcGrid).
    pub file_type: String,
}

fn link(href: &str, rel: &str, type_: &str, title: &str) -> Value {
    json!({
        "href": href,
        "rel": rel,
        "type": type_,
        "title": title,
    })
}

/// Build the OGC API - Coverages landing page (`GET /ogc/coverages`).
pub fn landing_page(base_url: &str) -> Value {
    json!({
        "title": "Terrane",
        "description": "Cloud-native spatial data server powered by Rust — OGC API Coverages",
        "links": [
            link(&format!("{}/ogc/coverages", base_url), "self", "application/json", "This document"),
            link(&format!("{}/ogc/coverages/conformance", base_url), "conformance", "application/json", "OGC API conformance classes"),
            link(&format!("{}/ogc/coverages/collections", base_url), "data", "application/json", "Coverage collections"),
        ],
    })
}

/// Build the OGC API - Coverages conformance declaration
/// (`GET /ogc/coverages/conformance`).
pub fn conformance() -> Value {
    json!({
        "conformsTo": [
            "http://www.opengis.net/spec/ogcapi-coverages-1/1.0/conf/core",
            "http://www.opengis.net/spec/ogcapi-coverages-1/1.0/conf/oas30",
            "http://www.opengis.net/spec/ogcapi-coverages-1/1.0/conf/html",
            "http://www.opengis.net/spec/ogcapi-coverages-1/1.0/conf/collections",
            "http://www.opengis.net/spec/ogcapi-coverages-1/1.0/conf/coverage",
        ]
    })
}

/// Build the coverage operation href for a collection.
fn coverage_href(base_url: &str, id: &str, f: &str) -> String {
    format!(
        "{}/ogc/coverages/collections/{}/coverage?f={}",
        base_url, id, f
    )
}

/// Build the `/collections` document listing every raster data source as a
/// coverage collection.
pub fn collections(base_url: &str, coverages: &[CoverageCollection]) -> Value {
    let colls: Vec<Value> = coverages
        .iter()
        .map(|c| {
            let coll = format!("{}/ogc/coverages/collections/{}", base_url, c.id);
            json!({
                "id": c.id,
                "title": c.title,
                "links": [
                    link(&coll, "self", "application/json", &c.id),
                    link(&coverage_href(base_url, &c.id, COVERAGE_TIFF_MIME), "coverage", COVERAGE_TIFF_MIME, "Coverage (GeoTIFF)"),
                    link(&coverage_href(base_url, &c.id, COVERAGE_PNG_MIME), "coverage", COVERAGE_PNG_MIME, "Coverage (PNG)"),
                ],
            })
        })
        .collect();
    json!({
        "collections": colls,
        "links": [link(&format!("{}/ogc/coverages/collections", base_url), "self", "application/json", "Coverage collections")],
    })
}

fn range_field(name: &str, description: String) -> Value {
    json!({
        "name": name,
        "description": description,
        "dataType": "number",
        "component": "array",
        "size": 1,
    })
}

/// Build a single coverage collection document
/// (`GET /ogc/coverages/collections/{id}`).
pub fn collection(base_url: &str, c: &CoverageCollection) -> Value {
    let coll = format!("{}/ogc/coverages/collections/{}", base_url, c.id);
    let b = &c.bbox;
    let bands: Vec<Value> = (0..c.band_count.max(1))
        .map(|i| {
            range_field(
                &format!("band_{}", i),
                format!("Raster band {} ({})", i, c.file_type),
            )
        })
        .collect();
    json!({
        "id": c.id,
        "title": c.title,
        "description": c.description.clone().unwrap_or_else(|| format!("Raster coverage ({})", c.file_type)),
        "extent": {
            "spatial": {
                "bbox": [[b.minx, b.miny, b.maxx, b.maxy]],
                "crs": "http://www.opengis.net/def/crs/OGC/1.3/CRS84"
            }
        },
        "dimensions": {
            "spatial": {
                "bbox": [[b.minx, b.miny, b.maxx, b.maxy]],
                "crs": "http://www.opengis.net/def/crs/OGC/1.3/CRS84",
                "grid": {
                    "type": "Grid",
                    "transform": {
                        "scale": [c.width.max(1), c.height.max(1)],
                        "type": "Index2D"
                    }
                }
            }
        },
        "ranges": { "fields": bands },
        "links": [
            link(&coll, "self", "application/json", &c.id),
            link(&coverage_href(base_url, &c.id, COVERAGE_TIFF_MIME), "coverage", COVERAGE_TIFF_MIME, "Coverage (GeoTIFF)"),
            link(&coverage_href(base_url, &c.id, COVERAGE_PNG_MIME), "coverage", COVERAGE_PNG_MIME, "Coverage (PNG)"),
            link(&coverage_href(base_url, &c.id, COVERAGE_JPEG_MIME), "coverage", COVERAGE_JPEG_MIME, "Coverage (JPEG)"),
        ],
    })
}

/// Parse an OGC API - Coverages `bbox` query value (`minx,miny,maxx,maxy`).
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

    fn sample_coverage() -> CoverageCollection {
        CoverageCollection {
            id: "dem".to_string(),
            title: "DEM".to_string(),
            description: Some("Digital elevation model".to_string()),
            bbox: Bounds::new(-10.0, 20.0, 30.0, 40.0),
            srs: "EPSG:4326".to_string(),
            width: 128,
            height: 64,
            band_count: 1,
            file_type: "GeoTIFF".to_string(),
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
        assert!(conforms
            .iter()
            .any(|x| x.as_str().unwrap().contains("coverage")));
        assert!(conforms
            .iter()
            .any(|x| x.as_str().unwrap().contains("collections")));
    }

    #[test]
    fn test_collections_lists_coverages() {
        let v = collections("http://localhost:8080", &[sample_coverage()]);
        let colls = v["collections"].as_array().unwrap();
        assert_eq!(colls.len(), 1);
        assert_eq!(colls[0]["id"], "dem");
        let href = colls[0]["links"][0]["href"].as_str().unwrap();
        assert!(href.contains("/ogc/coverages/collections/dem"));
    }

    #[test]
    fn test_collection_links_and_dimensions() {
        let v = collection("http://localhost:8080", &sample_coverage());
        assert_eq!(v["id"], "dem");
        let rels: Vec<&str> = v["links"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l["rel"].as_str().unwrap())
            .collect();
        assert!(rels.contains(&"coverage"));
        assert!(rels.contains(&"self"));
        // CRS84 spatial extent
        let bbox = v["extent"]["spatial"]["bbox"][0].as_array().unwrap();
        assert_eq!(bbox[0], -10.0);
        assert_eq!(bbox[3], 40.0);
        // band range field
        let fields = v["ranges"]["fields"].as_array().unwrap();
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0]["name"], "band_0");
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
    fn test_coverage_href_formats() {
        let tiff = coverage_href("http://localhost:8080", "dem", COVERAGE_TIFF_MIME);
        assert!(tiff.contains("/ogc/coverages/collections/dem/coverage?f=image/tiff"));
        let png = coverage_href("http://localhost:8080", "dem", COVERAGE_PNG_MIME);
        assert!(png.contains("?f=image/png"));
    }
}
