use actix_web::{HttpResponse, web};
use crate::services::wcs::{self, WcsRequest, WcsCapabilities, CoverageDescription};
use crate::state::AppState;
use crate::error::GeoServerError;
use crate::models::DataSourceType;
use crate::utils::geotiff;
use quick_xml::se::to_string;
use tracing::{info, warn};

pub async fn handle_wcs_request(
    query: web::Query<Vec<(String, String)>>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let params = query.as_ref();
    let wcs_request = wcs::parse_wcs_request(params)?;

    match wcs_request.request {
        wcs::WcsOperation::GetCapabilities => handle_get_capabilities(&state, &wcs_request).await,
        wcs::WcsOperation::DescribeCoverage => handle_describe_coverage(&state, &wcs_request).await,
        wcs::WcsOperation::GetCoverage => handle_get_coverage(&state, &wcs_request).await,
    }
}

/// 解析栅格文件的本地路径: 优先经栅格存储解析 (如管理内的栅格), 否则回退到
/// 数据源连接里记录的原始文件路径 (外部/绝对路径栅格)。
fn resolve_raster_path(state: &AppState, ds_name: &str, conn_file_path: Option<&String>) -> Option<std::path::PathBuf> {
    if let Some(rstore) = &state.raster_store {
        if let Some(p) = rstore.local_path(ds_name) {
            if p.exists() {
                return Some(p);
            }
        }
    }
    conn_file_path.map(|f| std::path::PathBuf::from(f))
}

/// 从数据源中查找栅格覆盖数据 (GeoTIFF / WorldImage / ArcGrid)
async fn find_raster_coverages(state: &AppState) -> Vec<(String, String, std::path::PathBuf)> {
    let mut coverages = Vec::new();

    if let Some(store) = &state.store {
        if let Ok(ds_list) = store.get_all_data_sources().await {
            for ds in &ds_list {
                if ds.data_source_type == DataSourceType::Geotiff
                    || ds.data_source_type == DataSourceType::WorldImage
                    || ds.data_source_type == DataSourceType::ArcGrid {
                    if let Some(conn) = &ds.connection {
                        if let Some(path) = resolve_raster_path(state, &ds.name, conn.file_path.as_ref()) {
                            if path.exists() {
                                coverages.push((ds.name.clone(), ds.name.clone(), path));
                            }
                        }
                    }
                }
            }
        }
    }

    coverages
}

async fn handle_get_capabilities(state: &AppState, _request: &WcsRequest) -> Result<HttpResponse, GeoServerError> {
    let base_url = format!("http://{}:{}", state.config.server.host, state.config.server.port);
    let mut capabilities = WcsCapabilities::new(&base_url);

    // 动态添加栅格覆盖数据 (GeoTIFF / WorldImage)
    let coverages = find_raster_coverages(state).await;
    for (name, title, _path) in &coverages {
        capabilities.add_coverage(name, title);
    }

    let xml = to_string(&capabilities)
        .map_err(|e| GeoServerError::ServiceError(format!("Failed to serialize capabilities: {}", e)))?;

    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
{}"#,
        xml
    );

    Ok(HttpResponse::Ok()
        .content_type("application/xml")
        .body(xml))
}

async fn handle_describe_coverage(state: &AppState, request: &WcsRequest) -> Result<HttpResponse, GeoServerError> {
    let coverage_ids = request.coverage_id.as_ref()
        .ok_or_else(|| GeoServerError::BadRequest("COVERAGEID parameter is required".to_string()))?;

    let mut descriptions = Vec::new();

    for coverage_id in coverage_ids {
        let mut description = CoverageDescription::new(coverage_id);

        // 尝试从 GeoTIFF 数据源获取真实元数据
        if let Some(store) = &state.store {
            if let Ok(Some(ds)) = store.get_data_source(coverage_id).await {
                if ds.data_source_type == DataSourceType::Geotiff {
                    if let Some(conn) = &ds.connection {
                        if let Some(path) = resolve_raster_path(state, &ds.name, conn.file_path.as_ref()) {
                            if let Ok(meta) = geotiff::read_geotiff_metadata(&path) {
                                if let Some(bounds) = meta.bounds {
                                    description.set_bounds(bounds.minx, bounds.miny, bounds.maxx, bounds.maxy);
                                }
                                description.set_size(meta.width, meta.height, meta.band_count);
                            }
                        }
                    }
                }
            }
        }

        descriptions.push(description);
    }

    let xml = to_string(&descriptions)
        .map_err(|e| GeoServerError::ServiceError(format!("Failed to serialize descriptions: {}", e)))?;

    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<wcs:CoverageDescriptions xmlns:wcs="http://www.opengis.net/wcs/2.0"
                          xmlns:gml="http://www.opengis.net/gml/3.2">
{}
</wcs:CoverageDescriptions>"#,
        xml
    );

    Ok(HttpResponse::Ok()
        .content_type("application/xml")
        .body(xml))
}

