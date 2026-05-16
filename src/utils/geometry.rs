use geo::{Coord, BoundingRect, Contains, Intersects, GeodesicDistance, Area, HaversineLength};
use geo_types::{Rect, Geometry};
use crate::models::{Bounds, GeoJsonGeometry};

pub fn calculate_bounds<T: IntoIterator<Item = Geometry<f64>>>(geometries: T) -> Option<Bounds> {
    let rect: Option<Rect<f64>> = geometries
        .into_iter()
        .filter_map(|g| g.bounding_rect())
        .reduce(|acc, r| {
            Rect::new(
                Coord {
                    x: acc.min().x.min(r.min().x),
                    y: acc.min().y.min(r.min().y),
                },
                Coord {
                    x: acc.max().x.max(r.max().x),
                    y: acc.max().y.max(r.max().y),
                },
            )
        });

    rect.map(Bounds::from_rect)
}

pub fn geometry_contains(outer: &GeoJsonGeometry, inner: &GeoJsonGeometry) -> bool {
    let outer_geo = outer.to_geo();
    let inner_geo = inner.to_geo();
    outer_geo.contains(&inner_geo)
}

pub fn geometry_intersects(geom1: &GeoJsonGeometry, geom2: &GeoJsonGeometry) -> bool {
    let geo1 = geom1.to_geo();
    let geo2 = geom2.to_geo();
    geo1.intersects(&geo2)
}

pub fn point_in_bounds(x: f64, y: f64, bounds: &Bounds) -> bool {
    bounds.contains(x, y)
}

pub fn clip_geometry_to_bounds(_geometry: &GeoJsonGeometry, _bounds: &Bounds) -> Option<GeoJsonGeometry> {
    None
}

pub fn simplify_geometry(geometry: &GeoJsonGeometry, _tolerance: f64) -> GeoJsonGeometry {
    geometry.clone()
}

pub fn buffer_geometry(_geometry: &GeoJsonGeometry, _distance: f64) -> Option<GeoJsonGeometry> {
    None
}

fn geojson_from_geo(geo: &Geometry<f64>) -> GeoJsonGeometry {
    match geo {
        Geometry::Point(p) => GeoJsonGeometry::Point {
            coordinates: vec![p.x(), p.y()],
        },
        Geometry::LineString(ls) => GeoJsonGeometry::LineString {
            coordinates: ls.coords().map(|c| vec![c.x, c.y]).collect(),
        },
        Geometry::Polygon(p) => GeoJsonGeometry::Polygon {
            coordinates: vec![p.exterior().coords().map(|c| vec![c.x, c.y]).collect()],
        },
        _ => GeoJsonGeometry::Point { coordinates: vec![0.0, 0.0] },
    }
}

pub fn calculate_distance(geom1: &GeoJsonGeometry, geom2: &GeoJsonGeometry) -> Option<f64> {
    let geo1 = geom1.to_geo();
    let geo2 = geom2.to_geo();
    
    match (&geo1, &geo2) {
        (Geometry::Point(p1), Geometry::Point(p2)) => {
            Some(p1.geodesic_distance(p2))
        }
        _ => None,
    }
}

pub fn calculate_area(geometry: &GeoJsonGeometry) -> f64 {
    let geo = geometry.to_geo();
    geo.unsigned_area()
}

pub fn calculate_length(geometry: &GeoJsonGeometry) -> f64 {
    let geo = geometry.to_geo();
    match &geo {
        Geometry::LineString(ls) => ls.haversine_length(),
        Geometry::Polygon(p) => p.exterior().haversine_length(),
        _ => 0.0,
    }
}

pub fn transform_coordinates(
    coords: &[f64],
    from_srs: &str,
    to_srs: &str,
) -> Result<Vec<f64>, String> {
    if from_srs == to_srs {
        return Ok(coords.to_vec());
    }
    
    if coords.len() < 2 {
        return Err("Insufficient coordinates".to_string());
    }
    
    let (x, y) = (coords[0], coords[1]);
    
    match (from_srs, to_srs) {
        ("EPSG:4326", "EPSG:3857") | ("4326", "3857") => {
            let lon = x;
            let lat = y;
            let x_3857 = lon * 20037508.34 / 180.0;
            let mut y_3857 = (lat * std::f64::consts::PI / 180.0).tan();
            y_3857 = 0.5 * (1.0 - y_3857.ln() / std::f64::consts::PI);
            let y_3857 = y_3857 * 20037508.34;
            Ok(vec![x_3857, y_3857])
        }
        ("EPSG:3857", "EPSG:4326") | ("3857", "4326") => {
            let x_4326 = x * 180.0 / 20037508.34;
            let mut y_4326 = y / 20037508.34;
            y_4326 = (std::f64::consts::PI / 2.0 - 2.0 * ((1.0 - y_4326.exp()) / (1.0 + y_4326.exp())).atan()) * 180.0 / std::f64::consts::PI;
            Ok(vec![x_4326, y_4326])
        }
        _ => Err(format!("Unsupported projection transformation: {} to {}", from_srs, to_srs)),
    }
}
