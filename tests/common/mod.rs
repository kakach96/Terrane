//! Shared helpers for the protocol integration tests.
//!
//! `tests/common/` is intentionally *not* a test crate (Rust convention): only
//! files directly under `tests/` are compiled as separate test binaries, so this
//! module is compiled into every test crate that does `#[macro_use] mod common;`.

use std::path::PathBuf;

/// Build a test `GeoServerConfig`:
/// - in-memory SQLite metadata store
/// - vector store reuses the metadata store (no files written to `./data/business`)
/// - tile cache disabled (no files written to `./data/gwc`)
/// - one default workspace `default` → store `shapes` → layer `world` (EPSG:4326)
pub fn create_test_config() -> terrane::config::GeoServerConfig {
    let mut config = terrane::config::GeoServerConfig::default();
    config.server.host = "127.0.0.1".to_string();
    config.server.port = 0;
    config.metadata.sqlite_path = ":memory:".into();

    // 矢量存储复用元数据存储, 避免测试写入 ./data/business
    config.vector = Some(terrane::config::VectorConfig {
        kind: "metadata".to_string(),
        dir: None,
        postgres: Default::default(),
    });
    // 禁用瓦片缓存, 避免测试写入 ./data/gwc
    config.cache = Some(terrane::config::CacheConfig {
        kind: "local".to_string(),
        cache_dir: PathBuf::from(std::env::temp_dir()).join("terrane-test-gwc"),
        meta_dir: PathBuf::from(std::env::temp_dir()).join("terrane-test-gwc-meta"),
        expire_after_secs: 0,
        max_tiles: 0,
        enabled: false,
        default_gridset: "EPSG:4326".to_string(),
        session_ttl_secs: 300,
    });

    config.workspaces = vec![
        terrane::config::WorkspaceConfig {
            name: "default".to_string(),
            uri: "http://geoserver.org/default".to_string(),
            stores: vec![
                terrane::config::StoreConfig {
                    name: "shapes".to_string(),
                    store_type: "DataStore".to_string(),
                    path: "./data".to_string(),
                    layers: vec![
                        terrane::config::LayerConfig {
                            name: "world".to_string(),
                            title: "World".to_string(),
                            abstract_text: "World layer".to_string(),
                            srs: "EPSG:4326".to_string(),
                            bounds: terrane::config::BoundsConfig {
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

/// Build a fully-initialized test application: in-memory SQLite + the default
/// `world` layer, wired to the real route table under `/geoserver`.
///
/// Consumed via `#[macro_use] mod common;` at the top of each protocol test
/// crate (e.g. `tests/wms_test.rs`). The macro uses fully-qualified paths, so
/// test files only need the `mod common;` declaration.
macro_rules! build_test_app {
    () => {{
        let config = common::create_test_config();
        let state = actix_web::web::Data::new(terrane::state::AppState::new(config).await);
        actix_web::test::init_service(
            actix_web::App::new()
                .app_data(state.clone())
                .wrap(actix_web::middleware::Logger::default())
                .configure(|svc| terrane::routes::configure_routes(svc, "/geoserver"))
        )
        .await
    }};
}

/// Log in as the default administrator (admin/geoserver) over HTTP and return
/// the JWT token. Used by authenticated REST endpoints (permissions/backup).
///
/// Implemented as a macro so that it expands at the call site where `$app`'s
/// concrete type is known, avoiding a generic helper signature for actix's
/// `App<S>` type.
#[allow(unused_macros)]
macro_rules! login_admin_token {
    ($app:expr) => {{
        let req = actix_web::test::TestRequest::post()
            .uri("/geoserver/auth/login")
            .set_json(&serde_json::json!({ "username": "admin", "password": "geoserver" }))
            .to_request();
        let resp = actix_web::test::call_service(&$app, req).await;
        let body: serde_json::Value = actix_web::test::read_body_json(resp).await;
        body["data"]["token"].as_str().unwrap_or("").to_string()
    }};
}
