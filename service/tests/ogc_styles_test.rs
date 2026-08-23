//! OGC API - Styles integration tests.
//!
//! Covers the OGC API - Styles surface (OGC 21-009): landing page,
//! /conformance, /styles (list / create), /styles/{styleId} (get / replace /
//! delete), /styles/{styleId}/metadata and the collection linkage
//! /collections + /collections/{id}/styles.

#[macro_use]
mod common;

use actix_web::test;

#[actix_rt::test]
async fn test_ogc_styles_landing() {
    let app = build_test_app!();

    let req = test::TestRequest::get().uri("/ogc/styles").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "landing 应返回 200, 实际: {}",
        resp.status()
    );
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["title"], "Terrane");
    let rels: Vec<&str> = body["links"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["rel"].as_str().unwrap())
        .collect();
    assert!(rels.contains(&"self"));
    assert!(rels.contains(&"conformance"));
    assert!(rels.contains(&"styles"));
    assert!(rels.contains(&"data"));
}

#[actix_rt::test]
async fn test_ogc_styles_conformance() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/ogc/styles/conformance")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: serde_json::Value = test::read_body_json(resp).await;
    let conforms = body["conformsTo"].as_array().unwrap();
    assert!(conforms
        .iter()
        .any(|x| x.as_str().unwrap().contains("core")));
    assert!(conforms
        .iter()
        .any(|x| x.as_str().unwrap().contains("styles-list")));
    assert!(conforms
        .iter()
        .any(|x| x.as_str().unwrap().contains("style-create-update-delete")));
}

#[actix_rt::test]
async fn test_ogc_styles_list_and_content() {
    let app = build_test_app!();

    // 内置 default 样式应存在
    let req = test::TestRequest::get()
        .uri("/ogc/styles/styles")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = test::read_body_json(resp).await;
    let items = body["styles"].as_array().unwrap();
    assert!(
        items.iter().any(|s| s["id"] == "default"),
        "样式列表应包含 default, 实际: {}",
        body
    );

    // 样式内容 → SLD XML
    let req = test::TestRequest::get()
        .uri("/ogc/styles/styles/default")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let content_type = resp
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(
        content_type.contains("sld") || content_type.contains("xml"),
        "default 样式 Content-Type 应为 SLD/XML, 实际: {}",
        content_type
    );
    let content = String::from_utf8_lossy(&test::read_body(resp).await).to_string();
    assert!(
        content.contains("StyledLayerDescriptor"),
        "default 样式内容应为 SLD, 实际: {}",
        &content[..content.len().min(200)]
    );

    // 未知样式 → 404
    let req = test::TestRequest::get()
        .uri("/ogc/styles/styles/bogus")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::NOT_FOUND);
}

#[actix_rt::test]
async fn test_ogc_styles_metadata() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/ogc/styles/styles/default/metadata")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["id"], "default");
    let rels: Vec<&str> = body["links"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["rel"].as_str().unwrap())
        .collect();
    assert!(rels.contains(&"style"));
    assert!(rels.contains(&"self"));

    // 未知样式 → 404
    let req = test::TestRequest::get()
        .uri("/ogc/styles/styles/bogus/metadata")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::NOT_FOUND);
}

#[actix_rt::test]
async fn test_ogc_styles_create_requires_auth() {
    let app = build_test_app!();

    // 未登录 → 401
    let req = test::TestRequest::post()
        .uri("/ogc/styles/styles")
        .set_json(serde_json::json!({
            "id": "my_style",
            "title": "My Style",
            "content": "<?xml version=\"1.0\"?><StyledLayerDescriptor version=\"1.0.0\"></StyledLayerDescriptor>",
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::UNAUTHORIZED);
}

#[actix_rt::test]
async fn test_ogc_styles_create_get_put_delete() {
    let app = build_test_app!();
    let token = login_admin_token!(&app);

    // 创建 → 201
    let req = test::TestRequest::post()
        .uri("/ogc/styles/styles")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(serde_json::json!({
            "id": "my_style",
            "title": "My Style",
            "description": "A test style",
            "content": "<?xml version=\"1.0\"?><StyledLayerDescriptor version=\"1.0.0\"></StyledLayerDescriptor>",
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        actix_web::http::StatusCode::CREATED,
        "创建样式应返回 201, 实际: {}",
        resp.status()
    );

    // 列表应包含新样式
    let req = test::TestRequest::get()
        .uri("/ogc/styles/styles")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(
        body["styles"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["id"] == "my_style"),
        "样式列表应包含 my_style"
    );

    // GET 内容 → SLD
    let req = test::TestRequest::get()
        .uri("/ogc/styles/styles/my_style")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let content = String::from_utf8_lossy(&test::read_body(resp).await).to_string();
    assert!(content.contains("StyledLayerDescriptor"));

    // PUT 替换 → 200, 内容更新为 CSS
    let css = "* { fill: #ff0000; }";
    let req = test::TestRequest::put()
        .uri("/ogc/styles/styles/my_style")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_payload(css.to_string())
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let req = test::TestRequest::get()
        .uri("/ogc/styles/styles/my_style")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let content_type = resp
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(
        content_type.contains("text/css"),
        "更新后 Content-Type 应为 text/css, 实际: {}",
        content_type
    );
    let content = String::from_utf8_lossy(&test::read_body(resp).await).to_string();
    assert!(content.contains("#ff0000"));

    // DELETE → 204, 之后 404
    let req = test::TestRequest::delete()
        .uri("/ogc/styles/styles/my_style")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::NO_CONTENT);
    let req = test::TestRequest::get()
        .uri("/ogc/styles/styles/my_style")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::NOT_FOUND);
}

#[actix_rt::test]
async fn test_ogc_styles_collections() {
    let app = build_test_app!();

    // world 图层应作为 collection 列出
    let req = test::TestRequest::get()
        .uri("/ogc/styles/collections")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = test::read_body_json(resp).await;
    let colls = body["collections"].as_array().unwrap();
    assert!(colls.iter().any(|c| c["id"] == "world"));

    // world 的样式 (default) 应可列出
    let req = test::TestRequest::get()
        .uri("/ogc/styles/collections/world/styles")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = test::read_body_json(resp).await;
    let styles = body["styles"].as_array().unwrap();
    assert!(
        styles.iter().any(|s| s["id"] == "default"),
        "world 的样式列表应包含 default, 实际: {}",
        body
    );

    // 未知 collection → 404
    let req = test::TestRequest::get()
        .uri("/ogc/styles/collections/bogus/styles")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::NOT_FOUND);
}
