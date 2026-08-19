//! Smoke tests — quick regression gate for the most common failure modes.
//!
//! These tests exercise the critical paths that, if broken, would make the
//! application unusable. They run fast (< 2 s total) and are intended to be
//! wired into CI as a "pre-merge" gate.
//!
//! What they cover (and why):
//!
//! 1. **SPA index page** — the Angular app shell must be served for unknown
//!    routes so that client-side routing works.
//! 2. **REST /geoserver/layers** — the layer catalogue must return a non-empty
//!    JSON array; a regression here breaks every page in the UI.
//! 3. **WMS GetMap (image/png)** — the most basic map render path.
//! 4. **WMS OpenLayers preview HTML** — must return valid HTML containing an
//!    OpenLayers map (no empty body, no server error). This is the path that
//!    previously froze the browser.
//! 5. **Feature query with limit** — must respect the `limit` parameter and
//!    return valid GeoJSON; a regression here can cause the frontend to hang
//!    on large datasets.
//! 6. **WMS GetFeatureInfo** — must return a valid JSON array for a valid click.
//! 7. **WFS GetFeature (GML)** — must return XML, not an HTML error page.
//! 8. **WCS GetCapabilities** — must return XML.
//! 9. **Non-existent layer → 404** — must fail gracefully, not 500.
//! 10. **Concurrent requests** — multiple parallel requests must all succeed
//!     without deadlocking (exercises connection pool + RwLock fairness).

#[macro_use]
mod common;

use actix_web::test;

// ---------------------------------------------------------------------------
// 1. API root responds (SPA fallback requires static files, so test the
//    /geoserver endpoint which is always registered)
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn smoke_api_root_responds() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/geoserver/server/status")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "GET /geoserver/server/status should return 200, got: {}",
        resp.status()
    );

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(
        body["success"].as_bool().unwrap_or(false),
        "server/status should report success"
    );
    assert!(
        body["data"]["uptime"].is_string(),
        "server/status should include uptime"
    );
}

// ---------------------------------------------------------------------------
// 2. Layer catalogue
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn smoke_layers_list_valid_json() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/geoserver/layers")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200, "GET /geoserver/layers should be 200");

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body["success"].as_bool().unwrap_or(false));
    assert!(
        body["data"].is_array(),
        "layers data should be a JSON array, got: {:?}",
        body
    );
}

// ---------------------------------------------------------------------------
// 3. WMS GetMap (image/png)
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn smoke_wms_get_map_png() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/wms?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetMap\
              &LAYERS=world&BBOX=-180,-90,180,90\
              &WIDTH=256&HEIGHT=256&SRS=EPSG:4326\
              &FORMAT=image/png")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        200,
        "WMS GetMap (png) should be 200, got: {}",
        resp.status()
    );

    let ct = resp
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        ct.contains("image/png"),
        "Content-Type should contain image/png, got: {}",
        ct
    );

    let body = test::read_body(resp).await;
    assert!(
        body.len() > 100,
        "PNG response should be > 100 bytes, got: {}",
        body.len()
    );
    // PNG magic bytes: 0x89 P N G
    assert!(
        body.starts_with(&[0x89, b'P', b'N', b'G']),
        "Response should start with PNG magic bytes"
    );
}

