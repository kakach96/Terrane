//! WMS (Web Map Service) integration tests.
//!
//! Covers: GetCapabilities, GetMap (1.1.1 / 1.3.0 / output formats / vendor
//! params), GetFeatureInfo, DescribeLayer, GetLegendGraphic, GetStyles.

#[macro_use]
mod common;

use actix_web::test;

const GET_MAP_BASE: &str = "/wms?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetMap&LAYERS=world\
    &BBOX=-180,-90,180,90&WIDTH=256&HEIGHT=256&SRS=EPSG:4326&FORMAT=image/png";

#[actix_rt::test]
async fn test_wms_get_capabilities() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/wms?SERVICE=WMS&REQUEST=GetCapabilities")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "WMS GetCapabilities 应返回成功, 实际: {}",
        resp.status()
    );

    let content_type = resp
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        content_type.contains("xml"),
        "Content-Type 应为 application/xml"
    );
}

#[actix_rt::test]
async fn test_wms_get_map_png() {
    let app = build_test_app!();

    let req = test::TestRequest::get().uri(GET_MAP_BASE).to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "WMS GetMap 应返回 200, 实际: {}",
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
}

#[actix_rt::test]
async fn test_wms_get_map_130_axis_order() {
    let app = build_test_app!();

    // WMS 1.3.0 + EPSG:4326: BBOX 轴序为 lat,lon (-90,-180,90,180)
    let req = test::TestRequest::get()
        .uri("/wms?SERVICE=WMS&VERSION=1.3.0&REQUEST=GetMap&LAYERS=world&BBOX=-90,-180,90,180&WIDTH=256&HEIGHT=256&FORMAT=image/png&CRS=EPSG:4326")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "WMS GetMap 1.3.0 应返回 200, 实际: {}",
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
}

#[actix_rt::test]
async fn test_wms_get_map_cql_filter() {
    let app = build_test_app!();

    // vendor 参数 CQL_FILTER (INCLUDE 为合法 CQL, 全部保留)
    let uri = format!("{}&CQL_FILTER=INCLUDE", GET_MAP_BASE);
    let req = test::TestRequest::get().uri(&uri).to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "WMS GetMap + CQL_FILTER 应返回 200, 实际: {}",
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
}

#[actix_rt::test]
async fn test_wms_get_map_time() {
    let app = build_test_app!();

    // vendor 参数 TIME (ISO 8601 时间过滤)
    let uri = format!("{}&TIME=2024-01-01T00:00:00Z", GET_MAP_BASE);
    let req = test::TestRequest::get().uri(&uri).to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "WMS GetMap + TIME 应返回 200, 实际: {}",
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
}

#[actix_rt::test]
async fn test_wms_get_map_elevation() {
    let app = build_test_app!();

    // vendor 参数 ELEVATION (数值高程过滤)
    let uri = format!("{}&ELEVATION=100", GET_MAP_BASE);
    let req = test::TestRequest::get().uri(&uri).to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "WMS GetMap + ELEVATION 应返回 200, 实际: {}",
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
}

#[actix_rt::test]
async fn test_wms_get_map_svg() {
    let app = build_test_app!();

    let uri = GET_MAP_BASE.replace("FORMAT=image/png", "FORMAT=image/svg+xml");
    let req = test::TestRequest::get().uri(&uri).to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "WMS GetMap (SVG) 应返回 200, 实际: {}",
        resp.status()
    );

    let content_type = resp
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(
        content_type.contains("image/svg+xml"),
        "Content-Type 应为 image/svg+xml, 实际: {}",
        content_type
    );
}

#[actix_rt::test]
async fn test_wms_get_map_kml() {
    let app = build_test_app!();

    let uri = GET_MAP_BASE.replace(
        "FORMAT=image/png",
        "FORMAT=application/vnd.google-earth.kml+xml",
    );
    let req = test::TestRequest::get().uri(&uri).to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "WMS GetMap (KML) 应返回 200, 实际: {}",
        resp.status()
    );

    let content_type = resp
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(
        content_type.contains("vnd.google-earth.kml"),
        "Content-Type 应为 KML, 实际: {}",
        content_type
    );
}

