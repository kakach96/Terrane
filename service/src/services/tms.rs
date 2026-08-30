//! # TMS (Tile Map Service) 1.0.0 implementation
//!
//! Served under `{api_context}/gwc/service/tms`, mirroring how the reference
//! GeoServer exposes GeoWebCache. Reference: TMS 1.0.0 specification
//! (https://wiki.osgeo.org/wiki/Tile_Map_Service_Specification).
//!
//! Supported access styles:
//! - GetCapabilities: RESTful `/1.0.0` (and KVP `?REQUEST=GetCapabilities`)
//! - TileMap document: `/1.0.0/{layer}@{gridset}@{format}`
//! - GetTile: `/1.0.0/{layer}@{gridset}@{format}/{z}/{x}/{y}.{ext}` where `y`
//!   is the *bottom-up* TMS row (the origin is the south-west corner), so the
//!   row is flipped before reaching the shared tile engine.

use crate::models::Layer;
use crate::utils::tile_grid;

/// Formats advertised by the TMS service: `(mime-type, file extension)`.
pub const FORMATS: &[(&str, &str)] = &[("image/png", "png"), ("image/jpeg", "jpeg")];

/// Grid sets advertised by the TMS service:
/// `(gridset id, SRS label, TMS profile label)`.
pub const GRIDSETS: &[(&str, &str, &str)] = &[
    ("EPSG:4326", "EPSG:4326", "global-geodetic"),
    ("EPSG:3857", "EPSG:3857", "global-mercator"),
];

/// Resolve a file extension to its MIME type (defaults to `image/png`).
pub fn mime_for_extension(ext: &str) -> &'static str {
    match ext {
        "png" => "image/png",
        "jpeg" | "jpg" => "image/jpeg",
        _ => "image/png",
    }
}

/// URL-encode a tile-map id for embedding in hrefs (`:` → `%3A`).
pub fn url_encode_id(id: &str) -> String {
    id.replace(':', "%3A")
}

/// Build a tile-map id: `{layer}@{gridset}@{ext}`.
pub fn tilemap_id(layer: &str, gridset: &str, ext: &str) -> String {
    format!("{}@{}@{}", layer, gridset, ext)
}

/// Parse a tile-map id like `sf:archsites@EPSG:4326@png` into
/// `(layer, gridset, format-extension)`.
pub fn parse_tilemap_id(id: &str) -> Option<(String, String, String)> {
    let parts: Vec<&str> = id.split('@').collect();
    if parts.len() == 3 {
        Some((
            parts[0].to_string(),
            parts[1].to_string(),
            parts[2].to_string(),
        ))
    } else {
        None
    }
}

/// A parsed TMS GetTile path (`{tilemap}/{z}/{x}/{y}.{ext}`).
pub struct TmsTilePath {
    pub layer: String,
    pub gridset: String,
    pub z: u32,
    pub x: u32,
    /// TMS (bottom-up) row.
    pub y_tms: u32,
    pub ext: String,
}

/// Parse a TMS tile path tail (everything after `/1.0.0/`).
pub fn parse_tile_path(tail: &str) -> Option<TmsTilePath> {
    let mut segs = tail.splitn(5, '/');
    let tilemap = segs.next()?;
    let z: u32 = segs.next()?.parse().ok()?;
    let x: u32 = segs.next()?.parse().ok()?;
    let y_ext = segs.next()?;
    if segs.next().is_some() {
        return None;
    }
    let (y_str, ext) = y_ext.rsplit_once('.')?;
    let y: u32 = y_str.parse().ok()?;
    let (layer, gridset, _id_format) = parse_tilemap_id(tilemap)?;
    Some(TmsTilePath {
        layer,
        gridset,
        z,
        x,
        y_tms: y,
        ext: ext.to_string(),
    })
}

