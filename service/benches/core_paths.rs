//! Micro-benchmarks for Terrane hot paths (criterion).
//!
//! Run the whole suite:
//!
//! ```bash
//! cargo bench
//! ```
//!
//! Run a single benchmark group:
//!
//! ```bash
//! cargo bench --bench core_paths -- cql_parse
//! ```
//!
//! Covered hot paths (see `docs/IMPLEMENTATION_PLAN.md` §5.3):
//! - CQL/ECQL filter parsing + evaluation (WMS/WFS `cql_filter` param)
//! - GML serialization (WFS GetFeature / WMS GetFeatureInfo output)
//! - Coordinate transforms (proj4rs path, EPSG:4326 → EPSG:3857)
//! - Bitmap-font label rendering (SLD TextSymbolizer)
//! - Vector map rendering to PNG (WMS GetMap / shared tile pipeline)
//! - Mapbox Vector Tile encoding (`/tiles` MVT output)
//! - WKB encode/decode round-trip (PostGIS/GeoPackage feature paths)

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use std::collections::HashMap;
use std::hint::black_box;
use terrane::models::{Bounds, Feature, GeoJsonGeometry, PropertyValue};
use terrane::utils::{bitmap_font, cql_filter, geometry, gml, mvt, rendering, wkb};

/// Deterministic point feature somewhere over France (EPSG:4326).
fn sample_point(i: usize) -> Feature {
    let lon = 2.0 + (i % 200) as f64 * 0.004;
    let lat = 48.5 + (i / 200) as f64 * 0.002;
    let mut properties = HashMap::new();
    properties.insert(
        "name".to_string(),
        PropertyValue::String(format!("place-{i}")),
    );
    properties.insert(
        "population".to_string(),
        PropertyValue::Integer((i % 90) as i64 * 1000),
    );
    properties.insert(
        "kind".to_string(),
        PropertyValue::String("city".to_string()),
    );
    Feature {
        id: format!("perf-{i}"),
        geometry: GeoJsonGeometry::Point {
            coordinates: vec![lon, lat],
        },
        properties,
    }
}

fn sample_points(n: usize) -> Vec<Feature> {
    (0..n).map(sample_point).collect()
}

fn france_bounds() -> Bounds {
    Bounds::new(2.0, 48.5, 3.0, 48.9)
}

fn bench_cql(c: &mut Criterion) {
    let expr = "population > 45000 AND kind = 'city' AND name LIKE 'place-1%'";

    c.bench_function("cql_parse/logical_comparison_like", |b| {
        b.iter(|| cql_filter::parse_cql(black_box(expr)).expect("parse"))
    });

    let features = sample_points(500);
    c.bench_function("cql_filter_features/500_points", |b| {
        b.iter(|| {
            let kept = cql_filter::filter_features(black_box(features.clone()), black_box(expr))
                .expect("filter");
            black_box(kept.len())
        })
    });

    c.bench_function("cql_parse/bbox_spatial", |b| {
        b.iter(|| {
            cql_filter::parse_cql(black_box("BBOX(geometry, 2.2, 48.6, 2.6, 48.85)"))
                .expect("parse")
        })
    });
}

fn bench_gml(c: &mut Criterion) {
    let feature = sample_point(42);

    c.bench_function("gml_feature_to_gml32/point", |b| {
        b.iter(|| gml::feature_to_gml32(black_box(&feature)))
    });

    c.bench_function("gml_escape_xml/label", |b| {
        b.iter(|| gml::escape_xml(black_box("Terrane <WMS> & \"OGC\" services — café")))
    });

    let polygon = GeoJsonGeometry::Polygon {
        coordinates: vec![vec![
            vec![2.25, 48.80],
            vec![2.42, 48.80],
            vec![2.42, 48.90],
            vec![2.25, 48.90],
            vec![2.25, 48.80],
        ]],
    };
    c.bench_function("gml_geometry_to_gml/polygon", |b| {
        b.iter(|| gml::geometry_to_gml(black_box(&polygon), black_box("2.0")))
    });
}

fn bench_transform(c: &mut Criterion) {
    c.bench_function("transform_coordinates/4326_to_3857", |b| {
        b.iter(|| {
            geometry::transform_coordinates(
                black_box(&[2.3522, 48.8566]),
                black_box("EPSG:4326"),
                black_box("EPSG:3857"),
            )
            .expect("transform")
        })
    });
}

fn bench_bitmap_font(c: &mut Criterion) {
    c.bench_function("bitmap_font_draw_text/12_chars", |b| {
        b.iter(|| {
            let mut pixels = Vec::new();
            bitmap_font::draw_text(0, 0, black_box("Hello Terrane"), 2.0, |x: u32, y: u32| {
                pixels.push((x, y))
            });
            black_box(pixels.len())
        })
    });
}

fn bench_render_map(c: &mut Criterion) {
    let mut group = c.benchmark_group("render_map");
    for n in [100usize, 500] {
        let features = sample_points(n);
        group.bench_with_input(
            BenchmarkId::new("points_png_256", n),
            &features,
            |b, feats| b.iter(|| rendering::render_map(black_box(feats), 256, 256)),
        );
    }
    group.finish();
}

fn bench_mvt(c: &mut Criterion) {
    let features = sample_points(200);
    let bounds = france_bounds();
    c.bench_function("mvt_encode_tile/200_points_extent4096", |b| {
        b.iter(|| {
            mvt::encode_tile(
                black_box(&features),
                black_box("perf"),
                black_box(&bounds),
                4096,
            )
        })
    });
}

fn bench_wkb(c: &mut Criterion) {
    let polygon = GeoJsonGeometry::Polygon {
        coordinates: vec![vec![
            vec![2.25, 48.80],
            vec![2.42, 48.80],
            vec![2.42, 48.90],
            vec![2.25, 48.90],
            vec![2.25, 48.80],
        ]],
    };
    let wkb_bytes = wkb::geometry_to_wkb(&polygon);

    c.bench_function("wkb_roundtrip/polygon", |b| {
        b.iter(|| {
            let encoded = wkb::geometry_to_wkb(black_box(&polygon));
            wkb::parse_wkb_geometry(black_box(&encoded))
        })
    });

    c.bench_function("wkb_parse/prepared_polygon", |b| {
        b.iter(|| wkb::parse_wkb_geometry(black_box(&wkb_bytes)))
    });
}

criterion_group!(
    benches,
    bench_cql,
    bench_gml,
    bench_transform,
    bench_bitmap_font,
    bench_render_map,
    bench_mvt,
    bench_wkb,
);
criterion_main!(benches);