#[actix_rt::test]
async fn test_wms_get_map_geojson() {
    let app = build_test_app!();

    let uri = GET_MAP_BASE.replace("FORMAT=image/png", "FORMAT=application/json");
    let req = test::TestRequest::get().uri(&uri).to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "WMS GetMap (GeoJSON) 应返回 200, 实际: {}",
        resp.status()
    );

    let content_type = resp
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(
        content_type.contains("geo+json"),
        "Content-Type 应为 application/geo+json, 实际: {}",
        content_type
    );
}

#[actix_rt::test]
async fn test_wms_get_map_georss() {
    let app = build_test_app!();

    let uri = GET_MAP_BASE.replace("FORMAT=image/png", "FORMAT=application/rss+xml");
    let req = test::TestRequest::get().uri(&uri).to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "WMS GetMap (GeoRSS) 应返回 200, 实际: {}",
        resp.status()
    );

    let content_type = resp
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(
        content_type.contains("application/rss+xml"),
        "Content-Type 应为 application/rss+xml, 实际: {}",
        content_type
    );

    let body = actix_web::test::read_body(resp).await;
    let text = String::from_utf8_lossy(&body);
    assert!(
        text.contains(r#"<rss xmlns:georss="http://www.georss.org/georss" version="2.0">"#),
        "GeoRSS 应包含 rss + georss 命名空间"
    );
    assert!(text.contains("<channel>"), "GeoRSS 应包含 channel");
    assert!(text.contains("</rss>"), "GeoRSS 应以 </rss> 结尾");
    // 注意: world 测试图层无要素, 故不校验 <item>/<georss:point>;
    // 几何输出 (lat lon 顺序) 由 rendering.rs 单元测试覆盖。
}

#[actix_rt::test]
async fn test_wms_get_map_pdf() {
    let app = build_test_app!();

    let uri = GET_MAP_BASE.replace("FORMAT=image/png", "FORMAT=application/pdf");
    let req = test::TestRequest::get().uri(&uri).to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "WMS GetMap (PDF) 应返回 200, 实际: {}",
        resp.status()
    );

    let content_type = resp
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(
        content_type.contains("application/pdf"),
        "Content-Type 应为 application/pdf, 实际: {}",
        content_type
    );

    let body = actix_web::test::read_body(resp).await;
    assert!(body.starts_with(b"%PDF-"), "PDF 应以 %PDF- 魔数开头");
    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("/Type /Page"), "PDF 应包含页面对象");
    assert!(text.contains("startxref"), "PDF 应包含 xref 起始偏移");
    assert!(text.trim_end().ends_with("%%EOF"), "PDF 应以 %%EOF 结尾");
}

#[actix_rt::test]
async fn test_wms_get_map_jpeg() {
    let app = build_test_app!();

    let uri = GET_MAP_BASE.replace("FORMAT=image/png", "FORMAT=image/jpeg");
    let req = test::TestRequest::get().uri(&uri).to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "WMS GetMap (JPEG) 应返回 200, 实际: {}",
        resp.status()
    );

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
async fn test_wms_get_feature_info() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/wms?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetFeatureInfo&LAYERS=world&QUERY_LAYERS=world&BBOX=-180,-90,180,90&WIDTH=256&HEIGHT=256&I=128&J=128&INFO_FORMAT=application/json")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "WMS GetFeatureInfo 应返回 200, 实际: {}",
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
async fn test_wms_describe_layer() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/wms?SERVICE=WMS&REQUEST=DescribeLayer&LAYERS=world")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "WMS DescribeLayer 应返回 200, 实际: {}",
        resp.status()
    );

    let body = test::read_body(resp).await;
    let xml = String::from_utf8_lossy(&body);
    assert!(
        xml.contains("DescribeLayerResponse"),
        "应返回 DescribeLayerResponse"
    );
    assert!(xml.contains("world"), "应包含 world 图层描述");
}

#[actix_rt::test]
async fn test_wms_get_legend_graphic() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/wms?SERVICE=WMS&REQUEST=GetLegendGraphic&LAYERS=world&FORMAT=image/png")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "WMS GetLegendGraphic 应返回 200, 实际: {}",
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
}

