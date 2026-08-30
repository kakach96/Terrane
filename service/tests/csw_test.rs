//! CSW (Catalog Service for the Web) 2.0.2 integration tests.
//!
//! Covers: GetCapabilities, DescribeRecord, GetRecords (KVP + XML POST, paging,
//! hits, element sets, CQL constraint), GetRecordById, GetDomain and OWS
//! exceptions. Catalog records are derived from the Terrane layer catalog.

#[macro_use]
mod common;

use actix_web::test;

/// 通过 REST 在 default workspace 下创建一层图层 (store=shapes)
macro_rules! create_layer {
    ($app:expr, $name:expr, $title:expr, $minx:expr, $miny:expr, $maxx:expr, $maxy:expr) => {{
        let req = test::TestRequest::post()
            .uri("/terrane/layers")
            .set_json(&serde_json::json!({
                "name": $name,
                "title": $title,
                "workspace": "default",
                "store": "shapes",
                "srs": "EPSG:4326",
                "minx": $minx, "miny": $miny, "maxx": $maxx, "maxy": $maxy,
            }))
            .to_request();
        let resp = test::call_service(&$app, req).await;
        assert_eq!(
            resp.status(),
            actix_web::http::StatusCode::CREATED,
            "创建图层应返回 201, 实际: {}",
            resp.status()
        );
    }};
}

#[actix_rt::test]
async fn test_csw_get_capabilities() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/csw?service=CSW&version=2.0.2&request=GetCapabilities")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "CSW GetCapabilities 应返回 200, 实际: {}",
        resp.status()
    );

    let bytes = test::read_body(resp).await;
    let xml = String::from_utf8_lossy(&bytes);
    assert!(
        xml.contains("<csw:Capabilities service=\"CSW\" version=\"2.0.2\""),
        "应包含 CSW 2.0.2 能力根元素"
    );
    assert!(xml.contains("<ows:OperationsMetadata>"));
    for op in [
        "GetCapabilities",
        "DescribeRecord",
        "GetRecords",
        "GetRecordById",
        "GetDomain",
    ] {
        assert!(
            xml.contains(&format!("<ows:Operation name=\"{}\">", op)),
            "应声明操作 {}",
            op
        );
    }
    assert!(xml.contains("<csw:FilterCapabilities>"));
    assert!(xml.contains("<ows:ServiceTypeVersion>2.0.2</ows:ServiceTypeVersion>"));
}

#[actix_rt::test]
async fn test_csw_describe_record() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/csw?service=CSW&request=DescribeRecord&typeNames=csw:Record")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "CSW DescribeRecord 应返回 200, 实际: {}",
        resp.status()
    );

    let bytes = test::read_body(resp).await;
    let xml = String::from_utf8_lossy(&bytes);
    assert!(xml.contains("<csw:DescribeRecordResponse"));
    assert!(xml.contains("<csw:SchemaComponent"));
    assert!(xml.contains("schemaLanguage=\"XMLSCHEMA\""));
    assert!(xml.contains("complexType name=\"Record\""));
    assert!(xml.contains("element name=\"identifier\""));
    assert!(xml.contains("element name=\"title\""));
}

#[actix_rt::test]
async fn test_csw_get_records_summary() {
    let app = build_test_app!();
    // 默认配置仅 world 一层
    let req = test::TestRequest::get()
        .uri("/csw?service=CSW&request=GetRecords&typeNames=csw:Record&elementSetName=summary")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "CSW GetRecords 应返回 200, 实际: {}",
        resp.status()
    );

    let bytes = test::read_body(resp).await;
    let xml = String::from_utf8_lossy(&bytes);
    assert!(xml.contains("<csw:GetRecordsResponse"));
    assert!(xml.contains("numberOfRecordsMatched=\"1\""));
    assert!(xml.contains("numberOfRecordsReturned=\"1\""));
    assert!(xml.contains("<csw:SummaryRecord>"));
    assert!(xml.contains("<dc:identifier>world</dc:identifier>"));
    assert!(xml.contains("<dc:title>World</dc:title>"));
    assert!(xml.contains("<dc:type>dataset</dc:type>"));
    assert!(xml.contains("<ows:BoundingBox crs=\"urn:ogc:def:crs:OGC:2:84\">"));
    assert!(xml.contains("<ows:LowerCorner>-180 -90</ows:LowerCorner>"));
    // summary 不含 format
    assert!(!xml.contains("<dc:format>"));
}

