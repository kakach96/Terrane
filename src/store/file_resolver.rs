//! Helpers to resolve a file-based data source's file for reading.
//!
//! Local data sources (`file_storage_type = "local"`) store the absolute path
//! in `file_path`; readers use it directly. Object-storage data sources
//! (`file_storage_type = "s3"`) store an object key, so the bytes must be
//! fetched before readers can consume them. The helpers here centralize that
//! logic so every handler resolves files the same way:
//!
//! - [`read_bytes`] — read the whole file as bytes (GeoJSON).
//! - [`materialize_file`] — resolve a single file to a local path (GeoPackage,
//!   GeoTIFF / ArcGrid rasters).
//! - [`materialize_dir`] — resolve a multi-file format to a local path while
//!   fetching sibling sidecar objects (Shapefile, WorldImage).

use crate::error::GeoServerError;
use crate::models::DataSourceConnection;
use crate::store::{FileStore, S3FileStore};
use std::path::PathBuf;
use tempfile::TempDir;

/// A file materialized to a local path.
///
/// For local data sources this is the original `file_path`. For object-storage
/// data sources the object is downloaded into a [`TempDir`] which is kept alive
/// by this guard for the duration of the request.
pub struct MaterializedFile {
    /// Kept alive so the temp file survives while the reader uses it.
    pub _dir: Option<TempDir>,
    pub path: PathBuf,
}

/// The normalized storage type of a connection ("local" default / "s3").
pub fn storage_type(conn: &DataSourceConnection) -> &str {
    match conn.file_storage_type.as_deref() {
        Some("s3") => "s3",
        _ => "local",
    }
}

fn map_store_err(e: crate::store::StoreError) -> GeoServerError {
    GeoServerError::InternalError(format!("File storage error: {}", e))
}

fn file_path_of(conn: &DataSourceConnection) -> Option<String> {
    conn.file_path
        .as_deref()
        .filter(|p| !p.trim().is_empty())
        .map(|p| p.to_string())
}

/// Build the file store for a connection (local or s3).
pub fn file_store_from_connection(
    conn: &DataSourceConnection,
) -> Result<Box<dyn FileStore>, GeoServerError> {
    match storage_type(conn) {
        "local" => Ok(Box::new(crate::store::LocalFileStore::new(
            PathBuf::from("."),
        ))),
        "s3" => Ok(Box::new(S3FileStore::from_connection(conn).map_err(map_store_err)?)),
        other => Err(GeoServerError::NotImplemented(format!(
            "Unsupported file storage type: {}",
            other
        ))),
    }
}

/// Read the whole data source file as bytes.
///
/// - local: reads `file_path` from disk.
/// - s3: downloads the object.
/// Returns `None` when the connection has no usable `file_path`.
pub async fn read_bytes(
    conn: &DataSourceConnection,
) -> Result<Option<Vec<u8>>, GeoServerError> {
    let file_path = match file_path_of(conn) {
        Some(p) => p,
        None => return Ok(None),
    };

    match storage_type(conn) {
        "local" => {
            let bytes = std::fs::read(&file_path).map_err(|e| {
                GeoServerError::InternalError(format!("读取文件失败 '{}': {}", file_path, e))
            })?;
            Ok(Some(bytes))
        },
        "s3" => {
            let store = S3FileStore::from_connection(conn).map_err(map_store_err)?;
            match store.get(&file_path).await.map_err(map_store_err)? {
                Some(bytes) => Ok(Some(bytes)),
                None => Err(GeoServerError::NotFound(format!(
                    "S3 object not found: {}",
                    file_path
                ))),
            }
        },
        other => Err(GeoServerError::NotImplemented(format!(
            "Unsupported file storage type: {}",
            other
        ))),
    }
}

