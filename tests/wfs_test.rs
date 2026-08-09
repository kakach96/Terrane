//! WFS (Web Feature Service) integration tests.
//!
//! Covers: GetCapabilities, DescribeFeatureType, GetFeature (GeoJSON / GML /
//! CSV), GetFeatureWithLock, and Transaction (WFS-T insert + update round-trip).

#[macro_use]
mod common;

use actix_web::test;

#[actix_rt::test]
async fn test_wfs_get_capabilities() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/wfs?SERVICE=WFS&REQUEST=GetCapabilities")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "WFS GetCapabilities 应返回 200, 实际: {}",
        resp.status()
    );
}

#[actix_rt::test]
async fn test_wfs_describe_feature_type() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/wfs?SERVICE=WFS&REQUEST=DescribeFeatureType&TYPENAME=world")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "WFS DescribeFeatureType 应返回 200, 实际: {}",
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
}

#[actix_rt::test]
async fn test_wfs_get_feature_geojson() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/wfs?SERVICE=WFS&REQUEST=GetFeature&TYPENAME=world&OUTPUTFORMAT=application/json")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "WFS GetFeature (GeoJSON) 应返回 200, 实际: {}",
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

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(
        body["features"].is_array(),
        "GeoJSON 应包含 features 数组, 实际: {}",
        body
    );
}

#[actix_rt::test]
async fn test_wfs_get_feature_gml2() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/wfs?SERVICE=WFS&REQUEST=GetFeature&TYPENAME=world&OUTPUTFORMAT=text/xml;subtype=gml/2.1.2")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "WFS GetFeature (GML2) 应返回 200, 实际: {}",
        resp.status()
    );

    let content_type = resp
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(
        content_type.contains("gml"),
        "Content-Type 应为 GML, 实际: {}",
        content_type
    );
}

#[actix_rt::test]
async fn test_wfs_get_feature_gml32() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/wfs?SERVICE=WFS&REQUEST=GetFeature&TYPENAME=world&OUTPUTFORMAT=text/xml;subtype=gml/3.2")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "WFS GetFeature (GML3.2) 应返回 200, 实际: {}",
        resp.status()
    );

    let content_type = resp
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(
        content_type.contains("gml"),
        "Content-Type 应为 GML, 实际: {}",
        content_type
    );
}

#[actix_rt::test]
async fn test_wfs_get_feature_csv() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/wfs?SERVICE=WFS&REQUEST=GetFeature&TYPENAME=world&OUTPUTFORMAT=csv")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "WFS GetFeature (CSV) 应返回 200, 实际: {}",
        resp.status()
    );

    let content_type = resp
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(
        content_type.contains("text/csv"),
        "Content-Type 应为 text/csv, 实际: {}",
        content_type
    );
}

#[actix_rt::test]
async fn test_wfs_get_feature_with_lock() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/wfs?SERVICE=WFS&REQUEST=GetFeatureWithLock&TYPENAME=world&OUTPUTFORMAT=application/json")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "WFS GetFeatureWithLock 应返回 200, 实际: {}",
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

#[actix_rt::test]
async fn test_wfs_transaction_not_supported() {
    // Terrane 是只读数据发布平台: WFS-T Transaction 应返回 501 Not Implemented
    let app = build_test_app!();

    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<wfs:Transaction xmlns:wfs="http://www.opengis.net/wfs/2.0" xmlns:gml="http://www.opengis.net/gml/3.2" service="WFS" version="2.0.0">
  <wfs:Insert>
    <wfs:TypeName>world</wfs:TypeName>
    <wfs:Feature>
      <gml:Point><gml:pos>10.0 20.0</gml:pos></gml:Point>
    </wfs:Feature>
  </wfs:Insert>
</wfs:Transaction>"#;

    let req = test::TestRequest::post()
        .uri("/wfs")
        .insert_header(("Content-Type", "application/xml"))
        .set_payload(xml)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status().as_u16(),
        501,
        "WFS-T Transaction 应返回 501 Not Implemented, 实际: {}",
        resp.status()
    );
}

// ---------------------------------------------------------------------------
// Batch 3: 默认 GML 3.1.1 输出
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn test_wfs_get_feature_gml311_default() {
    let app = build_test_app!();

    // 不带 OUTPUTFORMAT → 默认 GML 3.1.1
    let req = test::TestRequest::get()
        .uri("/wfs?SERVICE=WFS&REQUEST=GetFeature&TYPENAME=world")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "WFS GetFeature (默认 GML3.1.1) 应返回 200, 实际: {}",
        resp.status()
    );

    let content_type = resp
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(
        content_type.contains("gml"),
        "Content-Type 应为 GML, 实际: {}",
        content_type
    );

    let body = test::read_body(resp).await;
    let xml = String::from_utf8_lossy(&body);
    assert!(
        xml.contains("3.1.1") || xml.contains("3.1"),
        "默认应为 GML 3.1.1, 实际: {}",
        xml
    );
}

