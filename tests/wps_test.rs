//! WPS (Web Processing Service) 1.0.0 integration tests.
//!
//! Covers: GetCapabilities, DescribeProcess, Execute (KVP raw + document + XML
//! POST) for the built-in processes `vec:Centroid` / `vec:Buffer` / `gs:Bounds`.

#[macro_use]
mod common;

use actix_web::test;

#[actix_rt::test]
async fn test_wps_get_capabilities() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/wps?service=WPS&version=1.0.0&request=GetCapabilities")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "WPS GetCapabilities 应返回 200, 实际: {}",
        resp.status()
    );

    let bytes = test::read_body(resp).await;
    let xml = String::from_utf8_lossy(&bytes);
    assert!(
        xml.contains("<wps:Capabilities service=\"WPS\" version=\"1.0.0\""),
        "应包含 WPS 1.0.0 能力根元素"
    );
    assert!(xml.contains("<ows:OperationsMetadata>"));
    for op in ["GetCapabilities", "DescribeProcess", "Execute"] {
        assert!(
            xml.contains(&format!("<ows:Operation name=\"{}\">", op)),
            "应声明操作 {}",
            op
        );
    }
    assert!(xml.contains("<wps:ProcessOfferings>"));
    for p in ["vec:Centroid", "vec:Buffer", "gs:Bounds"] {
        assert!(
            xml.contains(&format!("<ows:Identifier>{}</ows:Identifier>", p)),
            "ProcessOfferings 应包含 {}",
            p
        );
    }
    assert!(xml.contains("<wps:Languages>"));
}

#[actix_rt::test]
async fn test_wps_describe_process() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/wps?service=WPS&request=DescribeProcess&identifier=vec:Buffer")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "WPS DescribeProcess 应返回 200, 实际: {}",
        resp.status()
    );

    let bytes = test::read_body(resp).await;
    let xml = String::from_utf8_lossy(&bytes);
    assert!(xml.contains("<wps:ProcessDescriptions"));
    assert!(xml.contains("<ProcessDescription"));
    assert!(xml.contains("<ows:Identifier>vec:Buffer</ows:Identifier>"));
    assert!(xml.contains("<DataInputs>"));
    assert!(xml.contains("<ows:Identifier>features</ows:Identifier>"));
    assert!(xml.contains("<ows:Identifier>distance</ows:Identifier>"));
    assert!(xml.contains("<LiteralData>"));
    assert!(xml.contains("xsd:double"));
    assert!(xml.contains("<ProcessOutputs>"));
}

#[actix_rt::test]
async fn test_wps_execute_centroid_raw() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/wps?service=WPS&request=Execute&identifier=vec:Centroid&version=1.0.0&response=raw&DataInputs=features=layer:world")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "WPS Execute (centroid) 应返回 200, 实际: {}",
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
        "raw 输出应为 JSON, 实际: {}",
        ct
    );

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["type"], "FeatureCollection");
    let features = body["features"].as_array().unwrap();
    assert_eq!(features.len(), 0);
}

#[actix_rt::test]
async fn test_wps_execute_buffer_raw() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/wps?service=WPS&request=Execute&identifier=vec:Buffer&version=1.0.0&response=raw&DataInputs=features=layer:world;distance=2")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "WPS Execute (buffer) 应返回 200, 实际: {}",
        resp.status()
    );

    let body: serde_json::Value = test::read_body_json(resp).await;
    let features = body["features"].as_array().unwrap();
    assert_eq!(features.len(), 0);
}

#[actix_rt::test]
async fn test_wps_execute_bounds_raw() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/wps?service=WPS&request=Execute&identifier=gs:Bounds&version=1.0.0&response=raw&DataInputs=features=layer:world")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "WPS Execute (bounds) 应返回 200, 实际: {}",
        resp.status()
    );

    let body: serde_json::Value = test::read_body_json(resp).await;
    let features = body["features"].as_array().unwrap();
    // world 图层空发布 → gs:Bounds 返回默认世界范围矩形
    assert_eq!(features.len(), 1);
    let geom = &features[0]["geometry"];
    assert_eq!(geom["type"], "Polygon");
}

#[actix_rt::test]
async fn test_wps_execute_xml_post() {
    let app = build_test_app!();

    let xml_body = r#"<?xml version="1.0" encoding="UTF-8"?>
<wps:Execute service="WPS" version="1.0.0" xmlns:wps="http://www.opengis.net/wps/1.0.0" xmlns:ows="http://www.opengis.net/ows/1.1" xmlns:xlink="http://www.w3.org/1999/xlink">
  <ows:Identifier>vec:Centroid</ows:Identifier>
  <wps:DataInputs>
    <wps:Input>
      <ows:Identifier>features</ows:Identifier>
      <wps:Reference xlink:href="layer:world"/>
    </wps:Input>
  </wps:DataInputs>
  <wps:ResponseForm>
    <wps:ResponseDocument>
      <wps:Output><ows:Identifier>result</ows:Identifier></wps:Output>
    </wps:ResponseDocument>
  </wps:ResponseForm>
</wps:Execute>"#;

    let req = test::TestRequest::post()
        .uri("/wps")
        .insert_header(("Content-Type", "application/xml"))
        .set_payload(xml_body)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "WPS Execute (XML POST) 应返回 200, 实际: {}",
        resp.status()
    );

    let ct = resp
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(ct.contains("xml"), "ExecuteResponse 应为 XML, 实际: {}", ct);

    let bytes = test::read_body(resp).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(body.contains("<wps:ExecuteResponse"));
    assert!(body.contains("<wps:ProcessSucceeded>"));
    assert!(body.contains("<ows:Identifier>vec:Centroid</ows:Identifier>"));
    // 结果 GeoJSON 经 XML 转义后嵌入 ComplexData
    assert!(body.contains("FeatureCollection"));
    assert!(body.contains("mimeType=\"application/json\""));
}