#[actix_rt::test]
async fn test_csw_get_records_full_and_constraint() {
    let app = build_test_app!();
    create_layer!(app, "usa", "USA States", -125.0, 24.0, -66.0, 49.0);

    // full element set + CQL constraint 过滤 title
    let req = test::TestRequest::get()
        .uri("/csw?service=CSW&request=GetRecords&elementSetName=full&constraint=Title%20like%20'%25states%25'")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let bytes = test::read_body(resp).await;
    let xml = String::from_utf8_lossy(&bytes);
    assert!(xml.contains("numberOfRecordsMatched=\"1\""));
    assert!(xml.contains("<csw:Record>"));
    assert!(xml.contains("<dc:identifier>usa</dc:identifier>"));
    assert!(xml.contains("<dc:subject>default</dc:subject>"));
    assert!(xml.contains("<dct:references"));
    assert!(xml.contains("<ows:LowerCorner>-125 24</ows:LowerCorner>"));
    // world 不应出现在结果里
    assert!(!xml.contains("<dc:identifier>world</dc:identifier>"));
}

#[actix_rt::test]
async fn test_csw_get_records_hits_and_paging() {
    let app = build_test_app!();
    create_layer!(app, "usa", "USA States", -125.0, 24.0, -66.0, 49.0);
    create_layer!(app, "parks", "National Parks", -125.0, 24.0, -66.0, 49.0);

    // hits → 只报匹配数, 不返回记录体
    let req = test::TestRequest::get()
        .uri("/csw?service=CSW&request=GetRecords&resultType=hits")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let bytes = test::read_body(resp).await;
    let xml = String::from_utf8_lossy(&bytes);
    assert!(xml.contains("numberOfRecordsMatched=\"3\""));
    assert!(xml.contains("numberOfRecordsReturned=\"0\""));
    assert!(!xml.contains("<csw:SummaryRecord>"));

    // 分页: maxRecords=1 startPosition=2 → 返回第 2 条 (usa), nextRecord=3
    let req = test::TestRequest::get()
        .uri("/csw?service=CSW&request=GetRecords&maxRecords=1&startPosition=2")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let bytes = test::read_body(resp).await;
    let xml = String::from_utf8_lossy(&bytes);
    assert!(xml.contains("numberOfRecordsMatched=\"3\""));
    assert!(xml.contains("numberOfRecordsReturned=\"1\""));
    assert!(xml.contains("nextRecord=\"3\""));
    assert!(xml.contains("<dc:identifier>usa</dc:identifier>"));
    assert!(!xml.contains("<dc:identifier>world</dc:identifier>"));
    assert!(!xml.contains("<dc:identifier>parks</dc:identifier>"));
}

#[actix_rt::test]
async fn test_csw_get_record_by_id() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/csw?service=CSW&request=GetRecordById&id=world&elementSetName=brief")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "CSW GetRecordById 应返回 200, 实际: {}",
        resp.status()
    );

    let bytes = test::read_body(resp).await;
    let xml = String::from_utf8_lossy(&bytes);
    assert!(xml.contains("<csw:GetRecordByIdResponse"));
    assert!(xml.contains("<csw:BriefRecord>"));
    assert!(xml.contains("<dc:identifier>world</dc:identifier>"));
    assert!(xml.contains("<dc:format>application/xml</dc:format>"));

    // 不存在的 id → 空响应
    let req = test::TestRequest::get()
        .uri("/csw?service=CSW&request=GetRecordById&id=nope")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let bytes = test::read_body(resp).await;
    let xml = String::from_utf8_lossy(&bytes);
    assert!(xml.contains("<csw:GetRecordByIdResponse"));
    assert!(!xml.contains("<dc:identifier>"));
}

