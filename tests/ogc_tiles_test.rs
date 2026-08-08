//! OGC API - Tiles integration tests.
//!
//! Covers the OGC API - Tiles surface: landing page, /conformance,
//! /tileMatrixSets (+ per-id definitions), /collections tileset listings and
//! raster tiles at `/collections/{id}/tiles/{tileMatrixSetId}/{tileMatrix}/{tileRow}/{tileCol}`.

#[macro_use]
mod common;

use actix_web::test;

#[actix_rt::test]
async fn test_ogc_tiles_landing() {
    let app = build_test_app!();

    let req = test::TestRequest::get().uri("/ogc/tiles").to_request();
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
async fn test_ogc_tiles_conformance() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/ogc/tiles/conformance")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: serde_json::Value = test::read_body_json(resp).await;
    let conforms = body["conformsTo"].as_array().unwrap();
    assert!(conforms.iter().any(|x| x.as_str().unwrap().contains("core")));
    assert!(conforms.iter().any(|x| x.as_str().unwrap().contains("tileset")));
    assert!(conforms.iter().any(|x| x.as_str().unwrap().contains("tilematrixset")));
}

#[actix_rt::test]
async fn test_ogc_tiles_tile_matrix_sets() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/ogc/tiles/tileMatrixSets")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: serde_json::Value = test::read_body_json(resp).await;
    let sets = body["tileMatrixSets"].as_array().unwrap();
    assert_eq!(sets.len(), 2);
    assert_eq!(sets[0]["id"], "EPSG:4326");
    assert_eq!(sets[1]["id"], "EPSG:3857");
}

#[actix_rt::test]
async fn test_ogc_tiles_tile_matrix_set() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/ogc/tiles/tileMatrixSets/EPSG:4326")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["id"], "EPSG:4326");
    assert_eq!(body["crs"], "http://www.opengis.net/def/crs/OGC/1.3/CRS84");
    let matrices = body["tileMatrices"].as_array().unwrap();
    assert_eq!(matrices[0]["id"], "0");
    assert_eq!(matrices[0]["matrixWidth"], 2);
    assert_eq!(matrices[0]["matrixHeight"], 1);
    assert_eq!(matrices[0]["cellSize"], 0.703125);
    assert_eq!(matrices[0]["pointOfOrigin"][0], -180.0);

    // 未知 tms → 404
    let req = test::TestRequest::get()
        .uri("/ogc/tiles/tileMatrixSets/bogus")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::NOT_FOUND);
}

#[actix_rt::test]
async fn test_ogc_tiles_collections_and_tilesets() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/ogc/tiles/collections")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = test::read_body_json(resp).await;
    let colls = body["collections"].as_array().unwrap();
    assert_eq!(colls.len(), 1);
    assert_eq!(colls[0]["id"], "world");

    // 单图层 tilesets
    let req = test::TestRequest::get()
        .uri("/ogc/tiles/collections/world/tiles")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = test::read_body_json(resp).await;
    let tilesets = body["tilesets"].as_array().unwrap();
    assert_eq!(tilesets.len(), 2);
    assert_eq!(
        tilesets[0]["tileMatrixSetURI"],
        "http://www.opengis.net/def/tilematrixset/OGC/1.0/global-geodetic"
    );
    assert_eq!(tilesets[1]["crs"], "http://www.opengis.net/def/crs/EPSG/0/3857");

    // 未知图层 → 404
    let req = test::TestRequest::get()
        .uri("/ogc/tiles/collections/nope/tiles")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::NOT_FOUND);
}

#[actix_rt::test]
async fn test_ogc_tile_png_and_jpeg() {
    let app = build_test_app!();

    // PNG tile (default)
    let req = test::TestRequest::get()
        .uri("/ogc/tiles/collections/world/tiles/EPSG:4326/0/0/0")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "PNG tile 应返回 200, 实际: {}",
        resp.status()
    );
    let ct = resp
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(ct.contains("image/png"), "CT 应为 image/png, 实际: {}", ct);
    let body = test::read_body(resp).await;
    let decoded = image::load_from_memory(&body).expect("应能解码 PNG 瓦片");
    assert_eq!(decoded.width(), 256);
    assert_eq!(decoded.height(), 256);

    // JPEG tile via f=image/jpeg
    let req = test::TestRequest::get()
        .uri("/ogc/tiles/collections/world/tiles/EPSG:3857/0/0/0?f=image/jpeg")
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
        ct.contains("image/jpeg"),
        "CT 应为 image/jpeg, 实际: {}",
        ct
    );
    let body = test::read_body(resp).await;
    let decoded = image::load_from_memory(&body).expect("应能解码 JPEG 瓦片");
    assert_eq!(decoded.width(), 256);
}

#[actix_rt::test]
async fn test_ogc_tile_not_found() {
    let app = build_test_app!();

    // 未知图层 → 404
    let req = test::TestRequest::get()
        .uri("/ogc/tiles/collections/nope/tiles/EPSG:4326/0/0/0")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::NOT_FOUND);

    // 越界 zoom (z=99) → 404
    let req = test::TestRequest::get()
        .uri("/ogc/tiles/collections/world/tiles/EPSG:4326/99/0/0")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::NOT_FOUND);

    // 越界 col → 404
    let req = test::TestRequest::get()
        .uri("/ogc/tiles/collections/world/tiles/EPSG:4326/0/0/5")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::NOT_FOUND);
}
