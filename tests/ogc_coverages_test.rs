//! OGC API - Coverages integration tests.
//!
//! Covers the OGC API - Coverages surface (OGC 19-088): landing page,
//! /conformance, /collections, /collections/{id} and the `coverage`
//! operation at /collections/{id}/coverage (GeoTIFF default, PNG / JPEG via
//! `?f=`), including bbox cropping on a real georeferenced GeoTIFF fixture.
//!
//! Each test uses its own temp dir (`line!()` suffix) so parallel tests never
//! delete each other's fixtures.

#[macro_use]
mod common;

use actix_web::test;

/// Generate an 8x8 GeoTIFF fixture with georeferencing tags:
/// - ModelPixelScaleTag (33550) = [1.0, 1.0, 0.0]
/// - ModelTiepointTag (33922)   = [0,0,0, 0,8,0]  (top-left model coord (0, 8))
/// -> bounds = (minx 0, miny 0, maxx 8, maxy 8)
fn create_georef_tiff_fixture(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
    use tiff::encoder::*;
    use tiff::tags::Tag;

    let path = dir.join(name);
    let file = std::fs::File::create(&path).unwrap();
    let mut tiff = TiffEncoder::new(file).unwrap();
    let mut image_enc = tiff.new_image::<colortype::RGB8>(8, 8).unwrap();
    let pixel_scale: &[f64] = &[1.0, 1.0, 0.0];
    let tiepoint: &[f64] = &[0.0, 0.0, 0.0, 0.0, 8.0, 0.0];
    image_enc
        .encoder()
        .write_tag(Tag::Unknown(33550), pixel_scale)
        .unwrap();
    image_enc
        .encoder()
        .write_tag(Tag::Unknown(33922), tiepoint)
        .unwrap();
    let mut data = Vec::with_capacity(8 * 8 * 3);
    for _y in 0..8 {
        for x in 0..8 {
            let v = (x * 30) as u8;
            data.extend_from_slice(&[v, 100, 200]);
        }
    }
    image_enc.write_data(&data).unwrap();
    path
}

/// Register a GeoTIFF data source over REST. Macro so it expands at the call
/// site where the concrete app type is known (same pattern as
/// `common::login_admin_token!`).
macro_rules! create_geotiff_ds {
    ($app:expr, $dir:expr) => {{
        let tif_path = create_georef_tiff_fixture(&$dir, "cov_geo.tif");
        let create = test::TestRequest::post()
            .uri("/geoserver/data-sources")
            .set_json(&serde_json::json!({
                "name": "cov_geo",
                "type": "geotiff",
                "workspace": "default",
                "enabled": true,
                "connection": { "file_path": tif_path.to_string_lossy(), "file_storage_type": "local" },
            }))
            .to_request();
        let resp = test::call_service(&$app, create).await;
        assert_eq!(
            resp.status(),
            actix_web::http::StatusCode::CREATED,
            "creating GeoTIFF data source should return 201, got: {}",
            resp.status()
        );
    }};
}

/// Unique temp dir per test (line!() differs per call site).
macro_rules! unique_temp_dir {
    () => {{
        std::env::temp_dir().join(format!(
            "terrane-ogc-coverages-{}-{}",
            std::process::id(),
            line!()
        ))
    }};
}

#[actix_rt::test]
async fn test_ogc_coverages_landing() {
    let app = build_test_app!();

    let req = test::TestRequest::get().uri("/ogc/coverages").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "landing should return 200, got: {}",
        resp.status()
    );
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["title"], "Terrane");
    let rels: Vec<&str> = body["links"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["rel"].as_str().unwrap())
        .collect();
    assert!(rels.contains(&"self"));
    assert!(rels.contains(&"conformance"));
    assert!(rels.contains(&"data"));
}

#[actix_rt::test]
async fn test_ogc_coverages_conformance() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/ogc/coverages/conformance")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: serde_json::Value = test::read_body_json(resp).await;
    let conforms = body["conformsTo"].as_array().unwrap();
    assert!(conforms
        .iter()
        .any(|x| x.as_str().unwrap().contains("core")));
    assert!(conforms
        .iter()
        .any(|x| x.as_str().unwrap().contains("coverage")));
    assert!(conforms
        .iter()
        .any(|x| x.as_str().unwrap().contains("collections")));
}

#[actix_rt::test]
async fn test_ogc_coverages_collections_empty() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/ogc/coverages/collections")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["collections"].as_array().unwrap().len(), 0);
}

#[actix_rt::test]
async fn test_ogc_coverages_collections_with_raster() {
    let app = build_test_app!();
    let dir = unique_temp_dir!();
    std::fs::create_dir_all(&dir).unwrap();
    create_geotiff_ds!(&app, dir);

    let req = test::TestRequest::get()
        .uri("/ogc/coverages/collections")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = test::read_body_json(resp).await;
    let colls = body["collections"].as_array().unwrap();
    assert_eq!(colls.len(), 1);
    assert_eq!(colls[0]["id"], "cov_geo");
    // each collection should carry a coverage link
    let rels: Vec<&str> = colls[0]["links"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["rel"].as_str().unwrap())
        .collect();
    assert!(rels.contains(&"coverage"));

    std::fs::remove_dir_all(&dir).ok();
}

