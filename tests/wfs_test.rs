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
    assert!(resp.status().is_success(), "WFS GetCapabilities 应返回 200, 实际: {}", resp.status());
}

#[actix_rt::test]
async fn test_wfs_describe_feature_type() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/wfs?SERVICE=WFS&REQUEST=DescribeFeatureType&TYPENAME=world")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "WFS DescribeFeatureType 应返回 200, 实际: {}", resp.status());

    let content_type = resp.headers()
        .get("Content-Type").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    assert!(content_type.contains("xml"), "Content-Type 应为 XML, 实际: {}", content_type);
}

#[actix_rt::test]
async fn test_wfs_get_feature_geojson() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/wfs?SERVICE=WFS&REQUEST=GetFeature&TYPENAME=world&OUTPUTFORMAT=application/json")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "WFS GetFeature (GeoJSON) 应返回 200, 实际: {}", resp.status());

    let content_type = resp.headers()
        .get("Content-Type").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    assert!(content_type.contains("application/json"), "Content-Type 应为 application/json, 实际: {}", content_type);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body["features"].is_array(), "GeoJSON 应包含 features 数组, 实际: {}", body);
}

#[actix_rt::test]
async fn test_wfs_get_feature_gml2() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/wfs?SERVICE=WFS&REQUEST=GetFeature&TYPENAME=world&OUTPUTFORMAT=text/xml;subtype=gml/2.1.2")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "WFS GetFeature (GML2) 应返回 200, 实际: {}", resp.status());

    let content_type = resp.headers()
        .get("Content-Type").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    assert!(content_type.contains("gml"), "Content-Type 应为 GML, 实际: {}", content_type);
}

#[actix_rt::test]
async fn test_wfs_get_feature_gml32() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/wfs?SERVICE=WFS&REQUEST=GetFeature&TYPENAME=world&OUTPUTFORMAT=text/xml;subtype=gml/3.2")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "WFS GetFeature (GML3.2) 应返回 200, 实际: {}", resp.status());

    let content_type = resp.headers()
        .get("Content-Type").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    assert!(content_type.contains("gml"), "Content-Type 应为 GML, 实际: {}", content_type);
}

#[actix_rt::test]
async fn test_wfs_get_feature_csv() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/wfs?SERVICE=WFS&REQUEST=GetFeature&TYPENAME=world&OUTPUTFORMAT=csv")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "WFS GetFeature (CSV) 应返回 200, 实际: {}", resp.status());

    let content_type = resp.headers()
        .get("Content-Type").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    assert!(content_type.contains("text/csv"), "Content-Type 应为 text/csv, 实际: {}", content_type);
}

#[actix_rt::test]
async fn test_wfs_get_feature_with_lock() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/wfs?SERVICE=WFS&REQUEST=GetFeatureWithLock&TYPENAME=world&OUTPUTFORMAT=application/json")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "WFS GetFeatureWithLock 应返回 200, 实际: {}", resp.status());

    let content_type = resp.headers()
        .get("Content-Type").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    assert!(content_type.contains("application/json"), "Content-Type 应为 application/json, 实际: {}", content_type);
}

#[actix_rt::test]
async fn test_wfs_transaction_insert_roundtrip() {
    let app = build_test_app!();

    // WFS-T Insert 一条 Point 要素
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
    assert!(resp.status().is_success(), "WFS-T Transaction 应返回 200, 实际: {}", resp.status());

    let body = test::read_body(resp).await;
    let xml_resp = String::from_utf8_lossy(&body);
    assert!(xml_resp.contains("totalInserted>1"), "应插入 1 条要素, 响应: {}", xml_resp);

    // 往返验证: GetFeature 应能读回刚插入的要素
    let req = test::TestRequest::get()
        .uri("/wfs?SERVICE=WFS&REQUEST=GetFeature&TYPENAME=world&OUTPUTFORMAT=application/json")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "WFS GetFeature 往返应返回 200, 实际: {}", resp.status());

    let body: serde_json::Value = test::read_body_json(resp).await;
    let features = body["features"].as_array().cloned().unwrap_or_default();
    assert!(features.len() >= 1, "往返后应至少 1 条要素, 实际: {}", features.len());
}

