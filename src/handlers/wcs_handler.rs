use actix_web::{HttpResponse, web};
use crate::services::wcs::{self, WcsRequest, WcsCapabilities, CoverageDescription};
use crate::state::AppState;
use crate::error::GeoServerError;
use crate::models::DataSourceType;
use crate::utils::geotiff;
use quick_xml::se::to_string;
use tracing::info;

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

/// 从数据源中查找 GeoTIFF 覆盖数据
async fn find_geotiff_coverages(state: &AppState) -> Vec<(String, String, std::path::PathBuf)> {
    let mut coverages = Vec::new();

    if let Some(store) = &state.store {
        if let Ok(ds_list) = store.get_all_data_sources().await {
            for ds in &ds_list {
                if ds.data_source_type == DataSourceType::Geotiff {
                    if let Some(conn) = &ds.connection {
                        if let Some(file_path) = &conn.file_path {
                            let path = std::path::PathBuf::from(file_path);
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

    // 动态添加 GeoTIFF 覆盖数据
    let coverages = find_geotiff_coverages(state).await;
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
                        if let Some(file_path) = &conn.file_path {
                            if let Ok(meta) = geotiff::read_geotiff_metadata(file_path) {
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

    // 尝试从数据源读取真实 GeoTIFF
    if let Some(store) = &state.store {
        if let Ok(Some(ds)) = store.get_data_source(coverage_id).await {
            if ds.data_source_type == DataSourceType::Geotiff {
                if let Some(conn) = &ds.connection {
                    if let Some(file_path) = &conn.file_path {
                        info!("[WCS] 从 GeoTIFF 读取覆盖: {:?}", file_path);

                        match geotiff::read_geotiff(file_path) {
                            Ok(coverage_data) => {
                                // 应用子集（裁剪）
                                let img = if let Some(ref subsets) = request.subsets {
                                    let mut bbox_minx = None;
                                    let mut bbox_miny = None;
                                    let mut bbox_maxx = None;
                                    let mut bbox_maxy = None;

                                    for subset in subsets {
                                        if let crate::services::wcs::SubsetType::Intervals { min, max, .. } = subset.subset_type {
                                            match subset.axis_label.to_lowercase().as_str() {
                                                "x" | "long" | "i" => {
                                                    bbox_minx = Some(min);
                                                    bbox_maxx = Some(max);
                                                }
                                                "y" | "lat" | "j" => {
                                                    bbox_miny = Some(min);
                                                    bbox_maxy = Some(max);
                                                }
                                                _ => {}
                                            }
                                        }
                                    }

                                    if let (Some(minx), Some(miny), Some(maxx), Some(maxy)) =
                                        (bbox_minx, bbox_miny, bbox_maxx, bbox_maxy)
                                    {
                                        let bounds = crate::models::Bounds::new(minx, miny, maxx, maxy);
                                        geotiff::crop_coverage(&coverage_data, &bounds)
                                            .unwrap_or(coverage_data.rgba_image.clone())
                                    } else {
                                        coverage_data.rgba_image.clone()
                                    }
                                } else {
                                    coverage_data.rgba_image.clone()
                                };

                                // 按请求的宽度/高度缩放
                                let mut width = img.width();
                                let mut height = img.height();
                                if let Some(ref size) = request.size {
                                    if size.len() >= 2 {
                                        width = size[0] as u32;
                                        height = size[1] as u32;
                                    }
                                }

                                let final_img = if width != img.width() || height != img.height() {
                                    image::imageops::resize(&img, width, height, image::imageops::FilterType::Lanczos3)
                                } else {
                                    img
                                };

                                // 编码输出
                                let mut buffer = Vec::new();
                                let (content_type, fmt) = match output_format.as_str() {
                                    "image/png" => ("image/png", image::ImageFormat::Png),
                                    "image/jpeg" | "image/jpg" => ("image/jpeg", image::ImageFormat::Jpeg),
                                    "image/tiff" | "image/tif" | _ => ("image/tiff", image::ImageFormat::Tiff),
                                };

                                if fmt == image::ImageFormat::Jpeg {
                                    let rgb = image::DynamicImage::ImageRgba8(final_img).to_rgb8();
                                    rgb.write_to(&mut std::io::Cursor::new(&mut buffer), fmt)
                                        .map_err(|e| GeoServerError::RenderingError(e.to_string()))?;
                                } else {
                                    // TIFF 编码：先转为 RGB（TIFF 不能直接编码 RGBA）
                                    let encoded = if fmt == image::ImageFormat::Tiff {
                                        let rgb = image::DynamicImage::ImageRgba8(final_img).to_rgb8();
                                        let mut b = Vec::new();
                                        rgb.write_to(&mut std::io::Cursor::new(&mut b), fmt)
                                            .map_err(|e| GeoServerError::RenderingError(e.to_string()))?;
                                        b
                                    } else {
                                        let mut b = Vec::new();
                                        final_img.write_to(&mut std::io::Cursor::new(&mut b), fmt)
                                            .map_err(|e| GeoServerError::RenderingError(e.to_string()))?;
                                        b
                                    };

                                    return Ok(HttpResponse::Ok()
                                        .content_type(content_type)
                                        .append_header(("Content-Description", format!("Coverage: {}", coverage_id)))
                                        .body(encoded));
                                }

                                return Ok(HttpResponse::Ok()
                                    .content_type(content_type)
                                    .append_header(("Content-Description", format!("Coverage: {}", coverage_id)))
                                    .body(buffer));
                            }
                            Err(e) => {
                                info!("[WCS] GeoTIFF 读取失败: {}, 回退到生成图像", e);
                            }
                        }
                    }
                }
            }
        }
    }

    // 回退：生成测试图像
    info!("[WCS] 未找到 GeoTIFF 数据源，生成测试图像");
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

    let mut buffer = Vec::new();
    let (content_type, fmt) = match output_format.as_str() {
        "image/png" => ("image/png", image::ImageFormat::Png),
        "image/jpeg" | "image/jpg" => ("image/jpeg", image::ImageFormat::Jpeg),
        "image/tiff" | "image/tif" | _ => ("image/tiff", image::ImageFormat::Tiff),
    };

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