/// Resolve a single file to a local path.
///
/// - local: returns `file_path` as-is.
/// - s3: downloads the object to a temp file (kept alive by the guard).
/// Returns `None` when the connection has no usable `file_path`.
pub async fn materialize_file(
    conn: &DataSourceConnection,
) -> Result<Option<MaterializedFile>, GeoServerError> {
    let file_path = match file_path_of(conn) {
        Some(p) => p,
        None => return Ok(None),
    };

    match storage_type(conn) {
        "local" => Ok(Some(MaterializedFile {
            _dir: None,
            path: PathBuf::from(&file_path),
        })),
        "s3" => {
            let store = S3FileStore::from_connection(conn).map_err(map_store_err)?;
            let bytes = store
                .get(&file_path)
                .await
                .map_err(map_store_err)?
                .ok_or_else(|| {
                    GeoServerError::NotFound(format!("S3 object not found: {}", file_path))
                })?;
            let dir = tempfile::Builder::new()
                .prefix("terrane-s3-")
                .tempdir()
                .map_err(GeoServerError::IoError)?;
            let file_name = file_path.rsplit('/').next().unwrap_or(&file_path);
            let path = dir.path().join(sanitize_name(file_name));
            std::fs::write(&path, &bytes).map_err(GeoServerError::IoError)?;
            Ok(Some(MaterializedFile {
                _dir: Some(dir),
                path,
            }))
        },
        other => Err(GeoServerError::NotImplemented(format!(
            "Unsupported file storage type: {}",
            other
        ))),
    }
}

/// Resolve a multi-file format to a local path, fetching sibling sidecars.
///
/// For Shapefile (`file_path` = "…/foo.shp") the `.dbf` / `.shx` / `.prj`
/// objects share the base name and are downloaded into the same temp dir;
/// for WorldImage the `.wld` sidecar is fetched the same way.
/// Returns the path to the main file.
pub async fn materialize_dir(
    conn: &DataSourceConnection,
) -> Result<Option<MaterializedFile>, GeoServerError> {
    let file_path = match file_path_of(conn) {
        Some(p) => p,
        None => return Ok(None),
    };

    match storage_type(conn) {
        "local" => Ok(Some(MaterializedFile {
            _dir: None,
            path: PathBuf::from(&file_path),
        })),
        "s3" => {
            let store = S3FileStore::from_connection(conn).map_err(map_store_err)?;
            // List all keys sharing the base name (without extension) so
            // sidecar files (.shp/.dbf/.shx/.prj/.wld) are fetched too.
            let base = strip_extension(&file_path);
            let keys = store.list_prefix(&base).await.map_err(map_store_err)?;
            if keys.is_empty() {
                return Err(GeoServerError::NotFound(format!(
                    "S3 object not found: {}",
                    file_path
                )));
            }
            let dir = tempfile::Builder::new()
                .prefix("terrane-s3-")
                .tempdir()
                .map_err(GeoServerError::IoError)?;
            for key in &keys {
                if let Some(bytes) = store.get(key).await.map_err(map_store_err)? {
                    let file_name = key.rsplit('/').next().unwrap_or(key);
                    std::fs::write(dir.path().join(sanitize_name(file_name)), &bytes)
                        .map_err(GeoServerError::IoError)?;
                }
            }
            let main_name = file_path.rsplit('/').next().unwrap_or(&file_path);
            let main_path = dir.path().join(sanitize_name(main_name));
            Ok(Some(MaterializedFile {
                _dir: Some(dir),
                path: main_path,
            }))
        },
        other => Err(GeoServerError::NotImplemented(format!(
            "Unsupported file storage type: {}",
            other
        ))),
    }
}

/// Strip the last extension of a path, but only when it is after the last
/// directory separator (e.g. "dir/foo.shp" -> "dir/foo").
fn strip_extension(path: &str) -> String {
    let last_slash = path.rfind('/').unwrap_or(0);
    match path.rfind('.') {
        Some(i) if i > last_slash => path[..i].to_string(),
        _ => path.to_string(),
    }
}

/// Replace path separators / illegal characters so a key can be a file name.
fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect()
}