#[actix_rt::test]
async fn test_wms_get_styles() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/wms?SERVICE=WMS&REQUEST=GetStyles&LAYERS=world")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "WMS GetStyles 应返回 200, 实际: {}",
        resp.status()
    );

    let content_type = resp
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(
        content_type.contains("sld+xml"),
        "Content-Type 应为 SLD XML, 实际: {}",
        content_type
    );

    let body = test::read_body(resp).await;
    let xml = String::from_utf8_lossy(&body);
    assert!(
        xml.contains("StyledLayerDescriptor"),
        "应返回 StyledLayerDescriptor"
    );
}

// ---------------------------------------------------------------------------
// Batch 3: 更多输出格式 (GIF) 与 vendor 参数 (ENV/ANGLE/FEATUREID) +
// GetFeatureInfo 的 text/html 与 text/plain 格式
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn test_wms_get_map_gif() {
    let app = build_test_app!();

    let uri = GET_MAP_BASE.replace("image/png", "image/gif");
    let req = test::TestRequest::get().uri(&uri).to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "WMS GetMap (GIF) 应返回 200, 实际: {}",
        resp.status()
    );

    let content_type = resp
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(
        content_type.contains("image/gif"),
        "Content-Type 应为 image/gif, 实际: {}",
        content_type
    );
}

#[actix_rt::test]
async fn test_wms_get_map_env_vendor_param() {
    let app = build_test_app!();

    // ENV vendor 参数: "key1:'val1';key2:'val2'" (SLD 样式变量替换)
    let uri = format!(
        "{}&ENV=fillColor:'%23ff0000';strokeColor:'%230000ff'",
        GET_MAP_BASE
    );
    let req = test::TestRequest::get().uri(&uri).to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "WMS GetMap (ENV) 应返回 200, 实际: {}",
        resp.status()
    );
}

#[actix_rt::test]
async fn test_wms_get_map_angle_vendor_param() {
    let app = build_test_app!();

    // ANGLE 旋转角度(度)
    let uri = format!("{}&ANGLE=45", GET_MAP_BASE);
    let req = test::TestRequest::get().uri(&uri).to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "WMS GetMap (ANGLE) 应返回 200, 实际: {}",
        resp.status()
    );
}

#[actix_rt::test]
async fn test_wms_get_map_feature_id() {
    let app = build_test_app!();

    // FEATUREID 过滤(逗号分隔)。world 图层初始无要素, 过滤为空也应返回图片。
    let uri = format!("{}&FEATUREID=world.1,world.2", GET_MAP_BASE);
    let req = test::TestRequest::get().uri(&uri).to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "WMS GetMap (FEATUREID) 应返回 200, 实际: {}",
        resp.status()
    );
}

#[actix_rt::test]
async fn test_wms_get_feature_info_html() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/wms?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetFeatureInfo&LAYERS=world&QUERY_LAYERS=world&BBOX=-180,-90,180,90&WIDTH=256&HEIGHT=256&I=128&J=128&INFO_FORMAT=text/html")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "WMS GetFeatureInfo (HTML) 应返回 200, 实际: {}",
        resp.status()
    );

    let content_type = resp
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(
        content_type.contains("html"),
        "Content-Type 应为 text/html, 实际: {}",
        content_type
    );
}

#[actix_rt::test]
async fn test_wms_get_feature_info_plain() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/wms?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetFeatureInfo&LAYERS=world&QUERY_LAYERS=world&BBOX=-180,-90,180,90&WIDTH=256&HEIGHT=256&I=128&J=128&INFO_FORMAT=text/plain")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "WMS GetFeatureInfo (text/plain) 应返回 200, 实际: {}",
        resp.status()
    );

    let content_type = resp
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(
        content_type.contains("text/plain"),
        "Content-Type 应为 text/plain, 实际: {}",
        content_type
    );
}

