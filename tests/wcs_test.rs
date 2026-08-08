//! WCS (Web Coverage Service) integration tests.
//!
//! Covers: GetCapabilities, DescribeCoverage, GetCoverage (TIFF / PNG, SUBSET +
//! SIZE, default-format fallback).

#[macro_use]
mod common;

use actix_web::test;

#[actix_rt::test]
async fn test_wcs_get_capabilities() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/wcs?SERVICE=WCS&REQUEST=GetCapabilities")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "WCS GetCapabilities 应返回 200, 实际: {}", resp.status());
}

#[actix_rt::test]
async fn test_wcs_describe_coverage() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/wcs?SERVICE=WCS&REQUEST=DescribeCoverage&COVERAGEID=world&VERSION=2.0.1")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let status = resp.status();
    let content_type = resp.headers()
        .get("Content-Type").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    let body = test::read_body(resp).await;
    assert!(status.is_success(),
        "WCS DescribeCoverage 应返回 200, 实际: {}, content-type: {}, body: {}",
        status, content_type, String::from_utf8_lossy(&body));

    assert!(content_type.contains("xml"), "Content-Type 应为 XML, 实际: {}", content_type);
    let xml = String::from_utf8_lossy(&body);
    assert!(xml.contains("world"), "应包含 world 覆盖描述");
}

#[actix_rt::test]
async fn test_wcs_get_coverage_tiff() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/wcs?SERVICE=WCS&REQUEST=GetCoverage&COVERAGEID=world&FORMAT=image/tiff&VERSION=2.0.1")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "WCS GetCoverage 应返回 200, 实际: {}", resp.status());

    let content_type = resp.headers()
        .get("Content-Type").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    assert!(content_type.contains("image/tiff"), "Content-Type 应为 image/tiff, 实际: {}", content_type);
}

#[actix_rt::test]
async fn test_wcs_get_coverage_png() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/wcs?SERVICE=WCS&REQUEST=GetCoverage&COVERAGEID=world&FORMAT=image/png&VERSION=2.0.1")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "WCS GetCoverage (PNG) 应返回 200, 实际: {}", resp.status());

    let content_type = resp.headers()
        .get("Content-Type").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    assert!(content_type.contains("image/png"), "Content-Type 应为 image/png, 实际: {}", content_type);
}

#[actix_rt::test]
async fn test_wcs_get_coverage_default_format() {
    let app = build_test_app!();

    // 未指定 FORMAT → 默认输出 TIFF
    let req = test::TestRequest::get()
        .uri("/wcs?SERVICE=WCS&REQUEST=GetCoverage&COVERAGEID=world&VERSION=2.0.1")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "WCS GetCoverage (默认格式) 应返回 200, 实际: {}", resp.status());

    let content_type = resp.headers()
        .get("Content-Type").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    assert!(content_type.contains("image/tiff"), "默认 Content-Type 应为 image/tiff, 实际: {}", content_type);
}

#[actix_rt::test]
async fn test_wcs_get_coverage_subset() {
    let app = build_test_app!();

    // WCS 2.0 SUBSET 子集 + SIZE 参数
    let req = test::TestRequest::get()
        .uri("/wcs?SERVICE=WCS&REQUEST=GetCoverage&COVERAGEID=world&FORMAT=image/tiff&VERSION=2.0.1&SUBSET=x(10,20)&SUBSET=y(10,20)&SIZE=64,64")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "WCS GetCoverage + SUBSET 应返回 200, 实际: {}", resp.status());

    let content_type = resp.headers()
        .get("Content-Type").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    assert!(content_type.contains("image/tiff"), "Content-Type 应为 image/tiff, 实际: {}", content_type);
}

// ---------------------------------------------------------------------------
// Batch 4: 真实 GeoTIFF 数据源 (非 fallback 渐变图)
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn test_wcs_get_coverage_real_geotiff() {
    use actix_web::http::StatusCode;

    let app = build_test_app!();

    // 1. 生成 8x8 小 GeoTIFF fixture
    let img = image::RgbaImage::from_pixel(8, 8, image::Rgba([200, 100, 50, 255]));
    let mut buf = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Tiff).unwrap();
    let dir = std::env::temp_dir().join(format!("terrane-wcs-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let tif_path = dir.join("cov1.tif");
    std::fs::write(&tif_path, &buf).unwrap();

    // 2. 创建 geotiff 数据源, connection.file_path 指向 fixture
    let create = test::TestRequest::post()
        .uri("/geoserver/data-sources")
        .set_json(&serde_json::json!({
            "name": "cov1",
            "type": "geotiff",
            "workspace": "default",
            "enabled": true,
            "connection": { "file_path": tif_path.to_string_lossy(), "file_storage_type": "local" },
        }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert_eq!(resp.status(), StatusCode::CREATED, "创建 GeoTIFF 数据源应返回 201, 实际: {}", resp.status());

    // 3. DescribeCoverage → 应返回真实 GeoTIFF 元数据 (8x8 尺寸, 来自 fixture)
    let req = test::TestRequest::get()
        .uri("/wcs?SERVICE=WCS&VERSION=2.0.1&REQUEST=DescribeCoverage&COVERAGEID=cov1")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "WCS DescribeCoverage 应返回 200, 实际: {}", resp.status());
    let body = test::read_body(resp).await;
    let xml = String::from_utf8_lossy(&body);
    assert!(xml.contains("cov1"), "DescribeCoverage 应包含覆盖 id, 实际: {}", xml);
    assert!(xml.contains("8x8"), "DescribeCoverage 应包含真实 GeoTIFF 尺寸 8x8, 实际: {}", xml);

    // 4. GetCoverage → 读取真实 8x8 数据 (fallback 是 512x512)
    let req = test::TestRequest::get()
        .uri("/wcs?SERVICE=WCS&VERSION=2.0.1&REQUEST=GetCoverage&COVERAGEID=cov1&FORMAT=image/tiff")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "WCS GetCoverage (真实 GeoTIFF) 应返回 200, 实际: {}", resp.status());

    let content_type = resp.headers()
        .get("Content-Type").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    assert!(content_type.contains("tiff"), "Content-Type 应为 image/tiff, 实际: {}", content_type);

    let body = test::read_body(resp).await;
    let decoded = image::load_from_memory(&body).expect("应能解码返回的 TIFF");
    assert_eq!(decoded.width(), 8, "应返回真实 GeoTIFF 尺寸 8x8");
    assert_eq!(decoded.height(), 8);

    // 清理 fixture
    std::fs::remove_dir_all(&dir).ok();
}
