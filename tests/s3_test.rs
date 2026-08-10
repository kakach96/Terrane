//! S3 object storage integration tests (live).
//!
//! Requires an S3-compatible server at `127.0.0.1:9000` (e.g. MinIO from
//! `build/docker-compose.yml`, default credentials `terrane` / `terrane-secret`).
//!
//! ```bash
//! docker compose -f build/docker-compose.yml up -d minio
//! cargo test --test s3_test -- --ignored
//! ```

#[macro_use]
mod common;

use terrane::models::DataSourceConnection;
use terrane::store::{FileStore, S3FileStore};

const ENDPOINT: &str = "http://127.0.0.1:9000";
const REGION: &str = "us-east-1";
const ACCESS_KEY: &str = "terrane";
const SECRET_KEY: &str = "terrane-secret";

fn s3_conn(bucket: &str, key: &str) -> DataSourceConnection {
    DataSourceConnection {
        file_storage_type: Some("s3".to_string()),
        file_path: Some(key.to_string()),
        s3_endpoint: Some(ENDPOINT.to_string()),
        s3_region: Some(REGION.to_string()),
        s3_bucket: Some(bucket.to_string()),
        s3_access_key: Some(ACCESS_KEY.to_string()),
        s3_secret_key: Some(SECRET_KEY.to_string()),
        ..Default::default()
    }
}

/// 创建一次性的 bucket (名字带 pid, 避免并行冲突); 已存在则跳过。
async fn ensure_bucket(bucket: &str) {
    use s3::creds::Credentials;
    use s3::{region::Region, Bucket, BucketConfiguration};

    let region = Region::Custom {
        region: REGION.into(),
        endpoint: ENDPOINT.into(),
    };
    let credentials =
        Credentials::new(Some(ACCESS_KEY), Some(SECRET_KEY), None, None, None).unwrap();

    let probe = Bucket::new(bucket, region.clone(), credentials.clone())
        .unwrap()
        .with_path_style();
    if probe.exists().await.unwrap_or(false) {
        return;
    }
    Bucket::create_with_path_style(bucket, region, credentials, BucketConfiguration::default())
        .await
        .unwrap();
}

/// 删除一次性 bucket (须为空)。
async fn delete_bucket(bucket: &str) {
    use s3::creds::Credentials;
    use s3::{region::Region, Bucket};

    let region = Region::Custom {
        region: REGION.into(),
        endpoint: ENDPOINT.into(),
    };
    let credentials =
        Credentials::new(Some(ACCESS_KEY), Some(SECRET_KEY), None, None, None).unwrap();
    if let Ok(b) = Bucket::new(bucket, region, credentials) {
        let _ = b.with_path_style().delete().await;
    }
}

#[actix_rt::test]
#[ignore]
async fn test_s3_file_store_roundtrip_and_browse() {
    let bucket = format!("terrane-s3test-{}", std::process::id());
    ensure_bucket(&bucket).await;

    let conn = s3_conn(&bucket, "data/sample.geojson");
    let store = S3FileStore::from_connection(&conn).unwrap();

    // put / get
    store
        .put(
            "data/sample.geojson",
            b"{\"type\":\"FeatureCollection\",\"features\":[]}",
        )
        .await
        .unwrap();
    assert!(
        store.get("data/sample.geojson").await.unwrap().is_some(),
        "get 应返回已上传对象"
    );
    assert!(
        store.get("missing.key").await.unwrap().is_none(),
        "不存在的对象应返回 None"
    );

    // list / list_prefix
    let keys = store.list_all_keys().await.unwrap();
    assert!(
        keys.iter().any(|k| k == "data/sample.geojson"),
        "list_all_keys 应包含 data/sample.geojson"
    );
    let prefixed = store.list_prefix("data/sample").await.unwrap();
    assert!(
        prefixed.iter().any(|k| k == "data/sample.geojson"),
        "list_prefix 应找到 data/sample.geojson"
    );

    // browse: 根目录应看到 data 目录; data/ 下应看到 sample.geojson
    let root = store.browse("").await.unwrap();
    assert!(
        root.iter()
            .any(|e| e.name == "data" && e.is_dir && e.path == "data/"),
        "根目录浏览应包含目录 data (path 带尾部斜杠), 实际: {:?}",
        root
    );
    let data = store.browse("data/").await.unwrap();
    assert!(
        data.iter().any(|e| e.name == "sample.geojson" && !e.is_dir),
        "data/ 浏览应包含文件 sample.geojson, 实际: {:?}",
        data
    );

    // delete
    store.delete("data/sample.geojson").await.unwrap();
    assert!(
        store.get("data/sample.geojson").await.unwrap().is_none(),
        "删除后 get 应返回 None"
    );

    delete_bucket(&bucket).await;
}