async fn handle_get_coverage(state: &AppState, request: &WcsRequest) -> Result<HttpResponse, GeoServerError> {
    let coverage_ids = request.coverage_id.as_ref()
        .ok_or_else(|| GeoServerError::BadRequest("COVERAGEID parameter is required".to_string()))?;

    let coverage_id = coverage_ids.first()
        .ok_or_else(|| GeoServerError::BadRequest("At least one COVERAGEID is required".to_string()))?;

    let output_format = request.output_format.as_deref().unwrap_or("image/tiff").to_string();

    // 尝试从数据源读取真实栅格数据 (GeoTIFF / WorldImage)
    let raster_result = if let Some(store) = &state.store {
        if let Ok(Some(ds)) = store.get_data_source(coverage_id).await {
            let is_raster = ds.data_source_type == DataSourceType::Geotiff
                         || ds.data_source_type == DataSourceType::WorldImage
                         || ds.data_source_type == DataSourceType::ArcGrid;
            if is_raster {
                if let Some(conn) = &ds.connection {
                    if let Some(path) = resolve_raster_path(state, &ds.name, conn.file_path.as_ref()) {
                        info!("[WCS] 从 {:?} 读取覆盖: {:?}", ds.data_source_type, path);
                        match ds.data_source_type {
                            DataSourceType::Geotiff => {
                                match crate::utils::geotiff::read_geotiff(&path) {
                                    Ok(cov) => Some((cov.rgba_image, cov.bounds)),
                                    Err(e) => {
                                        info!("[WCS] GeoTIFF 读取失败: {}", e);
                                        None
                                    }
                                }
                            }
                            DataSourceType::WorldImage => {
                                match crate::utils::worldimage::read_worldimage(&path) {
                                    Ok(wim) => Some((wim.rgba_image, Some(wim.bounds))),
                                    Err(e) => {
                                        info!("[WCS] WorldImage 读取失败: {}", e);
                                        None
                                    }
                                }
                            }
                            DataSourceType::ArcGrid => {
                                match crate::utils::arcgrid::read_arcgrid(&path) {
                                    Ok(ag) => Some((ag.rgba_image, Some(ag.bounds))),
                                    Err(e) => {
                                        info!("[WCS] ArcGrid 读取失败: {}", e);
                                        None
                                    }
                                }
                            }
                            _ => None,
                        }
                    } else { None }
                } else { None }
            } else { None }
        } else { None }
    } else { None };

    if let Some((rgba_image, cov_bounds)) = raster_result {
        let coverage_data = crate::utils::geotiff::CoverageData {
            name: coverage_id.to_string(),
            width: rgba_image.width(),
            height: rgba_image.height(),
            band_count: 4,
            color_type: "RGBA".to_string(),
            rgba_image,
            bounds: cov_bounds,
            crs: Some("EPSG:4326".to_string()),
            pixel_scale_x: None,
            pixel_scale_y: None,
            tie_point_x: None,
            tie_point_y: None,
        };

        // 1. 应用子集（空间裁剪 / 时间裁剪）
        let img = apply_coverage_subsets(&coverage_data, &request.subsets, &request.size);

        // 2. 按请求的宽度/高度缩放
        let (width, height) = calculate_output_size(img.width(), img.height(), &request.size);
        let final_img = if width != img.width() || height != img.height() {
            image::imageops::resize(&img, width, height, image::imageops::FilterType::Lanczos3)
        } else {
            img
        };

        // 3. 编码输出
        return encode_coverage_output(final_img, &output_format, coverage_id);
    }

    // 回退：生成测试图像
    info!("[WCS] 未找到栅格数据源，生成测试图像");
    let mut width = 512u32;
    let mut height = 512u32;
    if let Some(ref size) = request.size {
        if size.len() >= 2 {
            width = size[0] as u32;
            height = size[1] as u32;
        }
    }

    let mut img = image::RgbaImage::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let value = ((x as f64 / width as f64) * 255.0) as u8;
            img.put_pixel(x, y, image::Rgba([value, value, value, 255]));
        }
    }

    encode_coverage_output(img, &output_format, coverage_id)
}