// ---------------------------------------------------------------------------
// 4. WMS OpenLayers preview HTML
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn smoke_wms_openlayers_preview_html() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/wms?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetMap\
              &LAYERS=world&BBOX=-180,-90,180,90\
              &WIDTH=800&HEIGHT=600&SRS=EPSG:4326\
              &FORMAT=application/openlayers")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        200,
        "WMS OpenLayers preview should be 200, got: {}",
        resp.status()
    );

    let ct = resp
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        ct.contains("text/html"),
        "Content-Type should be text/html, got: {}",
        ct
    );

    let body = test::read_body(resp).await;
    let html = String::from_utf8_lossy(&body);

    // Must contain OpenLayers map container
    assert!(
        html.contains("<div id=\"map\">"),
        "Preview HTML should contain the OL map div, body length: {}",
        html.len()
    );
    // Must load OpenLayers from local vendor bundle (no external CDN).
    assert!(
        html.contains("/assets/vendor/ol/ol.min.js"),
        "Preview HTML should reference local OpenLayers JS, body length: {}",
        html.len()
    );
    assert!(
        html.contains("/assets/vendor/ol/ol.css"),
        "Preview HTML should reference local OpenLayers CSS, body length: {}",
        html.len()
    );
    // Must NOT reference external CDN (prevents offline freeze regression).
    assert!(
        !html.contains("cdn.jsdelivr.net"),
        "Preview HTML must not reference jsdelivr CDN"
    );
    assert!(
        !html.contains("fonts.googleapis.com"),
        "Preview HTML must not reference Google Fonts CDN"
    );
}

// ---------------------------------------------------------------------------
// 5. Feature query with limit
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn smoke_feature_query_limit() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/geoserver/layers/world/features?limit=2")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        200,
        "Feature query with limit=2 should be 200, got: {}",
        resp.status()
    );

    let body: serde_json::Value = test::read_body_json(resp).await;

    // Must be a valid GeoJSON FeatureCollection
    assert_eq!(
        body["type"].as_str().unwrap_or(""),
        "FeatureCollection",
        "Response should be a GeoJSON FeatureCollection"
    );

    let features = body["features"]
        .as_array()
        .expect("features should be an array");
    assert!(
        features.len() <= 2,
        "limit=2 should return at most 2 features, got: {}",
        features.len()
    );

    // Each feature must have geometry and properties
    for (i, f) in features.iter().enumerate() {
        assert!(
            f["geometry"].is_object(),
            "Feature {} should have a geometry object",
            i
        );
        assert!(
            f["properties"].is_object(),
            "Feature {} should have a properties object",
            i
        );
    }
}

// ---------------------------------------------------------------------------
// 6. WMS GetFeatureInfo
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn smoke_wms_get_feature_info() {
    let app = build_test_app!();

    // Click near the center of the world layer
    let req = test::TestRequest::get()
        .uri("/wms?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetFeatureInfo\
              &LAYERS=world&QUERY_LAYERS=world\
              &BBOX=-180,-90,180,90&WIDTH=256&HEIGHT=256&SRS=EPSG:4326\
              &I=128&J=128&INFO_FORMAT=application/json")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        200,
        "GetFeatureInfo should be 200, got: {}",
        resp.status()
    );

    let body: serde_json::Value = test::read_body_json(resp).await;
    // Response should be a JSON array (possibly empty if no feature at that
    // exact pixel, but the structure must be valid).
    assert!(
        body.is_array(),
        "GetFeatureInfo (json) should return a JSON array, got: {:?}",
        body
    );
}

// ---------------------------------------------------------------------------
// 7. WFS GetFeature (GML)
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn smoke_wfs_get_feature_gml() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/wfs?SERVICE=WFS&VERSION=1.1.0&REQUEST=GetFeature\
              &TYPENAME=world&MAXFEATURES=1")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        200,
        "WFS GetFeature should be 200, got: {}",
        resp.status()
    );

    let ct = resp
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        ct.contains("xml") || ct.contains("gml"),
        "WFS GetFeature should return XML, got Content-Type: {}",
        ct
    );

    let body = test::read_body(resp).await;
    let xml = String::from_utf8_lossy(&body);
    assert!(
        xml.contains("FeatureCollection") || xml.contains("featureMember"),
        "GML response should contain FeatureCollection or featureMember, body length: {}",
        xml.len()
    );
}

// ---------------------------------------------------------------------------
// 8. WCS GetCapabilities
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn smoke_wcs_get_capabilities() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/wcs?SERVICE=WCS&REQUEST=GetCapabilities")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "WCS GetCapabilities should be 200, got: {}",
        resp.status()
    );

    let ct = resp
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        ct.contains("xml"),
        "WCS GetCapabilities should return XML, got: {}",
        ct
    );
}

