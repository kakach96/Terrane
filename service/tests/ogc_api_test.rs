//! OGC API - Features integration tests.
//!
//! Covers the OGC API - Features Part 1 Core resources: landing page,
//! `/conformance`, `/collections`, `/collections/{id}`,
//! `/collections/{id}/items` (with `limit` / `offset` / `bbox`) and
//! `/collections/{id}/items/{featureId}`.

#[macro_use]
mod common;

use actix_web::test;

#[actix_rt::test]
async fn test_ogc_landing() {
    let app = build_test_app!();

    let req = test::TestRequest::get().uri("/ogc/features").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "landing 应返回 200, 实际: {}",
        resp.status()
    );
    let ct = resp
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(
        ct.contains("application/json"),
        "CT 应为 json, 实际: {}",
        ct
    );

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["title"], "Terrane");
    let links = body["links"].as_array().unwrap();
    let rels: Vec<&str> = links.iter().map(|l| l["rel"].as_str().unwrap()).collect();
    assert!(rels.contains(&"self"));
    assert!(rels.contains(&"conformance"));
    assert!(rels.contains(&"data"));
}

#[actix_rt::test]
async fn test_ogc_conformance() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/ogc/features/conformance")
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
        .any(|x| x.as_str().unwrap().contains("geojson")));
}

#[actix_rt::test]
async fn test_ogc_collections() {
    let app = build_test_app!();
    // 默认配置仅 world 一层
    let req = test::TestRequest::get()
        .uri("/ogc/features/collections")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: serde_json::Value = test::read_body_json(resp).await;
    let colls = body["collections"].as_array().unwrap();
    assert_eq!(colls.len(), 1);
    assert_eq!(colls[0]["id"], "world");
    assert_eq!(colls[0]["title"], "World");
    let bbox = &colls[0]["extent"]["spatial"]["bbox"][0];
    assert_eq!(bbox, &serde_json::json!([-180.0, -90.0, 180.0, 90.0]));
    let rels: Vec<&str> = colls[0]["links"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["rel"].as_str().unwrap())
        .collect();
    assert!(rels.contains(&"items"));
}

#[actix_rt::test]
async fn test_ogc_collection_single_and_not_found() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/ogc/features/collections/world")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["id"], "world");
    assert_eq!(body["links"][1]["type"], "application/geo+json");

    // 不存在的集合 → 404 JSON
    let req = test::TestRequest::get()
        .uri("/ogc/features/collections/nope")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::NOT_FOUND);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["code"], "NotFound");
}

#[actix_rt::test]
async fn test_ogc_items() {
    let app = build_test_app!();

    // world 图层空发布 (无数据源), items 返回空集合
    let req = test::TestRequest::get()
        .uri("/ogc/features/collections/world/items")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let ct = resp
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(
        ct.contains("geo+json"),
        "items CT 应为 geo+json, 实际: {}",
        ct
    );

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["type"], "FeatureCollection");
    assert_eq!(body["numberMatched"], 0);
    assert_eq!(body["numberReturned"], 0);
    assert_eq!(body["features"].as_array().unwrap().len(), 0);
}

#[actix_rt::test]
async fn test_ogc_items_limit_offset() {
    let app = build_test_app!();

    // world 图层空发布, limit/offset 下仍返回空集合
    let req = test::TestRequest::get()
        .uri("/ogc/features/collections/world/items?limit=2")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["numberMatched"], 0);
    assert_eq!(body["numberReturned"], 0);
    assert_eq!(body["features"].as_array().unwrap().len(), 0);

    let req = test::TestRequest::get()
        .uri("/ogc/features/collections/world/items?limit=1&offset=2")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["numberMatched"], 0);
    assert_eq!(body["numberReturned"], 0);
}

#[actix_rt::test]
async fn test_ogc_items_bbox() {
    let app = build_test_app!();

    // world 图层空发布, bbox 过滤返回空
    let req = test::TestRequest::get()
        .uri("/ogc/features/collections/world/items?bbox=0,0,10,10")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["numberMatched"], 0);
    assert_eq!(body["features"].as_array().unwrap().len(), 0);
}

#[actix_rt::test]
async fn test_ogc_item_by_id() {
    let app = build_test_app!();

    // world 图层空发布, 按 id 查询返回 404
    let req = test::TestRequest::get()
        .uri("/ogc/features/collections/world/items/does-not-exist")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::NOT_FOUND);
}
