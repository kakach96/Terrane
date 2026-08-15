//! `/resources` endpoints — manage files under the configured data directory.
//!
//! - `GET /resources?path=<rel>` — list a directory (relative to `data_dir`).
//! - `POST /resources?path=<rel>` — upload a file (multipart, admin auth).
//! - `DELETE /resources?path=<rel>` — delete a file (admin auth).
//!
//! Paths are resolved strictly inside `data_dir` (parent-dir components are
//! stripped) to prevent path-traversal.

use crate::error::GeoServerError;
use crate::handlers::auth_handler::require_auth;
use crate::handlers::rest_handler::ApiResponse;
use crate::state::AppState;
use actix_multipart::Multipart;
use actix_web::{web, HttpRequest, HttpResponse};
use futures_util::StreamExt;
use serde::Deserialize;
use std::path::{Component, Path, PathBuf};
use tokio::io::AsyncWriteExt;

/// Query parameters shared by the resources endpoints.
#[derive(Debug, Deserialize)]
pub struct ResourcesQuery {
    /// Path relative to the data directory (defaults to the data dir root).
    pub path: Option<String>,
}

/// Resolve a user-supplied relative path strictly inside `data_dir`.
/// Strips `..` / root / prefix components so traversal is impossible.
fn resolve_in_data_dir(data_dir: &Path, rel: &str) -> PathBuf {
    let clean: PathBuf = Path::new(rel)
        .components()
        .filter(|c| {
            !matches!(
                c,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
        .map(|c| c.as_os_str())
        .collect();
    if clean.as_os_str().is_empty() {
        data_dir.to_path_buf()
    } else {
        data_dir.join(clean)
    }
}

/// GET /resources — list a directory under the data directory.
pub async fn list_resources(
    query: web::Query<ResourcesQuery>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let dir = resolve_in_data_dir(&state.config.data_dir, query.path.as_deref().unwrap_or(""));

    let mut entries = Vec::new();
    if let Ok(read_dir) = std::fs::read_dir(&dir) {
        for entry in read_dir.flatten() {
            let ft = entry.file_type().ok();
            let is_dir = ft.map(|t| t.is_dir()).unwrap_or(false);
            let size = entry.metadata().ok().map(|m| m.len()).unwrap_or(0);
            entries.push(serde_json::json!({
                "name": entry.file_name().to_string_lossy(),
                "path": entry.path().to_string_lossy(),
                "is_dir": is_dir,
                "size": size,
            }));
        }
    }

    Ok(
        HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
            "path": dir.to_string_lossy(),
            "entries": entries,
        }))),
    )
}

/// POST /resources — upload a file into the data directory (admin auth).
pub async fn upload_resource(
    req: HttpRequest,
    query: web::Query<ResourcesQuery>,
    payload: Multipart,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    require_auth(&req)?;
    let dir = resolve_in_data_dir(&state.config.data_dir, query.path.as_deref().unwrap_or(""));
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| GeoServerError::InternalError(format!("无法创建目录: {}", e)))?;

    let mut saved: Option<String> = None;
    let mut stream = payload;
    while let Some(Ok(mut field)) = stream.next().await {
        let filename = field
            .content_disposition()
            .get_filename()
            .map(sanitize_resource_name)
            .unwrap_or_else(|| "upload".to_string());
        let file_path = dir.join(&filename);
        let mut file = tokio::fs::File::create(&file_path)
            .await
            .map_err(|e| GeoServerError::InternalError(format!("无法创建文件: {}", e)))?;
        while let Some(Ok(chunk)) = field.next().await {
            file.write_all(&chunk)
                .await
                .map_err(|e| GeoServerError::InternalError(format!("写入文件失败: {}", e)))?;
        }
        file.flush()
            .await
            .map_err(|e| GeoServerError::InternalError(format!("刷新文件失败: {}", e)))?;
        saved = Some(filename);
    }

    match saved {
        Some(name) => Ok(
            HttpResponse::Created().json(ApiResponse::success(serde_json::json!({
                "name": name,
                "path": dir.join(&name).to_string_lossy(),
                "message": "Resource uploaded",
            }))),
        ),
        None => Err(GeoServerError::BadRequest("未接收到上传文件".to_string())),
    }
}

/// DELETE /resources — delete a file under the data directory (admin auth).
pub async fn delete_resource(
    req: HttpRequest,
    query: web::Query<ResourcesQuery>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    require_auth(&req)?;
    let rel = query
        .path
        .clone()
        .ok_or_else(|| GeoServerError::BadRequest("path 参数必填".to_string()))?;
    let target = resolve_in_data_dir(&state.config.data_dir, &rel);

    let meta = tokio::fs::metadata(&target)
        .await
        .map_err(|_| GeoServerError::NotFound(format!("资源不存在: {}", rel)))?;
    if meta.is_dir() {
        return Err(GeoServerError::BadRequest(
            "仅支持删除文件, 目录请逐项删除".to_string(),
        ));
    }
    tokio::fs::remove_file(&target)
        .await
        .map_err(|e| GeoServerError::InternalError(format!("删除失败: {}", e)))?;

    Ok(
        HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
            "message": format!("Resource '{}' deleted", rel),
        }))),
    )
}

/// Sanitize an uploaded resource name (no separators, no `..`).
fn sanitize_resource_name(name: &str) -> String {
    name.replace(['\\', '/'], "_")
        .replace("..", "_")
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_in_data_dir_stays_inside() {
        let data = PathBuf::from("C:/data");
        assert_eq!(
            resolve_in_data_dir(&data, "../../etc/passwd"),
            PathBuf::from("C:/data/etc/passwd")
        );
        assert_eq!(
            resolve_in_data_dir(&data, "/abs/path"),
            PathBuf::from("C:/data/abs/path")
        );
        assert_eq!(
            resolve_in_data_dir(&data, "sub/dir/file.txt"),
            PathBuf::from("C:/data/sub/dir/file.txt")
        );
        assert_eq!(resolve_in_data_dir(&data, ""), data);
    }

    #[test]
    fn test_sanitize_resource_name() {
        assert_eq!(sanitize_resource_name("a/b\\c.txt"), "a_b_c.txt");
        assert_eq!(sanitize_resource_name("../../x"), "____x");
    }
}
