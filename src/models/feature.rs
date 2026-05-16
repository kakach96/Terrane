use serde::{Deserialize, Serialize};
use geo::{Coord, Geometry, Point, LineString, Polygon};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feature {
    pub id: String,
    pub geometry: GeoJsonGeometry,
    pub properties: HashMap<String, PropertyValue>,
}

impl Feature {
    pub fn new(geometry: GeoJsonGeometry, properties: HashMap<String, PropertyValue>) -> Self {
        Feature {
            id: Uuid::new_v4().to_string(),
            geometry,
            properties,
        }
    }

    pub fn with_id(id: String, geometry: GeoJsonGeometry, properties: HashMap<String, PropertyValue>) -> Self {
        Feature { id, geometry, properties }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum GeoJsonGeometry {
    Point { coordinates: Vec<f64> },
    LineString { coordinates: Vec<Vec<f64>> },
    Polygon { coordinates: Vec<Vec<Vec<f64>>> },
    MultiPoint { coordinates: Vec<Vec<f64>> },
    MultiLineString { coordinates: Vec<Vec<Vec<f64>>> },
    MultiPolygon { coordinates: Vec<Vec<Vec<Vec<f64>>>> },
    GeometryCollection { geometries: Vec<GeoJsonGeometry> },
}

impl GeoJsonGeometry {
    pub fn to_geo(&self) -> Geometry<f64> {
        match self {
            GeoJsonGeometry::Point { coordinates } => {
                if coordinates.len() >= 2 {
                    Geometry::Point(Point::new(coordinates[0], coordinates[1]))
                } else {
                    Geometry::Point(Point::new(0.0, 0.0))
                }
            }
            GeoJsonGeometry::LineString { coordinates } => {
                let points: Vec<Coord<f64>> = coordinates.iter()
                    .filter(|c| c.len() >= 2)
                    .map(|c| Coord { x: c[0], y: c[1] })
                    .collect();
                Geometry::LineString(LineString::new(points))
            }
            GeoJsonGeometry::Polygon { coordinates } => {
                let rings: Vec<LineString<f64>> = coordinates.iter()
                    .map(|ring| {
                        let points: Vec<Coord<f64>> = ring.iter()
                            .filter(|c| c.len() >= 2)
                            .map(|c| Coord { x: c[0], y: c[1] })
                            .collect();
                        LineString::new(points)
                    })
                    .collect();
                if !rings.is_empty() {
                    Geometry::Polygon(Polygon::new(rings[0].clone(), rings[1..].to_vec()))
                } else {
                    Geometry::Polygon(Polygon::new(LineString::new(vec![]), vec![]))
                }
            }
            _ => Geometry::Point(Point::new(0.0, 0.0)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PropertyValue {
    String(String),
    Number(f64),
    Integer(i64),
    Boolean(bool),
    Null,
    Array(Vec<PropertyValue>),
    Object(HashMap<String, PropertyValue>),
}

impl std::fmt::Display for PropertyValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PropertyValue::String(s) => write!(f, "{}", s),
            PropertyValue::Number(n) => write!(f, "{}", n),
            PropertyValue::Integer(i) => write!(f, "{}", i),
            PropertyValue::Boolean(b) => write!(f, "{}", b),
            PropertyValue::Null => write!(f, "null"),
            PropertyValue::Array(arr) => write!(f, "{:?}", arr),
            PropertyValue::Object(obj) => write!(f, "{:?}", obj),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureCollection {
    pub features: Vec<Feature>,
    pub total_count: usize,
}

impl FeatureCollection {
    pub fn new(features: Vec<Feature>) -> Self {
        let total_count = features.len();
        FeatureCollection { features, total_count }
    }

    pub fn empty() -> Self {
        FeatureCollection {
            features: vec![],
            total_count: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureType {
    pub type_name: String,
    pub properties: Vec<PropertyDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertyDefinition {
    pub name: String,
    pub property_type: PropertyType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PropertyType {
    String,
    Integer,
    Float,
    Boolean,
    Geometry,
}
