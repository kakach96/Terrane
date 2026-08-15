//! REST API + probes + MVT integration tests.
//!
//! Covers: health / split probes / metrics, server status, layers, and CRUD for
//! workspaces / namespaces / styles / layer-groups / features / sql-views /
//! data-sources, plus the MVT vector-tile endpoint.

#[macro_use]
mod common;

use actix_web::http::StatusCode;
use actix_web::test;

#[actix_rt::test]
async fn test_health_endpoint() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/geoserver/health")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "健康检查应返回 200");

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["status"], "healthy");
}

#[actix_rt::test]
async fn test_probes_and_metrics() {
    let app = build_test_app!();

    for path in ["/health/live", "/health/ready", "/metrics"] {
        let req = test::TestRequest::get().uri(path).to_request();
        let resp = test::call_service(&app, req).await;
        assert!(
            resp.status().is_success(),
            "{} 应返回 200, 实际: {}",
            path,
            resp.status()
        );
    }
}

#[actix_rt::test]
async fn test_server_status() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/geoserver/server/status")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "GET /server/status 应返回 200");

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body["success"].as_bool().unwrap_or(false));
    assert!(body["data"]["uptime"].is_string(), "应包含 uptime");
    assert!(
        body["data"]["layerCount"].as_i64().unwrap_or(0) > 0,
        "应包含 layerCount"
    );
}

#[actix_rt::test]
async fn test_rest_layers() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/geoserver/layers")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "GET /layers 应返回 200");

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body["success"].as_bool().unwrap_or(false));
    assert!(body["data"].is_array(), "layers 应为数组, 实际: {:?}", body);
}

// ---------------------------------------------------------------------------
// CRUD: Workspaces
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn test_rest_workspaces_crud() {
    let app = build_test_app!();

    let create = test::TestRequest::post()
        .uri("/geoserver/workspaces")
        .set_json(serde_json::json!({
            "name": "ws_test_1",
            "title": "Test Workspace",
            "description": "created by integration test",
        }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert_eq!(resp.status(), StatusCode::CREATED, "创建工作空间应返回 201");

    let req = test::TestRequest::get()
        .uri("/geoserver/workspaces")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    let names: Vec<String> = body["data"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|w| w["name"].as_str().map(|s| s.to_string()))
        .collect();
    assert!(
        names.contains(&"ws_test_1".to_string()),
        "工作空间列表应包含 ws_test_1, 实际: {:?}",
        names
    );

    let req = test::TestRequest::delete()
        .uri("/geoserver/workspaces/ws_test_1")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "删除工作空间应返回 2xx, 实际: {}",
        resp.status()
    );

    let req = test::TestRequest::get()
        .uri("/geoserver/workspaces/ws_test_1")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND, "删除后查询应返回 404");
}

