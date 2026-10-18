//! Bulk import (GeoServer Importer-style first surface).
//!
//! `POST /terrane/importer/uploads` accepts a single multipart file:
//! - a `.zip` archive — unpacked into `<data_dir>/imports/<uuid>/` and every
//!   recognized member is published (data source + layer + bounds),
//! - or a single recognized data file, published directly.
//!
//! Recognized formats: GeoJSON (`.geojson` / `.json`), GeoTIFF (`.tif` /
//! `.tiff`), GeoPackage (`.gpkg`), Shapefile (`.shp` — companion `.shx` /
//! `.dbf` / `.prj` members are unpacked alongside).
//!
//! The response is a per-file report (`created` / `skipped` / `failed`),
//! matching the Importer's job-report spirit.

use crate::error::TerraneError;
use crate::models::{DataSource, DataSourceConnection, DataSourceType};
use crate::store::types::Layer as StoreLayer;
use crate::store::Store;
use crate::utils::bounds::compute_layer_bounds;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;
use tracing::{info, warn};

/// Per-file entry of the import report.
#[derive(Debug, serde::Serialize)]
pub struct ImportEntry {
    /// Member file name (inside the archive) or the uploaded file name.
    pub file: String,
    /// Detected data type (geojson / geotiff / geopackage / shapefile).
    pub type_name: String,
    /// created | skipped | failed
    pub status: String,
    /// Published layer name (when published or already existing).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layer: Option<String>,
    /// Failure reason (when status = failed).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Full import report.
#[derive(Debug, serde::Serialize)]
pub struct ImportReport {
    /// Directory the archive was unpacked into (None for direct file import).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unpacked_to: Option<String>,
    /// Number of entries created / skipped / failed.
    pub created: usize,
    pub skipped: usize,
    pub failed: usize,
    pub entries: Vec<ImportEntry>,
}

/// Classify a member file by extension -> (type name, DataSourceType).
fn classify(ext: &str) -> Option<(&'static str, DataSourceType)> {
    match ext {
        "geojson" | "json" => Some(("geojson", DataSourceType::GeoJson)),
        "tif" | "tiff" => Some(("geotiff", DataSourceType::Geotiff)),
        "gpkg" => Some(("geopackage", DataSourceType::Geopackage)),
        "shp" => Some(("shapefile", DataSourceType::Shapefile)),
        _ => None,
    }
}

/// Sanitize a layer / data-source name: lowercase-safe identifier derived from
/// the file stem (letters / digits / underscore only).
fn layer_name_from(stem: &str) -> String {
    let cleaned: String = stem
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let name = cleaned.trim_matches('_').to_lowercase();
    if name.is_empty() {
        "imported_layer".to_string()
    } else if name.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        format!("layer_{}", name)
    } else {
        name
    }
}