// ---------------------------------------------------------------------------
// Batch 15: WMS-C 1.1.1 (GeoWebCache) — GetCapabilities + TILED=true GetMap
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn test_wmsc_get_capabilities() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/geoserver/gwc/service/wms?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetCapabilities")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "WMS-C GetCapabilities 应返回 200, 实际: {}",
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

    let bytes = test::read_body(resp).await;
    let body = String::from_utf8_lossy(&bytes);
    assert!(
        body.contains("<WMT_MS_Capabilities version=\"1.1.1\">"),
        "应包含 WMS 1.1.1 能力根元素"
    );
    assert!(body.contains("<Name>OGC:WMS</Name>"));
    assert!(body.contains("GeoWebCache"));
    assert!(body.contains("<GetMap>"));
    assert!(body.contains("<Name>world</Name>"));
    assert!(body.contains("<SRS>EPSG:4326</SRS>"));
    assert!(body.contains("<LatLonBoundingBox"));
}

#[actix_rt::test]
async fn test_wmsc_get_map_tiled() {
    let app = build_test_app!();

    // TILED=true + 网格对齐 BBOX (global-geodetic z=0 左半: -180,-90,0,90)
    let req = test::TestRequest::get()
        .uri("/geoserver/gwc/service/wms?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetMap&LAYERS=world&STYLES=&SRS=EPSG:4326&BBOX=-180,-90,0,90&WIDTH=256&HEIGHT=256&FORMAT=image/png&TILED=true")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "WMS-C TILED=true GetMap 应返回 200, 实际: {}",
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
}

#[actix_rt::test]
async fn test_wmsc_get_map_tiled_mercator() {
    let app = build_test_app!();

    // TILED=true on the mercator gridset: 整世界 z=0 (1x1).
    let req = test::TestRequest::get()
        .uri("/geoserver/gwc/service/wms?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetMap&LAYERS=world&STYLES=&SRS=EPSG:900913&BBOX=-20037508.34,-20037508.34,20037508.34,20037508.34&WIDTH=256&HEIGHT=256&FORMAT=image/png&TILED=true")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "WMS-C TILED=true (mercator) 应返回 200, 实际: {}",
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
}

#[actix_rt::test]
async fn test_wmsc_get_map_untiled() {
    let app = build_test_app!();

    // 无 TILED 参数 → 走标准 WMS 1.1.1 GetMap 管线.
    let req = test::TestRequest::get()
        .uri("/geoserver/gwc/service/wms?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetMap&LAYERS=world&STYLES=&SRS=EPSG:4326&BBOX=-180,-90,180,90&WIDTH=256&HEIGHT=256&FORMAT=image/png")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "WMS-C 普通 GetMap 应返回 200, 实际: {}",
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
}

// ---------------------------------------------------------------------------
// Batch 6: 级联 WMS 在线代理 (需要参考 GeoServer http://127.0.0.1:18080)
// ---------------------------------------------------------------------------

#[actix_rt::test]
#[ignore = "requires the reference GeoServer at http://127.0.0.1:18080"]
async fn test_wms_cascaded_live() {
    use actix_web::http::StatusCode;

    let app = build_test_app!();

    // 1. 创建级联 WMS 数据源, 指向参考 GeoServer 的 WMS 端点 (sf:archsites 点图层)
    let create_ds = test::TestRequest::post()
        .uri("/geoserver/data-sources")
        .set_json(serde_json::json!({
            "name": "casc1",
            "type": "cascaded_wms",
            "workspace": "default",
            "enabled": true,
            "connection": {
                "host": "127.0.0.1",
                "port": 18080,
                "database": "/geoserver/wms",
                "schema": "sf:archsites"
            },
        }))
        .to_request();
    let resp = test::call_service(&app, create_ds).await;
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "创建级联 WMS 数据源应返回 201, 实际: {}",
        resp.status()
    );

    // 2. 创建图层引用该级联数据源
    let create_layer = test::TestRequest::post()
        .uri("/geoserver/layers")
        .set_json(serde_json::json!({
            "name": "casc_layer",
            "title": "Cascaded Layer",
            "workspace": "default",
            "store": "casc1",
            "native_name": "sf:archsites",
            "srs": "EPSG:4326",
            "minx": -110.0, "miny": 20.0, "maxx": -80.0, "maxy": 50.0,
        }))
        .to_request();
    let resp = test::call_service(&app, create_layer).await;
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "创建图层应返回 201, 实际: {}",
        resp.status()
    );

    // 3. WMS GetMap → 应代理到上游参考 GeoServer 并返回 PNG
    let req = test::TestRequest::get()
        .uri("/wms?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetMap&LAYERS=casc_layer&BBOX=-110,20,-80,50&WIDTH=256&HEIGHT=256&SRS=EPSG:4326&FORMAT=image/png")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "级联 WMS GetMap 应返回 200, 实际: {}",
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
        "级联 GetMap 应返回 PNG, 实际: {}",
        content_type
    );

    let body = test::read_body(resp).await;
    let decoded = image::load_from_memory(&body).expect("应能解码级联返回的 PNG");
    assert_eq!(decoded.width(), 256, "级联返回瓦片应为 256x256");
    assert_eq!(decoded.height(), 256);
}

