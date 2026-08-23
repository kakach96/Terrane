//! OWS (OGC Web Services) unified dispatcher integration tests.
//!
//! Covers the GeoServer-style `/ows` endpoint registered under the API context
//! (`/geoserver/ows`): GET dispatch to WMS/WFS/WCS/WPS/CSW GetCapabilities,
//! missing/unsupported `service` handling, and POST dispatch to
//! WFS (KVP form), WPS (XML Execute) and CSW (XML GetRecords).

#[macro_use]
mod common;

use actix_web::test;

/// Send a GET GetCapabilities request through `/ows` and assert the response is
/// a successful XML document containing `marker`. Implemented as a macro (like
/// `build_test_app!`) so it expands where the app's concrete type is known.
macro_rules! assert_xml_capabilities {
    ($service:expr, $marker:expr) => {{
        let app = build_test_app!();
        let uri = format!(
            "/geoserver/ows?SERVICE={}&REQUEST=GetCapabilities",
            $service
        );
        let req = actix_web::test::TestRequest::get().uri(&uri).to_request();
        let resp = actix_web::test::call_service(&app, req).await;
        assert!(
            resp.status().is_success(),
            "{} GetCapabilities 应返回 200, 实际: {}",
            $service,
            resp.status()
        );

        let ct = resp
            .headers()
            .get("Content-Type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert!(
            ct.contains("xml"),
            "{} GetCapabilities Content-Type 应为 XML, 实际: {}",
            $service,
            ct
        );

        let bytes = actix_web::test::read_body(resp).await;
        let body = String::from_utf8_lossy(&bytes);
        assert!(
            body.contains($marker),
            "{} GetCapabilities 应包含标记 '{}', 实际 body 开头: {:?}",
            $service,
            $marker,
            body.chars().take(120).collect::<String>()
        );
    }};
}

// ---------------------------------------------------------------------------
// GET dispatch
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn test_ows_get_wms() {
    assert_xml_capabilities!("WMS", "WmsCapabilities");
}

#[actix_rt::test]
async fn test_ows_get_wfs() {
    assert_xml_capabilities!("WFS", "WfsCapabilities");
}

#[actix_rt::test]
async fn test_ows_get_wcs() {
    assert_xml_capabilities!("WCS", "WcsCapabilities");
}

#[actix_rt::test]
async fn test_ows_get_wps() {
    assert_xml_capabilities!("WPS", "wps:Capabilities");
}

#[actix_rt::test]
async fn test_ows_get_csw() {
    assert_xml_capabilities!("CSW", "csw:Capabilities");
}

#[actix_rt::test]
async fn test_ows_get_lowercase_service() {
    // `service` 参数大小写不敏感
    assert_xml_capabilities!("wms", "WmsCapabilities");
}

// ---------------------------------------------------------------------------
// Missing / unsupported service
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn test_ows_missing_service() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/geoserver/ows?REQUEST=GetCapabilities")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let bytes = test::read_body(resp).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(
        body.contains("ExceptionReport"),
        "应返回 OWS ExceptionReport"
    );
    assert!(
        body.contains("MissingParameterValue"),
        "应包含 MissingParameterValue"
    );
}

#[actix_rt::test]
async fn test_ows_unsupported_service() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/geoserver/ows?SERVICE=FOO&REQUEST=GetCapabilities")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let bytes = test::read_body(resp).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(
        body.contains("ExceptionReport"),
        "应返回 OWS ExceptionReport"
    );
    assert!(
        body.contains("InvalidParameterValue"),
        "应包含 InvalidParameterValue"
    );
}

// ---------------------------------------------------------------------------
// POST dispatch
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn test_ows_post_wfs_kvp() {
    // WFS POST 使用 KVP form body；service 由查询参数提供
    let app = build_test_app!();

    let req = test::TestRequest::post()
        .uri("/geoserver/ows?SERVICE=WFS")
        .insert_header(("Content-Type", "application/x-www-form-urlencoded"))
        .set_payload("SERVICE=WFS&REQUEST=GetCapabilities&VERSION=2.0.0")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "OWS POST WFS 应返回 200, 实际: {}",
        resp.status()
    );

    let bytes = test::read_body(resp).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(body.contains("WfsCapabilities"));
}

#[actix_rt::test]
async fn test_ows_post_wps_xml_sniff() {
    // 无 service 参数时按 body 嗅探 `<wps:` → WPS
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
        .uri("/geoserver/ows")
        .insert_header(("Content-Type", "application/xml"))
        .set_payload(xml_body)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "OWS POST WPS 应返回 200, 实际: {}",
        resp.status()
    );

    let bytes = test::read_body(resp).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(body.contains("<wps:ExecuteResponse"));
    assert!(body.contains("<wps:ProcessSucceeded>"));
}

#[actix_rt::test]
async fn test_ows_post_csw_xml_sniff() {
    // 无 service 参数时按 body 嗅探 `<csw:` → CSW
    let app = build_test_app!();

    let xml_body = r#"<?xml version="1.0" encoding="UTF-8"?>
    <csw:GetRecords service="CSW" version="2.0.2" resultType="results"
        outputSchema="http://www.opengis.net/cat/csw/2.0.2" startPosition="1" maxRecords="10"
        xmlns:csw="http://www.opengis.net/cat/csw/2.0.2">
      <csw:Query typeNames="csw:Record">
        <csw:ElementSetName>brief</csw:ElementSetName>
      </csw:Query>
    </csw:GetRecords>"#;

    let req = test::TestRequest::post()
        .uri("/geoserver/ows")
        .insert_header(("Content-Type", "application/xml"))
        .set_payload(xml_body)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "OWS POST CSW 应返回 200, 实际: {}",
        resp.status()
    );

    let bytes = test::read_body(resp).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(body.contains("<csw:GetRecordsResponse"));
}
