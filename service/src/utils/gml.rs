//! Shared GML serialization helpers (WFS GetFeature / GetGmlObject, WMS
//! GetFeatureInfo GML output).
//!
//! Kept intentionally small: one place for XML escaping and the basic
//! GeoJSON-geometry → GML converters so every OGC surface emits consistent
//! GML 2.1.2 / 3.1.1 / 3.2 markup.

use crate::models::{Feature, GeoJsonGeometry, PropertyValue};
use std::collections::HashMap;

/// XML-escape text for element/attribute content.
pub fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// GeoJSON geometry → GML. Supports Point / LineString / Polygon (the same
/// fidelity as the WFS output); other types yield an empty string.
pub fn geometry_to_gml(geometry: &GeoJsonGeometry, _version: &str) -> String {
    match geometry {
        GeoJsonGeometry::Point { coordinates } => {
            if coordinates.len() >= 2 {
                format!(
                    r#"<gml:Point srsName="EPSG:4326"><gml:pos>{} {}</gml:pos></gml:Point>"#,
                    coordinates[0], coordinates[1]
                )
            } else {
                String::new()
            }
        },
        GeoJsonGeometry::LineString { coordinates } => {
            let points: Vec<String> = coordinates
                .iter()
                .filter(|c| c.len() >= 2)
                .map(|c| format!("{} {}", c[0], c[1]))
                .collect();
            format!(
                r#"<gml:LineString srsName="EPSG:4326"><gml:posList>{}</gml:posList></gml:LineString>"#,
                points.join(" ")
            )
        },
        GeoJsonGeometry::Polygon { coordinates } => {
            if let Some(exterior) = coordinates.first() {
                let points: Vec<String> = exterior
                    .iter()
                    .filter(|c| c.len() >= 2)
                    .map(|c| format!("{} {}", c[0], c[1]))
                    .collect();
                format!(
                    r#"<gml:Polygon srsName="EPSG:4326"><gml:exterior><gml:LinearRing><gml:posList>{}</gml:posList></gml:LinearRing></gml:exterior></gml:Polygon>"#,
                    points.join(" ")
                )
            } else {
                String::new()
            }
        },
        _ => String::new(),
    }
}

/// Feature properties → `<feature:key>value</feature:key>` elements.
pub fn properties_to_gml(properties: &HashMap<String, PropertyValue>) -> String {
    let mut xml = String::new();
    for (key, value) in properties {
        xml.push_str(&format!(
            "                <feature:{}>{}</feature:{}>\n",
            key,
            escape_xml(&value.to_string()),
            key
        ));
    }
    xml
}

/// A single feature as a GML 3.2 `<Feature gml:id="...">` element (used by
/// WFS GetGmlObject members).
pub fn feature_to_gml32(feature: &Feature) -> String {
    format!(
        r#"            <Feature gml:id="{id}">
                <feature:geometry>
                    {geometry}
                </feature:geometry>
                {properties}
            </Feature>"#,
        id = feature.id,
        geometry = geometry_to_gml(&feature.geometry, "3.2"),
        properties = properties_to_gml(&feature.properties)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_xml() {
        assert_eq!(escape_xml("a<b>&\"'"), "a&lt;b&gt;&amp;&quot;&apos;");
        assert_eq!(escape_xml("plain"), "plain");
    }

    #[test]
    fn test_geometry_to_gml_point() {
        let g = GeoJsonGeometry::Point {
            coordinates: vec![10.0, 20.0],
        };
        let xml = geometry_to_gml(&g, "3.2");
        assert!(xml.contains("<gml:Point"));
        assert!(xml.contains("10 20"));
    }

    #[test]
    fn test_feature_to_gml32_includes_id_and_props() {
        let mut props = HashMap::new();
        props.insert(
            "name".to_string(),
            PropertyValue::String("alpha & co".to_string()),
        );
        let f = Feature::with_id(
            "f1".to_string(),
            GeoJsonGeometry::Point {
                coordinates: vec![1.0, 2.0],
            },
            props,
        );
        let xml = feature_to_gml32(&f);
        assert!(xml.contains("gml:id=\"f1\""));
        assert!(xml.contains("alpha &amp; co"));
    }
}
