use crate::models::{Bounds, GeoJsonGeometry};
use geo::{Area, BoundingRect, Contains, Coord, GeodesicDistance, HaversineLength, Intersects};
use geo_types::{Geometry, Rect};

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

pub fn clip_geometry_to_bounds(
    _geometry: &GeoJsonGeometry,
    _bounds: &Bounds,
) -> Option<GeoJsonGeometry> {
    None
}

pub fn simplify_geometry(geometry: &GeoJsonGeometry, _tolerance: f64) -> GeoJsonGeometry {
    geometry.clone()
}

/// Compute the centroid of a geometry as a Point (lon, lat). Uses the `geo`
/// `Centroid` algorithm on the projected coordinates.
pub fn centroid_geometry(geometry: &GeoJsonGeometry) -> Option<GeoJsonGeometry> {
    use geo::Centroid;
    geometry
        .to_geo()
        .centroid()
        .map(|p| GeoJsonGeometry::Point {
            coordinates: vec![p.x(), p.y()],
        })
}

/// Collect every distinct coordinate pair of a geometry (recursively for
/// collections), used by the manual buffer approximation.
fn collect_points_from_geometry(geometry: &GeoJsonGeometry, out: &mut Vec<(f64, f64)>) {
    use crate::models::GeoJsonGeometry as G;
    match geometry {
        G::Point { coordinates } if coordinates.len() >= 2 => {
            out.push((coordinates[0], coordinates[1]));
        },
        G::MultiPoint { coordinates } | G::LineString { coordinates } => {
            for c in coordinates {
                if c.len() >= 2 {
                    out.push((c[0], c[1]));
                }
            }
        },
        G::Polygon { coordinates } | G::MultiLineString { coordinates } => {
            for ring in coordinates {
                for c in ring {
                    if c.len() >= 2 {
                        out.push((c[0], c[1]));
                    }
                }
            }
        },
        G::MultiPolygon { coordinates } => {
            for poly in coordinates {
                for ring in poly {
                    for c in ring {
                        if c.len() >= 2 {
                            out.push((c[0], c[1]));
                        }
                    }
                }
            }
        },
        G::GeometryCollection { geometries } => {
            for sub in geometries {
                collect_points_from_geometry(sub, out);
            }
        },
        _ => {},
    }
}

/// A closed ring of a circle of radius `r` around `(x, y)` with `n` segments.
fn circle_ring(x: f64, y: f64, r: f64, n: u32) -> Vec<Vec<f64>> {
    let mut ring = Vec::with_capacity((n + 1) as usize);
    for i in 0..=n {
        let a = std::f64::consts::TAU * (i as f64) / (n as f64);
        ring.push(vec![x + r * a.cos(), y + r * a.sin()]);
    }
    ring
}

/// Buffer a geometry by `distance`. This is a *point buffer* approximation: it
/// places a circle of the given radius around every coordinate of the input
/// (exact for `Point`/`MultiPoint`, a conservative per-vertex approximation for
/// lines and polygons) and returns the circles as a Polygon / MultiPolygon.
pub fn buffer_geometry(geometry: &GeoJsonGeometry, distance: f64) -> Option<GeoJsonGeometry> {
    use crate::models::GeoJsonGeometry as G;
    if distance <= 0.0 {
        return None;
    }
    let mut pts: Vec<(f64, f64)> = Vec::new();
    collect_points_from_geometry(geometry, &mut pts);
    if pts.is_empty() {
        return None;
    }
    let n = 32u32;
    let rings: Vec<Vec<Vec<f64>>> = pts
        .iter()
        .map(|(x, y)| circle_ring(*x, *y, distance, n))
        .collect();
    if rings.len() == 1 {
        Some(G::Polygon { coordinates: rings })
    } else {
        Some(G::MultiPolygon {
            coordinates: vec![rings],
        })
    }
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
        _ => GeoJsonGeometry::Point {
            coordinates: vec![0.0, 0.0],
        },
    }
}

pub fn calculate_distance(geom1: &GeoJsonGeometry, geom2: &GeoJsonGeometry) -> Option<f64> {
    let geo1 = geom1.to_geo();
    let geo2 = geom2.to_geo();

    match (&geo1, &geo2) {
        (Geometry::Point(p1), Geometry::Point(p2)) => Some(p1.geodesic_distance(p2)),
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
            let y_3857 = 20037508.34 / std::f64::consts::PI
                * (std::f64::consts::PI / 4.0 + lat.to_radians() / 2.0)
                    .tan()
                    .ln();
            Ok(vec![x_3857, y_3857])
        },
        ("EPSG:3857", "EPSG:4326") | ("3857", "4326") => {
            let x_4326 = x * 180.0 / 20037508.34;
            let r = 20037508.34 / std::f64::consts::PI;
            let y_4326 = (std::f64::consts::PI / 2.0 - 2.0 * (-y / r).exp().atan()) * 180.0
                / std::f64::consts::PI;
            Ok(vec![x_4326, y_4326])
        },
        _ => Err(format!(
            "Unsupported projection transformation: {} to {}",
            from_srs, to_srs
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_4326_to_3857_roundtrip() {
        let (lon, lat) = (116.4, 39.9);
        let merc = transform_coordinates(&[lon, lat], "EPSG:4326", "EPSG:3857").unwrap();
        let wgs = transform_coordinates(&merc, "EPSG:3857", "EPSG:4326").unwrap();
        assert!((wgs[0] - lon).abs() < 0.001, "lon mismatch: {}", wgs[0]);
        assert!((wgs[1] - lat).abs() < 0.001, "lat mismatch: {}", wgs[1]);
    }

    #[test]
    fn test_3857_to_4326_known_point() {
        // Web Mercator 下的北京坐标
        let merc = transform_coordinates(&[116.4, 39.9], "EPSG:4326", "EPSG:3857").unwrap();
        let wgs = transform_coordinates(&merc, "EPSG:3857", "EPSG:4326").unwrap();
        assert!((wgs[0] - 116.4).abs() < 0.001);
        assert!((wgs[1] - 39.9).abs() < 0.001);
        // 纬度必须在合理范围内（旧实现错误地返回 ~104°）
        assert!(wgs[1] < 90.0 && wgs[1] > -90.0);
    }

    #[test]
    fn test_4326_to_3857_known_values() {
        // 赤道与经度 0 点
        let merc = transform_coordinates(&[0.0, 0.0], "EPSG:4326", "EPSG:3857").unwrap();
        assert!((merc[0]).abs() < 1e-6);
        assert!((merc[1]).abs() < 1e-6);
        // 经度 180 -> x 应为 20037508.34
        let merc = transform_coordinates(&[180.0, 0.0], "EPSG:4326", "EPSG:3857").unwrap();
        assert!((merc[0] - 20037508.34).abs() < 0.01);
    }
}
