//! 文件上传处理器
//!
//! 支持 Shapefile (.zip) 和 GeoTIFF (.tif/.tiff) 上传。
//! 上传后自动保存到数据目录并创建 DataSource 记录。

use actix_multipart::Multipart;
use actix_web::{web, HttpRequest, HttpResponse};
use futures_util::StreamExt;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;
use tracing::{debug, info, warn};

use super::rest_handler::ApiResponse;
use crate::error::TerraneError;
use crate::models::{DataSourceConnection, DataSourceType};
use crate::state::AppState;
use crate::store::FileStore;

/// 上传 Shapefile（接收 .zip 文件）
pub async fn upload_shapefile(
    req: HttpRequest,
    payload: Multipart,
    state: web::Data<AppState>,
) -> Result<HttpResponse, TerraneError> {
    let data_dir = state.config.data_dir.clone();
    let upload_dir = data_dir.join("uploads").join("shapefiles");
    tokio::fs::create_dir_all(&upload_dir)
        .await
        .map_err(|e| TerraneError::InternalError(format!("无法创建上传目录: {}", e)))?;

    // 从查询参数中读取图层名称（可选）
    let layer_name = req
        .query_string()
        .split('&')
        .find_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            if parts.next()? == "name" {
                parts.next()
            } else {
                None
            }
        })
        .map(|s| s.to_string());

    let saved_path = save_multipart_file(payload, &upload_dir).await?;
    info!("[Upload] Shapefile 已保存: {:?}", saved_path);

    // 验证 ZIP 内容
    let file = std::fs::File::open(&saved_path)
        .map_err(|e| TerraneError::InternalError(format!("无法打开上传文件: {}", e)))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| TerraneError::BadRequest(format!("无效的 ZIP 文件: {}", e)))?;

    let has_shp = (0..archive.len()).any(|i| {
        archive
            .by_index(i)
            .ok()
            .map(|e| e.name().to_lowercase().ends_with(".shp"))
            .unwrap_or(false)
    });

    if !has_shp {
        // 清理无效文件
        let _ = tokio::fs::remove_file(&saved_path).await;
        return Err(TerraneError::BadRequest(
            "ZIP 文件中未找到 .shp 文件".to_string(),
        ));
    }

    // 自动创建 DataSource
    let ds_name = layer_name.unwrap_or_else(|| {
        saved_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("uploaded_shapefile")
            .to_string()
    });

    let connection = DataSourceConnection::file(saved_path.to_string_lossy().to_string());

    if let Some(store) = &state.store {
        // 检查是否已存在同名数据源
        if let Ok(Some(_)) = store.get_data_source(&ds_name).await {
            return Err(TerraneError::Conflict(format!(
                "Data source '{}' already exists",
                ds_name
            )));
        }

        match store
            .create_data_source(
                &ds_name,
                &DataSourceType::Shapefile,
                Some("default".to_string()),
                true,
                &connection,
            )
            .await
        {
            Ok(ds) => {
                info!("[Upload] Shapefile 数据源已创建: {}", ds.name);
                Ok(HttpResponse::Created().json(ApiResponse::success(serde_json::json!({
                    "name": ds.name,
                    "type": "shapefile",
                    "file_path": ds.connection.as_ref().and_then(|c| c.file_path.as_ref()),
                    "message": format!("Shapefile '{}' uploaded and data source created", ds.name),
                }))))
            },
            Err(e) => {
                warn!("[Upload] 创建数据源失败: {}", e);
                Err(TerraneError::InternalError("创建数据源失败".to_string()))
            },
        }
    } else {
        Err(TerraneError::InternalError("数据库不可用".to_string()))
    }
}

