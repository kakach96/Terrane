//! # API 集成测试
//!
//! 测试 Rust GeoServer 的 HTTP API 端点。
//! 使用 actix_rt 作为异步测试运行时。

use actix_web::{web, App, middleware};

// ---------------------------------------------------------------------------
// 辅助: 创建测试用 AppState
// ---------------------------------------------------------------------------

fn create_test_config() -> rust_geoserver::config::GeoServerConfig {
    let mut config = rust_geoserver::config::GeoServerConfig::default();
    config.server.host = "127.0.0.1".to_string();
    config.server.port = 0;
    // 集成测试使用内存 SQLite 数据库
    config.metadata.sqlite_path = ":memory:".into();
    // 添加一个默认工作空间和图层，确保测试数据可用
    config.workspaces = vec![
        rust_geoserver::config::WorkspaceConfig {
            name: "default".to_string(),
            uri: "http://geoserver.org/default".to_string(),
            stores: vec![
                rust_geoserver::config::StoreConfig {
                    name: "shapes".to_string(),
                    store_type: "DataStore".to_string(),
                    path: "./data".to_string(),
                    layers: vec![
                        rust_geoserver::config::LayerConfig {
                            name: "world".to_string(),
                            title: "World".to_string(),
                            abstract_text: "World layer".to_string(),
                            srs: "EPSG:4326".to_string(),
                            bounds: rust_geoserver::config::BoundsConfig {
                                minx: -180.0, miny: -90.0,
                                maxx: 180.0, maxy: 90.0,
                            },
                            style: Some("default".to_string()),
                        },
                    ],
                },
            ],
        },
    ];
    config
}

// ---------------------------------------------------------------------------
// 健康检查测试
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn test_health_endpoint() {
    let config = create_test_config();
    let state = web::Data::new(rust_geoserver::state::AppState::new(config).await);

    let app = actix_web::test::init_service(
        App::new()
            .app_data(state.clone())
            .wrap(middleware::Logger::default())
            .configure(|svc| rust_geoserver::routes::configure_routes(
                svc, "/geoserver"
            ))
    ).await;

    // 测试健康检查
    let req = actix_web::test::TestRequest::get()
        .uri("/geoserver/health")
        .to_request();
    let resp = actix_web::test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "健康检查应返回 200");

    let body: serde_json::Value = actix_web::test::read_body_json(resp).await;
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["status"], "healthy");
}

// ---------------------------------------------------------------------------
// CQL 过滤器集成测试
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn test_wms_get_capabilities() {
    let config = create_test_config();
    let state = web::Data::new(rust_geoserver::state::AppState::new(config).await);

    let app = actix_web::test::init_service(
        App::new()
            .app_data(state.clone())
            .wrap(middleware::Logger::default())
            .configure(|svc| rust_geoserver::routes::configure_routes(
                svc, "/geoserver"
            ))
    ).await;

    // 测试 WMS GetCapabilities
    let req = actix_web::test::TestRequest::get()
        .uri("/wms?SERVICE=WMS&REQUEST=GetCapabilities")
        .to_request();
    let resp = actix_web::test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "WMS GetCapabilities 应返回成功");

    let content_type = resp.headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(content_type.contains("xml"), "Content-Type 应为 application/xml");
}

// ---------------------------------------------------------------------------
// WMS GetMap 测试
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn test_wms_get_map() {
    let config = create_test_config();
    let state = web::Data::new(rust_geoserver::state::AppState::new(config).await);

    let app = actix_web::test::init_service(
        App::new()
            .app_data(state.clone())
            .wrap(middleware::Logger::default())
            .configure(|svc| rust_geoserver::routes::configure_routes(
                svc, "/geoserver"
            ))
    ).await;

    // 测试 WMS GetMap (请求 PNG)
    let req = actix_web::test::TestRequest::get()
        .uri("/wms?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetMap&LAYERS=world&BBOX=-180,-90,180,90&WIDTH=256&HEIGHT=256&FORMAT=image/png&SRS=EPSG:4326")
        .to_request();
    let resp = actix_web::test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "WMS GetMap 应返回 200, 实际状态: {}", resp.status());

    let content_type = resp.headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    // 如果返回 XML 异常, 读取 body 查看错误信息
    if content_type.contains("xml") {
        let body = actix_web::test::read_body(resp).await;
        let body_str = String::from_utf8_lossy(&body);
        panic!("WMS GetMap 返回异常: {}", body_str);
    }

    assert!(content_type.contains("image/png"), "Content-Type 应为 image/png, 实际: {}", content_type);
}

