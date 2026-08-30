use crate::error::TerraneError;
use crate::models::{Bounds, DataSourceType};
use crate::services::wcs::{self, CoverageDescription, WcsCapabilities, WcsRequest};
use crate::state::AppState;
use crate::utils::geotiff;
use actix_web::{web, HttpRequest, HttpResponse};
use quick_xml::se::to_string;
use tracing::{info, warn};

pub async fn handle_wcs_request(
    req: HttpRequest,
    query: web::Query<Vec<(String, String)>>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, TerraneError> {
    let params = query.as_ref();
    let wcs_request = wcs::parse_wcs_request(params)?;

    match wcs_request.request {
        wcs::WcsOperation::GetCapabilities => handle_get_capabilities(&state, &wcs_request).await,
        wcs::WcsOperation::DescribeCoverage => handle_describe_coverage(&state, &wcs_request).await,
        wcs::WcsOperation::GetCoverage => handle_get_coverage(&state, &wcs_request, &req).await,
    }
}

/// 解析栅格数据源文件到本地路径 (支持 local / s3)。
///
/// WorldImage 需要读取 .wld 世界文件伴生对象, 走 `materialize_dir`;
/// ImageMosaic 是栅格目录, 走 `materialize_dir`; GeoTIFF / ArcGrid 单文件,
/// 走 `materialize_file`。
async fn materialize_raster(
    conn: &crate::models::DataSourceConnection,
    ds_type: &DataSourceType,
) -> Result<Option<crate::store::file_resolver::MaterializedFile>, TerraneError> {
    match ds_type {
        DataSourceType::WorldImage | DataSourceType::ImageMosaic | DataSourceType::ImagePyramid => {
            crate::store::materialize_dir(conn).await
        },
        _ => crate::store::materialize_file(conn).await,
    }
}

/// 从数据源中查找栅格覆盖数据 (GeoTIFF / WorldImage / ArcGrid / ImageMosaic)。
///
/// local 后端要求文件真实存在; s3 后端乐观纳入 (真实读取在 GetCoverage
/// 时经 materialize 校验), 避免每次 GetCapabilities 都下载对象。
async fn find_raster_coverages(state: &AppState) -> Vec<(String, String, std::path::PathBuf)> {
    let mut coverages = Vec::new();

    if let Some(store) = &state.store {
        if let Ok(ds_list) = store.get_all_data_sources().await {
            for ds in &ds_list {
                if ds.data_source_type == DataSourceType::Geotiff
                    || ds.data_source_type == DataSourceType::WorldImage
                    || ds.data_source_type == DataSourceType::ArcGrid
                    || ds.data_source_type == DataSourceType::ImageMosaic
                    || ds.data_source_type == DataSourceType::ImagePyramid
                {
                    if let Some(conn) = &ds.connection {
                        if let Some(file_path) = conn.file_path.as_ref() {
                            let is_local = crate::store::storage_type(conn) == "local";
                            let exists = !is_local || std::path::Path::new(file_path).exists();
                            if exists {
                                coverages.push((
                                    ds.name.clone(),
                                    ds.name.clone(),
                                    std::path::PathBuf::from(file_path),
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    coverages
}

async fn handle_get_capabilities(
    state: &AppState,
    _request: &WcsRequest,
) -> Result<HttpResponse, TerraneError> {
    let base_url = format!(
        "http://{}:{}",
        state.config.server.host, state.config.server.port
    );
    let mut capabilities = WcsCapabilities::new(&base_url);

    // 动态添加栅格覆盖数据 (GeoTIFF / WorldImage)
    let coverages = find_raster_coverages(state).await;
    for (name, title, _path) in &coverages {
        capabilities.add_coverage(name, title);
    }

    let xml = to_string(&capabilities).map_err(|e| {
        TerraneError::ServiceError(format!("Failed to serialize capabilities: {}", e))
    })?;

    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
{}"#,
        xml
    );

    Ok(HttpResponse::Ok().content_type("application/xml").body(xml))
}

async fn handle_describe_coverage(
    state: &AppState,
    request: &WcsRequest,
) -> Result<HttpResponse, TerraneError> {
    let coverage_ids = request
        .coverage_id
        .as_ref()
        .ok_or_else(|| TerraneError::BadRequest("COVERAGEID parameter is required".to_string()))?;

    let mut descriptions = Vec::new();

    for coverage_id in coverage_ids {
        let mut description = CoverageDescription::new(coverage_id);

        // 尝试从栅格数据源获取真实元数据 (GeoTIFF / ArcGrid / WorldImage)
        if let Some(store) = &state.store {
            if let Ok(Some(ds)) = store.get_data_source(coverage_id).await {
                if let Some(conn) = &ds.connection {
                    if let Ok(Some(materialized)) =
                        materialize_raster(conn, &ds.data_source_type).await
                    {
                        let path = materialized.path;
                        match ds.data_source_type {
                            DataSourceType::Geotiff => {
                                if let Ok(meta) = geotiff::read_geotiff_metadata(&path) {
                                    if let Some(bounds) = meta.bounds {
                                        description.set_bounds(
                                            bounds.minx,
                                            bounds.miny,
                                            bounds.maxx,
                                            bounds.maxy,
                                        );
                                    }
                                    description.set_size(meta.width, meta.height, meta.band_count);
                                }
                            },
                            DataSourceType::ArcGrid => {
                                if let Ok((bounds, width, height)) =
                                    crate::utils::arcgrid::read_arcgrid_meta(&path)
                                {
                                    description.set_bounds(
                                        bounds.minx,
                                        bounds.miny,
                                        bounds.maxx,
                                        bounds.maxy,
                                    );
                                    description.set_size(width, height, 1);
                                }
                            },
                            DataSourceType::WorldImage => {
                                if let Ok(meta) =
                                    crate::utils::worldimage::read_worldimage_meta(&path)
                                {
                                    description.set_bounds(
                                        meta.bounds.minx,
                                        meta.bounds.miny,
                                        meta.bounds.maxx,
                                        meta.bounds.maxy,
                                    );
                                    description.set_size(meta.width, meta.height, 4);
                                }
                            },
                            DataSourceType::ImageMosaic => {
                                // 目录马赛克: 聚合所有 granule 的边界。
                                let granules = crate::utils::mosaic::load_mosaic(&path);
                                if let Some(b) = crate::utils::mosaic::mosaic_bounds(&granules) {
                                    description.set_bounds(b.minx, b.miny, b.maxx, b.maxy);
                                }
                                let total: u64 =
                                    granules.iter().map(|g| g.image.width() as u64).sum();
                                description.set_size(total.max(1) as u32, 1, 4);
                            },
                            DataSourceType::ImagePyramid => {
                                // 金字塔: 聚合所有层级的边界。
                                let levels = crate::utils::pyramid::load_pyramid(&path);
                                if let Some(b) = crate::utils::pyramid::pyramid_bounds(&levels) {
                                    description.set_bounds(b.minx, b.miny, b.maxx, b.maxy);
                                }
                                description.set_size(1024, 1024, 4);
                            },
                            _ => {},
                        }
                    }
                }
            }
        }

        descriptions.push(description);
    }

    // quick-xml 无法直接序列化裸 Vec (无根标签), 逐条序列化后拼接
    let mut inner_xml = String::new();
    for description in &descriptions {
        let item = to_string(description).map_err(|e| {
            TerraneError::ServiceError(format!("Failed to serialize coverage description: {}", e))
        })?;
        inner_xml.push_str(&item);
        inner_xml.push('\n');
    }

    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<wcs:CoverageDescriptions xmlns:wcs="http://www.opengis.net/wcs/2.0"
                          xmlns:gml="http://www.opengis.net/gml/3.2">
{}
</wcs:CoverageDescriptions>"#,
        inner_xml
    );

    Ok(HttpResponse::Ok().content_type("application/xml").body(xml))
}

async fn handle_get_coverage(
    state: &AppState,
    request: &WcsRequest,
    req: &HttpRequest,
) -> Result<HttpResponse, TerraneError> {
    let coverage_ids = request
        .coverage_id
        .as_ref()
        .ok_or_else(|| TerraneError::BadRequest("COVERAGEID parameter is required".to_string()))?;

    // GeoFence: per-coverage layer access (opt-in via geofence_enabled).
    for cid in coverage_ids {
        if state.config.security.geofence_enabled {
            let layers_lock = state.layers.read().await;
            let resolved = layers_lock.iter().find(|l| {
                l.name == *cid || l.store == *cid || l.native_name.as_deref() == Some(cid.as_str())
            });
            match resolved {
                Some(layer) => {
                    crate::utils::geofence::enforce_layer_access(
                        state,
                        req,
                        &layer.workspace,
                        &layer.store,
                        &layer.name,
                        "read",
                    )
                    .await?;
                },
                None => {
                    crate::utils::geofence::enforce_layer_access(state, req, "", "", cid, "read")
                        .await?;
                },
            }
        }
    }

    let coverage_id = coverage_ids.first().ok_or_else(|| {
        TerraneError::BadRequest("At least one COVERAGEID is required".to_string())
    })?;

    let output_format = request
        .output_format
        .as_deref()
        .unwrap_or("image/tiff")
        .to_string();

    // ImageMosaic: 目录栅格马赛克 — 子集 bbox 选择相交 granule 并合成。
    if let Some(store) = &state.store {
        if let Ok(Some(ds)) = store.get_data_source(coverage_id).await {
            if ds.data_source_type == DataSourceType::ImageMosaic {
                if let Some(conn) = &ds.connection {
                    if let Ok(Some(materialized)) =
                        materialize_raster(conn, &ds.data_source_type).await
                    {
                        let dir = materialized.path;
                        let granules = crate::utils::mosaic::load_mosaic(&dir);
                        if let Some(mosaic_b) = crate::utils::mosaic::mosaic_bounds(&granules) {
                            // 目标范围: 子集 bbox ∩ 马赛克范围 (无子集 → 全范围)。
                            let (target, size) =
                                mosaic_target(&mosaic_b, &request.subsets, &request.size);
                            let (w, h) = size;
                            if let Some(img) =
                                crate::utils::mosaic::render_mosaic(&granules, &target, w, h)
                            {
                                let final_img =
                                    encode_coverage_output(img, &output_format, coverage_id);
                                return final_img;
                            }
                        }
                    }
                }
                // 读取失败/无 granule: 落入通用回退 (测试图像)。
            }
        }
    }

    // ImagePyramid: 金字塔栅格 — 按请求分辨率选择层级, 子集 bbox 选择
    // 该层相交 granule 并合成。
    if let Some(store) = &state.store {
        if let Ok(Some(ds)) = store.get_data_source(coverage_id).await {
            if ds.data_source_type == DataSourceType::ImagePyramid {
                if let Some(conn) = &ds.connection {
                    if let Ok(Some(materialized)) =
                        materialize_raster(conn, &ds.data_source_type).await
                    {
                        let dir = materialized.path;
                        let levels = crate::utils::pyramid::load_pyramid(&dir);
                        if let Some(pyr_b) = crate::utils::pyramid::pyramid_bounds(&levels) {
                            let (target, size) =
                                mosaic_target(&pyr_b, &request.subsets, &request.size);
                            let (w, h) = size;
                            // 目标分辨率 = 输出像素对应的地面分辨率。
                            let target_res =
                                (target.maxx - target.minx).max(1e-9) / w.max(1) as f64;
                            if let Some(lvl) =
                                crate::utils::pyramid::select_level(&levels, target_res)
                            {
                                if let Some(img) =
                                    crate::utils::pyramid::render_level(lvl, &target, w, h)
                                {
                                    let final_img =
                                        encode_coverage_output(img, &output_format, coverage_id);
                                    return final_img;
                                }
                            }
                        }
                    }
                }
                // 读取失败/无层级: 落入通用回退 (测试图像)。
            }
        }
    }

    // 尝试从数据源读取真实栅格数据 (GeoTIFF / WorldImage / ArcGrid)
    let raster_result = if let Some(store) = &state.store {
        if let Ok(Some(ds)) = store.get_data_source(coverage_id).await {
            let is_raster = ds.data_source_type == DataSourceType::Geotiff
                || ds.data_source_type == DataSourceType::WorldImage
                || ds.data_source_type == DataSourceType::ArcGrid;
            if is_raster {
                if let Some(conn) = &ds.connection {
                    if let Ok(Some(materialized)) =
                        materialize_raster(conn, &ds.data_source_type).await
                    {
                        let path = materialized.path;
                        info!("[WCS] 从 {:?} 读取覆盖: {:?}", ds.data_source_type, path);
                        match ds.data_source_type {
                            DataSourceType::Geotiff => {
                                match crate::utils::geotiff::read_geotiff(&path) {
                                    Ok(cov) => Some((cov.rgba_image, cov.bounds)),
                                    Err(e) => {
                                        info!("[WCS] GeoTIFF 读取失败: {}", e);
                                        None
                                    },
                                }
                            },
                            DataSourceType::WorldImage => {
                                match crate::utils::worldimage::read_worldimage(&path) {
                                    Ok(wim) => Some((wim.rgba_image, Some(wim.bounds))),
                                    Err(e) => {
                                        info!("[WCS] WorldImage 读取失败: {}", e);
                                        None
                                    },
                                }
                            },
                            DataSourceType::ArcGrid => {
                                match crate::utils::arcgrid::read_arcgrid(&path) {
                                    Ok(ag) => Some((ag.rgba_image, Some(ag.bounds))),
                                    Err(e) => {
                                        info!("[WCS] ArcGrid 读取失败: {}", e);
                                        None
                                    },
                                }
                            },
                            _ => None,
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

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

        // 1. 应用子集（空间裁剪 / 波段选择 / 时间记录）
        let img = apply_coverage_subsets(
            &coverage_data,
            &request.subsets,
            &request.size,
            request.interpolation.as_deref(),
        );

        // 2. 按请求的宽度/高度缩放 (使用请求的插值方式)
        let (width, height) = calculate_output_size(img.width(), img.height(), &request.size);
        let final_img = if width != img.width() || height != img.height() {
            image::imageops::resize(
                &img,
                width,
                height,
                resize_filter(request.interpolation.as_deref()),
            )
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

/// 应用 WCS 子集参数（空间裁剪、波段选择、时间记录等）。
fn apply_coverage_subsets(
    coverage: &crate::utils::geotiff::CoverageData,
    subsets: &Option<Vec<crate::services::wcs::Subset>>,
    size: &Option<Vec<i64>>,
    interpolation: Option<&str>,
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
    // 波段选择 (WCS 2.0 range 子集, 1 起始的波段号; 轴名 r/g/b/a 直接用 0 起始)
    let mut band_min: Option<f64> = None;
    let mut band_max: Option<f64> = None;

    for subset in subsets {
        match subset.subset_type {
            crate::services::wcs::SubsetType::Intervals {
                min,
                max,
                resolution,
            } => {
                match subset.axis_label.to_lowercase().as_str() {
                    "x" | "long" | "longitude" | "i" => {
                        bbox_minx = Some(min);
                        bbox_maxx = Some(max);
                        if resolution.is_some() {
                            use_resolution = resolution;
                        }
                    },
                    "y" | "lat" | "latitude" | "j" => {
                        bbox_miny = Some(min);
                        bbox_maxy = Some(max);
                        if resolution.is_some() {
                            use_resolution = resolution;
                        }
                    },
                    "time" | "t" | "elevation" | "z" | "e" => {
                        // 时间/高程子集: 对于 GeoTIFF，这些目前仅做记录
                        info!(
                            "[WCS] 非空间子集: axis={}, range=[{}, {}]",
                            subset.axis_label, min, max
                        );
                    },
                    // range 子集: 波段选择 (WCS 2.0 `Band(a:b)`, 1 起始)
                    "band" | "range" | "bands" => {
                        band_min = Some(min);
                        band_max = Some(max);
                        info!(
                            "[WCS] 波段子集: axis={}, range=[{}, {}]",
                            subset.axis_label, min, max
                        );
                    },
                    // 通道快捷轴: r/g/b/a (0 起始的通道下标)
                    "r" | "red" => {
                        band_min = Some(0.0);
                        band_max = Some(0.0);
                    },
                    "g" | "green" => {
                        band_min = Some(1.0);
                        band_max = Some(1.0);
                    },
                    "b" | "blue" => {
                        band_min = Some(2.0);
                        band_max = Some(2.0);
                    },
                    "a" | "alpha" => {
                        band_min = Some(3.0);
                        band_max = Some(3.0);
                    },
                    _ => {
                        info!(
                            "[WCS] 未知轴子集: axis={}, range=[{}, {}]",
                            subset.axis_label, min, max
                        );
                    },
                }
            },
            crate::services::wcs::SubsetType::Position { value } => {
                match subset.axis_label.to_lowercase().as_str() {
                    "time" | "t" => {
                        info!(
                            "[WCS] 时间位置子集: axis={}, value={}",
                            subset.axis_label, value
                        );
                    },
                    _ => {
                        info!(
                            "[WCS] 未知轴位置子集: axis={}, value={}",
                            subset.axis_label, value
                        );
                    },
                }
            },
        }
    }

    // 波段选择: 将选中的波段映射到输出通道 (单波段 → 灰度)。
    let img = match (band_min, band_max) {
        (Some(bmin), Some(bmax)) => select_bands(&img, bmin, bmax),
        _ => img,
    };

    // 应用空间裁剪
    if let (Some(minx), Some(miny), Some(maxx), Some(maxy)) =
        (bbox_minx, bbox_miny, bbox_maxx, bbox_maxy)
    {
        let bounds = crate::models::Bounds::new(minx, miny, maxx, maxy);
        match crate::utils::geotiff::crop_coverage(coverage, &bounds) {
            Some(cropped) => {
                info!(
                    "[WCS] 空间裁剪完成: [{}, {}, {}, {}]",
                    minx, miny, maxx, maxy
                );
                // 如果指定了 resolution，则按分辨率重采样
                if let Some(res) = use_resolution {
                    if res > 0.0 {
                        if let Some(ref cov_bounds) = coverage.bounds {
                            let cov_width = ((cov_bounds.maxx - cov_bounds.minx) / res) as u32;
                            let cov_height = ((cov_bounds.maxy - cov_bounds.miny) / res) as u32;
                            if cov_width > 0 && cov_height > 0 {
                                return image::imageops::resize(
                                    &cropped,
                                    cov_width,
                                    cov_height,
                                    resize_filter(interpolation),
                                );
                            }
                        }
                    }
                }
                // 如果指定了 SIZE，使用 SIZE
                if let Some(ref sz) = size {
                    if sz.len() >= 2 && sz[0] > 0 && sz[1] > 0 {
                        return image::imageops::resize(
                            &cropped,
                            sz[0] as u32,
                            sz[1] as u32,
                            resize_filter(interpolation),
                        );
                    }
                }
                return cropped;
            },
            None => {
                warn!("[WCS] 空间裁剪失败: 返回 None");
            },
        }
    }

    img
}

/// 波段选择: 将 `[bmin, bmax]` (1 起始的波段号, 轴 r/g/b/a 直接 0 起始) 中
/// 的波段映射到输出 RGBA 通道 — 单波段 → 灰度, 双波段 → RG, 三波段 → RGB。
/// 越界波段忽略; 无有效波段时返回原图。
fn select_bands(img: &image::RgbaImage, bmin: f64, bmax: f64) -> image::RgbaImage {
    // r/g/b/a 轴以 0 起始直接使用; band/range 轴按 WCS 1 起始约定减 1。
    let (lo, hi) = if (0.0..=3.0).contains(&bmin) && (0.0..=3.0).contains(&bmax) {
        (bmin.max(0.0) as usize, bmax.max(0.0) as usize)
    } else {
        return img.clone();
    };
    // band/range 轴为 1 起始: 若用户给了 >= 1 的号, 转 0 起始。
    let (lo, hi) = if lo >= 1 && hi >= 1 {
        (lo.saturating_sub(1), hi.saturating_sub(1))
    } else {
        (lo, hi)
    };

    let bands: Vec<usize> = (lo..=hi).filter(|b| *b < 4).collect();
    if bands.is_empty() || bands.len() == 4 {
        return img.clone();
    }

    let (w, h) = img.dimensions();
    let mut out = image::RgbaImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let p = img.get_pixel(x, y);
            let vals: Vec<u8> = bands.iter().map(|b| p[*b]).collect();
            let px = match vals.len() {
                1 => image::Rgba([vals[0], vals[0], vals[0], 255]),
                2 => image::Rgba([vals[0], vals[1], 0, 255]),
                _ => image::Rgba([vals[0], vals[1], vals[2], 255]),
            };
            out.put_pixel(x, y, px);
        }
    }
    out
}

/// WCS INTERPOLATION → image resize 滤镜。
/// - nearest → Nearest (WCS 2.0 默认语义)
/// - bilinear → Triangle (双线性)
/// - cubic → CatmullRom (三次)
/// - lanczos → Lanczos3 (本实现历史默认, 亦为 INTERPOLATION 缺省值)
/// - 未知值回退 lanczos, 不报错。
fn resize_filter(interpolation: Option<&str>) -> image::imageops::FilterType {
    match interpolation.map(|s| s.to_lowercase()).as_deref() {
        Some("nearest") | Some("nearestneighbor") | Some("nearest-neighbor") => {
            image::imageops::FilterType::Nearest
        },
        Some("bilinear") => image::imageops::FilterType::Triangle,
        Some("cubic") | Some("bicubic") => image::imageops::FilterType::CatmullRom,
        Some("lanczos") => image::imageops::FilterType::Lanczos3,
        _ => image::imageops::FilterType::Lanczos3,
    }
}

/// 计算输出尺寸
fn calculate_output_size(orig_width: u32, orig_height: u32, size: &Option<Vec<i64>>) -> (u32, u32) {
    match size {
        Some(sz) if sz.len() >= 2 => {
            let w = if sz[0] > 0 { sz[0] as u32 } else { orig_width };
            let h = if sz[1] > 0 { sz[1] as u32 } else { orig_height };
            (w, h)
        },
        _ => (orig_width, orig_height),
    }
}

/// 编码覆盖输出图像
fn encode_coverage_output(
    img: image::RgbaImage,
    output_format: &str,
    coverage_id: &str,
) -> Result<HttpResponse, TerraneError> {
    let (content_type, fmt) = match output_format {
        "image/png" => ("image/png", image::ImageFormat::Png),
        "image/jpeg" | "image/jpg" => ("image/jpeg", image::ImageFormat::Jpeg),
        "image/tiff" | "image/tif" => ("image/tiff", image::ImageFormat::Tiff),
        "image/gif" => ("image/gif", image::ImageFormat::Gif),
        "image/webp" => ("image/webp", image::ImageFormat::WebP),
        "application/x-grib" | "application/x-netcdf" => {
            // 不支持的原生格式，回退到 GeoTIFF
            ("image/tiff", image::ImageFormat::Tiff)
        },
        _ => ("image/tiff", image::ImageFormat::Tiff), // WCS 默认输出 TIFF
    };

    let mut buffer = Vec::new();

    // JPEG 和 TIFF 需要 RGB 而非 RGBA
    if fmt == image::ImageFormat::Jpeg || fmt == image::ImageFormat::Tiff {
        let rgb = image::DynamicImage::ImageRgba8(img).to_rgb8();
        rgb.write_to(&mut std::io::Cursor::new(&mut buffer), fmt)
            .map_err(|e| TerraneError::RenderingError(e.to_string()))?;
    } else {
        img.write_to(&mut std::io::Cursor::new(&mut buffer), fmt)
            .map_err(|e| TerraneError::RenderingError(e.to_string()))?;
    }

    Ok(HttpResponse::Ok()
        .content_type(content_type)
        .append_header(("Content-Description", format!("Coverage: {}", coverage_id)))
        .body(buffer))
}

/// ImageMosaic 目标范围与输出尺寸:
/// - 目标范围 = 子集 bbox ∩ 马赛克范围 (无子集 → 马赛克全范围)
/// - 输出尺寸 = WCS SIZE 参数, 无则按目标范围长宽比给 512 上限
fn mosaic_target(
    mosaic_b: &Bounds,
    subsets: &Option<Vec<crate::services::wcs::Subset>>,
    size: &Option<Vec<i64>>,
) -> (Bounds, (u32, u32)) {
    // 从子集中提取 X/Y 区间。
    let mut minx = mosaic_b.minx;
    let mut miny = mosaic_b.miny;
    let mut maxx = mosaic_b.maxx;
    let mut maxy = mosaic_b.maxy;
    if let Some(subs) = subsets {
        for sub in subs {
            if let crate::services::wcs::SubsetType::Intervals { min, max, .. } = sub.subset_type {
                match sub.axis_label.to_lowercase().as_str() {
                    "x" | "long" | "longitude" | "i" => {
                        minx = min.max(mosaic_b.minx);
                        maxx = max.min(mosaic_b.maxx);
                    },
                    "y" | "lat" | "latitude" | "j" => {
                        miny = min.max(mosaic_b.miny);
                        maxy = max.min(mosaic_b.maxy);
                    },
                    _ => {},
                }
            }
        }
    }
    let target = Bounds::new(minx, miny, maxx, maxy);

    let (w, h) = match size {
        Some(sz) if sz.len() >= 2 => (
            if sz[0] > 0 { sz[0] as u32 } else { 512 },
            if sz[1] > 0 { sz[1] as u32 } else { 512 },
        ),
        _ => {
            let range_x = (maxx - minx).max(1e-9);
            let range_y = (maxy - miny).max(1e-9);
            let base = 512u32;
            let (w, h) = if range_x >= range_y {
                (base, ((base as f64 * range_y / range_x).max(1.0)) as u32)
            } else {
                (((base as f64 * range_x / range_y).max(1.0)) as u32, base)
            };
            (w.max(1), h.max(1))
        },
    };
    (target, (w.max(1), h.max(1)))
}
