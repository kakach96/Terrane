//! Helpers to resolve a file-based data source's file for reading.
//!
//! Local data sources (`file_storage_type = "local"`) store an absolute path
//! in `file_path`; readers use it directly. Object-storage data sources
//! (`file_storage_type = "s3"`) store an object key, so the bytes must be
//! fetched before readers can consume them. The helpers here centralize that
//! logic so every handler resolves files the same way:
//!
//! - [`resolve_file_path`] — join a layer's `native_name` onto a directory
//!   data source (directory-level data sources publish one file per layer).
//! - [`read_bytes_for`] — read the whole file as bytes (GeoJSON).
//! - [`materialize_file_for`] — resolve a single file to a local path
//!   (GeoPackage, GeoTIFF / ArcGrid rasters).
//! - [`materialize_dir_for`] — resolve a multi-file format to a local path
//!   while fetching sibling sidecar objects (Shapefile, WorldImage).
//!
//! The legacy [`materialize_file`] helper resolves the data source's own
//! `file_path` and is kept for callers that have no layer context; the others
//! take an optional `native_name` which defaults to the data source `file_path`.

use crate::error::TerraneError;
use crate::models::DataSourceConnection;
use crate::store::{FileStore, S3FileStore};
use std::path::{Path, PathBuf};
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

fn map_store_err(e: crate::store::StoreError) -> TerraneError {
    TerraneError::InternalError(format!("File storage error: {}", e))
}

fn file_path_of(conn: &DataSourceConnection) -> Option<String> {
    conn.file_path
        .as_deref()
        .filter(|p| !p.trim().is_empty())
        .map(|p| p.to_string())
}

/// Normalize a stored path/key to forward slashes for joining decisions.
fn normalized(file_path: &str) -> String {
    file_path.replace('\\', "/")
}

/// Resolve the actual file a layer reads from a file-based data source.
///
/// Directory-level file data sources (`file_path` points at a directory /
/// object prefix) publish one file per layer: the layer's `native_name` is
/// the file name (or relative key) inside the directory. Returns the joined
/// path so callers can pass it to the materialize helpers below.
///
/// Fallback for legacy single-file data sources: when `native_name` is empty
/// or the connection has no `file_path`, `file_path` itself is returned.
pub fn resolve_file_path(conn: &DataSourceConnection, native_name: Option<&str>) -> Option<String> {
    let file_path = file_path_of(conn)?;
    let native = native_name
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .unwrap_or("");
    if native.is_empty() {
        return Some(file_path);
    }

    match storage_type(conn) {
        "s3" => {
            // Object keys always use '/' separators. Only join when the
            // connection is directory-level (prefix ending in '/') AND the
            // native_name is a single-level file name — otherwise a legacy
            // single-file object (e.g. a .gpkg whose native_name is an internal
            // table) must keep `file_path` untouched.
            if is_directory_connection(conn) && is_single_component(native) {
                let base = normalized(file_path.trim_end_matches('/'));
                let name = normalized(native.trim_start_matches('/'));
                if base.is_empty() {
                    Some(name)
                } else {
                    Some(format!("{}/{}", base, name))
                }
            } else {
                Some(file_path)
            }
        },
        _ => {
            let base = Path::new(&file_path);
            // Only join when `file_path` is a directory; a file path with a
            // stray `native_name` keeps the legacy behavior (use file_path).
            // `native` must be a plain file name (no `..`, no separators) to
            // avoid escaping the directory.
            if base.is_dir() && is_single_component(native) {
                Some(base.join(native).to_string_lossy().to_string())
            } else {
                Some(file_path)
            }
        },
    }
}

/// True when `name` is a single plain file name (no path separators, no
/// parent/root/dir components), used to guard directory joins against traversal.
fn is_single_component(name: &str) -> bool {
    if name.is_empty() || name.contains(['/', '\\']) {
        return false;
    }
    let mut comps = Path::new(name).components();
    match comps.next() {
        Some(std::path::Component::Normal(_)) => comps.next().is_none(),
        _ => false,
    }
}