// ---------------------------------------------------------------------------
// Batch 11: 级联 WMS 厂商参数 (CQL_FILTER / TIME) 透传到上游
// ---------------------------------------------------------------------------

#[actix_rt::test]
#[ignore = "requires the reference GeoServer at http://127.0.0.1:18080"]
async fn test_wms_cascaded_vendor_params_live() {
    use actix_web::http::StatusCode;

    let app = build_test_app!();

    // 1. 创建级联 WMS 数据源 (参考 GeoServer sf:archsites, 原生 CRS EPSG:26713)
    let create_ds = test::TestRequest::post()
        .uri("/geoserver/data-sources")
        .set_json(serde_json::json!({
            "name": "casc_vendor",
            "type": "cascaded_wms",
            "workspace": "default",
            "enabled": true,
            "connection": {
                "host": "127.0.0.1",
                "port": 18080,
                "database": "/geoserver/wms",
                "schema": "sf:archsites"
            },
        }))
        .to_request();
    let resp = test::call_service(&app, create_ds).await;
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "创建级联数据源应返回 201"
    );

    let create_layer = test::TestRequest::post()
        .uri("/geoserver/layers")
        .set_json(serde_json::json!({
            "name": "casc_vendor_layer",
            "title": "Cascaded Vendor Layer",
            "workspace": "default",
            "store": "casc_vendor",
            "native_name": "sf:archsites",
            "srs": "EPSG:26713",
            "minx": 590000.0, "miny": 4910000.0, "maxx": 610000.0, "maxy": 4930000.0,
        }))
        .to_request();
    let resp = test::call_service(&app, create_layer).await;
    assert_eq!(resp.status(), StatusCode::CREATED, "创建图层应返回 201");

    let base = "/wms?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetMap&LAYERS=casc_vendor_layer&BBOX=590000,4910000,610000,4930000&SRS=EPSG:26713&WIDTH=256&HEIGHT=256&FORMAT=image/png";

    // 2. 有效 CQL_FILTER 透传 → 上游应用过滤并返回 PNG
    let req = test::TestRequest::get()
        .uri(&format!(
            "{}&CQL_FILTER=str1%3D%27Signature%20Rock%27",
            base
        ))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "CQL_FILTER 透传应返回 200, 实际: {}",
        resp.status()
    );
    let ct = resp
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(
        ct.contains("image/png"),
        "有效 CQL_FILTER 应返回 PNG, 实际: {}",
        ct
    );

    // 3. 无效 CQL_FILTER 透传 → 上游返回 OGC 异常 (非 PNG), 证明参数确实到达上游
    let req = test::TestRequest::get()
        .uri(&format!("{}&CQL_FILTER=INVALID%20SYNTAX%20BROKEN", base))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "无效 CQL_FILTER 透传应仍返回 200 (OGC 异常), 实际: {}",
        resp.status()
    );
    let ct = resp
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(
        !ct.contains("image/png"),
        "无效 CQL_FILTER 应返回上游异常 (非 PNG), 证明透传生效, 实际: {}",
        ct
    );

    // 4. TIME 透传 → 上游 (无时间维度图层) 忽略并返回 PNG
    let req = test::TestRequest::get()
        .uri(&format!("{}&TIME=2024-01-01", base))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "TIME 透传应返回 200, 实际: {}",
        resp.status()
    );
    let ct = resp
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(
        ct.contains("image/png"),
        "TIME 透传应返回 PNG, 实际: {}",
        ct
    );
}
