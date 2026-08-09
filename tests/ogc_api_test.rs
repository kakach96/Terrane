//! OGC API - Features integration tests.
//!
//! Covers the OGC API - Features Part 1 Core resources: landing page,
//! `/conformance`, `/collections`, `/collections/{id}`,
//! `/collections/{id}/items` (with `limit` / `offset` / `bbox`) and
//! `/collections/{id}/items/{featureId}`.

#[macro_use]
mod common;

use actix_web::test;

/// 通过 REST 向 world 图层创建一条几何要素 (点 + 属性)
macro_rules! create_feature {
    ($app:expr, $geom:tt, $props:tt) => {{
        let req = test::TestRequest::post()
            .uri("/geoserver/layers/world/features")
            .set_json(&serde_json::json!({
                "geometry": $geom,
                "properties": $props,
            }))
            .to_request();
        let resp = test::call_service(&$app, req).await;
        assert_eq!(
            resp.status(),
            actix_web::http::StatusCode::CREATED,
            "创建要素应返回 201, 实际: {}",
            resp.status()
        );
        let body: serde_json::Value = test::read_body_json(resp).await;
        body["id"].as_str().expect("应返回要素 id").to_string()
    }};
}

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
    create_feature!(
        app,
        { "type": "Point", "coordinates": [10.0, 20.0] },
        { "name": "alpha" }
    );

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
    assert_eq!(body["numberMatched"], 1);
    assert_eq!(body["numberReturned"], 1);
    let feats = body["features"].as_array().unwrap();
    assert_eq!(feats.len(), 1);
    assert_eq!(feats[0]["type"], "Feature");
    assert_eq!(feats[0]["properties"]["name"], "alpha");
    assert_eq!(feats[0]["geometry"]["coordinates"][0], 10.0);
}

#[actix_rt::test]
async fn test_ogc_items_limit_offset() {
    let app = build_test_app!();
    create_feature!(
        app,
        { "type": "Point", "coordinates": [1.0, 1.0] },
        { "name": "p1" }
    );
    create_feature!(
        app,
        { "type": "Point", "coordinates": [2.0, 2.0] },
        { "name": "p2" }
    );
    create_feature!(
        app,
        { "type": "Point", "coordinates": [3.0, 3.0] },
        { "name": "p3" }
    );

    // limit=2 → 前两条 + next 链接
    let req = test::TestRequest::get()
        .uri("/ogc/features/collections/world/items?limit=2")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["numberMatched"], 3);
    assert_eq!(body["numberReturned"], 2);
    assert_eq!(body["features"].as_array().unwrap().len(), 2);
    let rels: Vec<&str> = body["links"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["rel"].as_str().unwrap())
        .collect();
    assert!(rels.contains(&"next"), "第一页应有 next 链接");

    // offset=2 limit=1 → 第 3 条, 无 next
    let req = test::TestRequest::get()
        .uri("/ogc/features/collections/world/items?limit=1&offset=2")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["numberMatched"], 3);
    assert_eq!(body["numberReturned"], 1);
    assert_eq!(
        body["features"].as_array().unwrap()[0]["properties"]["name"],
        "p3"
    );
    let rels: Vec<&str> = body["links"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["rel"].as_str().unwrap())
        .collect();
    assert!(!rels.contains(&"next"), "最后一页不应有 next");
}

#[actix_rt::test]
async fn test_ogc_items_bbox() {
    let app = build_test_app!();
    create_feature!(
        app,
        { "type": "Point", "coordinates": [5.0, 5.0] },
        { "name": "inside" }
    );
    create_feature!(
        app,
        { "type": "Point", "coordinates": [100.0, 100.0] },
        { "name": "outside" }
    );

    let req = test::TestRequest::get()
        .uri("/ogc/features/collections/world/items?bbox=0,0,10,10")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["numberMatched"], 1);
    let feats = body["features"].as_array().unwrap();
    assert_eq!(feats.len(), 1);
    assert_eq!(feats[0]["properties"]["name"], "inside");
}

#[actix_rt::test]
async fn test_ogc_item_by_id() {
    let app = build_test_app!();
    let id = create_feature!(
        app,
        { "type": "Point", "coordinates": [12.0, 34.0] },
        { "name": "single" }
    );

    let req = test::TestRequest::get()
        .uri(&format!("/ogc/features/collections/world/items/{}", id))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["type"], "Feature");
    assert_eq!(body["id"], id);
    assert_eq!(body["properties"]["name"], "single");

    // 不存在的 feature → 404
    let req = test::TestRequest::get()
        .uri("/ogc/features/collections/world/items/does-not-exist")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::NOT_FOUND);
}
