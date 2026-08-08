//! # WMS-C (Cached WMS) 1.1.1 implementation
//!
//! Served under `{api_context}/gwc/service/wms`, mirroring how the reference
//! GeoServer exposes GeoWebCache's WMS-C endpoint. It behaves like WMS 1.1.1
//! but additionally supports the `TILED=true` vendor parameter: a grid-aligned
//! GetMap then resolves to a single tile through the shared tile engine.

use crate::models::{Bounds, Layer};

/// Minimal XML attribute escaping.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Web-Mercator projection of a geographic bounds (EPSG:4326 → EPSG:3857).
pub fn to_mercator_bounds(b: &Bounds) -> Bounds {
    let x = |lon: f64| lon * (20037508.34 / 180.0);
    let y = |lat: f64| {
        let r = lat.to_radians();
        let t = (std::f64::consts::FRAC_PI_4 + r / 2.0).tan();
        t.ln() * 20037508.34 / std::f64::consts::PI
    };
    Bounds::new(
        x(b.minx).max(-20037508.34),
        y(b.miny).max(-20037508.34),
        x(b.maxx).min(20037508.34),
        y(b.maxy).min(20037508.34),
    )
}

/// Build the WMS-C GetCapabilities document (WMS 1.1.1 `WMT_MS_Capabilities`).
pub fn build_capabilities(base_url: &str, layers: &[Layer]) -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\n");
    out.push_str(
        "<!DOCTYPE WMT_MS_Capabilities SYSTEM \"http://schemas.opengis.net/wms/1.1.1/capabilities_1_1_1.dtd\">\n\n",
    );
    out.push_str("<WMT_MS_Capabilities version=\"1.1.1\">\n");
    out.push_str("  <Service>\n");
    out.push_str("    <Name>OGC:WMS</Name>\n");
    out.push_str("    <Title>Web Map Service - GeoWebCache</Title>\n");
    out.push_str(&format!(
        "    <OnlineResource xmlns:xlink=\"http://www.w3.org/1999/xlink\" xlink:type=\"simple\" xlink:href=\"{}/gwc/service/wms?SERVICE=WMS&amp;\"/>\n",
        base_url
    ));
    out.push_str("  </Service>\n");
    out.push_str("  <Capability>\n");
    out.push_str("    <Request>\n");
    out.push_str("      <GetCapabilities>\n");
    out.push_str("        <Format>application/vnd.ogc.wms_xml</Format>\n");
    out.push_str(&format!(
        "        <DCPType><HTTP><Get><OnlineResource xlink:type=\"simple\" xlink:href=\"{}/gwc/service/wms?\"/></Get></HTTP></DCPType>\n",
        base_url
    ));
    out.push_str("      </GetCapabilities>\n");
    out.push_str("      <GetMap>\n");
    for f in ["image/png", "image/jpeg", "image/gif"] {
        out.push_str(&format!("        <Format>{}</Format>\n", f));
    }
    out.push_str(&format!(
        "        <DCPType><HTTP><Get><OnlineResource xlink:type=\"simple\" xlink:href=\"{}/gwc/service/wms?\"/></Get></HTTP></DCPType>\n",
        base_url
    ));
    out.push_str("      </GetMap>\n");
    out.push_str("    </Request>\n");
    out.push_str("    <Exception><Format>application/vnd.ogc.se_xml</Format></Exception>\n");
    out.push_str("    <Layer>\n");
    out.push_str("      <Title>Terrane</Title>\n");
    for layer in layers {
        out.push_str("      <Layer queryable=\"1\">\n");
        out.push_str(&format!(
            "        <Name>{}</Name>\n",
            xml_escape(&layer.name)
        ));
        out.push_str(&format!(
            "        <Title>{}</Title>\n",
            xml_escape(&layer.title)
        ));
        out.push_str("        <SRS>EPSG:4326</SRS>\n");
        out.push_str("        <SRS>EPSG:3857</SRS>\n");
        let lb = &layer.native_bounds.bounds;
        out.push_str(&format!(
            "        <LatLonBoundingBox minx=\"{}\" miny=\"{}\" maxx=\"{}\" maxy=\"{}\"/>\n",
            lb.minx, lb.miny, lb.maxx, lb.maxy
        ));
        out.push_str(&format!(
            "        <BoundingBox SRS=\"EPSG:4326\" minx=\"{}\" miny=\"{}\" maxx=\"{}\" maxy=\"{}\"/>\n",
            lb.minx, lb.miny, lb.maxx, lb.maxy
        ));
        let mb = to_mercator_bounds(lb);
        out.push_str(&format!(
            "        <BoundingBox SRS=\"EPSG:3857\" minx=\"{}\" miny=\"{}\" maxx=\"{}\" maxy=\"{}\"/>\n",
            mb.minx, mb.miny, mb.maxx, mb.maxy
        ));
        out.push_str("      </Layer>\n");
    }
    out.push_str("    </Layer>\n");
    out.push_str("  </Capability>\n");
    out.push_str("</WMT_MS_Capabilities>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        layer::{LayerResource, ResourceType},
        BoundingBox, CoordinateReferenceSystem,
    };

    fn sample_layer(name: &str) -> Layer {
        Layer {
            name: name.to_string(),
            title: name.to_string(),
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
        }
    }

    #[test]
    fn test_build_capabilities_structure() {
        let layers = vec![sample_layer("world")];
        let doc = build_capabilities("http://127.0.0.1:8080/geoserver", &layers);
        assert!(doc.contains("<WMT_MS_Capabilities version=\"1.1.1\">"));
        assert!(doc.contains("<Name>OGC:WMS</Name>"));
        assert!(doc.contains("Web Map Service - GeoWebCache"));
        assert!(doc.contains("<GetMap>"));
        assert!(doc.contains("<Format>image/png</Format>"));
        assert!(doc.contains("<Name>world</Name>"));
        assert!(doc.contains("<SRS>EPSG:4326</SRS>"));
        assert!(doc.contains("<LatLonBoundingBox minx=\"-180\""));
    }

    #[test]
    fn test_to_mercator_bounds() {
        let b = Bounds::new(-180.0, -90.0, 180.0, 90.0);
        let m = to_mercator_bounds(&b);
        assert!((m.minx - -20037508.34).abs() < 1.0);
        assert!((m.maxx - 20037508.34).abs() < 1.0);
        // The mercator y of ±85.0511 (not ±90) → clamped to the world bounds.
        assert!((m.miny - -20037508.34).abs() < 1.0);
        assert!((m.maxy - 20037508.34).abs() < 1.0);
    }
}
