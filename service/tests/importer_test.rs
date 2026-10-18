//! Bulk import (Importer first surface) integration tests —
//! `POST /terrane/importer/uploads`.
//!
//! Covers: zip import publishing every recognized member (GeoJSON + GeoTIFF,
//! reusing the curated `service/samples/` files), layer creation with
//! computed bounds, idempotent re-import (skipped), and single-file import.
//! Unpacked files land in a unique temp dir per test — `service/data` is
//! never touched.

#[macro_use]
mod common;

use actix_web::{http::StatusCode, test};

/// Same as `build_test_app!` but redirects `data_dir` into a unique temp
/// directory so the importer never writes into `service/data`.
macro_rules! build_import_app {
    ($dir:expr) => {{
        let mut config = common::create_test_config();
        config.data_dir = $dir.clone();
        let state = actix_web::web::Data::new(terrane::state::AppState::new(config).await);
        actix_web::test::init_service(
            actix_web::App::new()
                .app_data(state.clone())
                .wrap(actix_web::middleware::Logger::default())
                .configure(|svc| terrane::routes::configure_routes(svc, "/terrane")),
        )
        .await
    }};
}

/// Build a zip archive in memory from (name, bytes) pairs.
fn build_zip(members: &[(&str, Vec<u8>)]) -> Vec<u8> {
    use std::io::Write;
    let mut cursor = std::io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut cursor);
        for (name, bytes) in members {
            let opts = zip::write::SimpleFileOptions::default();
            zip.start_file(*name, opts).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
    }
    cursor.into_inner()
}

fn read_sample(name: &str) -> Vec<u8> {
    std::fs::read(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("samples")
            .join(name),
    )
    .unwrap_or_else(|e| panic!("read sample {}: {}", name, e))
}

fn multipart_body(boundary: &str, filename: &str, content_type: &str, data: &[u8]) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(
        format!(
            "Content-Disposition: form-data; name=\"file\"; filename=\"{}\"\r\n",
            filename
        )
        .as_bytes(),
    );
    body.extend_from_slice(format!("Content-Type: {}\r\n\r\n", content_type).as_bytes());
    body.extend_from_slice(data);
    body.extend_from_slice(format!("\r\n--{}--\r\n", boundary).as_bytes());
    body
}

/// Zip import: publishes GeoJSON vector + GeoTIFF raster from the curated
/// samples, then a re-import is reported as skipped (idempotent).
#[actix_web::test]
async fn test_importer_zip_publishes_members() {
    let dir = std::env::temp_dir().join(format!("terrane-import-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let app = build_import_app!(dir);

    let zip_data = build_zip(&[
        ("major_cities.geojson", read_sample("major_cities.geojson")),
        ("sample_elevation.tif", read_sample("sample_elevation.tif")),
        ("readme.txt", b"ignored".to_vec()),
    ]);

    let boundary = "----terrane-import-boundary";
    let body = multipart_body(boundary, "batch.zip", "application/zip", &zip_data);

    let req = test::TestRequest::post()
        .uri("/terrane/importer/uploads?workspace=importws")
        .insert_header((
            actix_web::http::header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={}", boundary),
        ))
        .set_payload(body)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "importer zip upload should succeed, got {}",
        resp.status()
    );
    let report: serde_json::Value = test::read_body_json(resp).await;
    let data = &report["data"];
    assert_eq!(
        data["created"], 2,
        "two members should be published: {}",
        report
    );
    assert_eq!(data["skipped"], 1, "unrecognized readme.txt is skipped");

    // Re-import: everything already exists → all skipped.
    let boundary2 = "----terrane-import-boundary2";
    let body2 = multipart_body(boundary2, "batch.zip", "application/zip", &zip_data);
    let req = test::TestRequest::post()
        .uri("/terrane/importer/uploads?workspace=importws")
        .insert_header((
            actix_web::http::header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={}", boundary2),
        ))
        .set_payload(body2)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let report2: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(report2["data"]["created"], 0, "re-import creates nothing");
    assert_eq!(report2["data"]["skipped"], 3);

    std::fs::remove_dir_all(&dir).ok();
}

/// Single-file import: a GeoJSON file published directly (data source + layer).
#[actix_web::test]
async fn test_importer_single_geojson_file() {
    let dir = std::env::temp_dir().join(format!("terrane-import-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let app = build_import_app!(dir);

    let geojson = read_sample("major_cities.geojson");
    let boundary = "----terrane-import-single";
    let body = multipart_body(
        boundary,
        "my_cities.geojson",
        "application/geo+json",
        &geojson,
    );

    let req = test::TestRequest::post()
        .uri("/terrane/importer/uploads?workspace=default")
        .insert_header((
            actix_web::http::header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={}", boundary),
        ))
        .set_payload(body)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "single file import failed: {}",
        resp.status()
    );
    let report: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(report["data"]["created"], 1);
    assert_eq!(report["data"]["entries"][0]["type_name"], "geojson");
    assert_eq!(report["data"]["entries"][0]["layer"], "my_cities");

    std::fs::remove_dir_all(&dir).ok();
}

/// Unsupported format → 400.
#[actix_web::test]
async fn test_importer_unsupported_format_rejected() {
    let dir = std::env::temp_dir().join(format!("terrane-import-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let app = build_import_app!(dir);

    let boundary = "----terrane-import-bad";
    let body = multipart_body(boundary, "notes.txt", "text/plain", b"hello");
    let req = test::TestRequest::post()
        .uri("/terrane/importer/uploads")
        .insert_header((
            actix_web::http::header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={}", boundary),
        ))
        .set_payload(body)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    std::fs::remove_dir_all(&dir).ok();
}
