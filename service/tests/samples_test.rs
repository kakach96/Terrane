//! Built-in sample data integrity tests.
//!
//! The curated `service/samples/` files must stay loadable by the backend
//! readers — they power the first-startup demo seeding (`samples.rs`) and are
//! reused here as offline fixtures (no runtime-generated data needed):
//! - the three GeoJSON vectors parse and contain features,
//! - the GeoTIFF raster reads back its georeferenced bounds and dimensions.

use terrane::utils::samples::{SampleKind, SAMPLE_DATASETS};

/// Absolute path of the curated samples directory in the source tree.
fn samples_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("samples")
}

#[test]
fn sample_catalog_lists_all_curated_datasets() {
    assert_eq!(SAMPLE_DATASETS.len(), 4, "curated sample dataset count");
    let dir = samples_dir();
    for ds in SAMPLE_DATASETS {
        assert!(
            dir.join(ds.file).exists(),
            "sample file {} missing in {}",
            ds.file,
            dir.display()
        );
    }
}

#[test]
fn vector_samples_parse_as_geojson_feature_collections() {
    let dir = samples_dir();
    for ds in SAMPLE_DATASETS
        .iter()
        .filter(|d| d.kind == SampleKind::Vector)
    {
        let raw = std::fs::read_to_string(dir.join(ds.file))
            .unwrap_or_else(|e| panic!("read {}: {}", ds.file, e));
        let json: serde_json::Value =
            serde_json::from_str(&raw).unwrap_or_else(|e| panic!("parse {}: {}", ds.file, e));
        assert_eq!(json["type"], "FeatureCollection", "{}", ds.file);
        let features = json["features"]
            .as_array()
            .unwrap_or_else(|| panic!("{} has no features array", ds.file));
        assert!(!features.is_empty(), "{} has no features", ds.file);
        for f in features {
            let geom = &f["geometry"];
            assert!(geom.is_object(), "{}: feature without geometry", ds.file);
            let coords = geom["coordinates"].to_string();
            assert!(coords.len() > 2, "{}: empty coordinates", ds.file);
        }
    }
}

#[test]
fn raster_sample_reads_georeferenced_bounds() {
    let dir = samples_dir();
    let path = dir.join("sample_elevation.tif");
    let coverage = terrane::utils::geotiff::read_geotiff(&path)
        .unwrap_or_else(|e| panic!("read sample_elevation.tif: {}", e));

    // Dimensions
    assert_eq!(coverage.width, 64, "sample raster width");
    assert_eq!(coverage.height, 64, "sample raster height");

    // Georeferencing: tiepoint (116, 40), pixel scale 0.05 deg -> 3.2 deg span
    let bounds = coverage
        .bounds
        .unwrap_or_else(|| panic!("sample raster has no georeferenced bounds"));
    let b = &bounds;
    assert!((b.minx - 116.0).abs() < 1e-6, "minx = {}", b.minx);
    assert!((b.maxx - 119.2).abs() < 1e-6, "maxx = {}", b.maxx);
    assert!((b.maxy - 40.0).abs() < 1e-6, "maxy = {}", b.maxy);
    assert!((b.miny - 36.8).abs() < 1e-6, "miny = {}", b.miny);
}