/// 应用 WCS 子集参数（空间裁剪、时间裁剪等）
fn apply_coverage_subsets(
    coverage: &crate::utils::geotiff::CoverageData,
    subsets: &Option<Vec<crate::services::wcs::Subset>>,
    size: &Option<Vec<i64>>,
) -> image::RgbaImage {
    let img = coverage.rgba_image.clone();

    let subsets = match subsets {
        Some(s) => s,
        None => return img,
    };

    let mut bbox_minx: Option<f64> = None;
    let mut bbox_miny: Option<f64> = None;
    let mut bbox_maxx: Option<f64> = None;
    let mut bbox_maxy: Option<f64> = None;
    let mut use_resolution: Option<f64> = None;

    for subset in subsets {
        match subset.subset_type {
            crate::services::wcs::SubsetType::Intervals { min, max, resolution } => {
                match subset.axis_label.to_lowercase().as_str() {
                    "x" | "long" | "longitude" | "i" => {
                        bbox_minx = Some(min);
                        bbox_maxx = Some(max);
                        if resolution.is_some() { use_resolution = resolution; }
                    }
                    "y" | "lat" | "latitude" | "j" => {
                        bbox_miny = Some(min);
                        bbox_maxy = Some(max);
                        if resolution.is_some() { use_resolution = resolution; }
                    }
                    "time" | "t" | "elevation" | "z" | "e" => {
                        // 时间/高程子集: 对于 GeoTIFF，这些目前仅做记录
                        info!("[WCS] 非空间子集: axis={}, range=[{}, {}]",
                            subset.axis_label, min, max);
                    }
                    _ => {
                        info!("[WCS] 未知轴子集: axis={}, range=[{}, {}]",
                            subset.axis_label, min, max);
                    }
                }
            }
            crate::services::wcs::SubsetType::Position { value } => {
                match subset.axis_label.to_lowercase().as_str() {
                    "time" | "t" => {
                        info!("[WCS] 时间位置子集: axis={}, value={}", subset.axis_label, value);
                    }
                    _ => {
                        info!("[WCS] 未知轴位置子集: axis={}, value={}", subset.axis_label, value);
                    }
                }
            }
        }
    }

    // 应用空间裁剪
    if let (Some(minx), Some(miny), Some(maxx), Some(maxy)) = (bbox_minx, bbox_miny, bbox_maxx, bbox_maxy) {
        let bounds = crate::models::Bounds::new(minx, miny, maxx, maxy);
        match crate::utils::geotiff::crop_coverage(coverage, &bounds) {
            Some(cropped) => {
                info!("[WCS] 空间裁剪完成: [{}, {}, {}, {}]", minx, miny, maxx, maxy);
                // 如果指定了 resolution，则按分辨率重采样
                if let Some(res) = use_resolution {
                    if res > 0.0 {
                        if let Some(ref cov_bounds) = coverage.bounds {
                        let cov_width = ((cov_bounds.maxx - cov_bounds.minx) / res) as u32;
                        let cov_height = ((cov_bounds.maxy - cov_bounds.miny) / res) as u32;
                        if cov_width > 0 && cov_height > 0 {
                            return image::imageops::resize(&cropped, cov_width, cov_height,
                                image::imageops::FilterType::Lanczos3);
                        }
                        }
                    }
                }
                // 如果指定了 SIZE，使用 SIZE
                if let Some(ref sz) = size {
                    if sz.len() >= 2 && sz[0] > 0 && sz[1] > 0 {
                        return image::imageops::resize(&cropped, sz[0] as u32, sz[1] as u32,
                            image::imageops::FilterType::Lanczos3);
                    }
                }
                return cropped;
            }
            None => {
                warn!("[WCS] 空间裁剪失败: 返回 None");
            }
        }
    }

    img
}

/// 计算输出尺寸
fn calculate_output_size(orig_width: u32, orig_height: u32, size: &Option<Vec<i64>>) -> (u32, u32) {
    match size {
        Some(sz) if sz.len() >= 2 => {
            let w = if sz[0] > 0 { sz[0] as u32 } else { orig_width };
            let h = if sz[1] > 0 { sz[1] as u32 } else { orig_height };
            (w, h)
        }
        _ => (orig_width, orig_height),
    }
}

/// 编码覆盖输出图像
fn encode_coverage_output(
    img: image::RgbaImage,
    output_format: &str,
    coverage_id: &str,
) -> Result<HttpResponse, GeoServerError> {
    let (content_type, fmt) = match output_format {
        "image/png" => ("image/png", image::ImageFormat::Png),
        "image/jpeg" | "image/jpg" => ("image/jpeg", image::ImageFormat::Jpeg),
        "image/tiff" | "image/tif" => ("image/tiff", image::ImageFormat::Tiff),
        "image/gif" => ("image/gif", image::ImageFormat::Gif),
        "image/webp" => ("image/webp", image::ImageFormat::WebP),
        "application/x-grib" | "application/x-netcdf" => {
            // 不支持的原生格式，回退到 GeoTIFF
            ("image/tiff", image::ImageFormat::Tiff)
        }
        _ => ("image/tiff", image::ImageFormat::Tiff), // WCS 默认输出 TIFF
    };

    let mut buffer = Vec::new();

    // JPEG 和 TIFF 需要 RGB 而非 RGBA
    if fmt == image::ImageFormat::Jpeg || fmt == image::ImageFormat::Tiff {
        let rgb = image::DynamicImage::ImageRgba8(img).to_rgb8();
        rgb.write_to(&mut std::io::Cursor::new(&mut buffer), fmt)
            .map_err(|e| GeoServerError::RenderingError(e.to_string()))?;
    } else {
        img.write_to(&mut std::io::Cursor::new(&mut buffer), fmt)
            .map_err(|e| GeoServerError::RenderingError(e.to_string()))?;
    }

    Ok(HttpResponse::Ok()
        .content_type(content_type)
        .append_header(("Content-Description", format!("Coverage: {}", coverage_id)))
        .body(buffer))
}
