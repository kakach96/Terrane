//! Browse endpoints for file-based data source configuration.
//!
//! - `GET /data-sources/browse` — list a local server directory.
//! - `POST /data-sources/s3/browse` — list an S3 bucket directory using the
//!   connection details supplied in the request body.
//!
//! Both return a `{ path, entries }` payload where `entries` is a list of
//! [`StoreEntry`] (`name` / `path` / `is_dir` / `size`), so the frontend
//! directory picker can reuse a single list model for local and S3 browsing.

use crate::error::GeoServerError;
use crate::handlers::rest_handler::ApiResponse;
use crate::models::DataSourceConnection;
use crate::state::AppState;
use crate::store::{S3FileStore, StoreEntry};
use actix_web::{web, HttpResponse};
use serde::Deserialize;

/// Query parameters for the local browse endpoint.
#[derive(Debug, Deserialize)]
pub struct BrowseLocalQuery {
    /// Absolute directory path to list; defaults to the configured data dir.
    pub path: Option<String>,
}

/// S3 connection fields supplied for a browse request (mirrors the `s3_*`
/// fields on [`DataSourceConnection`]).
#[derive(Debug, Clone, Deserialize)]
pub struct S3BrowseConnection {
    pub s3_endpoint: Option<String>,
    pub s3_region: Option<String>,
    pub s3_bucket: Option<String>,
    pub s3_access_key: Option<String>,
    pub s3_secret_key: Option<String>,
}

/// Request body for the S3 browse endpoint.
#[derive(Debug, Deserialize)]
pub struct BrowseS3Request {
    /// S3 connection fields used to build the client.
    #[serde(flatten)]
    pub connection: S3BrowseConnection,
    /// Object key prefix to list ("" for the bucket root, e.g. "data/").
    pub prefix: Option<String>,
}

/// Response payload shared by both browse endpoints.
#[derive(Debug, serde::Serialize)]
pub struct BrowseResponse {
    /// The directory / prefix that was listed (for breadcrumb display).
    pub path: String,
    pub entries: Vec<StoreEntry>,
}

/// List a local server directory (defaults to `<data_dir>`).
pub async fn browse_local(
    query: web::Query<BrowseLocalQuery>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let root = query
        .path
        .clone()
        .filter(|p| !p.trim().is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| state.config.data_dir.clone());

    let mut entries = Vec::new();
    if let Ok(read_dir) = std::fs::read_dir(&root) {
        for entry in read_dir.flatten() {
            let ft = entry.file_type().ok();
            let is_dir = ft.map(|t| t.is_dir()).unwrap_or(false);
            let size = entry.metadata().ok().map(|m| m.len()).unwrap_or(0);
            entries.push(StoreEntry {
                name: entry.file_name().to_string_lossy().to_string(),
                path: entry.path().to_string_lossy().to_string(),
                is_dir,
                size,
            });
        }
    }
    entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.cmp(&b.name)));

    Ok(
        HttpResponse::Ok().json(ApiResponse::success(BrowseResponse {
            path: root.to_string_lossy().to_string(),
            entries,
        })),
    )
}

/// List an S3 bucket directory using the connection details in the request.
pub async fn browse_s3(body: web::Json<BrowseS3Request>) -> Result<HttpResponse, GeoServerError> {
    let conn = DataSourceConnection {
        file_storage_type: Some("s3".to_string()),
        s3_endpoint: body.connection.s3_endpoint.clone(),
        s3_region: body.connection.s3_region.clone(),
        s3_bucket: body.connection.s3_bucket.clone(),
        s3_access_key: body.connection.s3_access_key.clone(),
        s3_secret_key: body.connection.s3_secret_key.clone(),
        ..Default::default()
    };

    let store = S3FileStore::from_connection(&conn)
        .map_err(|e| GeoServerError::BadRequest(format!("S3 configuration error: {}", e)))?;

    let prefix = body.prefix.clone().unwrap_or_default();
    let entries = store
        .browse(&prefix)
        .await
        .map_err(|e| GeoServerError::BadRequest(format!("S3 browse error: {}", e)))?;

    Ok(
        HttpResponse::Ok().json(ApiResponse::success(BrowseResponse {
            path: prefix,
            entries,
        })),
    )
}
