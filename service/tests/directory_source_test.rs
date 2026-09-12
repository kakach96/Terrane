//! Directory-level file data source integration tests.
//!
//! Covers: directory-scoped GeoJSON data sources publishing one layer per file
//! (`native_name` = file name inside the directory) — tables listing, per-layer
//! feature separation, and native_name validation.

#[macro_use]
mod common;

use actix_web::test;

/// Write two small GeoJSON FeatureCollections into a unique temp directory and return it.
/// Each test gets its own directory (tests run concurrently in one process).
fn make_geojson_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("terrane-dir-src-{}-{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let points = serde_json::json!({
        "type": "FeatureCollection",
        "features": [
            { "type": "Feature", "id": "p1", "properties": { "name": "Alpha" },
              "geometry": { "type": "Point", "coordinates": [10.0, 20.0] } },
            { "type": "Feature", "id": "p2", "properties": { "name": "Beta" },
              "geometry": { "type": "Point", "coordinates": [30.0, 40.0] } }
        ]
    });
    let lines = serde_json::json!({
        "type": "FeatureCollection",
        "features": [
            { "type": "Feature", "id": "r1", "properties": { "route": "R1" },
              "geometry": { "type": "LineString",
                            "coordinates": [[0.0, 0.0], [1.0, 1.0], [2.0, 0.5]] } }
        ]
    });
    std::fs::write(dir.join("sample_points.geojson"), points.to_string()).unwrap();
    std::fs::write(dir.join("sample_routes.geojson"), lines.to_string()).unwrap();
    dir
}

/// Create the directory-level GeoJSON data source pointing at `dir`.
#[allow(unused_macros)]
macro_rules! create_geojson_directory_source {
    ($app:expr, $dir:expr) => {{
        let req = test::TestRequest::post()
            .uri("/terrane/data-sources")
            .set_json(serde_json::json!({
                "name": "geojson_dir_src",
                "type": "geojson",
                "workspace": "default",
                "enabled": true,
                "connection": {
                    "file_path": $dir.to_string_lossy().replace('\\', "/"),
                    "file_storage_type": "local",
                },
            }))
            .to_request();
        let resp = test::call_service(&$app, req).await;
        assert!(
            resp.status().is_success(),
            "创建目录级 GeoJSON 数据源应成功: {}",
            resp.status()
        );
    }};
}

