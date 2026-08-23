//! OGC API - Maps integration tests.
//!
//! Covers the OGC API - Maps surface (OGC 20-058): landing page,
//! /conformance, /collections, /collections/{id}, /collections/{id}/styles
//! and the `map` operation at /collections/{id}/map (PNG / JPEG), which
//! reuses the shared WMS GetMap pipeline.

#[macro_use]
mod common;

use actix_web::test;

#[actix_rt::test]
async fn test_ogc_maps_landing() {
    let app = build_test_app!();

    let req = test::TestRequest::get().uri("/ogc/maps").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "landing 应返回 200, 实际: {}",
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
async fn test_ogc_maps_conformance() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/ogc/maps/conformance")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: serde_json::Value = test::read_body_json(resp).await;
    let conforms = body["conformsTo"].as_array().unwrap();
    assert!(conforms
        .iter()
        .any(|x| x.as_str().unwrap().contains("core")));
    assert!(conforms.iter().any(|x| x.as_str().unwrap().contains("map")));
    assert!(conforms
        .iter()
        .any(|x| x.as_str().unwrap().contains("collections")));
}

#[actix_rt::test]
async fn test_ogc_maps_collections() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/ogc/maps/collections")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = test::read_body_json(resp).await;
    let colls = body["collections"].as_array().unwrap();
    assert_eq!(colls.len(), 1);
    assert_eq!(colls[0]["id"], "world");
    // 每个 collection 应带 map 链接
    let rels: Vec<&str> = colls[0]["links"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["rel"].as_str().unwrap())
        .collect();
    assert!(rels.contains(&"map"));
}

#[actix_rt::test]
async fn test_ogc_maps_collection() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/ogc/maps/collections/world")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["id"], "world");
    assert_eq!(body["title"], "World");
    let rels: Vec<&str> = body["links"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["rel"].as_str().unwrap())
        .collect();
    assert!(rels.contains(&"map"));
    assert!(rels.contains(&"styles"));
    // CRS84 空间范围
    let bbox = body["extent"]["spatial"]["bbox"][0].as_array().unwrap();
    assert_eq!(bbox[0], -180.0);

    // 未知 collection → 404
    let req = test::TestRequest::get()
        .uri("/ogc/maps/collections/bogus")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::NOT_FOUND);
}

#[actix_rt::test]
async fn test_ogc_maps_styles() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/ogc/maps/collections/world/styles")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body["styles"].is_array());
    assert_eq!(body["links"][0]["rel"], "self");

    // 未知 collection → 404
    let req = test::TestRequest::get()
        .uri("/ogc/maps/collections/bogus/styles")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::NOT_FOUND);
}

#[actix_rt::test]
async fn test_ogc_maps_map_png() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri(
            "/ogc/maps/collections/world/map?bbox=-180,-90,180,90&width=256&height=256&f=image/png",
        )
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "map 操作应返回 200, 实际: {}",
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
    let body = test::read_body(resp).await;
    let img = image::load_from_memory(&body).expect("应能解码 OGC API Maps 返回的 PNG");
    assert_eq!(img.width(), 256);
    assert_eq!(img.height(), 256);
}

#[actix_rt::test]
async fn test_ogc_maps_map_jpeg() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/ogc/maps/collections/world/map?bbox=-180,-90,180,90&width=256&height=256&f=image/jpeg")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let content_type = resp
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(
        content_type.contains("image/jpeg"),
        "Content-Type 应为 image/jpeg, 实际: {}",
        content_type
    );
}

#[actix_rt::test]
async fn test_ogc_maps_map_errors() {
    let app = build_test_app!();

    // 缺 bbox → 400
    let req = test::TestRequest::get()
        .uri("/ogc/maps/collections/world/map?width=256&height=256")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::BAD_REQUEST);

    // 未知 collection → 404
    let req = test::TestRequest::get()
        .uri("/ogc/maps/collections/bogus/map?bbox=-180,-90,180,90&width=256&height=256")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::NOT_FOUND);
}