/// 上传 GeoTIFF（接收 .tif/.tiff 文件）
///
/// 保存到服务数据目录 `<data_dir>/rasters/` (本地存储后端) 并以
/// `file_storage_type = "local"` 登记为 GeoTIFF 数据源; 或当查询参数
/// `storage=s3` (附 `bucket` / `endpoint` / `region` / `access_key` /
/// `secret_key`) 时, 对象写入 S3/MinIO 并以 `file_storage_type = "s3"`
/// 登记, 供多副本共享。
pub async fn upload_geotiff(
    req: HttpRequest,
    payload: Multipart,
    state: web::Data<AppState>,
) -> Result<HttpResponse, TerraneError> {
    // 从 multipart 读取全部字节与原始文件名
    let (data, filename) = read_multipart_bytes(payload).await?;

    // 验证文件扩展名
    let ext = Path::new(&filename)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();
    if ext != "tif" && ext != "tiff" {
        return Err(TerraneError::BadRequest(format!(
            "不支持的文件格式: .{}, 仅支持 .tif/.tiff",
            ext
        )));
    }

    let layer_name = req
        .query_string()
        .split('&')
        .find_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            if parts.next()? == "name" {
                parts.next()
            } else {
                None
            }
        })
        .map(|s| s.to_string());

    let ds_name = layer_name.unwrap_or_else(|| {
        Path::new(&filename)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("uploaded_geotiff")
            .to_string()
    });

    // 提前检查重名, 避免已写入文件后才发现冲突
    if let Some(store) = &state.store {
        if let Ok(Some(_)) = store.get_data_source(&ds_name).await {
            return Err(TerraneError::Conflict(format!(
                "Data source '{}' already exists",
                ds_name
            )));
        }
    }

    // 存储后端选择: 查询参数 `storage=s3` 时写入 S3/MinIO, 否则本地磁盘。
    let storage = req
        .query_string()
        .split('&')
        .find_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            if parts.next()? == "storage" {
                parts.next()
            } else {
                None
            }
        })
        .map(|s| s.to_string());

    let file_name = format!("{}.tif", ds_name);

    let (connection, storage_type) = if storage.as_deref() == Some("s3") {
        // 从查询参数读取 S3 连接字段 (endpoint/bucket/region/access_key/secret_key)
        let q = |key: &str| -> Option<String> {
            req.query_string().split('&').find_map(|pair| {
                let mut parts = pair.splitn(2, '=');
                if parts.next()? == key {
                    parts.next().map(|v| v.to_string())
                } else {
                    None
                }
            })
        };
        let bucket = q("bucket")
            .filter(|s| !s.is_empty())
            .ok_or_else(|| TerraneError::BadRequest("storage=s3 需要 bucket 参数".to_string()))?;
        let mut conn = DataSourceConnection::file(file_name.clone());
        conn.file_storage_type = Some("s3".to_string());
        conn.s3_endpoint = q("endpoint");
        conn.s3_region = q("region");
        conn.s3_bucket = Some(bucket);
        conn.s3_access_key = q("access_key");
        conn.s3_secret_key = q("secret_key");

        let store = crate::store::S3FileStore::from_connection(&conn)
            .map_err(|e| TerraneError::InternalError(format!("S3 配置无效: {}", e)))?;
        store
            .put(&file_name, &data)
            .await
            .map_err(|e| TerraneError::InternalError(format!("S3 上传失败: {}", e)))?;
        info!("[Upload] GeoTIFF 已上传至 S3: {}", file_name);
        (conn, "s3")
    } else {
        // 保存到服务数据目录 <data_dir>/rasters/<ds_name>.tif (本地存储后端)
        let data_dir = state.config.data_dir.clone();
        let raster_dir = data_dir.join("rasters");
        let file_store = crate::store::LocalFileStore::new(raster_dir.clone());
        file_store
            .put(&file_name, &data)
            .await
            .map_err(|e| TerraneError::InternalError(format!("保存栅格文件失败: {}", e)))?;
        let file_path = file_store
            .local_path(&file_name)
            .unwrap_or_else(|| raster_dir.join(&file_name));
        info!("[Upload] GeoTIFF 已保存: {:?}", file_path);
        (
            DataSourceConnection::file(file_path.to_string_lossy().to_string()),
            "local",
        )
    };

    if let Some(store) = &state.store {
        match store
            .create_data_source(
                &ds_name,
                &DataSourceType::Geotiff,
                Some("default".to_string()),
                true,
                &connection,
            )
            .await
        {
            Ok(ds) => {
                info!("[Upload] GeoTIFF 数据源已创建: {}", ds.name);
                Ok(HttpResponse::Created().json(ApiResponse::success(serde_json::json!({
                    "name": ds.name,
                    "type": "geotiff",
                    "file_path": ds.connection.as_ref().and_then(|c| c.file_path.as_ref()),
                    "file_storage_type": storage_type,
                    "message": format!("GeoTIFF '{}' uploaded and data source created", ds.name),
                }))))
            },
            Err(e) => {
                warn!("[Upload] 创建数据源失败: {}", e);
                Err(TerraneError::InternalError("创建数据源失败".to_string()))
            },
        }
    } else {
        Err(TerraneError::InternalError("数据库不可用".to_string()))
    }
}