#[actix_rt::test]
async fn test_csw_post_get_records_xml() {
    let app = build_test_app!();

    let xml_body = r#"<?xml version="1.0" encoding="UTF-8"?>
    <csw:GetRecords service="CSW" version="2.0.2" resultType="results"
        outputSchema="http://www.opengis.net/cat/csw/2.0.2" startPosition="1" maxRecords="10"
        xmlns:csw="http://www.opengis.net/cat/csw/2.0.2">
      <csw:Query typeNames="csw:Record">
        <csw:ElementSetName>brief</csw:ElementSetName>
        <csw:Constraint version="1.1.0">
          <ogc:Filter xmlns:ogc="http://www.opengis.net/ogc">
            <ogc:PropertyIsEqualTo>
              <ogc:PropertyName>Title</ogc:PropertyName>
              <ogc:Literal>World</ogc:Literal>
            </ogc:PropertyIsEqualTo>
          </ogc:Filter>
        </csw:Constraint>
      </csw:Query>
    </csw:GetRecords>"#;

    let req = test::TestRequest::post()
        .uri("/csw")
        .insert_header(("Content-Type", "application/xml"))
        .set_payload(xml_body)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "CSW XML GetRecords 应返回 200, 实际: {}",
        resp.status()
    );

    let bytes = test::read_body(resp).await;
    let xml = String::from_utf8_lossy(&bytes);
    assert!(xml.contains("<csw:GetRecordsResponse"));
    assert!(xml.contains("numberOfRecordsMatched=\"1\""));
    assert!(xml.contains("<csw:BriefRecord>"));
    assert!(xml.contains("<dc:identifier>world</dc:identifier>"));
}

#[actix_rt::test]
async fn test_csw_post_get_record_by_id_xml() {
    let app = build_test_app!();

    let xml_body = r#"<csw:GetRecordById xmlns:csw="http://www.opengis.net/cat/csw/2.0.2"
        service="CSW" version="2.0.2" elementSetName="full">
      <csw:Id>world</csw:Id>
    </csw:GetRecordById>"#;

    let req = test::TestRequest::post()
        .uri("/csw")
        .insert_header(("Content-Type", "application/xml"))
        .set_payload(xml_body)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "CSW XML GetRecordById 应返回 200, 实际: {}",
        resp.status()
    );

    let bytes = test::read_body(resp).await;
    let xml = String::from_utf8_lossy(&bytes);
    assert!(xml.contains("<csw:GetRecordByIdResponse"));
    assert!(xml.contains("<csw:Record>"));
    assert!(xml.contains("<dc:subject>default</dc:subject>"));
    assert!(xml.contains("<dc:identifier>world</dc:identifier>"));
}

#[actix_rt::test]
async fn test_csw_get_domain() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/csw?service=CSW&request=GetDomain&parameterName=GetRecordsResultType")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let bytes = test::read_body(resp).await;
    let xml = String::from_utf8_lossy(&bytes);
    assert!(xml.contains("<csw:GetDomainResponse"));
    assert!(xml.contains("<csw:ParameterName>GetRecordsResultType</csw:ParameterName>"));
    assert!(xml.contains("<csw:Value>hits</csw:Value>"));
    assert!(xml.contains("<csw:Value>results</csw:Value>"));
}

#[actix_rt::test]
async fn test_csw_invalid_service() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/csw?service=WFS&request=GetCapabilities")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "异常报告也应返回 200");

    let bytes = test::read_body(resp).await;
    let xml = String::from_utf8_lossy(&bytes);
    assert!(xml.contains("<ows:ExceptionReport"));
    assert!(xml.contains("exceptionCode=\"InvalidParameterValue\""));
}