/// Minimal XML attribute escaping.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Build the TMS GetCapabilities document (`TileMapService`).
pub fn build_tile_map_service(base_url: &str, layers: &[Layer]) -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\n");
    out.push_str(&format!(
        "<TileMapService version=\"1.0.0\" services=\"{}/gwc/\">\n",
        base_url
    ));
    out.push_str("  <Title>Tile Map Service</Title>\n");
    out.push_str("  <Abstract>A Tile Map Service served by GeoWebCache</Abstract>\n");
    out.push_str("  <TileMaps>\n");
    for layer in layers {
        for &(gridset, srs, profile) in GRIDSETS {
            for &(_mime, ext) in FORMATS {
                let id = url_encode_id(&tilemap_id(&layer.name, gridset, ext));
                out.push_str(&format!(
                    "    <TileMap title=\"{}\" srs=\"{}\" profile=\"{}\" href=\"{}/gwc/service/tms/1.0.0/{}\"/>\n",
                    xml_escape(&layer.title),
                    srs,
                    profile,
                    base_url,
                    id
                ));
            }
        }
    }
    out.push_str("  </TileMaps>\n");
    out.push_str("</TileMapService>\n");
    out
}

/// Build a TMS `TileMap` document for a `layer@gridset@format` id. Returns
/// `None` when the gridset or format is unknown.
pub fn build_tile_map(
    base_url: &str,
    layer: &Layer,
    gridset: &str,
    format: &str,
) -> Option<String> {
    tile_grid::gridset_profile(gridset)?;
    let ext = format;
    let mime = mime_for_extension(ext);
    let id = url_encode_id(&tilemap_id(&layer.name, gridset, ext));
    let profile = tile_grid::profile_label(gridset);
    let b = &layer.native_bounds.bounds;
    let max_zoom = tile_grid::MAX_ZOOM;

    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\n");
    out.push_str(&format!(
        "<TileMap version=\"1.0.0\" tilemapservice=\"{}/gwc/service/tms/1.0.0\">\n",
        base_url
    ));
    out.push_str(&format!("  <Title>{}</Title>\n", xml_escape(&layer.title)));
    out.push_str(&format!(
        "  <Abstract>{}</Abstract>\n",
        xml_escape(&layer.abstract_text.clone().unwrap_or_default())
    ));
    out.push_str(&format!("  <SRS>{}</SRS>\n", gridset));
    out.push_str(&format!(
        "  <BoundingBox minx=\"{}\" miny=\"{}\" maxx=\"{}\" maxy=\"{}\"/>\n",
        b.minx, b.miny, b.maxx, b.maxy
    ));
    out.push_str(&format!("  <Origin x=\"{}\" y=\"{}\"/>\n", b.minx, b.miny));
    out.push_str(&format!(
        "  <TileFormat width=\"256\" height=\"256\" mime-type=\"{}\" extension=\"{}\"/>\n",
        mime, ext
    ));
    out.push_str(&format!("  <TileSets profile=\"{}\">\n", profile));
    for z in 0..=max_zoom {
        let upp = tile_grid::units_per_pixel(gridset, z);
        out.push_str(&format!(
            "    <TileSet href=\"{}/gwc/service/tms/1.0.0/{}/{}\" units-per-pixel=\"{}\" order=\"{}\"/>\n",
            base_url, id, z, upp, z
        ));
    }
    out.push_str("  </TileSets>\n");
    out.push_str("</TileMap>\n");
    Some(out)
}