/// 目录级数据源的 tables 接口列出目录内可发布文件 (按类型扩展名过滤)。
#[actix_rt::test]
async fn test_directory_tables_listing() {
    let app = build_test_app!();
    let dir = make_geojson_dir("tables");
    create_geojson_directory_source!(app, dir);

    let req = test::TestRequest::get()
        .uri("/terrane/data-sources/geojson_dir_src/tables")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "tables 接口应返回 200");
    let body: serde_json::Value = test::read_body_json(resp).await;
    let tables = body["data"].as_array().cloned().unwrap_or_default();
    let names: Vec<String> = tables
        .iter()
        .filter_map(|t| t.as_str().map(|s| s.to_string()))
        .collect();
    assert!(
        names.contains(&"sample_points.geojson".to_string()),
        "应包含 sample_points.geojson, 实际: {:?}",
        names
    );
    assert!(
        names.contains(&"sample_routes.geojson".to_string()),
        "应包含 sample_routes.geojson, 实际: {:?}",
        names
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// 目录级数据源按文件发布两个图层, 要素互不串扰; native_name 无效时报 400。
#[actix_rt::test]
async fn test_directory_layers_publishing() {
    let app = build_test_app!();
    let dir = make_geojson_dir("layers");
    create_geojson_directory_source!(app, dir);

    // 图层 1: sample_points.geojson
    let create = test::TestRequest::post()
        .uri("/terrane/layers")
        .set_json(serde_json::json!({
            "name": "dir_points",
            "title": "Dir Points",
            "workspace": "default",
            "store": "geojson_dir_src",
            "native_name": "sample_points.geojson",
            "srs": "EPSG:4326",
        }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert!(
        resp.status().is_success(),
        "图层 dir_points 创建应成功: {}",
        resp.status()
    );

    // 图层 2: sample_routes.geojson
    let create = test::TestRequest::post()
        .uri("/terrane/layers")
        .set_json(serde_json::json!({
            "name": "dir_routes",
            "title": "Dir Routes",
            "workspace": "default",
            "store": "geojson_dir_src",
            "native_name": "sample_routes.geojson",
            "srs": "EPSG:4326",
        }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert!(
        resp.status().is_success(),
        "图层 dir_routes 创建应成功: {}",
        resp.status()
    );

    // 各自的要素查询应只返回自己文件的内容 (响应体为裸 FeatureCollection)
    let req = test::TestRequest::get()
        .uri("/terrane/layers/dir_points/features")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let status = resp.status();
    let err_body: serde_json::Value = test::read_body_json(resp).await;
    assert!(
        status.is_success(),
        "dir_points 要素查询应成功: {}, body: {}",
        status,
        err_body
    );
    let names: Vec<String> = err_body["features"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|f| f["properties"]["name"].as_str().unwrap_or("").to_string())
        .collect();
    assert!(names.contains(&"Alpha".to_string()), "应包含 Alpha");
    assert_eq!(names.len(), 2, "dir_points 应有 2 个要素");

    let req = test::TestRequest::get()
        .uri("/terrane/layers/dir_routes/features")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "dir_routes 要素查询应成功: {}",
        resp.status()
    );
    let body: serde_json::Value = test::read_body_json(resp).await;
    let features = body["features"].as_array().cloned().unwrap_or_default();
    assert_eq!(features.len(), 1, "dir_routes 应有 1 个要素");
    assert_eq!(
        features[0]["properties"]["route"].as_str().unwrap_or(""),
        "R1",
        "dir_routes 的要素应来自 sample_routes.geojson"
    );

    // native_name 指向目录内不存在的文件 → 创建被拒绝 (400)
    let create = test::TestRequest::post()
        .uri("/terrane/layers")
        .set_json(serde_json::json!({
            "name": "dir_bad",
            "title": "Dir Bad",
            "workspace": "default",
            "store": "geojson_dir_src",
            "native_name": "missing.geojson",
            "srs": "EPSG:4326",
        }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert_eq!(
        resp.status(),
        actix_web::http::StatusCode::BAD_REQUEST,
        "目录内不存在的 native_name 应返回 400"
    );

    // 目录级数据源缺少 native_name → 创建被拒绝 (400)
    let create = test::TestRequest::post()
        .uri("/terrane/layers")
        .set_json(serde_json::json!({
            "name": "dir_noname",
            "title": "Dir NoName",
            "workspace": "default",
            "store": "geojson_dir_src",
            "srs": "EPSG:4326",
        }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert_eq!(
        resp.status(),
        actix_web::http::StatusCode::BAD_REQUEST,
        "目录级数据源缺少 native_name 应返回 400"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// 单文件 (旧模型) GeoJSON 数据源不受影响: 忽略 native_name, 仍读 file_path 文件。
#[actix_rt::test]
async fn test_single_file_source_regression() {
    let app = build_test_app!();
    let dir = make_geojson_dir("single");
    let file_path = dir.join("sample_points.geojson");

    let create = test::TestRequest::post()
        .uri("/terrane/data-sources")
        .set_json(serde_json::json!({
            "name": "geojson_single_src",
            "type": "geojson",
            "workspace": "default",
            "enabled": true,
            "connection": {
                "file_path": file_path.to_string_lossy().replace('\\', "/"),
                "file_storage_type": "local",
            },
        }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert!(resp.status().is_success(), "单文件数据源创建应成功");

    // 单文件数据源无文件级联: tables 返回空列表 (而非报错), 供前端隐藏级联。
    let req = test::TestRequest::get()
        .uri("/terrane/data-sources/geojson_single_src/tables")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "单文件数据源 tables 应返回 200");
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(
        body["data"].as_array().map(|a| a.len()).unwrap_or(1),
        0,
        "单文件数据源 tables 应为空列表"
    );

    let create = test::TestRequest::post()
        .uri("/terrane/layers")
        .set_json(serde_json::json!({
            "name": "single_points",
            "title": "Single Points",
            "workspace": "default",
            "store": "geojson_single_src",
            "srs": "EPSG:4326",
        }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert!(
        resp.status().is_success(),
        "单文件数据源无 native_name 建层应成功: {}",
        resp.status()
    );

    let req = test::TestRequest::get()
        .uri("/terrane/layers/single_points/features")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "单文件图层要素查询应成功: {}",
        resp.status()
    );
    let body: serde_json::Value = test::read_body_json(resp).await;
    let features = body["features"].as_array().cloned().unwrap_or_default();
    assert_eq!(features.len(), 2, "旧模型单文件图层仍应返回全部要素");

    let _ = std::fs::remove_dir_all(&dir);
}

/// 目录级 GeoPackage 数据源: tables 接口应列出目录内的 .gpkg 文件
/// (而非把目录当 .gpkg 文件读取返回空)。
#[actix_rt::test]
async fn test_directory_geopackage_tables_listing() {
    let app = build_test_app!();
    let dir = std::env::temp_dir().join(format!("terrane-dir-gpkg-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("roads.gpkg"), b"not a real gpkg").unwrap();
    std::fs::write(dir.join("buildings.gpkg"), b"not a real gpkg").unwrap();

    let create = test::TestRequest::post()
        .uri("/terrane/data-sources")
        .set_json(serde_json::json!({
            "name": "gpkg_dir_src",
            "type": "geopackage",
            "workspace": "default",
            "enabled": true,
            "connection": {
                "file_path": dir.to_string_lossy().replace('\\', "/"),
                "file_storage_type": "local",
            },
        }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert!(
        resp.status().is_success(),
        "目录级 GeoPackage 数据源创建应成功"
    );

    let req = test::TestRequest::get()
        .uri("/terrane/data-sources/gpkg_dir_src/tables")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "目录级 GeoPackage tables 应返回 200: {}",
        resp.status()
    );
    let body: serde_json::Value = test::read_body_json(resp).await;
    let names: Vec<String> = body["data"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|t| t.as_str().map(|s| s.to_string()))
        .collect();
    assert!(
        names.contains(&"roads.gpkg".to_string()),
        "应列出目录内 roads.gpkg, 实际: {:?}",
        names
    );
    assert!(
        names.contains(&"buildings.gpkg".to_string()),
        "应列出目录内 buildings.gpkg, 实际: {:?}",
        names
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// ImageMosaic 是目录语义数据源: 不要求 native_name, 目录本身即数据源。
/// (回归: 目录级校验不应误伤 mosaic / pyramid 图层发布。)
#[actix_rt::test]
async fn test_mosaic_layer_create_without_native_name() {
    let app = build_test_app!();
    let dir = std::env::temp_dir().join(format!("terrane-dir-mosaic-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let create = test::TestRequest::post()
        .uri("/terrane/data-sources")
        .set_json(serde_json::json!({
            "name": "mosaic_src",
            "type": "image_mosaic",
            "workspace": "default",
            "enabled": true,
            "connection": {
                "file_path": dir.to_string_lossy().replace('\\', "/"),
                "file_storage_type": "local",
            },
        }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert!(resp.status().is_success(), "ImageMosaic 数据源创建应成功");

    // 无 native_name 也能发布图层 (mosaic 的 file_path 本身就是目录)
    let create = test::TestRequest::post()
        .uri("/terrane/layers")
        .set_json(serde_json::json!({
            "name": "mosaic_layer",
            "title": "Mosaic Layer",
            "workspace": "default",
            "store": "mosaic_src",
            "srs": "EPSG:4326",
        }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert!(
        resp.status().is_success(),
        "ImageMosaic 图层无 native_name 创建应成功: {}",
        resp.status()
    );

    let _ = std::fs::remove_dir_all(&dir);
}