#[actix_rt::test]
#[ignore]
async fn test_s3_geojson_data_source_features() {
    let bucket = format!("terrane-s3feat-{}", std::process::id());
    ensure_bucket(&bucket).await;

    // 上传含 2 个点的 GeoJSON 到 bucket
    let store = S3FileStore::from_connection(&s3_conn(&bucket, "points.geojson")).unwrap();
    let geojson = r#"{
        "type": "FeatureCollection",
        "features": [
            { "type": "Feature", "id": "1", "geometry": { "type": "Point", "coordinates": [10, 20] }, "properties": { "name": "A" } },
            { "type": "Feature", "id": "2", "geometry": { "type": "Point", "coordinates": [30, 40] }, "properties": { "name": "B" } }
        ]
    }"#;
    store.put("points.geojson", geojson.as_bytes()).await.unwrap();

    // 测试 app (内存 SQLite + 路由)
    let config = common::create_test_config();
    let state = actix_web::web::Data::new(terrane::state::AppState::new(config).await);
    let app = actix_web::test::init_service(
        actix_web::App::new()
            .app_data(state.clone())
            .configure(|svc| terrane::routes::configure_routes(svc, "/geoserver")),
    )
    .await;

    // 创建 S3 数据源
    let create_ds = actix_web::test::TestRequest::post()
        .uri("/geoserver/data-sources")
        .set_json(&serde_json::json!({
            "name": "s3_ds",
            "type": "geojson",
            "workspace": "default",
            "enabled": true,
            "connection": {
                "file_path": "points.geojson",
                "file_storage_type": "s3",
                "s3_endpoint": ENDPOINT,
                "s3_region": REGION,
                "s3_bucket": bucket,
                "s3_access_key": ACCESS_KEY,
                "s3_secret_key": SECRET_KEY
            }
        }))
        .to_request();
    let resp = actix_web::test::call_service(&app, create_ds).await;
    assert_eq!(resp.status(), 201, "创建 S3 数据源应返回 201");

    // 创建图层
    let create_layer = actix_web::test::TestRequest::post()
        .uri("/geoserver/layers")
        .set_json(&serde_json::json!({
            "name": "s3_points",
            "title": "S3 Points",
            "workspace": "default",
            "store": "s3_ds",
            "native_name": "points.geojson",
            "srs": "EPSG:4326",
            "minx": 0.0, "miny": 0.0, "maxx": 40.0, "maxy": 40.0
        }))
        .to_request();
    let resp = actix_web::test::call_service(&app, create_layer).await;
    assert_eq!(resp.status(), 201, "创建图层应返回 201");

    // 查询要素 (应读到 S3 上的 GeoJSON, 2 条)
    let req = actix_web::test::TestRequest::get()
        .uri("/geoserver/layers/s3_points/features")
        .to_request();
    let resp = actix_web::test::call_service(&app, req).await;
    let status = resp.status();
    if !status.is_success() {
        let bytes = actix_web::test::read_body(resp).await;
        panic!(
            "要素查询应成功, status={:?}, body={:?}",
            status,
            String::from_utf8_lossy(&bytes)
        );
    }
    let body: serde_json::Value = actix_web::test::read_body_json(resp).await;
    let empty: Vec<serde_json::Value> = Vec::new();
    // /layers/{name}/features 返回裸 GeoJSON FeatureCollection: { type, totalFeatures, features }
    let features = body["features"].as_array().unwrap_or(&empty);
    assert_eq!(
        features.len(),
        2,
        "S3 GeoJSON 数据源应返回 2 条要素, 实际: {:?}",
        features
    );

    delete_bucket(&bucket).await;
}