/// True when the connection's `file_path` points at a directory (local) or
/// ends with a `/` prefix marker (s3). Used to decide whether a data source
/// is directory-level.
pub fn is_directory_connection(conn: &DataSourceConnection) -> bool {
    match file_path_of(conn) {
        Some(p) => match storage_type(conn) {
            "s3" => normalized(p.as_str()).ends_with('/') || p.trim_end_matches('/').is_empty(),
            _ => Path::new(&p).is_dir(),
        },
        None => false,
    }
}

/// Build the file store for a connection (local or s3).
pub fn file_store_from_connection(
    conn: &DataSourceConnection,
) -> Result<Box<dyn FileStore>, TerraneError> {
    match storage_type(conn) {
        "local" => Ok(Box::new(crate::store::LocalFileStore::new(PathBuf::from(
            ".",
        )))),
        "s3" => Ok(Box::new(
            S3FileStore::from_connection(conn).map_err(map_store_err)?,
        )),
        other => Err(TerraneError::NotImplemented(format!(
            "Unsupported file storage type: {}",
            other
        ))),
    }
}

/// Read the whole data source file as bytes.
///
/// - local: reads `file_path` from disk.
/// - s3: downloads the object.
///
/// Returns `None` when the connection has no usable `file_path`.
///
/// Resolves `native_name` inside a directory-level data source first; pass
/// `None` to read the data source's own `file_path`.
pub async fn read_bytes_for(
    conn: &DataSourceConnection,
    native_name: Option<&str>,
) -> Result<Option<Vec<u8>>, TerraneError> {
    let file_path = match resolve_file_path(conn, native_name) {
        Some(p) => p,
        None => return Ok(None),
    };

    match storage_type(conn) {
        "local" => {
            let bytes = std::fs::read(&file_path).map_err(|e| {
                TerraneError::InternalError(format!("读取文件失败 '{}': {}", file_path, e))
            })?;
            Ok(Some(bytes))
        },
        "s3" => {
            let store = S3FileStore::from_connection(conn).map_err(map_store_err)?;
            match store.get(&file_path).await.map_err(map_store_err)? {
                Some(bytes) => Ok(Some(bytes)),
                None => Err(TerraneError::NotFound(format!(
                    "S3 object not found: {}",
                    file_path
                ))),
            }
        },
        other => Err(TerraneError::NotImplemented(format!(
            "Unsupported file storage type: {}",
            other
        ))),
    }
}

/// Resolve a single file to a local path.
///
/// - local: returns `file_path` as-is.
/// - s3: downloads the object to a temp file (kept alive by the guard).
///
/// Returns `None` when the connection has no usable `file_path`.
pub async fn materialize_file(
    conn: &DataSourceConnection,
) -> Result<Option<MaterializedFile>, TerraneError> {
    materialize_file_for(conn, None).await
}