/// Parse a TMS KVP GetTile request parameter map. Keys are matched
/// case-insensitively; returns `None` when a required key is missing.
pub fn parse_kvp_tile(params: &[(String, String)]) -> Option<TmsTilePath> {
    let get = |key: &str| {
        params
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.clone())
    };
    let layer = get("LAYER")?;
    let gridset = get("TILEMATRIXSET").unwrap_or_else(|| "EPSG:4326".to_string());
    let z: u32 = get("TILEMATRIX")?.parse().ok()?;
    let x: u32 = get("TILECOL")?.parse().ok()?;
    let y: u32 = get("TILEROW")?.parse().ok()?;
    let format = get("FORMAT").unwrap_or_else(|| "image/png".to_string());
    let ext = match FORMATS.iter().find(|(m, _)| *m == format) {
        Some((_, e)) => e.to_string(),
        None => "png".to_string(),
    };
    Some(TmsTilePath {
        layer,
        gridset,
        z,
        x,
        y_tms: y,
        ext,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        layer::{LayerResource, ResourceType},
        BoundingBox, Bounds, CoordinateReferenceSystem,
    };

    fn sample_layer(name: &str, title: &str) -> Layer {
        Layer {
            name: name.to_string(),
            title: title.to_string(),
            abstract_text: None,
            workspace: "default".to_string(),
            store: "shapes".to_string(),
            native_name: Some(name.to_string()),
            native_bounds: BoundingBox::new(
                CoordinateReferenceSystem::EPSG4326,
                Bounds::new(-180.0, -90.0, 180.0, 90.0),
            ),
            lat_lon_bounds: BoundingBox::new(
                CoordinateReferenceSystem::EPSG4326,
                Bounds::new(-180.0, -90.0, 180.0, 90.0),
            ),
            srs: CoordinateReferenceSystem::EPSG4326,
            styles: vec![],
            resource: LayerResource {
                resource_type: ResourceType::FeatureType,
                path: None,
            },
            enabled: true,
            cache_store: None,
        }
    }

    #[test]
    fn test_parse_tilemap_id() {
        let (layer, gridset, format) = parse_tilemap_id("sf:archsites@EPSG:4326@png").unwrap();
        assert_eq!(layer, "sf:archsites");
        assert_eq!(gridset, "EPSG:4326");
        assert_eq!(format, "png");
        assert!(parse_tilemap_id("bad").is_none());
        assert!(parse_tilemap_id("a@b@c@d").is_none());
    }

    #[test]
    fn test_parse_tile_path() {
        let p = parse_tile_path("sf:archsites@EPSG:4326@png/2/1/3.png").unwrap();
        assert_eq!(p.layer, "sf:archsites");
        assert_eq!(p.gridset, "EPSG:4326");
        assert_eq!(p.z, 2);
        assert_eq!(p.x, 1);
        assert_eq!(p.y_tms, 3);
        assert_eq!(p.ext, "png");

        // Extra path segment → not a tile path.
        assert!(parse_tile_path("sf:archsites@EPSG:4326@png/2/1/3.png/extra").is_none());
        // Non-numeric segments → rejected.
        assert!(parse_tile_path("sf:archsites@EPSG:4326@png/a/1/3.png").is_none());
        // Missing extension → rejected.
        assert!(parse_tile_path("sf:archsites@EPSG:4326@png/2/1/3").is_none());
    }

    #[test]
    fn test_url_encode_id() {
        assert_eq!(
            url_encode_id("sf:archsites@EPSG:4326@png"),
            "sf%3Aarchsites@EPSG%3A4326@png"
        );
    }

    #[test]
    fn test_build_tile_map_service_lists_layers() {
        let layers = vec![sample_layer(
            "sf:archsites",
            "Spearfish archeological sites",
        )];
        let doc = build_tile_map_service("http://127.0.0.1:8080/terrane", &layers);
        assert!(doc.contains("<TileMapService version=\"1.0.0\""));
        assert!(doc.contains("<TileMaps>"));
        // 2 gridsets × 2 formats per layer.
        assert_eq!(doc.matches("<TileMap ").count(), 4);
        assert!(doc.contains("sf%3Aarchsites@EPSG%3A4326@png"));
        assert!(doc.contains("sf%3Aarchsites@EPSG%3A3857@jpeg"));
        assert!(doc.contains("profile=\"global-geodetic\""));
        assert!(doc.contains("profile=\"global-mercator\""));
    }

    #[test]
    fn test_build_tile_map_document() {
        let layer = sample_layer("world", "World");
        let doc =
            build_tile_map("http://127.0.0.1:8080/terrane", &layer, "EPSG:4326", "png").unwrap();
        assert!(doc.contains("<TileMap version=\"1.0.0\""));
        assert!(doc.contains("<SRS>EPSG:4326</SRS>"));
        assert!(doc.contains("<BoundingBox minx=\"-180\""));
        assert!(doc.contains("<Origin x=\"-180\" y=\"-90\"/>"));
        assert!(doc.contains("mime-type=\"image/png\" extension=\"png\""));
        assert!(doc.contains("<TileSets profile=\"global-geodetic\">"));
        assert!(doc.contains("units-per-pixel=\"0.703125\" order=\"0\""));
        assert!(doc.contains("order=\"18\""));
        // Unknown gridset → None.
        assert!(build_tile_map("http://x", &layer, "EPSG:9999", "png").is_none());
    }
}