/// Publish one recognized data file: create the data source, compute bounds
/// and create the layer. Returns the report entry.
async fn publish_file(
    store: &Arc<dyn Store>,
    file: &str,
    path: &Path,
    workspace: &str,
) -> ImportEntry {
    let ext = Path::new(file)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();
    let Some((type_name, ds_type)) = classify(&ext) else {
        return ImportEntry {
            file: file.to_string(),
            type_name: "unknown".to_string(),
            status: "skipped".to_string(),
            layer: None,
            message: Some("unrecognized extension".to_string()),
        };
    };

    let name = layer_name_from(
        Path::new(file)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("imported"),
    );

    // Skip silently when the data source already exists (idempotent re-import).
    if matches!(store.get_data_source(&name).await, Ok(Some(_)) | Err(_)) {
        return ImportEntry {
            file: file.to_string(),
            type_name: type_name.to_string(),
            status: "skipped".to_string(),
            layer: Some(name),
            message: Some("data source already exists".to_string()),
        };
    }

    let connection = DataSourceConnection::file(path.to_string_lossy().to_string());
    if let Err(e) = store
        .create_data_source(
            &name,
            &ds_type,
            Some(workspace.to_string()),
            true,
            &connection,
        )
        .await
    {
        return ImportEntry {
            file: file.to_string(),
            type_name: type_name.to_string(),
            status: "failed".to_string(),
            layer: None,
            message: Some(format!("create data source: {}", e)),
        };
    }

    let data_source = DataSource {
        name: name.clone(),
        data_source_type: ds_type.clone(),
        workspace: Some(workspace.to_string()),
        enabled: true,
        connection: Some(connection),
        created: None,
        modified: None,
    };

    let native_name = Path::new(file)
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string());
    let computed = compute_layer_bounds(&data_source, native_name.as_deref(), None)
        .await
        .ok()
        .flatten();

    let (minx, miny, maxx, maxy) = computed
        .as_ref()
        .map(|b| (b.bounds.minx, b.bounds.miny, b.bounds.maxx, b.bounds.maxy))
        .unwrap_or((0.0, 0.0, 1.0, 1.0));

    let store_layer = StoreLayer {
        name: name.clone(),
        title: name.clone(),
        workspace: workspace.to_string(),
        store: name.clone(),
        srs: "EPSG:4326".to_string(),
        abstract_text: Some(format!("Imported from {}", file)),
        native_name: Some(native_name.unwrap_or_else(|| name.clone())),
        enabled: true,
        minx,
        miny,
        maxx,
        maxy,
        created: String::new(),
        modified: String::new(),
        cache_store: None,
    };
    if let Err(e) = store.create_layer(&store_layer).await {
        return ImportEntry {
            file: file.to_string(),
            type_name: type_name.to_string(),
            status: "failed".to_string(),
            layer: Some(name),
            message: Some(format!("create layer: {}", e)),
        };
    }

    info!(
        "[Importer] published '{}' -> layer '{}' (workspace={})",
        file, name, workspace
    );
    ImportEntry {
        file: file.to_string(),
        type_name: type_name.to_string(),
        status: "created".to_string(),
        layer: Some(name),
        message: None,
    }
}

/// Process one imported archive directory: classify every member and publish
/// the recognized ones (shapefile companions are unpacked but not published
/// individually).
async fn publish_dir_members(
    store: &Arc<dyn Store>,
    dir: &Path,
    workspace: &str,
) -> Vec<ImportEntry> {
    let mut members: Vec<String> = Vec::new();
    if let Ok(read_dir) = std::fs::read_dir(dir) {
        for entry in read_dir.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                members.push(name.to_string());
            }
        }
    }
    members.sort();

    let mut entries = Vec::new();
    for member in &members {
        let ext = Path::new(member)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();
        // Shapefile companions belong to their .shp — skip as sidecar files.
        if matches!(ext.as_str(), "shx" | "dbf" | "prj" | "cpg" | "sbn" | "sbx") {
            continue;
        }
        let path = dir.join(member);
        if !path.is_file() {
            continue;
        }
        entries.push(publish_file(store, member, &path, workspace).await);
    }
    entries
}

/// Run the import: unpack a zip (or handle a single file) and publish members.
///
/// Returns the per-file report; unpacked files stay in
/// `<data_dir>/imports/<uuid>/` so the published data sources can reference
/// them directly.
pub async fn run_import(
    state: &crate::state::AppState,
    data: &[u8],
    filename: &str,
    workspace: &str,
) -> Result<ImportReport, TerraneError> {
    let store = state
        .store
        .as_ref()
        .ok_or_else(|| TerraneError::InternalError("数据库不可用".to_string()))?
        .clone();

    let ext = Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    // Single recognized file: publish directly, no unpacking.
    if ext != "zip" {
        if classify(&ext).is_none() {
            return Err(TerraneError::BadRequest(format!(
                "不支持的导入格式: .{} (支持 zip 批量导入, 或 geojson/tif/gpkg/shp 单文件)",
                ext
            )));
        }
        let dir = state
            .config
            .data_dir
            .join("imports")
            .join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(&dir)
            .map_err(|e| TerraneError::InternalError(format!("无法创建导入目录: {}", e)))?;
        let path = dir.join(sanitize_member_name(filename));
        std::fs::write(&path, data)
            .map_err(|e| TerraneError::InternalError(format!("写入导入文件失败: {}", e)))?;
        let entry = publish_file(&store, filename, &path, workspace).await;
        return Ok(build_report(None, vec![entry]));
    }

    // Zip archive: unpack then publish each recognized member.
    let unpack_dir = state
        .config
        .data_dir
        .join("imports")
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&unpack_dir)
        .map_err(|e| TerraneError::InternalError(format!("无法创建导入目录: {}", e)))?;

    let cursor = std::io::Cursor::new(data.to_vec());
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|e| TerraneError::BadRequest(format!("无法读取 zip 压缩包: {}", e)))?;

    for i in 0..archive.len() {
        let mut member = archive
            .by_index(i)
            .map_err(|e| TerraneError::BadRequest(format!("zip 成员读取失败: {}", e)))?;
        if member.is_dir() {
            continue;
        }
        // enclosed_name 防路径遍历 (zip slip)。
        let Some(rel) = member.enclosed_name() else {
            warn!("[Importer] 跳过不安全路径的 zip 成员: {}", member.name());
            continue;
        };
        let mut out_path = unpack_dir.clone();
        for part in rel.components() {
            out_path.push(sanitize_member_name(
                part.as_os_str().to_str().unwrap_or("_"),
            ));
        }
        if let Some(parent) = out_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let mut buf = Vec::new();
        member
            .read_to_end(&mut buf)
            .map_err(|e| TerraneError::BadRequest(format!("zip 成员解压失败: {}", e)))?;
        std::fs::write(&out_path, buf)
            .map_err(|e| TerraneError::InternalError(format!("写入解压文件失败: {}", e)))?;
    }

    let entries = publish_dir_members(&store, &unpack_dir, workspace).await;
    Ok(build_report(
        Some(unpack_dir.to_string_lossy().to_string()),
        entries,
    ))
}