#[actix_rt::test]
async fn test_wfs_transaction_update() {
    let app = build_test_app!();

    // 同一事务内 Insert 一条要素并 Update 其属性
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<wfs:Transaction xmlns:wfs="http://www.opengis.net/wfs/2.0" xmlns:gml="http://www.opengis.net/gml/3.2" service="WFS" version="2.0.0">
  <wfs:Insert>
    <wfs:TypeName>world</wfs:TypeName>
    <wfs:Feature>
      <gml:Point><gml:pos>30.0 40.0</gml:pos></gml:Point>
    </wfs:Feature>
  </wfs:Insert>
  <wfs:Update>
    <wfs:TypeName>world</wfs:TypeName>
    <wfs:Property>
      <wfs:Name>name</wfs:Name>
      <wfs:Value>updated</wfs:Value>
    </wfs:Property>
  </wfs:Update>
</wfs:Transaction>"#;

    let req = test::TestRequest::post()
        .uri("/wfs")
        .insert_header(("Content-Type", "application/xml"))
        .set_payload(xml)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "WFS-T Transaction (insert+update) 应返回 200, 实际: {}", resp.status());

    let body = test::read_body(resp).await;
    let xml_resp = String::from_utf8_lossy(&body);
    assert!(xml_resp.contains("totalInserted>1"), "应插入 1 条要素, 响应: {}", xml_resp);
    assert!(xml_resp.contains("totalUpdated>1"), "应更新 1 条要素, 响应: {}", xml_resp);

    // 验证属性已更新
    let req = test::TestRequest::get()
        .uri("/wfs?SERVICE=WFS&REQUEST=GetFeature&TYPENAME=world&OUTPUTFORMAT=application/json")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    let features = body["features"].as_array().cloned().unwrap_or_default();
    assert!(features.len() >= 1, "更新后应至少 1 条要素, 实际: {}", features.len());
    let name = features[0]["properties"]["name"].as_str().unwrap_or("");
    assert_eq!(name, "updated", "要素 name 属性应更新为 'updated', 实际: {:?}", name);
}

// ---------------------------------------------------------------------------
// Batch 3: 默认 GML 3.1.1 输出 + Transaction Delete (ogc:FeatureId 过滤)
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn test_wfs_get_feature_gml311_default() {
    let app = build_test_app!();

    // 不带 OUTPUTFORMAT → 默认 GML 3.1.1
    let req = test::TestRequest::get()
        .uri("/wfs?SERVICE=WFS&REQUEST=GetFeature&TYPENAME=world")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "WFS GetFeature (默认 GML3.1.1) 应返回 200, 实际: {}", resp.status());

    let content_type = resp.headers()
        .get("Content-Type").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    assert!(content_type.contains("gml"), "Content-Type 应为 GML, 实际: {}", content_type);

    let body = test::read_body(resp).await;
    let xml = String::from_utf8_lossy(&body);
    assert!(xml.contains("3.1.1") || xml.contains("3.1"), "默认应为 GML 3.1.1, 实际: {}", xml);
}

#[actix_rt::test]
async fn test_wfs_transaction_delete_by_feature_id() {
    let app = build_test_app!();

    // 先通过 REST 创建一条要素以获取稳定的 feature id
    let create = test::TestRequest::post()
        .uri("/geoserver/layers/world/features")
        .set_json(&serde_json::json!({
            "geometry": { "type": "Point", "coordinates": [10.0, 20.0] },
            "properties": { "name": "to-delete" },
        }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::CREATED, "创建要素应返回 201");
    let body: serde_json::Value = test::read_body_json(resp).await;
    let fid = body["id"].as_str().expect("应返回要素 id").to_string();

    // WFS-T Delete: 通过 ogc:FeatureId 删除刚创建的要素
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<wfs:Transaction xmlns:wfs="http://www.opengis.net/wfs/2.0" xmlns:ogc="http://www.opengis.net/ogc" service="WFS" version="2.0.0">
  <wfs:Delete>
    <wfs:TypeName>world</wfs:TypeName>
    <wfs:Filter>
      <ogc:FeatureId fid="{}"/>
    </wfs:Filter>
  </wfs:Delete>
</wfs:Transaction>"#,
        fid
    );

    let req = test::TestRequest::post()
        .uri("/wfs")
        .insert_header(("Content-Type", "application/xml"))
        .set_payload(xml)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "WFS-T Transaction Delete 应返回 200, 实际: {}", resp.status());

    let body = test::read_body(resp).await;
    let xml_resp = String::from_utf8_lossy(&body);
    assert!(xml_resp.contains("totalDeleted>1"), "应删除 1 条要素, 响应: {}", xml_resp);

    // 验证: 通过 REST 查询该要素应返回 404
    let req = test::TestRequest::get()
        .uri(&format!("/geoserver/layers/world/features/{}", fid))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::NOT_FOUND, "删除后要素应不存在");
}