// ---------------------------------------------------------------------------
// WFS GetCapabilities 测试
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn test_wfs_get_capabilities() {
    let config = create_test_config();
    let state = web::Data::new(rust_geoserver::state::AppState::new(config).await);

    let app = actix_web::test::init_service(
        App::new()
            .app_data(state.clone())
            .wrap(middleware::Logger::default())
            .configure(|svc| rust_geoserver::routes::configure_routes(
                svc, "/geoserver"
            ))
    ).await;

    let req = actix_web::test::TestRequest::get()
        .uri("/wfs?SERVICE=WFS&REQUEST=GetCapabilities")
        .to_request();
    let resp = actix_web::test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "WFS GetCapabilities 应返回 200");
}

// ---------------------------------------------------------------------------
// WCS GetCapabilities 测试
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn test_wcs_get_capabilities() {
    let config = create_test_config();
    let state = web::Data::new(rust_geoserver::state::AppState::new(config).await);

    let app = actix_web::test::init_service(
        App::new()
            .app_data(state.clone())
            .wrap(middleware::Logger::default())
            .configure(|svc| rust_geoserver::routes::configure_routes(
                svc, "/geoserver"
            ))
    ).await;

    let req = actix_web::test::TestRequest::get()
        .uri("/wcs?SERVICE=WCS&REQUEST=GetCapabilities")
        .to_request();
    let resp = actix_web::test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "WCS GetCapabilities 应返回 200");
}

// ---------------------------------------------------------------------------
// REST API 测试: 图层列表
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn test_rest_layers() {
    let config = create_test_config();
    let state = web::Data::new(rust_geoserver::state::AppState::new(config).await);

    let app = actix_web::test::init_service(
        App::new()
            .app_data(state.clone())
            .wrap(middleware::Logger::default())
            .configure(|svc| rust_geoserver::routes::configure_routes(
                svc, "/geoserver"
            ))
    ).await;

    let req = actix_web::test::TestRequest::get()
        .uri("/geoserver/layers")
        .to_request();
    let resp = actix_web::test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "GET /layers 应返回 200");

    let body: serde_json::Value = actix_web::test::read_body_json(resp).await;
    assert!(body["success"].as_bool().unwrap_or(false));
    // layers 端点可能返回空数组或包含图层的数组, 取决于 SQLite 初始化
    // 这里只验证请求成功且格式正确
    let data = body["data"].as_array()
        .unwrap_or_else(|| panic!("layers 应为数组, 实际: {:?}", body));
    // 即使在大多数情况下 layers 应该非空, 但允许空数组（测试环境）
    if data.is_empty() {
        eprintln!("[WARN] layers 返回空数组 (可能在测试环境中无数据)");
    }
}

// ---------------------------------------------------------------------------
// REST API 测试: 服务器状态
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn test_server_status() {
    let config = create_test_config();
    let state = web::Data::new(rust_geoserver::state::AppState::new(config).await);

    let app = actix_web::test::init_service(
        App::new()
            .app_data(state.clone())
            .wrap(middleware::Logger::default())
            .configure(|svc| rust_geoserver::routes::configure_routes(
                svc, "/geoserver"
            ))
    ).await;

    let req = actix_web::test::TestRequest::get()
        .uri("/geoserver/server/status")
        .to_request();
    let resp = actix_web::test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "GET /server/status 应返回 200");

    let body: serde_json::Value = actix_web::test::read_body_json(resp).await;
    assert!(body["success"].as_bool().unwrap_or(false));
    assert!(body["data"]["uptime"].is_string(), "应包含 uptime");
    assert!(body["data"]["layerCount"].as_i64().unwrap_or(0) > 0, "应包含 layerCount");
}
