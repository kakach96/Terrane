//! 文件上传处理器
//!
//! 支持 Shapefile (.zip) 和 GeoTIFF (.tif/.tiff) 上传。
//! 上传后自动保存到数据目录并创建 DataSource 记录。

use actix_web::{HttpRequest, HttpResponse, web};
use actix_multipart::Multipart;
use futures_util::StreamExt;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;
use tracing::{info, warn, debug};

use crate::state::AppState;
use crate::models::{DataSourceType, DataSourceConnection};
use crate::error::GeoServerError;
use super::rest_handler::ApiResponse;

/// 上传 Shapefile（接收 .zip 文件）
pub async fn upload_shapefile(
    req: HttpRequest,
    payload: Multipart,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let data_dir = state.config.data_dir.clone();
    let upload_dir = data_dir.join("uploads").join("shapefiles");
    tokio::fs::create_dir_all(&upload_dir).await
        .map_err(|e| GeoServerError::InternalError(format!("无法创建上传目录: {}", e)))?;

    // 从查询参数中读取图层名称（可选）
    let layer_name = req.query_string()
        .split('&')
        .find_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            if parts.next()? == "name" { parts.next() } else { None }
        })
        .map(|s| s.to_string());

    let saved_path = save_multipart_file(payload, &upload_dir).await?;
    info!("[Upload] Shapefile 已保存: {:?}", saved_path);

    // 验证 ZIP 内容
    let file = std::fs::File::open(&saved_path)
        .map_err(|e| GeoServerError::InternalError(format!("无法打开上传文件: {}", e)))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| GeoServerError::BadRequest(format!("无效的 ZIP 文件: {}", e)))?;

    let has_shp = (0..archive.len()).any(|i| {
        archive.by_index(i).ok()
            .map(|e| e.name().to_lowercase().ends_with(".shp"))
            .unwrap_or(false)
    });

    if !has_shp {
        // 清理无效文件
        let _ = tokio::fs::remove_file(&saved_path).await;
        return Err(GeoServerError::BadRequest(
            "ZIP 文件中未找到 .shp 文件".to_string()
        ));
    }

    // 自动创建 DataSource
    let ds_name = layer_name.unwrap_or_else(|| {
        saved_path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("uploaded_shapefile")
            .to_string()
    });

    let connection = DataSourceConnection::file(saved_path.to_string_lossy().to_string());

    if let Some(store) = &state.store {
        // 检查是否已存在同名数据源
        match store.get_data_source(&ds_name).await {
            Ok(Some(_)) => {
                return Err(GeoServerError::Conflict(
                    format!("Data source '{}' already exists", ds_name)
                ));
            }
            _ => {}
        }

        match store.create_data_source(
            &ds_name,
            &DataSourceType::Shapefile,
            Some("default".to_string()),
            true,
            &connection,
        ).await {
            Ok(ds) => {
                info!("[Upload] Shapefile 数据源已创建: {}", ds.name);
                Ok(HttpResponse::Created().json(ApiResponse::success(serde_json::json!({
                    "name": ds.name,
                    "type": "shapefile",
                    "file_path": ds.connection.as_ref().and_then(|c| c.file_path.as_ref()),
                    "message": format!("Shapefile '{}' uploaded and data source created", ds.name),
                }))))
            }
            Err(e) => {
                warn!("[Upload] 创建数据源失败: {}", e);
                Err(GeoServerError::InternalError("创建数据源失败".to_string()))
            }
        }
    } else {
        Err(GeoServerError::InternalError("数据库不可用".to_string()))
    }
}

