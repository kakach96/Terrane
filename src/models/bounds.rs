use geo::Coord;
use geo_types::Rect;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bounds {
    pub minx: f64,
    pub miny: f64,
    pub maxx: f64,
    pub maxy: f64,
}

impl Bounds {
    pub fn new(minx: f64, miny: f64, maxx: f64, maxy: f64) -> Self {
        Bounds {
            minx,
            miny,
            maxx,
            maxy,
        }
    }

    pub fn from_rect(rect: Rect<f64>) -> Self {
        Bounds {
            minx: rect.min().x,
            miny: rect.min().y,
            maxx: rect.max().x,
            maxy: rect.max().y,
        }
    }

    pub fn to_rect(&self) -> Rect<f64> {
        Rect::new(
            Coord {
                x: self.minx,
                y: self.miny,
            },
            Coord {
                x: self.maxx,
                y: self.maxy,
            },
        )
    }

    pub fn intersects(&self, other: &Bounds) -> bool {
        self.minx <= other.maxx
            && self.maxx >= other.minx
            && self.miny <= other.maxy
            && self.maxy >= other.miny
    }

    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.minx && x <= self.maxx && y >= self.miny && y <= self.maxy
    }

    pub fn expand_to_include(&mut self, other: &Bounds) {
        self.minx = self.minx.min(other.minx);
        self.miny = self.miny.min(other.miny);
        self.maxx = self.maxx.max(other.maxx);
        self.maxy = self.maxy.max(other.maxy);
    }
}

impl Default for Bounds {
    fn default() -> Self {
        Bounds {
            minx: -180.0,
            miny: -90.0,
            maxx: 180.0,
            maxy: 90.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CoordinateReferenceSystem {
    EPSG4326,
    EPSG3857,
    Custom(String),
}

impl CoordinateReferenceSystem {
    pub fn from_epsg(code: &str) -> Self {
        match code {
            "EPSG:4326" | "4326" => CoordinateReferenceSystem::EPSG4326,
            "EPSG:3857" | "3857" | "EPSG:900913" | "900913" => CoordinateReferenceSystem::EPSG3857,
            _ => CoordinateReferenceSystem::Custom(code.to_string()),
        }
    }

    pub fn to_epsg(&self) -> String {
        match self {
            CoordinateReferenceSystem::EPSG4326 => "EPSG:4326".to_string(),
            CoordinateReferenceSystem::EPSG3857 => "EPSG:3857".to_string(),
            CoordinateReferenceSystem::Custom(code) => code.clone(),
        }
    }

    pub fn is_geographic(&self) -> bool {
        matches!(self, CoordinateReferenceSystem::EPSG4326)
    }

    pub fn is_projected(&self) -> bool {
        matches!(self, CoordinateReferenceSystem::EPSG3857)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundingBox {
    pub crs: CoordinateReferenceSystem,
    pub bounds: Bounds,
}

impl BoundingBox {
    pub fn new(crs: CoordinateReferenceSystem, bounds: Bounds) -> Self {
        BoundingBox { crs, bounds }
    }

    pub fn world(crs: CoordinateReferenceSystem) -> Self {
        let bounds = match &crs {
            CoordinateReferenceSystem::EPSG4326 => Bounds::new(-180.0, -90.0, 180.0, 90.0),
            CoordinateReferenceSystem::EPSG3857 => {
                Bounds::new(-20037508.34, -20037508.34, 20037508.34, 20037508.34)
            },
            CoordinateReferenceSystem::Custom(code) => {
                if code.contains("3857") || code.contains("900913") {
                    Bounds::new(-20037508.34, -20037508.34, 20037508.34, 20037508.34)
                } else {
                    Bounds::new(-180.0, -90.0, 180.0, 90.0)
                }
            },
        };
        BoundingBox { crs, bounds }
    }
}