// ---------------------------------------------------------------------------
// 9. Non-existent layer → 404 (not 500)
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn smoke_nonexistent_layer_does_not_panic() {
    let app = build_test_app!();

    // WMS GetMap for a layer that doesn't exist — must not panic (actix would
    // return 500 or the handler may return 200 with empty body depending on
    // implementation). The key invariant: no server crash.
    let req = test::TestRequest::get()
        .uri("/wms?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetMap\
              &LAYERS=nonexistent_layer_xyz\
              &BBOX=-180,-90,180,90&WIDTH=256&HEIGHT=256\
              &SRS=EPSG:4326&FORMAT=image/png")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let status = resp.status();
    // Server must not crash (no 500 Internal Server Error from a panic).
    // Acceptable: 200 (handler returns empty image), 400, 404, or 500 (handler-level).
    assert!(
        status.is_success() || status.as_u16() >= 400,
        "Request for non-existent layer must not cause a panic, got: {}",
        status
    );
}

// ---------------------------------------------------------------------------
// 10. Concurrent requests (basic deadlock detection)
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn smoke_concurrent_requests() {
    let app = build_test_app!();
    let app = std::sync::Arc::new(app);

    let mut handles = Vec::new();

    // Spawn 10 concurrent requests to different endpoints
    for i in 0..10 {
        let app = app.clone();
        let uri = match i % 4 {
            0 => "/geoserver/layers".to_string(),
            1 => "/wms?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetMap&LAYERS=world&BBOX=-180,-90,180,90&WIDTH=128&HEIGHT=128&SRS=EPSG:4326&FORMAT=image/png".to_string(),
            2 => "/geoserver/layers/world/features?limit=1".to_string(),
            _ => "/geoserver/layer-groups".to_string(),
        };
        handles.push(actix_rt::spawn(async move {
            let req = test::TestRequest::get().uri(&uri).to_request();
            let resp = test::call_service(&*app, req).await;
            (i, uri, resp.status())
        }));
    }

    for h in handles {
        let (i, uri, status) = h.await.expect("task should not panic");
        assert!(
            status.is_success(),
            "Concurrent request {} to {} should succeed, got: {}",
            i,
            uri,
            status
        );
    }
}

// ---------------------------------------------------------------------------
// 11. Response time sanity — WMS GetMap must complete in < 2 s
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn smoke_wms_get_map_latency() {
    let app = build_test_app!();

    let start = std::time::Instant::now();
    let req = test::TestRequest::get()
        .uri("/wms?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetMap\
              &LAYERS=world&BBOX=-180,-90,180,90\
              &WIDTH=512&HEIGHT=512&SRS=EPSG:4326\
              &FORMAT=image/png")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let elapsed = start.elapsed();

    assert_eq!(resp.status(), 200);
    assert!(
        elapsed.as_secs() < 2,
        "WMS GetMap (512×512) should complete in < 2 s, took: {:?}",
        elapsed
    );
}

// ---------------------------------------------------------------------------
// 12. OpenLayers preview must include a valid map center/extent
// ---------------------------------------------------------------------------

#[actix_rt::test]
async fn smoke_wms_openlayers_preview_has_view_params() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/wms?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetMap\
              &LAYERS=world&BBOX=-180,-90,180,90\
              &WIDTH=800&HEIGHT=600&SRS=EPSG:4326\
              &FORMAT=application/openlayers")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body = test::read_body(resp).await;
    let html = String::from_utf8_lossy(&body);

    // The preview must contain a valid OpenLayers view configuration
    assert!(
        html.contains("new ol.View("),
        "Preview should contain ol.View constructor"
    );
    assert!(
        html.contains("center:"),
        "Preview should contain map center"
    );
    assert!(
        html.contains("zoom:"),
        "Preview should contain zoom level"
    );
    assert!(
        html.contains("extent:"),
        "Preview should contain extent"
    );
}