/// [`materialize_file`] with layer context: resolves `native_name` inside a
/// directory-level data source first.
pub async fn materialize_file_for(
    conn: &DataSourceConnection,
    native_name: Option<&str>,
) -> Result<Option<MaterializedFile>, TerraneError> {
    let file_path = match resolve_file_path(conn, native_name) {
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
                    TerraneError::NotFound(format!("S3 object not found: {}", file_path))
                })?;
            let dir = tempfile::Builder::new()
                .prefix("terrane-s3-")
                .tempdir()
                .map_err(TerraneError::IoError)?;
            let file_name = file_path.rsplit('/').next().unwrap_or(&file_path);
            let path = dir.path().join(sanitize_name(file_name));
            std::fs::write(&path, &bytes).map_err(TerraneError::IoError)?;
            Ok(Some(MaterializedFile {
                _dir: Some(dir),
                path,
            }))
        },
        other => Err(TerraneError::NotImplemented(format!(
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
///
/// Resolves `native_name` inside a directory-level data source first, so
/// sidecar keys are derived from the resolved file instead of the directory
/// prefix; pass `None` for the data source's own `file_path`.
pub async fn materialize_dir_for(
    conn: &DataSourceConnection,
    native_name: Option<&str>,
) -> Result<Option<MaterializedFile>, TerraneError> {
    let file_path = match resolve_file_path(conn, native_name) {
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
                return Err(TerraneError::NotFound(format!(
                    "S3 object not found: {}",
                    file_path
                )));
            }
            let dir = tempfile::Builder::new()
                .prefix("terrane-s3-")
                .tempdir()
                .map_err(TerraneError::IoError)?;
            for key in &keys {
                if let Some(bytes) = store.get(key).await.map_err(map_store_err)? {
                    let file_name = key.rsplit('/').next().unwrap_or(key);
                    std::fs::write(dir.path().join(sanitize_name(file_name)), &bytes)
                        .map_err(TerraneError::IoError)?;
                }
            }
            let main_name = file_path.rsplit('/').next().unwrap_or(&file_path);
            let main_path = dir.path().join(sanitize_name(main_name));
            Ok(Some(MaterializedFile {
                _dir: Some(dir),
                path: main_path,
            }))
        },
        other => Err(TerraneError::NotImplemented(format!(
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

#[cfg(test)]
mod tests {
    use super::*;

    fn conn(file_path: &str, storage: &str) -> DataSourceConnection {
        DataSourceConnection {
            file_path: Some(file_path.to_string()),
            file_storage_type: Some(storage.to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn test_is_single_component() {
        assert!(is_single_component("a.geojson"));
        assert!(is_single_component("a_b.c.d"));
        assert!(!is_single_component(""));
        assert!(!is_single_component(".."));
        assert!(!is_single_component("."));
        assert!(!is_single_component("sub/a.geojson"));
        assert!(!is_single_component("../a.geojson"));
        assert!(!is_single_component("a\\b.geojson"));
        assert!(!is_single_component("/abs"));
    }

    #[test]
    fn test_resolve_file_path_guards_traversal() {
        let dir = std::env::temp_dir().join("terrane-resolve-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let dir_str = dir.to_string_lossy().to_string();

        // 合法文件名 → join
        let c = conn(&dir_str, "local");
        assert_eq!(
            resolve_file_path(&c, Some("a.geojson")),
            Some(
                std::path::Path::new(&dir_str)
                    .join("a.geojson")
                    .to_string_lossy()
                    .to_string()
            )
        );
        // 穿越尝试 → 保持原 file_path (不逃逸目录)
        assert_eq!(
            resolve_file_path(&c, Some("../a.geojson")),
            Some(dir_str.clone())
        );
        assert_eq!(
            resolve_file_path(&c, Some("sub/a.geojson")),
            Some(dir_str.clone())
        );
        assert_eq!(resolve_file_path(&c, Some("..")), Some(dir_str.clone()));
        // 单文件数据源 (file_path 指向文件) → 忽略 native_name
        let f = dir.join("single.geojson");
        std::fs::write(&f, "{}").unwrap();
        let c = conn(f.to_string_lossy().as_ref(), "local");
        assert_eq!(
            resolve_file_path(&c, Some("other.geojson")),
            Some(f.to_string_lossy().to_string())
        );
        // s3: 目录前缀 + 合法文件名 → 拼接
        let c = conn("bucket/data/", "s3");
        assert_eq!(
            resolve_file_path(&c, Some("a.geojson")),
            Some("bucket/data/a.geojson".to_string())
        );
        // s3: 穿越尝试 → 保持前缀
        assert_eq!(
            resolve_file_path(&c, Some("../../a.geojson")),
            Some("bucket/data/".to_string())
        );
        // s3 单文件对象 (非目录前缀): native_name 不参与拼接
        // (旧模型 GeoPackage 的 native_name 是文件内表名)。
        let c = conn("bucket/data/roads.gpkg", "s3");
        assert_eq!(
            resolve_file_path(&c, Some("roads")),
            Some("bucket/data/roads.gpkg".to_string())
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
