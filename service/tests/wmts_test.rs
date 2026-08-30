//! WMTS (Web Map Tile Service) 1.0.0 integration tests.
//!
//! Covers: GetCapabilities, GetTile (KVP + RESTful template), GetFeatureInfo.

#[macro_use]
mod common;

use actix_web::test;

#[actix_rt::test]
async fn test_wmts_get_capabilities() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/wmts?SERVICE=WMTS&REQUEST=GetCapabilities")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "WMTS GetCapabilities 应返回 200, 实际: {}",
        resp.status()
    );

    let content_type = resp
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(
        content_type.contains("xml"),
        "Content-Type 应为 XML, 实际: {}",
        content_type
    );

    let body = test::read_body(resp).await;
    let xml = String::from_utf8_lossy(&body);
    assert!(xml.contains("GetTile"), "能力文档应包含 GetTile 操作");
    assert!(xml.contains("world"), "能力文档应包含 world 图层");
}

#[actix_rt::test]
async fn test_wmts_get_tile_kvp() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/wmts?SERVICE=WMTS&REQUEST=GetTile&LAYER=world&STYLE=default&TILEMATRIXSET=EPSG:4326&TILEMATRIX=EPSG:4326:0&TILEROW=0&TILECOL=0&FORMAT=image/png")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "WMTS GetTile 应返回 200, 实际: {}",
        resp.status()
    );

    let content_type = resp
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(
        content_type.contains("image/png"),
        "Content-Type 应为 image/png, 实际: {}",
        content_type
    );
}

#[actix_rt::test]
async fn test_wmts_get_tile_restful() {
    let app = build_test_app!();

    // RESTful 模板: /terrane/wmts/{layer}/{tileMatrixSet}/{tileMatrix}/{tileCol}/{tileRow}
    let req = test::TestRequest::get()
        .uri("/terrane/wmts/world/EPSG:4326/0/0/0")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "WMTS RESTful 瓦片应返回 200, 实际: {}",
        resp.status()
    );

    let content_type = resp
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(
        content_type.contains("image/png"),
        "Content-Type 应为 image/png, 实际: {}",
        content_type
    );
}

#[actix_rt::test]
async fn test_wmts_get_feature_info() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/wmts?SERVICE=WMTS&REQUEST=GetFeatureInfo&LAYER=world&STYLE=default&TILEMATRIXSET=EPSG:4326&TILEMATRIX=0&TILEROW=0&TILECOL=0&I=1&J=1&INFOFORMAT=application/json")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "WMTS GetFeatureInfo 应返回 200, 实际: {}",
        resp.status()
    );

    let content_type = resp
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(
        content_type.contains("application/json"),
        "Content-Type 应为 application/json, 实际: {}",
        content_type
    );
}
