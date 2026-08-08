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