// ---------------------------------------------------------------------------
// Batch 4: LockFeature 契约固化 (WfsOperation 中声明, 但 GET handler 未实现)
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn test_wfs_lock_feature_returns_400() {
    let app = build_test_app!();

    // 当前契约: LockFeature 已声明但未实现, GET handler 返回
    // 400 "Operation not implemented"。固化该行为以防意外回归。
    let req = test::TestRequest::get()
        .uri("/wfs?SERVICE=WFS&REQUEST=LockFeature&TYPENAME=world")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        actix_web::http::StatusCode::BAD_REQUEST,
        "LockFeature 未实现应返回 400, 实际: {}",
        resp.status()
    );
}

// ---------------------------------------------------------------------------
// Batch 14: GeoPackage 图层的 WFS DescribeFeatureType 返回真实类型化列
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn test_wfs_describe_feature_type_geopackage() {
    use std::collections::HashMap;

    // 1. 在临时目录写入带类型化属性的 GeoPackage
    let dir = std::env::temp_dir().join(format!("terrane-gpkg-wfs-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("typed.gpkg");

    let mut props1 = HashMap::new();
    props1.insert(
        "name".to_string(),
        terrane::models::PropertyValue::String("alpha".to_string()),
    );
    props1.insert(
        "count".to_string(),
        terrane::models::PropertyValue::Integer(10),
    );
    props1.insert(
        "price".to_string(),
        terrane::models::PropertyValue::Number(9.5),
    );
    props1.insert(
        "active".to_string(),
        terrane::models::PropertyValue::Boolean(true),
    );
    let mut props2 = HashMap::new();
    props2.insert(
        "name".to_string(),
        terrane::models::PropertyValue::String("beta".to_string()),
    );
    props2.insert(
        "count".to_string(),
        terrane::models::PropertyValue::Integer(20),
    );
    props2.insert(
        "price".to_string(),
        terrane::models::PropertyValue::Number(19.25),
    );
    props2.insert(
        "active".to_string(),
        terrane::models::PropertyValue::Boolean(false),
    );

    let features = vec![
        terrane::models::Feature::with_id(
            "f1".into(),
            terrane::models::GeoJsonGeometry::Point {
                coordinates: vec![1.0, 1.0],
            },
            props1,
        ),
        terrane::models::Feature::with_id(
            "f2".into(),
            terrane::models::GeoJsonGeometry::Point {
                coordinates: vec![2.0, 2.0],
            },
            props2,
        ),
    ];
    let bounds = terrane::models::Bounds::new(1.0, 1.0, 2.0, 2.0);
    terrane::utils::geopackage::write_geopackage_features(
        &path, "typed", "POINT", 4326, &features, &bounds,
    )
    .expect("应能写入 GeoPackage");

    let app = build_test_app!();

    // 2. 通过 REST 发布: geopackage 数据源 + 图层
    let create = test::TestRequest::post()
        .uri("/geoserver/data-sources")
        .set_json(&serde_json::json!({
            "name": "gpkg_ds",
            "type": "geopackage",
            "workspace": "default",
            "enabled": true,
            "connection": { "file_path": path.to_str(), "file_storage_type": "local" },
        }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert_eq!(
        resp.status(),
        actix_web::http::StatusCode::CREATED,
        "创建 GeoPackage 数据源应返回 201, 实际: {}",
        resp.status()
    );

    let create = test::TestRequest::post()
        .uri("/geoserver/layers")
        .set_json(&serde_json::json!({
            "name": "typed_layer",
            "title": "Typed",
            "workspace": "default",
            "store": "gpkg_ds",
            "native_name": "typed",
            "srs": "EPSG:4326",
            "minx": 1.0, "miny": 1.0, "maxx": 2.0, "maxy": 2.0,
        }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert_eq!(
        resp.status(),
        actix_web::http::StatusCode::CREATED,
        "创建图层应返回 201, 实际: {}",
        resp.status()
    );

    // 3. WFS DescribeFeatureType → 真实类型化列 (不再返回硬编码 id/name/geometry)
    let req = test::TestRequest::get()
        .uri("/wfs?SERVICE=WFS&REQUEST=DescribeFeatureType&TYPENAME=typed_layer")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "WFS DescribeFeatureType (GeoPackage) 应返回 200, 实际: {}",
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

    let body = actix_web::test::read_body(resp).await;
    let text = String::from_utf8_lossy(&body);
    // 真实列名
    for col in ["name", "count", "price", "active"] {
        assert!(
            text.contains(col),
            "DescribeFeatureType 应包含列 '{}', 实际 body: {}",
            col,
            text
        );
    }
    // XSD 类型映射 (INTEGER→long, REAL→double, BOOLEAN→boolean, 几何→GeometryPropertyType)
    for (col, xsd) in [
        ("count", "xsd:long"),
        ("price", "xsd:double"),
        ("active", "xsd:boolean"),
        ("geom", "gml:GeometryPropertyType"),
    ] {
        assert!(
            text.contains(xsd),
            "列 '{}' 应映射为 {}, 实际 body: {}",
            col,
            xsd,
            text
        );
    }

    // 清理
    std::fs::remove_dir_all(&dir).ok();
}