#[actix_rt::test]
async fn test_wfs_transaction_delete_no_match() {
    let app = build_test_app!();

    // Delete 一个不存在的 FeatureId → totalDeleted=0 (Delete 操作本身可执行)
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<wfs:Transaction xmlns:wfs="http://www.opengis.net/wfs/2.0" xmlns:ogc="http://www.opengis.net/ogc" service="WFS" version="2.0.0">
  <wfs:Delete>
    <wfs:TypeName>world</wfs:TypeName>
    <wfs:Filter>
      <ogc:FeatureId fid="nonexistent-xyz"/>
    </wfs:Filter>
  </wfs:Delete>
</wfs:Transaction>"#;

    let req = test::TestRequest::post()
        .uri("/wfs")
        .insert_header(("Content-Type", "application/xml"))
        .set_payload(xml)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "WFS-T Delete (无匹配) 应返回 200, 实际: {}", resp.status());

    let body = test::read_body(resp).await;
    let xml_resp = String::from_utf8_lossy(&body);
    assert!(xml_resp.contains("totalDeleted>0"), "无匹配应删除 0 条, 响应: {}", xml_resp);
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
    assert_eq!(resp.status(), actix_web::http::StatusCode::BAD_REQUEST,
        "LockFeature 未实现应返回 400, 实际: {}", resp.status());
}

// ---------------------------------------------------------------------------
// Batch 8: WFS FILTER= (OGC XML + ECQL) 与 CQL_FILTER 厂商参数
// ---------------------------------------------------------------------------

/// 通过 REST 向 world 图层创建一条带 name / elevation 属性的 Point 要素
macro_rules! create_world_feature {
    ($app:expr, $name:expr, $elev:expr, $x:expr, $y:expr) => {{
        let req = test::TestRequest::post()
            .uri("/geoserver/layers/world/features")
            .set_json(&serde_json::json!({
                "geometry": { "type": "Point", "coordinates": [$x, $y] },
                "properties": { "name": $name, "elevation": $elev },
            }))
            .to_request();
        let resp = test::call_service(&$app, req).await;
        assert_eq!(resp.status(), actix_web::http::StatusCode::CREATED, "创建要素应返回 201");
        let body: serde_json::Value = test::read_body_json(resp).await;
        body["id"].as_str().expect("应返回要素 id").to_string()
    }};
}

/// 简单百分号编码 XML 中的保留字符, 用于 FILTER 参数
fn urlencode_xml(xml: &str) -> String {
    xml.chars().map(|c| match c {
        '<' => "%3C".to_string(),
        '>' => "%3E".to_string(),
        '/' => "%2F".to_string(),
        ' ' => "%20".to_string(),
        '=' => "%3D".to_string(),
        '\'' => "%27".to_string(),
        _ => c.to_string(),
    }).collect()
}

#[actix_rt::test]
async fn test_wfs_get_feature_ecql_filter_equals() {
    let app = build_test_app!();
    create_world_feature!(app, "alpha", 100, 10.0, 10.0);
    create_world_feature!(app, "beta", 200, 50.0, 50.0);

    // ECQL 等值过滤 (FILTER 参数, GeoServer 厂商扩展)
    let req = test::TestRequest::get()
        .uri("/wfs?SERVICE=WFS&REQUEST=GetFeature&TYPENAME=world&OUTPUTFORMAT=application/json&FILTER=name='alpha'")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "GetFeature + FILTER(ECQL) 应返回 200, 实际: {}", resp.status());

    let body: serde_json::Value = test::read_body_json(resp).await;
    let features = body["features"].as_array().cloned().unwrap_or_default();
    assert_eq!(features.len(), 1, "FILTER=name='alpha' 应只返回 1 条要素, 实际: {}", features.len());
    let name = features[0]["properties"]["name"].as_str().unwrap_or("");
    assert_eq!(name, "alpha", "应只返回 alpha, 实际: {:?}", name);
}

#[actix_rt::test]
async fn test_wfs_get_feature_ecql_filter_bbox() {
    let app = build_test_app!();
    create_world_feature!(app, "inside", 100, 10.0, 10.0);
    create_world_feature!(app, "outside", 200, 50.0, 50.0);

    // ECQL bbox 空间过滤
    let req = test::TestRequest::get()
        .uri("/wfs?SERVICE=WFS&REQUEST=GetFeature&TYPENAME=world&OUTPUTFORMAT=application/json&FILTER=bbox(geometry,0,0,20,20)")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "GetFeature + FILTER(bbox) 应返回 200, 实际: {}", resp.status());

    let body: serde_json::Value = test::read_body_json(resp).await;
    let features = body["features"].as_array().cloned().unwrap_or_default();
    assert_eq!(features.len(), 1, "bbox(0,0,20,20) 应只返回 1 条要素, 实际: {}", features.len());
    let name = features[0]["properties"]["name"].as_str().unwrap_or("");
    assert_eq!(name, "inside", "应只返回 inside, 实际: {:?}", name);
}