// ---------------------------------------------------------------------------
// CRUD: Namespaces
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn test_rest_namespaces_crud() {
    let app = build_test_app!();

    let create = test::TestRequest::post()
        .uri("/geoserver/namespaces")
        .set_json(serde_json::json!({
            "prefix": "ns_test_1",
            "uri": "http://example.com/ns_test_1",
        }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert_eq!(resp.status(), StatusCode::CREATED, "创建命名空间应返回 201");

    let req = test::TestRequest::get()
        .uri("/geoserver/namespaces")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    let prefixes: Vec<String> = body["data"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|n| n["prefix"].as_str().map(|s| s.to_string()))
        .collect();
    assert!(
        prefixes.contains(&"ns_test_1".to_string()),
        "命名空间列表应包含 ns_test_1, 实际: {:?}",
        prefixes
    );

    let req = test::TestRequest::delete()
        .uri("/geoserver/namespaces/ns_test_1")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "删除命名空间应返回 2xx, 实际: {}",
        resp.status()
    );
}

// ---------------------------------------------------------------------------
// CRUD: Styles
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn test_rest_styles_crud() {
    let app = build_test_app!();

    let create = test::TestRequest::post()
        .uri("/geoserver/styles")
        .set_json(serde_json::json!({
            "name": "style_test_1",
            "title": "Test Style",
            "content": "<StyledLayerDescriptor version=\"1.0.0\"></StyledLayerDescriptor>",
            "format": "SLD",
        }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert_eq!(resp.status(), StatusCode::CREATED, "创建样式应返回 201");

    let req = test::TestRequest::get()
        .uri("/geoserver/styles")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    let names: Vec<String> = body["data"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|s| s["name"].as_str().map(|v| v.to_string()))
        .collect();
    assert!(
        names.contains(&"style_test_1".to_string()),
        "样式列表应包含 style_test_1, 实际: {:?}",
        names
    );
}

// ---------------------------------------------------------------------------
// CRUD: Layer groups
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn test_rest_layer_groups_crud() {
    let app = build_test_app!();

    let create = test::TestRequest::post()
        .uri("/geoserver/layer-groups")
        .set_json(serde_json::json!({
            "name": "lg_test_1",
            "title": "Test Layer Group",
            "layers": ["world"],
        }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert_eq!(resp.status(), StatusCode::CREATED, "创建图层组应返回 201");

    let req = test::TestRequest::get()
        .uri("/geoserver/layer-groups")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    let names: Vec<String> = body["data"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|g| g["name"].as_str().map(|v| v.to_string()))
        .collect();
    assert!(
        names.contains(&"lg_test_1".to_string()),
        "图层组列表应包含 lg_test_1, 实际: {:?}",
        names
    );
}

// ---------------------------------------------------------------------------
// Features: read-only browsing (数据发布平台, 无要素写入接口)
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn test_rest_features_readonly() {
    let app = build_test_app!();

    // GET 读取要素 (world 图层空发布 → 200 空集合)
    let req = test::TestRequest::get()
        .uri("/geoserver/layers/world/features")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "读取图层要素应返回 200, 实际: {}",
        resp.status()
    );

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(
        body["totalFeatures"].as_i64().unwrap_or(-1),
        0,
        "world 图层空发布, 应返回 0 条要素"
    );
}

#[actix_rt::test]
async fn test_rest_feature_write_not_supported() {
    let app = build_test_app!();

    // POST 创建要素 → 405 (要素写入接口尚未实现)
    let create = test::TestRequest::post()
        .uri("/geoserver/layers/world/features")
        .set_json(serde_json::json!({
            "geometry": { "type": "Point", "coordinates": [10.0, 20.0] },
            "properties": { "name": "integration-test" },
        }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert!(
        !resp.status().is_success(),
        "要素写入接口已移除, POST 不应成功, 实际: {}",
        resp.status()
    );
}

// ---------------------------------------------------------------------------
// CRUD: SQL views
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn test_rest_sql_views_crud() {
    let app = build_test_app!();

    let create = test::TestRequest::post()
        .uri("/geoserver/sql-views")
        .set_json(serde_json::json!({
            "name": "sqlview_test_1",
            "sql": "SELECT id, geom FROM cities",
            "workspace": "default",
            "store": "shapes",
            "geometry_column": "geom",
            "geometry_type": "Point",
            "crs": "EPSG:4326",
        }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "创建 SQL 视图应返回 201"
    );

    let req = test::TestRequest::get()
        .uri("/geoserver/sql-views")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    let names: Vec<String> = body["data"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|v| v["name"].as_str().map(|s| s.to_string()))
        .collect();
    assert!(
        names.contains(&"sqlview_test_1".to_string()),
        "SQL 视图列表应包含 sqlview_test_1, 实际: {:?}",
        names
    );
}

// ---------------------------------------------------------------------------
// CRUD: Data sources
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn test_rest_data_sources_crud() {
    let app = build_test_app!();

    let create = test::TestRequest::post()
        .uri("/geoserver/data-sources")
        .set_json(serde_json::json!({
            "name": "ds_test_1",
            "type": "shapefile",
            "workspace": "default",
            "enabled": true,
            "connection": { "file_path": "./data/test.shp", "file_storage_type": "local" },
        }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert_eq!(resp.status(), StatusCode::CREATED, "创建数据源应返回 201");

    let req = test::TestRequest::get()
        .uri("/geoserver/data-sources")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    let names: Vec<String> = body["data"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|d| d["name"].as_str().map(|s| s.to_string()))
        .collect();
    assert!(
        names.contains(&"ds_test_1".to_string()),
        "数据源列表应包含 ds_test_1, 实际: {:?}",
        names
    );
}

#[actix_rt::test]
async fn test_rest_image_pyramid_data_source_crud() {
    let app = build_test_app!();

    // 建一个含数字层级子目录的临时金字塔目录。
    let dir = std::env::temp_dir().join(format!("terrane-pyramid-rest-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("0")).unwrap();
    std::fs::write(dir.join("0").join("tile.tif"), b"fake").unwrap();
    std::fs::write(dir.join("properties"), "levels=1\n").unwrap();

    let create = test::TestRequest::post()
        .uri("/geoserver/data-sources")
        .set_json(serde_json::json!({
            "name": "pyr_test_1",
            "type": "image_pyramid",
            "workspace": "default",
            "enabled": true,
            "connection": {
                "file_path": dir.to_string_lossy().replace('\\', "/"),
                "file_storage_type": "local",
            },
        }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "创建 ImagePyramid 数据源应返回 201"
    );

    // 列表应包含且类型持久化为 image_pyramid。
    let req = test::TestRequest::get()
        .uri("/geoserver/data-sources")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    let found = body["data"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .find(|d| d["name"] == "pyr_test_1");
    let found = found.expect("ImagePyramid 数据源应存在");
    assert_eq!(
        found["type"], "image_pyramid",
        "类型应持久化为 image_pyramid"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[actix_rt::test]
async fn test_rest_mysql_data_source_crud() {
    let app = build_test_app!();

    let create = test::TestRequest::post()
        .uri("/geoserver/data-sources")
        .set_json(serde_json::json!({
            "name": "mysql_test_1",
            "type": "mysql",
            "workspace": "default",
            "enabled": true,
            "connection": {
                "host": "127.0.0.1",
                "port": 3306,
                "database": "geodb",
                "username": "root",
                "password": "secret",
            },
        }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "创建 MySQL 数据源应返回 201"
    );

    // 列表应包含且类型持久化为 mysql。
    let req = test::TestRequest::get()
        .uri("/geoserver/data-sources")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    let found = body["data"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .find(|d| d["name"] == "mysql_test_1");
    let found = found.expect("MySQL 数据源应存在");
    assert_eq!(found["type"], "mysql", "类型应持久化为 mysql");
    assert_eq!(found["connection"]["port"], 3306);
}

#[actix_rt::test]
async fn test_rest_mongo_data_source_crud() {
    let app = build_test_app!();

    let create = test::TestRequest::post()
        .uri("/geoserver/data-sources")
        .set_json(serde_json::json!({
            "name": "mongo_test_1",
            "type": "mongo",
            "workspace": "default",
            "enabled": true,
            "connection": {
                "host": "127.0.0.1",
                "port": 27017,
                "database": "geodb",
                "username": "admin",
                "password": "secret",
            },
        }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "创建 MongoDB 数据源应返回 201"
    );

    // 列表应包含且类型持久化为 mongo。
    let req = test::TestRequest::get()
        .uri("/geoserver/data-sources")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    let found = body["data"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .find(|d| d["name"] == "mongo_test_1");
    let found = found.expect("MongoDB 数据源应存在");
    assert_eq!(found["type"], "mongo", "类型应持久化为 mongo");
    assert_eq!(found["connection"]["port"], 27017);
}

#[actix_rt::test]
async fn test_browse_local_directory() {
    let app = build_test_app!();

    // 建一个临时目录, 内含子目录与文件
    let dir = std::env::temp_dir().join(format!("terrane-browse-{}", std::process::id()));
    let sub = dir.join("subdir");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(dir.join("a.geojson"), b"{}").unwrap();
    std::fs::write(sub.join("b.txt"), b"hi").unwrap();

    // Windows 路径用正斜杠编码进查询串
    let url = format!(
        "/geoserver/data-sources/browse?path={}",
        dir.to_string_lossy().replace('\\', "/")
    );
    let req = test::TestRequest::get().uri(&url).to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "本地浏览应返回 200");

    let body: serde_json::Value = test::read_body_json(resp).await;
    let entries = body["data"]["entries"].as_array().unwrap();
    // 目录优先排序, 应同时包含子目录与文件
    assert!(
        entries
            .iter()
            .any(|e| e["name"] == "subdir" && e["is_dir"].as_bool() == Some(true)),
        "应包含子目录 subdir"
    );
    assert!(
        entries
            .iter()
            .any(|e| e["name"] == "a.geojson" && e["is_dir"].as_bool() == Some(false)),
        "应包含文件 a.geojson"
    );

    // 子目录浏览
    let url2 = format!(
        "/geoserver/data-sources/browse?path={}",
        sub.to_string_lossy().replace('\\', "/")
    );
    let req2 = test::TestRequest::get().uri(&url2).to_request();
    let resp2 = test::call_service(&app, req2).await;
    assert!(resp2.status().is_success(), "子目录浏览应返回 200");
    let body2: serde_json::Value = test::read_body_json(resp2).await;
    let entries2 = body2["data"]["entries"].as_array().unwrap();
    assert!(
        entries2.iter().any(|e| e["name"] == "b.txt"),
        "子目录应包含 b.txt"
    );

    std::fs::remove_dir_all(&dir).ok();
}

// ---------------------------------------------------------------------------
// MVT vector tiles
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn test_mvt_endpoint() {
    let app = build_test_app!();

    // /mvt/ 专用路由 (与 .pbf 路由等价; .pbf 遮蔽已修复, 见 test_rest_mvt_pbf_route)
    let req = test::TestRequest::get()
        .uri("/geoserver/mvt/world/0/0/0")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "MVT 瓦片应返回 200, 实际: {}",
        resp.status()
    );

    let content_type = resp
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(
        content_type.contains("mapbox-vector-tile"),
        "Content-Type 应为 MVT, 实际: {}",
        content_type
    );
}

// ---------------------------------------------------------------------------
// Batch 3: 单要素只读查询 + 认证 + 权限 + 备份 + 上传
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn test_rest_feature_readonly() {
    let app = build_test_app!();

    // world 图层空发布 (无数据源), GET 单要素应返回 404 (要素不存在)
    let req = test::TestRequest::get()
        .uri("/geoserver/layers/world/features/nonexistent")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success() || resp.status() == StatusCode::NOT_FOUND,
        "GET 单要素应返回 200 或 404, 实际: {}",
        resp.status()
    );
}

// ---------------------------------------------------------------------------
// Batch 3: 认证 (登录 / 验证 / 用户管理)
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn test_rest_auth_login_and_verify() {
    let app = build_test_app!();

    // 默认管理员登录
    let login = test::TestRequest::post()
        .uri("/geoserver/auth/login")
        .set_json(serde_json::json!({ "username": "admin", "password": "geoserver" }))
        .to_request();
    let resp = test::call_service(&app, login).await;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "admin 登录应返回 200, 实际: {}",
        resp.status()
    );
    let body: serde_json::Value = test::read_body_json(resp).await;
    let token = body["data"]["token"]
        .as_str()
        .expect("应返回 token")
        .to_string();
    assert_eq!(body["data"]["username"], "admin");
    assert_eq!(body["data"]["role"], "admin");

    // 用 token 验证身份
    let verify = test::TestRequest::get()
        .uri("/geoserver/auth/verify")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();
    let resp = test::call_service(&app, verify).await;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "verify 应返回 200, 实际: {}",
        resp.status()
    );
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["username"], "admin");

    // 错误密码 → 400
    let bad = test::TestRequest::post()
        .uri("/geoserver/auth/login")
        .set_json(serde_json::json!({ "username": "admin", "password": "wrong" }))
        .to_request();
    let resp = test::call_service(&app, bad).await;
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "错误密码应返回 400, 实际: {}",
        resp.status()
    );
}

#[actix_rt::test]
async fn test_rest_auth_users_crud() {
    let app = build_test_app!();
    let token = login_admin_token!(app);

    // 创建用户
    let create = test::TestRequest::post()
        .uri("/geoserver/auth/users")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(
            serde_json::json!({ "username": "tester1", "password": "secret123", "role": "guest" }),
        )
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "创建用户应返回 201, 实际: {}",
        resp.status()
    );

    // 列出用户 → 包含 tester1
    let list = test::TestRequest::get()
        .uri("/geoserver/auth/users")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();
    let resp = test::call_service(&app, list).await;
    assert!(
        resp.status().is_success(),
        "列出用户应返回 200, 实际: {}",
        resp.status()
    );
    let body: serde_json::Value = test::read_body_json(resp).await;
    let names: Vec<String> = body["data"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|u| u["username"].as_str().map(|s| s.to_string()))
        .collect();
    assert!(
        names.contains(&"tester1".to_string()),
        "用户列表应包含 tester1, 实际: {:?}",
        names
    );

    // 新用户可登录
    let login2 = test::TestRequest::post()
        .uri("/geoserver/auth/login")
        .set_json(serde_json::json!({ "username": "tester1", "password": "secret123" }))
        .to_request();
    let resp = test::call_service(&app, login2).await;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "新用户应能登录, 实际: {}",
        resp.status()
    );

    // 删除用户
    let del = test::TestRequest::delete()
        .uri("/geoserver/auth/users/tester1")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();
    let resp = test::call_service(&app, del).await;
    assert!(
        resp.status().is_success(),
        "删除用户应返回 200, 实际: {}",
        resp.status()
    );
}

// ---------------------------------------------------------------------------
// Batch 3: 权限管理
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn test_rest_permissions_crud() {
    let app = build_test_app!();
    let token = login_admin_token!(app);

    // 创建权限 (layer/world 只读)
    let create = test::TestRequest::post()
        .uri("/geoserver/permissions")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(serde_json::json!({
            "username": "admin",
            "resource_type": "layer",
            "resource_name": "world",
            "access_mode": "read",
            "effect": "allow",
        }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "创建权限应返回 201, 实际: {}",
        resp.status()
    );
    let body: serde_json::Value = test::read_body_json(resp).await;
    let perm_id = body["data"]["id"].as_i64().unwrap_or(0);

    // 列出权限 → 包含该资源
    let list = test::TestRequest::get()
        .uri("/geoserver/permissions")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();
    let resp = test::call_service(&app, list).await;
    assert!(
        resp.status().is_success(),
        "列出权限应返回 200, 实际: {}",
        resp.status()
    );
    let body: serde_json::Value = test::read_body_json(resp).await;
    let perms = body["data"].as_array().cloned().unwrap_or_default();
    assert!(
        perms
            .iter()
            .any(|p| p["resourceType"] == "layer" && p["resourceName"] == "world"),
        "权限列表应包含 layer/world, 实际: {:?}",
        perms
    );

    // 删除权限
    let del = test::TestRequest::delete()
        .uri(&format!("/geoserver/permissions/{}", perm_id))
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();
    let resp = test::call_service(&app, del).await;
    assert!(
        resp.status().is_success(),
        "删除权限应返回 200, 实际: {}",
        resp.status()
    );
}

// ---------------------------------------------------------------------------
// Batch 3: 备份导出 / 上传 GeoJSON
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn test_rest_backup_export() {
    let app = build_test_app!();
    let token = login_admin_token!(app);

    let req = test::TestRequest::get()
        .uri("/geoserver/backup/export")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "备份导出应返回 200, 实际: {}",
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
        "备份应导出为 JSON, 实际: {}",
        content_type
    );

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(
        body["success"].as_bool().unwrap_or(false),
        "备份导出应成功, 实际: {:?}",
        body
    );
}

#[actix_rt::test]
async fn test_rest_upload_geojson() {
    let app = build_test_app!();

    // 注意: FeatureCollection 需要 total_count, Feature 需要 id (模型无默认值)
    let payload = serde_json::json!({
        "type": "FeatureCollection",
        "total_count": 1,
        "features": [
            {
                "id": "uploaded-1",
                "type": "Feature",
                "geometry": { "type": "Point", "coordinates": [12.0, 34.0] },
                "properties": { "name": "uploaded-feature", "layer_name": "uploaded_layer" }
            }
        ]
    });
    let req = test::TestRequest::post()
        .uri("/geoserver/data/upload")
        .set_json(&payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "上传 GeoJSON 应返回 201, 实际: {}",
        resp.status()
    );

    // 上传登记为 geojson 文件数据源 (文件数据源登记, 数据发布平台只读)
    let req = test::TestRequest::get()
        .uri("/geoserver/data-sources/uploaded_layer")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "数据源应已创建, 实际: {}",
        resp.status()
    );
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["type"], "geojson", "应为 geojson 数据源");
    assert_eq!(
        body["data"]["connection"]["file_storage_type"], "local",
        "应为本地存储"
    );

    // 发布图层 (store = 数据源名) 后可查询上传的要素
    let create = test::TestRequest::post()
        .uri("/geoserver/layers")
        .set_json(serde_json::json!({
            "name": "uploaded_layer",
            "title": "Uploaded",
            "workspace": "default",
            "store": "uploaded_layer",
            "native_name": "uploaded_layer",
            "srs": "EPSG:4326",
            "minx": -180.0, "miny": -90.0, "maxx": 180.0, "maxy": 90.0,
        }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "创建图层应返回 201, 实际: {}",
        resp.status()
    );

    let req = test::TestRequest::get()
        .uri("/geoserver/layers/uploaded_layer/features")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "查询上传图层应返回 200, 实际: {}",
        resp.status()
    );
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(
        body["totalFeatures"].as_i64().unwrap_or(0) >= 1,
        "上传图层应有要素, 实际: {:?}",
        body
    );
}

// ---------------------------------------------------------------------------
// Batch 5: /tiles 瓦片端点 + 瓦片缓存 clear/stats + 备份导入往返
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn test_rest_tiles_endpoint() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/geoserver/tiles/world/0/0/0")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "/tiles/world/0/0/0 应返回 200, 实际: {}",
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
        "瓦片 Content-Type 应为 image/png, 实际: {}",
        content_type
    );

    let body = test::read_body(resp).await;
    let decoded = image::load_from_memory(&body).expect("应能解码瓦片 PNG");
    assert_eq!(decoded.width(), 256, "瓦片应为 256x256");
    assert_eq!(decoded.height(), 256);
}

#[actix_rt::test]
async fn test_rest_tile_cache_stats_and_clear() {
    let app = build_test_app!();

    // 缓存统计
    let req = test::TestRequest::get()
        .uri("/geoserver/tiles/cache/stats")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "/tiles/cache/stats 应返回 200, 实际: {}",
        resp.status()
    );
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(
        body["success"].as_bool().unwrap_or(false),
        "stats 应成功, 实际: {:?}",
        body
    );
    assert!(
        body["data"].is_object(),
        "stats 应返回对象, 实际: {:?}",
        body
    );
    assert!(
        body["data"]["hits"].is_number(),
        "stats 应含 hits, 实际: {:?}",
        body
    );
    assert!(
        body["data"]["misses"].is_number(),
        "stats 应含 misses, 实际: {:?}",
        body
    );

    // 清除图层缓存
    let req = test::TestRequest::delete()
        .uri("/geoserver/tiles/cache/clear/world")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "/tiles/cache/clear/world 应返回 200, 实际: {}",
        resp.status()
    );
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(
        body["success"].as_bool().unwrap_or(false),
        "clear 应成功, 实际: {:?}",
        body
    );
}

#[actix_rt::test]
async fn test_rest_tile_cache_hit() {
    // 使用启用缓存的自定义配置 (独立临时目录), 验证 MISS → HIT
    let mut config = common::create_test_config();
    config.cache = terrane::config::CacheConfig {
        kind: "local".to_string(),
        cache_dir: std::env::temp_dir().join(format!("terrane-rest-gwc-{}", std::process::id())),
        meta_dir: std::env::temp_dir()
            .join(format!("terrane-rest-gwc-meta-{}", std::process::id())),
        expire_after_secs: 0,
        max_tiles: 0,
        layer_quota_bytes: 0,
        enabled: true,
        default_gridset: "EPSG:4326".to_string(),
        session_ttl_secs: 300,
    };
    let state = actix_web::web::Data::new(terrane::state::AppState::new(config).await);
    let app = actix_web::test::init_service(
        actix_web::App::new()
            .app_data(state.clone())
            .wrap(actix_web::middleware::Logger::default())
            .configure(|svc| terrane::routes::configure_routes(svc, "/geoserver")),
    )
    .await;

    let uri = "/geoserver/tiles/world/0/0/0";
    let req = test::TestRequest::get().uri(uri).to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "首次瓦片请求应返回 200, 实际: {}",
        resp.status()
    );
    let hdr = resp
        .headers()
        .get("X-Tile-Cache")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert_eq!(hdr, "MISS", "首次请求应为 MISS, 实际: {}", hdr);

    let req = test::TestRequest::get().uri(uri).to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "二次瓦片请求应返回 200, 实际: {}",
        resp.status()
    );
    let hdr = resp
        .headers()
        .get("X-Tile-Cache")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert_eq!(hdr, "HIT", "二次请求应命中缓存 HIT, 实际: {}", hdr);

    // 清理临时缓存目录
    std::fs::remove_dir_all(
        std::env::temp_dir().join(format!("terrane-rest-gwc-{}", std::process::id())),
    )
    .ok();
    std::fs::remove_dir_all(
        std::env::temp_dir().join(format!("terrane-rest-gwc-meta-{}", std::process::id())),
    )
    .ok();
}

#[actix_rt::test]
async fn test_layer_cache_store_persists_through_api() {
    // 创建 Redis 数据源 → 创建图层并指定 cache_store → 读取确认持久化
    let app = build_test_app!();

    // 1. 创建 Redis 缓存数据源
    let create_ds = test::TestRequest::post()
        .uri("/geoserver/data-sources")
        .set_json(serde_json::json!({
            "name": "my_redis_cache",
            "type": "redis",
            "workspace": "default",
            "enabled": true,
            "connection": {
                "host": "127.0.0.1",
                "port": 6379,
                "database": "0"
            }
        }))
        .to_request();
    let resp = test::call_service(&app, create_ds).await;
    assert!(
        resp.status().is_success(),
        "创建 Redis 数据源应成功, 实际: {}",
        resp.status()
    );

    // 2. 创建图层并指定 cache_store = my_redis_cache
    let create_layer = test::TestRequest::post()
        .uri("/geoserver/layers")
        .set_json(serde_json::json!({
            "name": "cached_layer",
            "title": "Cached Layer",
            "workspace": "default",
            "store": "shapes",
            "native_name": "world",
            "cache_store": "my_redis_cache"
        }))
        .to_request();
    let resp = test::call_service(&app, create_layer).await;
    assert!(
        resp.status().is_success(),
        "创建图层应成功, 实际: {}",
        resp.status()
    );

    // 3. GET /layers/{name} 应回显 cache_store
    let get_layer = test::TestRequest::get()
        .uri("/geoserver/layers/cached_layer")
        .to_request();
    let resp = test::call_service(&app, get_layer).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(
        body["data"]["cache_store"], "my_redis_cache",
        "图层详情应返回 cache_store, 实际: {:?}",
        body
    );

    // 4. PUT 更新 cache_store = null (回到默认缓存)
    let update_layer = test::TestRequest::put()
        .uri("/geoserver/layers/cached_layer")
        .set_json(serde_json::json!({ "cache_store": null }))
        .to_request();
    let resp = test::call_service(&app, update_layer).await;
    assert!(
        resp.status().is_success(),
        "更新图层应成功, 实际: {}",
        resp.status()
    );

    let get_layer = test::TestRequest::get()
        .uri("/geoserver/layers/cached_layer")
        .to_request();
    let resp = test::call_service(&app, get_layer).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(
        body["data"]["cache_store"].is_null(),
        "清除后 cache_store 应为 null, 实际: {:?}",
        body["data"]["cache_store"]
    );
}

#[actix_rt::test]
async fn test_event_driven_refresh_updates_memory_immediately() {
    // 事件驱动目录刷新: store 模式下 PUT 更新图层后, 内存 `state.layers`
    // 应立刻反映 (无需等待目录刷新周期), 使 WMS/WMTS 等读内存的服务无
    // "写后读旧"窗口。
    let mut config = common::create_test_config();
    config.server.catalog_refresh_secs = 0; // 关闭周期刷新, 只验证事件驱动

    let state = actix_web::web::Data::new(terrane::state::AppState::new(config).await);
    let app = actix_web::test::init_service(
        actix_web::App::new()
            .app_data(state.clone())
            .wrap(actix_web::middleware::Logger::default())
            .configure(|svc| terrane::routes::configure_routes(svc, "/geoserver")),
    )
    .await;

    // 创建图层 (持久化 + 内存)。
    let create_layer = test::TestRequest::post()
        .uri("/geoserver/layers")
        .set_json(serde_json::json!({
            "name": "evt_refresh",
            "title": "Original Title",
            "workspace": "default",
            "store": "shapes",
            "native_name": "world"
        }))
        .to_request();
    let resp = test::call_service(&app, create_layer).await;
    assert!(resp.status().is_success(), "创建图层应成功");

    // PUT 更新 title。
    let update_layer = test::TestRequest::put()
        .uri("/geoserver/layers/evt_refresh")
        .set_json(serde_json::json!({ "title": "Updated Title" }))
        .to_request();
    let resp = test::call_service(&app, update_layer).await;
    assert!(resp.status().is_success(), "更新图层应成功");

    // 立即读内存目录: 标题必须已是新值 (事件刷新生效, 无 sleep)。
    let layers = state.layers.read().await;
    let updated = layers
        .iter()
        .find(|l| l.name == "evt_refresh")
        .expect("内存中应存在图层");
    assert_eq!(
        updated.title, "Updated Title",
        "事件刷新后内存标题应立即更新, 实际: {}",
        updated.title
    );
}

#[actix_rt::test]
async fn test_catalog_refresh_reloads_layers() {
    // 启用目录定时刷新 (极短周期), 验证从元数据存储重载后内存目录包含新图层
    let mut config = common::create_test_config();
    config.server.catalog_refresh_secs = 1;

    let state = actix_web::web::Data::new(terrane::state::AppState::new(config).await);
    let app = actix_web::test::init_service(
        actix_web::App::new()
            .app_data(state.clone())
            .wrap(actix_web::middleware::Logger::default())
            .configure(|svc| terrane::routes::configure_routes(svc, "/geoserver")),
    )
    .await;

    // 通过 API 创建图层 (持久化到元数据存储)
    let create_layer = test::TestRequest::post()
        .uri("/geoserver/layers")
        .set_json(serde_json::json!({
            "name": "refresh_target",
            "title": "Refresh Target",
            "workspace": "default",
            "store": "shapes",
            "native_name": "world"
        }))
        .to_request();
    let resp = test::call_service(&app, create_layer).await;
    assert!(resp.status().is_success(), "创建图层应成功");

    // 等待一个刷新周期, 让后台任务从存储重载
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

    let layers = state.layers.read().await;
    assert!(
        layers.iter().any(|l| l.name == "refresh_target"),
        "目录刷新后内存缓存应包含新图层 'refresh_target'"
    );
}

#[actix_rt::test]
async fn test_rest_backup_import_roundtrip() {
    // App A: 创建自定义工作空间后导出备份
    let app_a = build_test_app!();
    let token_a = login_admin_token!(app_a);

    let create = test::TestRequest::post()
        .uri("/geoserver/workspaces")
        .set_json(serde_json::json!({
            "name": "ws_import",
            "title": "Import Workspace",
            "description": "created for backup import round-trip",
        }))
        .to_request();
    let resp = test::call_service(&app_a, create).await;
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "创建工作空间应返回 201, 实际: {}",
        resp.status()
    );

    let req = test::TestRequest::get()
        .uri("/geoserver/backup/export")
        .insert_header(("Authorization", format!("Bearer {}", token_a)))
        .to_request();
    let resp = test::call_service(&app_a, req).await;
    assert!(
        resp.status().is_success(),
        "备份导出应返回 200, 实际: {}",
        resp.status()
    );
    let body: serde_json::Value = test::read_body_json(resp).await;
    let backup = body["data"].clone();
    let has_ws = backup["workspaces"]
        .as_array()
        .map(|a| a.iter().any(|w| w["name"] == "ws_import"))
        .unwrap_or(false);
    assert!(
        has_ws,
        "导出应包含 ws_import, 实际: {:?}",
        backup["workspaces"]
    );

    // App B (全新实例): 导入备份
    let app_b = build_test_app!();
    let token_b = login_admin_token!(app_b);

    let req = test::TestRequest::post()
        .uri("/geoserver/backup/import")
        .set_json(&backup)
        .insert_header(("Authorization", format!("Bearer {}", token_b)))
        .to_request();
    let resp = test::call_service(&app_b, req).await;
    assert!(
        resp.status().is_success(),
        "备份导入应返回 200, 实际: {}",
        resp.status()
    );
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(
        body["success"].as_bool().unwrap_or(false),
        "导入应成功, 实际: {:?}",
        body
    );
    assert!(
        body["data"]["report"].is_object(),
        "应返回导入报告, 实际: {:?}",
        body
    );

    // 验证 ws_import 已在 App B 中创建
    let req = test::TestRequest::get()
        .uri("/geoserver/workspaces")
        .to_request();
    let resp = test::call_service(&app_b, req).await;
    assert!(
        resp.status().is_success(),
        "查询工作空间应返回 200, 实际: {}",
        resp.status()
    );
    let body: serde_json::Value = test::read_body_json(resp).await;
    let names: Vec<String> = body["data"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|w| w["name"].as_str().map(|s| s.to_string()))
        .collect();
    assert!(
        names.contains(&"ws_import".to_string()),
        "导入后 App B 应包含 ws_import, 实际: {:?}",
        names
    );
}

// ---------------------------------------------------------------------------
// Batch 7: PostGIS 数据源 HTTP 层集成 (live, 需本机 PostGIS 容器)
// ---------------------------------------------------------------------------

/// 连接参数可用 `GEOSERVER_TEST_PG_*` 环境变量覆盖 (与 store 层 live 测试一致)。
/// 返回 (host, port, user, password, database)。
fn pg_http_test_params() -> (String, u16, String, String, String) {
    let host = std::env::var("GEOSERVER_TEST_PG_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let port: u16 = std::env::var("GEOSERVER_TEST_PG_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5433);
    let user = std::env::var("GEOSERVER_TEST_PG_USER").unwrap_or_else(|_| "terrane".into());
    let password = std::env::var("GEOSERVER_TEST_PG_PASSWORD").unwrap_or_else(|_| "terrane".into());
    let instance = std::env::var("GEOSERVER_TEST_PG_DB").unwrap_or_else(|_| "terrane".into());
    (host, port, user, password, instance)
}

/// 直接连接 PostGIS, 创建 schema + 带 PostGIS 几何列的测试表。
async fn pg_http_setup_schema(schema: &str) -> tokio_postgres::Client {
    let (host, port, user, password, instance) = pg_http_test_params();
    let (client, connection) = tokio_postgres::connect(
        &format!(
            "host={} port={} dbname={} user={} password={}",
            host, port, instance, user, password
        ),
        tokio_postgres::NoTls,
    )
    .await
    .expect("应能连接本地 PostGIS (postgis 容器)");
    tokio::spawn(connection);

    client
        .batch_execute(&format!(
            "CREATE SCHEMA IF NOT EXISTS {};
             CREATE TABLE {}.cities (
                id SERIAL PRIMARY KEY,
                name TEXT,
                geom GEOMETRY(Point, 4326)
             );
             INSERT INTO {}.cities (name, geom) VALUES
                ('Amsterdam', ST_SetSRID(ST_MakePoint(4.9, 52.37), 4326)),
                ('Rotterdam', ST_SetSRID(ST_MakePoint(4.48, 51.92), 4326));",
            schema, schema, schema
        ))
        .await
        .expect("应能创建 schema 与测试表 (需要 PostGIS 扩展)");
    client
}

/// 删除测试 schema (清理)。
async fn pg_http_drop_schema(schema: &str) {
    let (host, port, user, password, instance) = pg_http_test_params();
    let (client, connection) = tokio_postgres::connect(
        &format!(
            "host={} port={} dbname={} user={} password={}",
            host, port, instance, user, password
        ),
        tokio_postgres::NoTls,
    )
    .await
    .unwrap();
    tokio::spawn(connection);
    let _ = client
        .batch_execute(&format!("DROP SCHEMA IF EXISTS {} CASCADE", schema))
        .await;
}

#[actix_rt::test]
#[ignore = "requires a live PostGIS (e.g. docker compose -f build/docker-compose.yml up -d)"]
async fn test_live_rest_postgis_data_source_http() {
    use actix_web::http::StatusCode;

    let (pg_host, pg_port, pg_user, pg_password, pg_db) = pg_http_test_params();
    // 每进程独立 schema, 避免并行运行互相清理
    let schema = format!("terrane_http_test_{}", std::process::id());
    let setup_client = pg_http_setup_schema(&schema).await;

    let app = build_test_app!();

    // 1. 通过 REST 创建 postgis 数据源 (指向真实容器)
    let create = test::TestRequest::post()
        .uri("/geoserver/data-sources")
        .set_json(serde_json::json!({
            "name": "pg_http_ds",
            "type": "postgis",
            "workspace": "default",
            "enabled": true,
            "connection": {
                "host": pg_host,
                "port": pg_port,
                "database": pg_db,
                "schema": schema,
                "username": pg_user,
                "password": pg_password,
            },
        }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "创建 PostGIS 数据源应返回 201, 实际: {}",
        resp.status()
    );

    // 2. 数据源列表应包含
    let req = test::TestRequest::get()
        .uri("/geoserver/data-sources")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    let names: Vec<String> = body["data"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|d| d["name"].as_str().map(|s| s.to_string()))
        .collect();
    assert!(
        names.contains(&"pg_http_ds".to_string()),
        "数据源列表应包含 pg_http_ds, 实际: {:?}",
        names
    );

    // 3. 表列表 → 应包含 cities
    let req = test::TestRequest::get()
        .uri("/geoserver/data-sources/pg_http_ds/tables")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "表列表应返回 200, 实际: {}",
        resp.status()
    );
    let body: serde_json::Value = test::read_body_json(resp).await;
    let tables: Vec<String> = body["data"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|t| t.as_str().map(|s| s.to_string()))
        .collect();
    assert!(
        tables.contains(&"cities".to_string()),
        "表列表应包含 cities, 实际: {:?}",
        tables
    );

    // 4. 创建图层 (引用数据源 + native table name)
    let create = test::TestRequest::post()
        .uri("/geoserver/layers")
        .set_json(serde_json::json!({
            "name": "cities_layer",
            "title": "Cities",
            "workspace": "default",
            "store": "pg_http_ds",
            "native_name": "cities",
            "srs": "EPSG:4326",
            "minx": 3.0, "miny": 51.0, "maxx": 6.0, "maxy": 53.0,
        }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "创建图层应返回 201, 实际: {}",
        resp.status()
    );

    // 5. feature-type → 应返回表结构 (name / geom)
    let req = test::TestRequest::get()
        .uri("/geoserver/layers/cities_layer/feature-type")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "feature-type 应返回 200, 实际: {}",
        resp.status()
    );
    let body: serde_json::Value = test::read_body_json(resp).await;
    let cols = body["data"].as_array().cloned().unwrap_or_default();
    let col_names: Vec<String> = cols
        .iter()
        .filter_map(|c| c["name"].as_str().map(|s| s.to_string()))
        .collect();
    assert!(
        col_names.contains(&"name".to_string()),
        "feature-type 应包含 name 列, 实际: {:?}",
        col_names
    );
    assert!(
        col_names.contains(&"geom".to_string()),
        "feature-type 应包含 geom 列, 实际: {:?}",
        col_names
    );

    // 清理: 释放 setup 连接, DROP SCHEMA
    drop(setup_client);
    pg_http_drop_schema(&schema).await;
}

// ---------------------------------------------------------------------------
// Batch 10: GeoPackage 类型化属性 + FeatureType describe over REST
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn test_rest_geopackage_feature_type() {
    use std::collections::HashMap;

    // 1. 在临时目录写入带类型化属性的 GeoPackage
    let dir = std::env::temp_dir().join(format!("terrane-gpkg-rest-{}", std::process::id()));
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
    let layer_info = terrane::utils::geopackage::write_geopackage_features(
        &path, "typed", "POINT", 4326, &features, &bounds,
    )
    .expect("应能写入 GeoPackage");
    assert_eq!(layer_info.feature_count, 2);

    let app = build_test_app!();

    // 2. 通过 REST 创建 geopackage 数据源
    let create = test::TestRequest::post()
        .uri("/geoserver/data-sources")
        .set_json(serde_json::json!({
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
        StatusCode::CREATED,
        "创建 GeoPackage 数据源应返回 201, 实际: {}",
        resp.status()
    );

    // 3. 表列表 → 应包含 typed
    let req = test::TestRequest::get()
        .uri("/geoserver/data-sources/gpkg_ds/tables")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "表列表应返回 200, 实际: {}",
        resp.status()
    );
    let body: serde_json::Value = test::read_body_json(resp).await;
    let tables: Vec<String> = body["data"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|t| t.as_str().map(|s| s.to_string()))
        .collect();
    assert!(
        tables.contains(&"typed".to_string()),
        "表列表应包含 typed, 实际: {:?}",
        tables
    );

    // 4. 创建图层 (store=gpkg_ds, native_name=typed)
    let create = test::TestRequest::post()
        .uri("/geoserver/layers")
        .set_json(serde_json::json!({
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
        StatusCode::CREATED,
        "创建图层应返回 201, 实际: {}",
        resp.status()
    );

    // 5. feature-type → 返回类型化列 (name TEXT / count INTEGER / price REAL / active BOOLEAN)
    let req = test::TestRequest::get()
        .uri("/geoserver/layers/typed_layer/feature-type")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "feature-type 应返回 200, 实际: {}",
        resp.status()
    );
    let body: serde_json::Value = test::read_body_json(resp).await;
    let cols = body["data"].as_array().cloned().unwrap_or_default();
    let col_map: HashMap<String, String> = cols
        .iter()
        .filter_map(|c| {
            let n = c["name"].as_str().map(|s| s.to_string());
            let t = c["type"].as_str().map(|s| s.to_string());
            n.zip(t)
        })
        .collect();
    assert_eq!(
        col_map.get("name"),
        Some(&"TEXT".to_string()),
        "name 列应为 TEXT, 实际: {:?}",
        col_map
    );
    assert_eq!(
        col_map.get("count"),
        Some(&"INTEGER".to_string()),
        "count 列应为 INTEGER, 实际: {:?}",
        col_map
    );
    assert_eq!(
        col_map.get("price"),
        Some(&"REAL".to_string()),
        "price 列应为 REAL, 实际: {:?}",
        col_map
    );
    assert_eq!(
        col_map.get("active"),
        Some(&"BOOLEAN".to_string()),
        "active 列应为 BOOLEAN, 实际: {:?}",
        col_map
    );

    // 清理
    std::fs::remove_dir_all(&dir).ok();
}

// ---------------------------------------------------------------------------
// R1: Stores CRUD / workspace stores / layer-group PUT / user PUT / .pbf 路由
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn test_rest_stores_crud() {
    let app = build_test_app!();

    // 创建 store (postgis 类型, 无真实连接, 仅测元数据 CRUD)
    let create = test::TestRequest::post()
        .uri("/geoserver/stores")
        .set_json(serde_json::json!({
            "name": "store_r1",
            "type": "postgis",
            "workspace": "default",
            "enabled": true,
            "connection": { "host": "127.0.0.1", "port": 5432, "database": "geoserver" },
        }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "创建 store 应返回 201, 实际: {}",
        resp.status()
    );

    // 列表包含新 store
    let req = test::TestRequest::get()
        .uri("/geoserver/stores")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    let names: Vec<String> = body["data"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|s| s["name"].as_str().map(|v| v.to_string()))
        .collect();
    assert!(
        names.contains(&"store_r1".to_string()),
        "stores 列表应包含 store_r1, 实际: {:?}",
        names
    );

    // 详情: postgis → DataStore
    let req = test::TestRequest::get()
        .uri("/geoserver/stores/store_r1")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "GET store 应返回 200");
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(
        body["data"]["type"].as_str(),
        Some("DataStore"),
        "postgis store 应映射为 DataStore, 实际: {}",
        body
    );

    // 更新 (禁用)
    let update = test::TestRequest::put()
        .uri("/geoserver/stores/store_r1")
        .set_json(serde_json::json!({ "enabled": false }))
        .to_request();
    let resp = test::call_service(&app, update).await;
    assert!(resp.status().is_success(), "PUT store 应返回 200");
    let req = test::TestRequest::get()
        .uri("/geoserver/stores/store_r1")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["enabled"], serde_json::json!(false));

    // 删除 → 404
    let del = test::TestRequest::delete()
        .uri("/geoserver/stores/store_r1")
        .to_request();
    let resp = test::call_service(&app, del).await;
    assert!(resp.status().is_success(), "DELETE store 应返回 200");
    let req = test::TestRequest::get()
        .uri("/geoserver/stores/store_r1")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "删除后 GET store 应返回 404"
    );
}

#[actix_rt::test]
async fn test_rest_create_store_in_workspace() {
    let app = build_test_app!();

    // 先创建工作空间 (测试配置的 default 工作空间仅存在于 config, 不在元数据存储)
    let create = test::TestRequest::post()
        .uri("/geoserver/workspaces")
        .set_json(serde_json::json!({
            "name": "ws_r1",
            "uri": "http://geoserver.org/ws_r1",
        }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert!(
        resp.status().is_success(),
        "创建工作空间应成功, 实际: {}",
        resp.status()
    );

    // 在工作空间创建 store
    let create = test::TestRequest::post()
        .uri("/geoserver/workspaces/ws_r1/stores")
        .set_json(serde_json::json!({
            "name": "store_ws1",
            "type": "shapefile",
            "enabled": true,
            "connection": { "file_path": "C:/tmp/ws1.shp", "file_storage_type": "local" },
        }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "在工作空间创建 store 应返回 201, 实际: {}",
        resp.status()
    );

    // 按工作空间列表包含
    let req = test::TestRequest::get()
        .uri("/geoserver/workspaces/ws_r1/stores")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    let names: Vec<String> = body["data"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|s| s["name"].as_str().map(|v| v.to_string()))
        .collect();
    assert!(
        names.contains(&"store_ws1".to_string()),
        "工作空间 stores 应包含 store_ws1, 实际: {:?}",
        names
    );

    // 不存在的工作空间 → 404
    let create = test::TestRequest::post()
        .uri("/geoserver/workspaces/nonexistent/stores")
        .set_json(serde_json::json!({
            "name": "store_bad",
            "type": "shapefile",
            "connection": { "file_path": "C:/tmp/bad.shp", "file_storage_type": "local" },
        }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "不存在工作空间创建 store 应返回 404, 实际: {}",
        resp.status()
    );
}

#[actix_rt::test]
async fn test_rest_update_layer_group() {
    let app = build_test_app!();

    // 创建
    let create = test::TestRequest::post()
        .uri("/geoserver/layer-groups")
        .set_json(serde_json::json!({
            "name": "lg_r1",
            "title": "Original",
            "layers": ["world"],
        }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    // PUT 更新标题 + 成员
    let update = test::TestRequest::put()
        .uri("/geoserver/layer-groups/lg_r1")
        .set_json(serde_json::json!({
            "title": "Updated",
            "layers": ["world", "world"],
            "styles": [null, "default"],
        }))
        .to_request();
    let resp = test::call_service(&app, update).await;
    assert!(
        resp.status().is_success(),
        "PUT layer-group 应返回 200, 实际: {}",
        resp.status()
    );

    // GET 验证
    let req = test::TestRequest::get()
        .uri("/geoserver/layer-groups/lg_r1")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["title"].as_str(), Some("Updated"));
    assert_eq!(
        body["data"]["layers"].as_array().map(|a| a.len()),
        Some(2),
        "layers 应更新为 2 个成员, 实际: {}",
        body
    );

    // 更新不存在的组 → 404
    let update = test::TestRequest::put()
        .uri("/geoserver/layer-groups/no_such_group")
        .set_json(serde_json::json!({ "title": "X" }))
        .to_request();
    let resp = test::call_service(&app, update).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[actix_rt::test]
async fn test_rest_update_user() {
    let app = build_test_app!();
    let token = login_admin_token!(app);

    // 创建用户
    let create = test::TestRequest::post()
        .uri("/geoserver/auth/users")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(serde_json::json!({ "username": "u_r1", "password": "pass123", "role": "user" }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert_eq!(resp.status(), StatusCode::CREATED, "创建用户应返回 201");

    // PUT 改角色 + 重置密码
    let update = test::TestRequest::put()
        .uri("/geoserver/auth/users/u_r1")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(serde_json::json!({ "role": "manager", "password": "newpass456" }))
        .to_request();
    let resp = test::call_service(&app, update).await;
    assert!(
        resp.status().is_success(),
        "PUT user 应返回 200, 实际: {}",
        resp.status()
    );

    // 新密码可登录
    let login = test::TestRequest::post()
        .uri("/geoserver/auth/login")
        .set_json(serde_json::json!({ "username": "u_r1", "password": "newpass456" }))
        .to_request();
    let resp = test::call_service(&app, login).await;
    assert!(
        resp.status().is_success(),
        "新密码应能登录, 实际: {}",
        resp.status()
    );

    // 旧密码失效
    let login = test::TestRequest::post()
        .uri("/geoserver/auth/login")
        .set_json(serde_json::json!({ "username": "u_r1", "password": "pass123" }))
        .to_request();
    let resp = test::call_service(&app, login).await;
    assert!(
        !resp.status().is_success(),
        "旧密码应登录失败, 实际: {}",
        resp.status()
    );

    // 更新不存在用户 → 404
    let update = test::TestRequest::put()
        .uri("/geoserver/auth/users/no_such_user")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(serde_json::json!({ "role": "admin" }))
        .to_request();
    let resp = test::call_service(&app, update).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[actix_rt::test]
async fn test_rest_mvt_pbf_route() {
    let app = build_test_app!();

    // .pbf 尾缀路由不再被 /tiles/{layer}/{z}/{x}/{y} 遮蔽
    let req = test::TestRequest::get()
        .uri("/geoserver/tiles/world/0/0/0.pbf")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "MVT .pbf 瓦片应返回 200, 实际: {}",
        resp.status()
    );

    let content_type = resp
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(
        content_type.contains("mapbox-vector-tile"),
        "Content-Type 应为 MVT, 实际: {}",
        content_type
    );
}

// ---------------------------------------------------------------------------
// R2: 工作空间维度端点 + OGC 服务设置
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn test_rest_workspace_dimension_endpoints() {
    let app = build_test_app!();

    // 创建 workspace
    let create = test::TestRequest::post()
        .uri("/geoserver/workspaces")
        .set_json(serde_json::json!({
            "name": "ws_dim",
            "uri": "http://geoserver.org/ws_dim",
        }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert!(resp.status().is_success(), "创建工作空间应成功");

    // datastore (postgis) + coveragestore (geotiff)
    let create = test::TestRequest::post()
        .uri("/geoserver/data-sources")
        .set_json(serde_json::json!({
            "name": "ds_vec",
            "type": "postgis",
            "workspace": "ws_dim",
            "enabled": true,
            "connection": { "host": "127.0.0.1", "port": 5432, "database": "geo" },
        }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let create = test::TestRequest::post()
        .uri("/geoserver/data-sources")
        .set_json(serde_json::json!({
            "name": "ds_raster",
            "type": "geotiff",
            "workspace": "ws_dim",
            "enabled": true,
            "connection": { "file_path": "C:/tmp/x.tif", "file_storage_type": "local" },
        }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    // datastores → 仅 ds_vec
    let req = test::TestRequest::get()
        .uri("/geoserver/workspaces/ws_dim/datastores")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    let names: Vec<String> = body["data"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|s| s["name"].as_str().map(|v| v.to_string()))
        .collect();
    assert_eq!(
        names,
        vec!["ds_vec".to_string()],
        "datastores 应只含 ds_vec"
    );

    // coveragestores → 仅 ds_raster
    let req = test::TestRequest::get()
        .uri("/geoserver/workspaces/ws_dim/coveragestores")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    let names: Vec<String> = body["data"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|s| s["name"].as_str().map(|v| v.to_string()))
        .collect();
    assert_eq!(
        names,
        vec!["ds_raster".to_string()],
        "coveragestores 应只含 ds_raster"
    );

    // 创建图层 → /workspaces/ws_dim/layers 包含
    let create = test::TestRequest::post()
        .uri("/geoserver/layers")
        .set_json(serde_json::json!({
            "name": "layer_dim",
            "title": "Dim",
            "workspace": "ws_dim",
            "store": "ds_vec",
            "srs": "EPSG:4326",
            "minx": -180.0, "miny": -90.0, "maxx": 180.0, "maxy": 90.0,
        }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let req = test::TestRequest::get()
        .uri("/geoserver/workspaces/ws_dim/layers")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    let names: Vec<String> = body["data"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|s| s["name"].as_str().map(|v| v.to_string()))
        .collect();
    assert!(
        names.contains(&"layer_dim".to_string()),
        "工作空间 layers 应包含 layer_dim, 实际: {:?}",
        names
    );

    // 不存在工作空间 → 404
    let req = test::TestRequest::get()
        .uri("/geoserver/workspaces/nonexistent/layers")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[actix_rt::test]
async fn test_rest_service_settings() {
    let app = build_test_app!();
    let token = login_admin_token!(app);

    // GET 默认 (未设置 → 空)
    let req = test::TestRequest::get()
        .uri("/geoserver/services/wms/settings")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "GET settings 应返回 200");
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["service"].as_str(), Some("wms"));

    // PUT 设置标题 (需 admin)
    let update = test::TestRequest::put()
        .uri("/geoserver/services/wms/settings")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(serde_json::json!({
            "title": "Terrane WMS Custom",
            "abstract": "Custom abstract",
            "keywords": ["WMS", "Terrane"],
        }))
        .to_request();
    let resp = test::call_service(&app, update).await;
    assert!(
        resp.status().is_success(),
        "PUT settings 应返回 200, 实际: {}",
        resp.status()
    );

    // GET 验证回读
    let req = test::TestRequest::get()
        .uri("/geoserver/services/wms/settings")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["title"].as_str(), Some("Terrane WMS Custom"));

    // WMS GetCapabilities 反映标题
    let req = test::TestRequest::get()
        .uri("/wms?SERVICE=WMS&REQUEST=GetCapabilities")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let xml = String::from_utf8_lossy(&test::read_body(resp).await).to_string();
    assert!(
        xml.contains("Terrane WMS Custom"),
        "GetCapabilities 应包含自定义标题, 实际: {}",
        xml
    );

    // 未认证 PUT → 400
    let update = test::TestRequest::put()
        .uri("/geoserver/services/wms/settings")
        .set_json(serde_json::json!({ "title": "X" }))
        .to_request();
    let resp = test::call_service(&app, update).await;
    assert!(!resp.status().is_success(), "未认证 PUT 应失败");

    // 未知服务 → 400
    let req = test::TestRequest::get()
        .uri("/geoserver/services/foo/settings")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 400);
}

// ---------------------------------------------------------------------------
// R3: /about + /resources + feature-type PUT
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn test_rest_about_endpoints() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/geoserver/about/version")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "/about/version 应返回 200");
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(
        body["data"]["version"].as_str().is_some(),
        "应包含版本号, 实际: {}",
        body
    );

    let req = test::TestRequest::get()
        .uri("/geoserver/about/system-status")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "/about/system-status 应返回 200"
    );
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body["data"]["uptime"].as_str().is_some());
}

#[actix_rt::test]
async fn test_rest_resources() {
    let app = build_test_app!();
    let token = login_admin_token!(app);

    // GET 列表 (data_dir 根)
    let req = test::TestRequest::get()
        .uri("/geoserver/resources")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "GET /resources 应返回 200");

    // POST 上传 (multipart, 需认证)
    let boundary = "terraneboundary";
    let multipart_body = format!(
        "--{b}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"res_test.txt\"\r\nContent-Type: text/plain\r\n\r\nhello resources\r\n--{b}--\r\n",
        b = boundary
    );
    let req = test::TestRequest::post()
        .uri("/geoserver/resources?path=uploads")
        .insert_header((
            "Content-Type",
            format!("multipart/form-data; boundary={}", boundary),
        ))
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_payload(multipart_body.clone())
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status().as_u16(),
        201,
        "上传资源应返回 201, 实际: {}",
        resp.status()
    );

    // GET 验证目录包含文件
    let req = test::TestRequest::get()
        .uri("/geoserver/resources?path=uploads")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    let names: Vec<String> = body["data"]["entries"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|e| e["name"].as_str().map(|s| s.to_string()))
        .collect();
    assert!(
        names.contains(&"res_test.txt".to_string()),
        "uploads 目录应包含 res_test.txt, 实际: {:?}",
        names
    );

    // 未认证上传 → 400
    let req = test::TestRequest::post()
        .uri("/geoserver/resources")
        .insert_header((
            "Content-Type",
            format!("multipart/form-data; boundary={}", boundary),
        ))
        .set_payload(multipart_body.clone())
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(!resp.status().is_success(), "未认证上传应失败");

    // DELETE 删除 (需认证)
    let req = test::TestRequest::delete()
        .uri("/geoserver/resources?path=uploads/res_test.txt")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success(), "删除资源应返回 200");

    // 删除后不存在 → 404
    let req = test::TestRequest::delete()
        .uri("/geoserver/resources?path=uploads/res_test.txt")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[actix_rt::test]
async fn test_rest_update_feature_type() {
    use std::collections::HashMap;

    // 1. 写入 GeoPackage fixture
    let dir = std::env::temp_dir().join(format!("terrane-gpkg-ft-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("ft.gpkg");
    let mut p1 = HashMap::new();
    p1.insert(
        "name".to_string(),
        terrane::models::PropertyValue::String("alpha".to_string()),
    );
    let features = vec![terrane::models::Feature::with_id(
        "f1".into(),
        terrane::models::GeoJsonGeometry::Point {
            coordinates: vec![1.0, 1.0],
        },
        p1,
    )];
    let bounds = terrane::models::Bounds::new(1.0, 1.0, 2.0, 2.0);
    terrane::utils::geopackage::write_geopackage_features(
        &path, "ft", "POINT", 4326, &features, &bounds,
    )
    .expect("应能写入 GeoPackage");

    let app = build_test_app!();
    let token = login_admin_token!(app);

    // 2. 发布数据源 + 图层
    let create = test::TestRequest::post()
        .uri("/geoserver/data-sources")
        .set_json(serde_json::json!({
            "name": "gpkg_ft",
            "type": "geopackage",
            "workspace": "default",
            "enabled": true,
            "connection": { "file_path": path.to_str(), "file_storage_type": "local" },
        }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let create = test::TestRequest::post()
        .uri("/geoserver/layers")
        .set_json(serde_json::json!({
            "name": "ft_layer",
            "title": "FT",
            "workspace": "default",
            "store": "gpkg_ft",
            "native_name": "ft",
            "srs": "EPSG:4326",
            "minx": 1.0, "miny": 1.0, "maxx": 2.0, "maxy": 2.0,
        }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    // 3. PUT 新增列
    let update = test::TestRequest::put()
        .uri("/geoserver/layers/ft_layer/feature-type")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(serde_json::json!({
            "properties": [
                { "name": "population", "type": "INTEGER" },
                { "name": "note", "type": "TEXT" }
            ]
        }))
        .to_request();
    let resp = test::call_service(&app, update).await;
    assert!(
        resp.status().is_success(),
        "PUT feature-type 应返回 200, 实际: {}",
        resp.status()
    );
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["added"].as_array().map(|a| a.len()), Some(2));

    // 4. GET 验证新列存在
    let req = test::TestRequest::get()
        .uri("/geoserver/layers/ft_layer/feature-type")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    let names: Vec<String> = body["data"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|c| c["name"].as_str().map(|s| s.to_string()))
        .collect();
    for col in ["population", "note"] {
        assert!(
            names.contains(&col.to_string()),
            "feature-type 应包含新列 {}, 实际: {:?}",
            col,
            names
        );
    }

    // 5. 重复新增已存在列 → 400
    let update = test::TestRequest::put()
        .uri("/geoserver/layers/ft_layer/feature-type")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(serde_json::json!({
            "properties": [ { "name": "population", "type": "INTEGER" } ]
        }))
        .to_request();
    let resp = test::call_service(&app, update).await;
    assert_eq!(resp.status().as_u16(), 400, "重复列应返回 400");

    // 6. 非 GeoPackage 图层 → 4xx
    let update = test::TestRequest::put()
        .uri("/geoserver/layers/world/feature-type")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(serde_json::json!({
            "properties": [ { "name": "x", "type": "TEXT" } ]
        }))
        .to_request();
    let resp = test::call_service(&app, update).await;
    assert!(
        !resp.status().is_success(),
        "非 GeoPackage 应返回 4xx, 实际: {}",
        resp.status()
    );

    std::fs::remove_dir_all(&dir).ok();
}

// ---------------------------------------------------------------------------
// T1: 瓦片种子任务 (seed / cancel / truncate)
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn test_tiles_seed_completes() {
    let app = build_test_app!();
    let token = login_admin_token!(app);

    // 创建种子任务: world z0 (global-geodetic 2 瓦片)
    let create = test::TestRequest::post()
        .uri("/geoserver/tiles/seed")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(serde_json::json!({
            "layer": "world",
            "gridset": "EPSG:4326",
            "z_min": 0,
            "z_max": 0,
            "format": "png",
        }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert_eq!(
        resp.status().as_u16(),
        201,
        "创建种子任务应返回 201, 实际: {}",
        resp.status()
    );
    let body: serde_json::Value = test::read_body_json(resp).await;
    let job_id = body["data"]["job"]["id"].as_str().unwrap_or("").to_string();
    assert!(!job_id.is_empty(), "应返回 job id, 实际: {}", body);

    // 轮询直至 Completed
    let mut status = String::new();
    for _ in 0..200 {
        let req = test::TestRequest::get()
            .uri(&format!("/geoserver/tiles/seed/{}", job_id))
            .to_request();
        let resp = test::call_service(&app, req).await;
        let body: serde_json::Value = test::read_body_json(resp).await;
        status = body["data"]["status"].as_str().unwrap_or("").to_string();
        if status == "Completed" {
            assert_eq!(body["data"]["done"].as_u64(), Some(2), "z0 应渲染 2 瓦片");
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert_eq!(
        status, "Completed",
        "种子任务应在超时前完成, 实际 status: {}",
        status
    );

    // 任务列表包含
    let req = test::TestRequest::get()
        .uri("/geoserver/tiles/seed")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    let ids: Vec<String> = body["data"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|j| j["id"].as_str().map(|s| s.to_string()))
        .collect();
    assert!(
        ids.contains(&job_id),
        "seed 列表应包含任务 {}, 实际: {:?}",
        job_id,
        ids
    );
}

#[actix_rt::test]
async fn test_tiles_seed_cancel() {
    let app = build_test_app!();
    let token = login_admin_token!(app);

    // 大范围任务 → 立即取消
    let create = test::TestRequest::post()
        .uri("/geoserver/tiles/seed")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(serde_json::json!({
            "layer": "world",
            "gridset": "EPSG:4326",
            "z_min": 6,
            "z_max": 8,
        }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert_eq!(resp.status().as_u16(), 201);
    let body: serde_json::Value = test::read_body_json(resp).await;
    let job_id = body["data"]["job"]["id"].as_str().unwrap_or("").to_string();

    let del = test::TestRequest::delete()
        .uri(&format!("/geoserver/tiles/seed/{}", job_id))
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();
    let resp = test::call_service(&app, del).await;
    assert!(
        resp.status().is_success(),
        "取消任务应返回 200, 实际: {}",
        resp.status()
    );

    // 轮询直至 Cancelled
    let mut status = String::new();
    for _ in 0..200 {
        let req = test::TestRequest::get()
            .uri(&format!("/geoserver/tiles/seed/{}", job_id))
            .to_request();
        let resp = test::call_service(&app, req).await;
        let body: serde_json::Value = test::read_body_json(resp).await;
        status = body["data"]["status"].as_str().unwrap_or("").to_string();
        if status == "Cancelled" || status == "Completed" {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert_eq!(status, "Cancelled", "任务应被取消, 实际 status: {}", status);
}

#[actix_rt::test]
async fn test_tiles_seed_truncate_and_validation() {
    let app = build_test_app!();
    let token = login_admin_token!(app);

    // truncate (world; 缓存未启用时 removed=0 也返回 200)
    let truncate = test::TestRequest::post()
        .uri("/geoserver/tiles/seed/truncate")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(serde_json::json!({ "layer": "world", "gridset": "EPSG:4326" }))
        .to_request();
    let resp = test::call_service(&app, truncate).await;
    assert!(
        resp.status().is_success(),
        "truncate 应返回 200, 实际: {}",
        resp.status()
    );
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["data"]["layer"].as_str(), Some("world"));

    // truncate 不存在图层 → 404
    let truncate = test::TestRequest::post()
        .uri("/geoserver/tiles/seed/truncate")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(serde_json::json!({ "layer": "no_such_layer" }))
        .to_request();
    let resp = test::call_service(&app, truncate).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // 未认证创建 → 400
    let create = test::TestRequest::post()
        .uri("/geoserver/tiles/seed")
        .set_json(serde_json::json!({
            "layer": "world", "z_min": 0, "z_max": 0
        }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert!(!resp.status().is_success(), "未认证创建应失败");

    // 无效 gridset → 400
    let create = test::TestRequest::post()
        .uri("/geoserver/tiles/seed")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(serde_json::json!({
            "layer": "world", "gridset": "EPSG:9999", "z_min": 0, "z_max": 0
        }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert_eq!(resp.status().as_u16(), 400);

    // z_min > z_max → 400
    let create = test::TestRequest::post()
        .uri("/geoserver/tiles/seed")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(serde_json::json!({
            "layer": "world", "z_min": 3, "z_max": 1
        }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert_eq!(resp.status().as_u16(), 400);

    // 不存在图层 → 404
    let create = test::TestRequest::post()
        .uri("/geoserver/tiles/seed")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(serde_json::json!({
            "layer": "no_such_layer", "z_min": 0, "z_max": 0
        }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
