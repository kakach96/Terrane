use crate::models::CoordinateReferenceSystem;

pub struct ProjectionTransformer {
    source_crs: CoordinateReferenceSystem,
    target_crs: CoordinateReferenceSystem,
}

impl ProjectionTransformer {
    pub fn new(source: CoordinateReferenceSystem, target: CoordinateReferenceSystem) -> Self {
        ProjectionTransformer {
            source_crs: source,
            target_crs: target,
        }
    }

    pub fn transform_point(&self, x: f64, y: f64) -> Result<(f64, f64), String> {
        if self.source_crs == self.target_crs {
            return Ok((x, y));
        }

        let from_epsg = self.source_crs.to_epsg();
        let to_epsg = self.target_crs.to_epsg();

        super::geometry::transform_coordinates(&[x, y], &from_epsg, &to_epsg)
            .map(|coords| (coords[0], coords[1]))
    }

    pub fn transform_bounds(&self, minx: f64, miny: f64, maxx: f64, maxy: f64) -> Result<(f64, f64, f64, f64), String> {
        let (x1, y1) = self.transform_point(minx, miny)?;
        let (x2, y2) = self.transform_point(maxx, maxy)?;
        Ok((x1.min(x2), y1.min(y2), x1.max(x2), y1.max(y2)))
    }

    pub fn needs_reprojection(&self) -> bool {
        self.source_crs != self.target_crs
    }
}

pub fn get_transform_definition(from: &str, to: &str) -> Option<String> {
    match (from, to) {
        ("EPSG:4326", "EPSG:3857") | ("4326", "3857") => {
            Some("+proj=merc +a=6378137 +b=6378137 +lat_ts=0.0 +lon_0=0.0 +x_0=0.0 +y_0=0.0 +k=1.0 +units=m +nadgrids=@null +wktext +no_defs".to_string())
        }
        ("EPSG:3857", "EPSG:4326") | ("3857", "4326") => {
            Some("+proj=longlat +a=6378137 +b=6378137 +lat_ts=0.0 +lon_0=0.0 +x_0=0.0 +y_0=0.0 +k=1.0 +units=m +nadgrids=@null +wktext +no_defs".to_string())
        }
        _ => None,
    }
}

pub fn validate_crs(crs: &str) -> bool {
    matches!(
        crs,
        "EPSG:4326" | "4326" | "EPSG:3857" | "3857" | "EPSG:900913" | "900913"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_transform() {
        let transformer = ProjectionTransformer::new(
            CoordinateReferenceSystem::EPSG4326,
            CoordinateReferenceSystem::EPSG4326,
        );
        assert!(!transformer.needs_reprojection());
        let (x, y) = transformer.transform_point(100.0, 30.0).unwrap();
        assert!((x - 100.0).abs() < 0.001);
        assert!((y - 30.0).abs() < 0.001);
    }

    #[test]
    fn test_4326_to_3857_needs_reprojection() {
        let transformer = ProjectionTransformer::new(
            CoordinateReferenceSystem::EPSG4326,
            CoordinateReferenceSystem::EPSG3857,
        );
        assert!(transformer.needs_reprojection());
    }

    #[test]
    fn test_get_transform_definition() {
        let def = get_transform_definition("EPSG:4326", "EPSG:3857");
        assert!(def.is_some());
        assert!(def.unwrap().contains("+proj=merc"));

        let def = get_transform_definition("EPSG:3857", "EPSG:4326");
        assert!(def.is_some());
        assert!(def.unwrap().contains("+proj=longlat"));
    }

    #[test]
    fn test_get_transform_definition_unsupported() {
        let def = get_transform_definition("EPSG:4326", "EPSG:4269");
        assert!(def.is_none());
    }

    #[test]
    fn test_validate_crs() {
        assert!(validate_crs("EPSG:4326"));
        assert!(validate_crs("EPSG:3857"));
        assert!(validate_crs("4326"));
        assert!(!validate_crs("EPSG:9999"));
        assert!(!validate_crs("invalid"));
    }

    #[test]
    fn test_normalize_crs() {
        assert_eq!(normalize_crs("EPSG:4326"), "EPSG:4326");
        assert_eq!(normalize_crs("4326"), "EPSG:4326");
        assert_eq!(normalize_crs("EPSG:3857"), "EPSG:3857");
        assert_eq!(normalize_crs("EPSG:900913"), "EPSG:3857");
        assert_eq!(normalize_crs("900913"), "EPSG:3857");
        assert_eq!(normalize_crs("EPSG:4269"), "EPSG:4269");
    }

    #[test]
    fn test_transform_bounds_same_crs() {
        let transformer = ProjectionTransformer::new(
            CoordinateReferenceSystem::EPSG4326,
            CoordinateReferenceSystem::EPSG4326,
        );
        let result = transformer.transform_bounds(-180.0, -90.0, 180.0, 90.0).unwrap();
        assert!((result.0 - (-180.0)).abs() < 0.001);
        assert!((result.1 - (-90.0)).abs() < 0.001);
        assert!((result.2 - 180.0).abs() < 0.001);
        assert!((result.3 - 90.0).abs() < 0.001);
    }
}

pub fn normalize_crs(crs: &str) -> String {
    match crs {
        "EPSG:4326" | "4326" => "EPSG:4326".to_string(),
        "EPSG:3857" | "3857" | "EPSG:900913" | "900913" => "EPSG:3857".to_string(),
        _ => crs.to_string(),
    }
}