/// 从 multipart 读取单个文件的全部字节与原始文件名 (供栅格存储直接落盘)。
async fn read_multipart_bytes(mut payload: Multipart) -> Result<(Vec<u8>, String), TerraneError> {
    let mut data: Vec<u8> = Vec::new();
    let mut filename = "upload".to_string();
    while let Some(Ok(mut field)) = payload.next().await {
        if let Some(f) = field.content_disposition().and_then(|cd| cd.get_filename()) {
            filename = sanitize_filename(f);
        }
        while let Some(Ok(chunk)) = field.next().await {
            data.extend_from_slice(&chunk);
        }
    }
    if data.is_empty() {
        return Err(TerraneError::BadRequest("未接收到上传文件".to_string()));
    }
    Ok((data, filename))
}

/// 保存 multipart 文件到磁盘
async fn save_multipart_file(
    mut payload: Multipart,
    upload_dir: &Path,
) -> Result<PathBuf, TerraneError> {
    let mut saved_path: Option<PathBuf> = None;

    while let Some(Ok(mut field)) = payload.next().await {
        // 获取文件名
        let filename = field
            .content_disposition()
            .and_then(|cd| cd.get_filename())
            .map(sanitize_filename)
            .unwrap_or_else(|| "upload".to_string());

        let file_path = upload_dir.join(&filename);
        let mut file = tokio::fs::File::create(&file_path)
            .await
            .map_err(|e| TerraneError::InternalError(format!("无法创建文件: {}", e)))?;

        // 流式写入
        while let Some(Ok(chunk)) = field.next().await {
            file.write_all(&chunk)
                .await
                .map_err(|e| TerraneError::InternalError(format!("写入文件失败: {}", e)))?;
        }

        file.flush()
            .await
            .map_err(|e| TerraneError::InternalError(format!("刷新文件失败: {}", e)))?;

        debug!("[Upload] 保存文件: {:?}", file_path);
        saved_path = Some(file_path);
    }

    saved_path.ok_or_else(|| TerraneError::BadRequest("未接收到上传文件".to_string()))
}

/// 清理文件名，防止路径遍历攻击
fn sanitize_filename(name: &str) -> String {
    let name = name
        .replace(['\\', '/'], "_")
        .replace("..", "_")
        .replace(std::path::MAIN_SEPARATOR, "_");
    // 只保留安全字符
    name.chars()
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
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("test.shp"), "test.shp");
        assert_eq!(sanitize_filename("../../etc/passwd"), "____etc_passwd");
        assert_eq!(sanitize_filename("hello world.zip"), "hello_world.zip");
        assert_eq!(sanitize_filename("a\\b\\c.tif"), "a_b_c.tif");
    }
}
