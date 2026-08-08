//! TMS (Tile Map Service) 1.0.0 integration tests.
//!
//! Covers: GetCapabilities (RESTful + KVP), TileMap documents, GetTile
//! (geodetic / mercator gridsets, PNG / JPEG), served under the GeoWebCache
//! compatible path `{api_context}/gwc/service/tms`.

#[macro_use]
mod common;

use actix_web::test;

fn content_type<B>(resp: &actix_web::dev::ServiceResponse<B>) -> String {
    resp.headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string()
}

#[actix_rt::test]
async fn test_tms_get_capabilities_restful() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/geoserver/gwc/service/tms/1.0.0")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "TMS capabilities 应返回 200, 实际: {}",
        resp.status()
    );
    assert!(
        content_type(&resp).contains("xml"),
        "Content-Type 应为 XML, 实际: {}",
        content_type(&resp)
    );

    let bytes = test::read_body(resp).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(
        body.contains("<TileMapService version=\"1.0.0\""),
        "应包含 TileMapService 根元素"
    );
    assert!(body.contains("<TileMaps>"));
    // world 图层: 2 gridsets × 2 formats
    assert!(body.contains("world@EPSG%3A4326@png"));
    assert!(body.contains("world@EPSG%3A4326@jpeg"));
    assert!(body.contains("world@EPSG%3A3857@png"));
    assert!(body.contains("world@EPSG%3A3857@jpeg"));
    assert!(body.contains("profile=\"global-geodetic\""));
    assert!(body.contains("profile=\"global-mercator\""));
}

#[actix_rt::test]
async fn test_tms_get_capabilities_kvp() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/geoserver/gwc/service/tms?REQUEST=GetCapabilities")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "TMS KVP capabilities 应返回 200, 实际: {}",
        resp.status()
    );
    let bytes = test::read_body(resp).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(body.contains("<TileMapService version=\"1.0.0\""));
    assert!(body.contains("world@EPSG%3A4326@png"));
}

#[actix_rt::test]
async fn test_tms_tile_map_document() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/geoserver/gwc/service/tms/1.0.0/world@EPSG%3A4326@png")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "TMS TileMap 文档应返回 200, 实际: {}",
        resp.status()
    );
    assert!(
        content_type(&resp).contains("xml"),
        "Content-Type 应为 XML, 实际: {}",
        content_type(&resp)
    );

    let bytes = test::read_body(resp).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(body.contains("<TileMap version=\"1.0.0\""));
    assert!(body.contains("<Title>World</Title>"));
    assert!(body.contains("<SRS>EPSG:4326</SRS>"));
    assert!(body.contains("<BoundingBox minx=\"-180\""));
    assert!(body.contains("<Origin x=\"-180\" y=\"-90\"/>"));
    assert!(body.contains("mime-type=\"image/png\" extension=\"png\""));
    assert!(body.contains("<TileSets profile=\"global-geodetic\">"));
    assert!(body.contains("units-per-pixel=\"0.703125\" order=\"0\""));
    assert!(body.contains("order=\"18\""));
}

#[actix_rt::test]
async fn test_tms_get_tile_geodetic_png() {
    let app = build_test_app!();

    // z=0, x=0, y_tms=0 (bottom row) — global-geodetic 网格 2x1.
    let req = test::TestRequest::get()
        .uri("/geoserver/gwc/service/tms/1.0.0/world@EPSG:4326@png/0/0/0.png")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "TMS geodetic 瓦片应返回 200, 实际: {}",
        resp.status()
    );
    assert_eq!(content_type(&resp), "image/png");
    let bytes = test::read_body(resp).await;
    assert_eq!(&bytes[..4], b"\x89PNG", "应返回 PNG 魔数");
}

#[actix_rt::test]
async fn test_tms_get_tile_mercator() {
    let app = build_test_app!();

    // EPSG:3857 global-mercator, z=0 (1x1).
    let req = test::TestRequest::get()
        .uri("/geoserver/gwc/service/tms/1.0.0/world@EPSG:3857@png/0/0/0.png")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "TMS mercator 瓦片应返回 200, 实际: {}",
        resp.status()
    );
    assert_eq!(content_type(&resp), "image/png");
}

#[actix_rt::test]
async fn test_tms_get_tile_jpeg() {
    let app = build_test_app!();

    // JPEG 扩展 → image/jpeg 输出.
    let req = test::TestRequest::get()
        .uri("/geoserver/gwc/service/tms/1.0.0/world@EPSG:4326@jpeg/0/0/0.jpeg")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "TMS jpeg 瓦片应返回 200, 实际: {}",
        resp.status()
    );
    assert_eq!(content_type(&resp), "image/jpeg");
    let bytes = test::read_body(resp).await;
    assert_eq!(&bytes[..2], b"\xff\xd8", "应返回 JPEG 魔数");
}

#[actix_rt::test]
async fn test_tms_kvp_get_tile() {
    let app = build_test_app!();

    // KVP GetTile: TILEROW 是 TMS 自底向上的行号.
    let req = test::TestRequest::get()
        .uri("/geoserver/gwc/service/tms?SERVICE=TMS&REQUEST=GetTile&VERSION=1.0.0&LAYER=world&FORMAT=image/png&TILEMATRIXSET=EPSG:4326&TILEMATRIX=0&TILEROW=0&TILECOL=0")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "TMS KVP 瓦片应返回 200, 实际: {}",
        resp.status()
    );
    assert_eq!(content_type(&resp), "image/png");
}
