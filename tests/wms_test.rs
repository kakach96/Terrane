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
        .set_json(&serde_json::json!({
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
        .set_json(&serde_json::json!({
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