/// Build the summary counts from per-file entries.
fn build_report(unpacked_to: Option<String>, entries: Vec<ImportEntry>) -> ImportReport {
    let created = entries.iter().filter(|e| e.status == "created").count();
    let skipped = entries.iter().filter(|e| e.status == "skipped").count();
    let failed = entries.iter().filter(|e| e.status == "failed").count();
    ImportReport {
        unpacked_to,
        created,
        skipped,
        failed,
        entries,
    }
}

/// Flatten a member file name: strip directories / traversal characters.
fn sanitize_member_name(name: &str) -> String {
    name.replace(['\\', '/', ':', '*'], "_").replace("..", "_")
}

/// Ensure the import target workspace exists (same pattern as samples.rs).
pub async fn ensure_workspace(store: &Arc<dyn Store>, workspace: &str) -> Result<(), TerraneError> {
    if matches!(store.get_workspace(workspace).await, Ok(Some(_))) {
        return Ok(());
    }
    let request = crate::handlers::CreateWorkspaceRequest {
        name: workspace.to_string(),
        title: Some(workspace.to_string()),
        description: Some("Created by importer".to_string()),
    };
    let ws = store
        .create_workspace(&request)
        .await
        .map_err(|e| TerraneError::InternalError(format!("创建工作空间失败: {}", e)))?;
    let ns_uri = format!("http://geoserver.org/{}", ws.name);
    let _ = store
        .create_namespace(&ws.name, &ns_uri, Some(&ws.name), false)
        .await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layer_name_from() {
        assert_eq!(layer_name_from("major_cities"), "major_cities");
        assert_eq!(layer_name_from("My Data-Set 2"), "my_data_set_2");
        assert_eq!(layer_name_from("2024 survey"), "layer_2024_survey");
        assert_eq!(layer_name_from("___"), "imported_layer");
        assert_eq!(
            layer_name_from("Zone A/b.shp"),
            "zone_a_b_shp" /* stem */
        );
    }

    #[test]
    fn test_classify() {
        assert!(classify("geojson").is_some());
        assert!(classify("json").is_some());
        assert!(classify("tif").is_some());
        assert!(classify("tiff").is_some());
        assert!(classify("gpkg").is_some());
        assert!(classify("shp").is_some());
        assert!(classify("shx").is_none(), "sidecar 文件不算独立数据");
        assert!(classify("txt").is_none());
        assert!(classify("").is_none());
    }

    #[test]
    fn test_sanitize_member_name() {
        assert_eq!(sanitize_member_name("a.geojson"), "a.geojson");
        assert_eq!(sanitize_member_name("../../etc/passwd"), "____etc_passwd");
        assert_eq!(sanitize_member_name("dir\\file.tif"), "dir_file.tif");
        assert_eq!(sanitize_member_name("x:y*"), "x_y_");
    }
}