#[actix_rt::test]
async fn test_ogc_coverages_collection_detail() {
    let app = build_test_app!();
    let dir = unique_temp_dir!();
    std::fs::create_dir_all(&dir).unwrap();
    create_geotiff_ds!(&app, dir);

    let req = test::TestRequest::get()
        .uri("/ogc/coverages/collections/cov_geo")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["id"], "cov_geo");
    // real GeoTIFF metadata: 8x8 size
    let scale = body["dimensions"]["spatial"]["grid"]["transform"]["scale"]
        .as_array()
        .unwrap();
    assert_eq!(scale[0], 8.0);
    assert_eq!(scale[1], 8.0);
    // CRS84 spatial extent (0,0 -> 8,8)
    let bbox = body["extent"]["spatial"]["bbox"][0].as_array().unwrap();
    assert_eq!(bbox[0], 0.0);
    assert_eq!(bbox[1], 0.0);
    assert_eq!(bbox[2], 8.0);
    assert_eq!(bbox[3], 8.0);
    // band range field
    let fields = body["ranges"]["fields"].as_array().unwrap();
    assert!(fields.len() >= 1);

    // unknown collection -> 404
    let req = test::TestRequest::get()
        .uri("/ogc/coverages/collections/bogus")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::NOT_FOUND);

    std::fs::remove_dir_all(&dir).ok();
}

#[actix_rt::test]
async fn test_ogc_coverages_coverage_tiff_default() {
    let app = build_test_app!();
    let dir = unique_temp_dir!();
    std::fs::create_dir_all(&dir).unwrap();
    create_geotiff_ds!(&app, dir);

    // default format -> GeoTIFF, 8x8 (real raster)
    let req = test::TestRequest::get()
        .uri("/ogc/coverages/collections/cov_geo/coverage")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "coverage operation should return 200, got: {}",
        resp.status()
    );
    let content_type = resp
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(
        content_type.contains("image/tiff"),
        "Content-Type should be image/tiff, got: {}",
        content_type
    );
    let body = test::read_body(resp).await;
    let decoded = image::load_from_memory(&body).expect("should decode the returned TIFF");
    assert_eq!(decoded.width(), 8);
    assert_eq!(decoded.height(), 8);

    std::fs::remove_dir_all(&dir).ok();
}

#[actix_rt::test]
async fn test_ogc_coverages_coverage_png_jpeg() {
    let app = build_test_app!();
    let dir = unique_temp_dir!();
    std::fs::create_dir_all(&dir).unwrap();
    create_geotiff_ds!(&app, dir);

    for (f, mime) in [("image/png", "image/png"), ("image/jpeg", "image/jpeg")] {
        let req = test::TestRequest::get()
            .uri(&format!(
                "/ogc/coverages/collections/cov_geo/coverage?f={}",
                f
            ))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(
            resp.status().is_success(),
            "coverage ?f={} should succeed, got: {}",
            f,
            resp.status()
        );
        let content_type = resp
            .headers()
            .get("Content-Type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        assert!(
            content_type.contains(mime),
            "Content-Type should be {}, got: {}",
            mime,
            content_type
        );
        let body = test::read_body(resp).await;
        let decoded = image::load_from_memory(&body).expect("should decode the returned image");
        assert_eq!(decoded.width(), 8);
    }

    std::fs::remove_dir_all(&dir).ok();
}

#[actix_rt::test]
async fn test_ogc_coverages_coverage_bbox_crop() {
    let app = build_test_app!();
    let dir = unique_temp_dir!();
    std::fs::create_dir_all(&dir).unwrap();
    create_geotiff_ds!(&app, dir);

    // bbox=0,0,2,2 (extent 0,0 -> 8,8) -> cropped to 2x2
    let req = test::TestRequest::get()
        .uri("/ogc/coverages/collections/cov_geo/coverage?bbox=0,0,2,2&f=image/png")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "coverage + bbox should return 200, got: {}",
        resp.status()
    );
    let body = test::read_body(resp).await;
    let decoded = image::load_from_memory(&body).expect("should decode the PNG");
    assert_eq!(decoded.width(), 2, "bbox crop should output 2x2");
    assert_eq!(decoded.height(), 2);

    // bbox + width/height -> resample
    let req = test::TestRequest::get()
        .uri(
            "/ogc/coverages/collections/cov_geo/coverage?bbox=0,0,2,2&width=8&height=8&f=image/png",
        )
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let body = test::read_body(resp).await;
    let decoded = image::load_from_memory(&body).expect("should decode the PNG");
    assert_eq!(
        decoded.width(),
        8,
        "bbox + width/height should resample to 8x8"
    );
    assert_eq!(decoded.height(), 8);

    // disjoint bbox -> 400
    let req = test::TestRequest::get()
        .uri("/ogc/coverages/collections/cov_geo/coverage?bbox=100,100,200,200&f=image/png")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::BAD_REQUEST);

    std::fs::remove_dir_all(&dir).ok();
}

#[actix_rt::test]
async fn test_ogc_coverages_errors() {
    let app = build_test_app!();

    // unknown collection coverage operation -> 404
    let req = test::TestRequest::get()
        .uri("/ogc/coverages/collections/bogus/coverage")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::NOT_FOUND);
}
