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
        .set_json(&serde_json::json!({
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
        .set_json(&serde_json::json!({
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
        .set_json(&serde_json::json!({
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
        .set_json(&serde_json::json!({
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
// CRUD: Features
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn test_rest_features_crud() {
    let app = build_test_app!();

    let create = test::TestRequest::post()
        .uri("/geoserver/layers/world/features")
        .set_json(&serde_json::json!({
            "geometry": { "type": "Point", "coordinates": [10.0, 20.0] },
            "properties": { "name": "integration-test" },
        }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert_eq!(resp.status(), StatusCode::CREATED, "创建要素应返回 201");

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body["id"].is_string(), "应返回要素 id");

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
    let total = body["totalFeatures"].as_i64().unwrap_or(0);
    assert!(total >= 1, "应至少 1 条要素, 实际: {}", total);
}

// ---------------------------------------------------------------------------
// CRUD: SQL views
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn test_rest_sql_views_crud() {
    let app = build_test_app!();

    let create = test::TestRequest::post()
        .uri("/geoserver/sql-views")
        .set_json(&serde_json::json!({
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
        .set_json(&serde_json::json!({
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

// ---------------------------------------------------------------------------
// MVT vector tiles
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn test_mvt_endpoint() {
    let app = build_test_app!();

    // 注意: /tiles/{layer}/{z}/{x}/{y} 通用路由会先匹配 .pbf, 故用专用 /mvt/ 路由
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
// Batch 3: 单要素查询/更新/删除 + 认证 + 权限 + 备份 + 上传
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn test_rest_feature_single_get_update_delete() {
    let app = build_test_app!();

    // 1. 创建要素, 获取 id
    let create = test::TestRequest::post()
        .uri("/geoserver/layers/world/features")
        .set_json(&serde_json::json!({
            "geometry": { "type": "Point", "coordinates": [50.0, 60.0] },
            "properties": { "name": "before-update" },
        }))
        .to_request();
    let resp = test::call_service(&app, create).await;
    assert_eq!(resp.status(), StatusCode::CREATED, "创建要素应返回 201");
    let body: serde_json::Value = test::read_body_json(resp).await;
    let fid = body["id"].as_str().expect("应返回要素 id").to_string();

    // 2. 单条查询
    let req = test::TestRequest::get()
        .uri(&format!("/geoserver/layers/world/features/{}", fid))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "GET 单要素应返回 200, 实际: {}",
        resp.status()
    );
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["id"], fid, "应返回同一要素 id");

    // 3. 更新属性
    let update = test::TestRequest::put()
        .uri(&format!("/geoserver/layers/world/features/{}", fid))
        .set_json(&serde_json::json!({
            "properties": { "name": "after-update" },
        }))
        .to_request();
    let resp = test::call_service(&app, update).await;
    assert!(
        resp.status().is_success(),
        "PUT 单要素应返回 200, 实际: {}",
        resp.status()
    );
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["properties"]["name"], "after-update", "属性应已更新");

    // 4. 删除
    let del = test::TestRequest::delete()
        .uri(&format!("/geoserver/layers/world/features/{}", fid))
        .to_request();
    let resp = test::call_service(&app, del).await;
    assert!(
        resp.status().is_success(),
        "DELETE 单要素应返回 200, 实际: {}",
        resp.status()
    );

    // 5. 删除后再查 → 404
    let req = test::TestRequest::get()
        .uri(&format!("/geoserver/layers/world/features/{}", fid))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND, "删除后查询应返回 404");
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
        .set_json(&serde_json::json!({ "username": "admin", "password": "geoserver" }))
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
        .set_json(&serde_json::json!({ "username": "admin", "password": "wrong" }))
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
            &serde_json::json!({ "username": "tester1", "password": "secret123", "role": "guest" }),
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
        .set_json(&serde_json::json!({ "username": "tester1", "password": "secret123" }))
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
        .set_json(&serde_json::json!({
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

    // 上传后可查询该图层要素
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
    config.cache = Some(terrane::config::CacheConfig {
        kind: "local".to_string(),
        cache_dir: std::env::temp_dir().join(format!("terrane-rest-gwc-{}", std::process::id())),
        meta_dir: std::env::temp_dir()
            .join(format!("terrane-rest-gwc-meta-{}", std::process::id())),
        expire_after_secs: 0,
        max_tiles: 0,
        enabled: true,
        default_gridset: "EPSG:4326".to_string(),
        session_ttl_secs: 300,
    });
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
async fn test_rest_backup_import_roundtrip() {
    // App A: 创建自定义工作空间后导出备份
    let app_a = build_test_app!();
    let token_a = login_admin_token!(app_a);

    let create = test::TestRequest::post()
        .uri("/geoserver/workspaces")
        .set_json(&serde_json::json!({
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
    let password =
        std::env::var("GEOSERVER_TEST_PG_PASSWORD").unwrap_or_else(|_| "terrane".into());
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
        .set_json(&serde_json::json!({
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
        .set_json(&serde_json::json!({
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