/// 上传 GeoTIFF（接收 .tif/.tiff 文件）
pub async fn upload_geotiff(
    req: HttpRequest,
    payload: Multipart,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let data_dir = state.config.data_dir.clone();
    let upload_dir = data_dir.join("uploads").join("geotiffs");
    tokio::fs::create_dir_all(&upload_dir).await
        .map_err(|e| GeoServerError::InternalError(format!("无法创建上传目录: {}", e)))?;

    let layer_name = req.query_string()
        .split('&')
        .find_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            if parts.next()? == "name" { parts.next() } else { None }
        })
        .map(|s| s.to_string());

    let saved_path = save_multipart_file(payload, &upload_dir).await?;
    info!("[Upload] GeoTIFF 已保存: {:?}", saved_path);

    // 验证文件扩展名
    let ext = saved_path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    if ext != "tif" && ext != "tiff" {
        let _ = tokio::fs::remove_file(&saved_path).await;
        return Err(GeoServerError::BadRequest(
            format!("不支持的文件格式: .{}, 仅支持 .tif/.tiff", ext)
        ));
    }

    // 自动创建 DataSource
    let ds_name = layer_name.unwrap_or_else(|| {
        saved_path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("uploaded_geotiff")
            .to_string()
    });

    let connection = DataSourceConnection::file(saved_path.to_string_lossy().to_string());

    if let Some(store) = &state.store {
        match store.get_data_source(&ds_name).await {
            Ok(Some(_)) => {
                return Err(GeoServerError::Conflict(
                    format!("Data source '{}' already exists", ds_name)
                ));
            }
            _ => {}
        }

        match store.create_data_source(
            &ds_name,
            &DataSourceType::Geotiff,
            Some("default".to_string()),
            true,
            &connection,
        ).await {
            Ok(ds) => {
                info!("[Upload] GeoTIFF 数据源已创建: {}", ds.name);
                Ok(HttpResponse::Created().json(ApiResponse::success(serde_json::json!({
                    "name": ds.name,
                    "type": "geotiff",
                    "file_path": ds.connection.as_ref().and_then(|c| c.file_path.as_ref()),
                    "message": format!("GeoTIFF '{}' uploaded and data source created", ds.name),
                }))))
            }
            Err(e) => {
                warn!("[Upload] 创建数据源失败: {}", e);
                Err(GeoServerError::InternalError("创建数据源失败".to_string()))
            }
        }
    } else {
        Err(GeoServerError::InternalError("数据库不可用".to_string()))
    }
}

/// 保存 multipart 文件到磁盘
async fn save_multipart_file(
    mut payload: Multipart,
    upload_dir: &Path,
) -> Result<PathBuf, GeoServerError> {
    let mut saved_path: Option<PathBuf> = None;

    while let Some(Ok(mut field)) = payload.next().await {
        // 获取文件名
        let content_disposition = field.content_disposition().clone();
        let filename = content_disposition.get_filename()
            .map(|f| sanitize_filename(f))
            .unwrap_or_else(|| "upload".to_string());

        let file_path = upload_dir.join(&filename);
        let mut file = tokio::fs::File::create(&file_path).await
            .map_err(|e| GeoServerError::InternalError(format!("无法创建文件: {}", e)))?;

        // 流式写入
        while let Some(Ok(chunk)) = field.next().await {
            file.write_all(&chunk).await
                .map_err(|e| GeoServerError::InternalError(format!("写入文件失败: {}", e)))?;
        }

        file.flush().await
            .map_err(|e| GeoServerError::InternalError(format!("刷新文件失败: {}", e)))?;

        debug!("[Upload] 保存文件: {:?}", file_path);
        saved_path = Some(file_path);
    }

    saved_path.ok_or_else(|| GeoServerError::BadRequest("未接收到上传文件".to_string()))
}

/// 清理文件名，防止路径遍历攻击
fn sanitize_filename(name: &str) -> String {
    let name = name.replace('\\', "_")
        .replace('/', "_")
        .replace("..", "_")
        .replace(std::path::MAIN_SEPARATOR, "_");
    // 只保留安全字符
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("test.shp"), "test.shp");
        assert_eq!(sanitize_filename("../../etc/passwd"), "____etc_passwd");
        assert_eq!(sanitize_filename("hello world.zip"), "hello_world.zip");
        assert_eq!(sanitize_filename("a\\b\\c.tif"), "a_b_c.tif");
    }
}