#[actix_rt::test]
async fn test_wfs_get_feature_ecql_filter_and_like() {
    let app = build_test_app!();
    create_world_feature!(app, "alpha", 100, 10.0, 10.0);
    create_world_feature!(app, "alpine", 300, 15.0, 15.0);
    create_world_feature!(app, "beta", 50, 5.0, 5.0);

    // ECQL AND + LIKE 组合
    let req = test::TestRequest::get()
        .uri("/wfs?SERVICE=WFS&REQUEST=GetFeature&TYPENAME=world&OUTPUTFORMAT=application/json&FILTER=name%20LIKE%20'alp%25'%20AND%20elevation%20%3E%20100")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "GetFeature + FILTER(AND/LIKE) 应返回 200, 实际: {}", resp.status());

    let body: serde_json::Value = test::read_body_json(resp).await;
    let features = body["features"].as_array().cloned().unwrap_or_default();
    assert_eq!(features.len(), 1, "name LIKE 'alp%' AND elevation>100 应返回 1 条, 实际: {}", features.len());
    let name = features[0]["properties"]["name"].as_str().unwrap_or("");
    assert_eq!(name, "alpine", "应只返回 alpine, 实际: {:?}", name);
}

#[actix_rt::test]
async fn test_wfs_get_feature_cql_filter_param() {
    let app = build_test_app!();
    create_world_feature!(app, "alpha", 100, 10.0, 10.0);
    create_world_feature!(app, "beta", 200, 50.0, 50.0);

    // GeoServer CQL_FILTER 厂商参数 (WFS 同样支持)
    let req = test::TestRequest::get()
        .uri("/wfs?SERVICE=WFS&REQUEST=GetFeature&TYPENAME=world&OUTPUTFORMAT=application/json&CQL_FILTER=name='beta'")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "GetFeature + CQL_FILTER 应返回 200, 实际: {}", resp.status());

    let body: serde_json::Value = test::read_body_json(resp).await;
    let features = body["features"].as_array().cloned().unwrap_or_default();
    assert_eq!(features.len(), 1, "CQL_FILTER=name='beta' 应只返回 1 条要素, 实际: {}", features.len());
    let name = features[0]["properties"]["name"].as_str().unwrap_or("");
    assert_eq!(name, "beta", "应只返回 beta, 实际: {:?}", name);
}

#[actix_rt::test]
async fn test_wfs_get_feature_xml_filter_equals() {
    let app = build_test_app!();
    create_world_feature!(app, "alpha", 100, 10.0, 10.0);
    create_world_feature!(app, "beta", 200, 50.0, 50.0);

    // WFS 标准: FILTER 为 URL 编码的 OGC XML Filter
    let xml = urlencode_xml(
        "<Filter><PropertyIsEqualTo><PropertyName>name</PropertyName><Literal>alpha</Literal></PropertyIsEqualTo></Filter>",
    );
    let uri = format!("/wfs?SERVICE=WFS&REQUEST=GetFeature&TYPENAME=world&OUTPUTFORMAT=application/json&FILTER={}", xml);
    let req = test::TestRequest::get().uri(&uri).to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "GetFeature + FILTER(XML) 应返回 200, 实际: {}", resp.status());

    let body: serde_json::Value = test::read_body_json(resp).await;
    let features = body["features"].as_array().cloned().unwrap_or_default();
    assert_eq!(features.len(), 1, "XML PropertyIsEqualTo 应只返回 1 条要素, 实际: {}", features.len());
    let name = features[0]["properties"]["name"].as_str().unwrap_or("");
    assert_eq!(name, "alpha", "应只返回 alpha, 实际: {:?}", name);
}

#[actix_rt::test]
async fn test_wfs_get_feature_xml_filter_numeric() {
    let app = build_test_app!();
    create_world_feature!(app, "low", 100, 10.0, 10.0);
    create_world_feature!(app, "high", 200, 50.0, 50.0);

    // XML 数值比较 (PropertyIsGreaterThan) → 数值比较而非字符串比较
    let xml = urlencode_xml(
        "<Filter><PropertyIsGreaterThan><PropertyName>elevation</PropertyName><Literal>150</Literal></PropertyIsGreaterThan></Filter>",
    );
    let uri = format!("/wfs?SERVICE=WFS&REQUEST=GetFeature&TYPENAME=world&OUTPUTFORMAT=application/json&FILTER={}", xml);
    let req = test::TestRequest::get().uri(&uri).to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "GetFeature + FILTER(XML numeric) 应返回 200, 实际: {}", resp.status());

    let body: serde_json::Value = test::read_body_json(resp).await;
    let features = body["features"].as_array().cloned().unwrap_or_default();
    assert_eq!(features.len(), 1, "elevation>150 应只返回 1 条要素, 实际: {}", features.len());
    let name = features[0]["properties"]["name"].as_str().unwrap_or("");
    assert_eq!(name, "high", "应只返回 high, 实际: {:?}", name);
}
