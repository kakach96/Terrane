//! GeoTIFF 解析器
//!
//! 使用 `image` crate 读取 GeoTIFF 文件，提取栅格数据和元数据
//! 支持: TIFF/GeoTIFF 图像读取、波段信息提取、基础地理配准

use image::{ColorType, DynamicImage, ImageFormat, RgbaImage};
use std::io::BufReader;
use std::path::Path;
use tracing::{debug, info, warn};

use crate::models::Bounds;

/// GeoTIFF 覆盖数据
#[derive(Debug, Clone)]
pub struct CoverageData {
    /// 数据名称
    pub name: String,
    /// 图像宽度（像素）
    pub width: u32,
    /// 图像高度（像素）
    pub height: u32,
    /// 波段数
    pub band_count: usize,
    /// 颜色类型
    pub color_type: String,
    /// 像素数据 (RGBA)
    pub rgba_image: RgbaImage,
    /// 地理边界（如无法从 GeoTIFF 标签读取，则为空）
    pub bounds: Option<Bounds>,
    /// CRS / EPSG 代码
    pub crs: Option<String>,
    /// 像素尺寸 X（地理单位/像素）
    pub pixel_scale_x: Option<f64>,
    /// 像素尺寸 Y（地理单位/像素）
    pub pixel_scale_y: Option<f64>,
    /// 左上角 X 坐标
    pub tie_point_x: Option<f64>,
    /// 左上角 Y 坐标
    pub tie_point_y: Option<f64>,
}

/// GeoTIFF 元数据摘要（轻量，不含完整像素数据）
#[derive(Debug, Clone)]
pub struct GeoTiffMetadata {
    pub width: u32,
    pub height: u32,
    pub band_count: usize,
    pub color_type: String,
    pub bounds: Option<Bounds>,
    pub crs: Option<String>,
    pub file_size_bytes: u64,
}

/// 读取 GeoTIFF 文件并返回完整覆盖数据
pub fn read_geotiff<P: AsRef<Path>>(path: P) -> Result<CoverageData, String> {
    let path = path.as_ref();
    info!("[GeoTIFF] 开始读取: {:?}", path);

    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    // 使用 image crate 读取 TIFF
    let img = image::open(path).map_err(|e| format!("无法打开 GeoTIFF '{:?}': {}", path, e))?;

    let (width, height) = (img.width(), img.height());
    let color_type = format!("{:?}", img.color());
    let band_count = match img.color() {
        ColorType::L8 | ColorType::La8 => 1,
        ColorType::Rgb8 | ColorType::Rgb16 => 3,
        ColorType::Rgba8 | ColorType::Rgba16 => 4,
        _ => {
            warn!("[GeoTIFF] 未知颜色类型 {:?}, 假设 3 波段", img.color());
            3
        },
    };

    // 转为 RGBA
    let rgba = img.to_rgba8();

    // 尝试读取 GeoTIFF 标签（地理配准信息）
    let (bounds, crs, pixel_scale_x, pixel_scale_y, tie_point_x, tie_point_y) =
        read_geotiff_tags(path);

    info!(
        "[GeoTIFF] 读取完成: {}x{}, {} 波段, color={}, bounds={:?}, crs={:?}",
        width, height, band_count, color_type, bounds, crs
    );

    Ok(CoverageData {
        name,
        width,
        height,
        band_count,
        color_type,
        rgba_image: rgba,
        bounds,
        crs,
        pixel_scale_x,
        pixel_scale_y,
        tie_point_x,
        tie_point_y,
    })
}

/// 仅读取 GeoTIFF 元数据（不加载完整像素数据）
pub fn read_geotiff_metadata<P: AsRef<Path>>(path: P) -> Result<GeoTiffMetadata, String> {
    let path = path.as_ref();

    // 使用 image crate 获取基本信息
    let reader = image::io::Reader::open(path)
        .map_err(|e| format!("无法打开 GeoTIFF '{:?}': {}", path, e))?
        .with_guessed_format()
        .map_err(|e| format!("格式检测失败: {}", e))?;

    let (width, height) = reader
        .into_dimensions()
        .map_err(|e| format!("无法读取图像尺寸: {}", e))?;

    let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

    let (bounds, crs, ..) = read_geotiff_tags(path);

    Ok(GeoTiffMetadata {
        width,
        height,
        band_count: 3, // 无法从 reader 获取，后续可优化
        color_type: "RGB".to_string(),
        bounds,
        crs,
        file_size_bytes: file_size,
    })
}

/// 从 GeoTIFF 文件中读取地理标签
///
/// 使用 `tiff` crate 读取 TIFF 标签中的 GEOTIFF 地理配准信息。
/// 由于 `image` crate 不暴露原生 TIFF 标签，我们在此用 `tiff` crate 直接读取文件。
fn read_geotiff_tags(
    path: &Path,
) -> (
    Option<Bounds>,
    Option<String>,
    Option<f64>,
    Option<f64>,
    Option<f64>,
    Option<f64>,
) {
    // 尝试使用 tiff crate（如果可用）读取地理标签
    // 如果 tiff crate 不在依赖中，回退到仅返回 None
    // 目前使用简化的方法：检查文件扩展名，尝试解析已知格式

    // 尝试用 tiff crate 读取地理标签
    match try_read_geotiff_tags_native(path) {
        Ok(result) => return result,
        Err(e) => debug!("[GeoTIFF] 原生标签读取失败 (可选): {}", e),
    }

    // 如果 tiff crate 不可用，尝试以文本方式搜索已知标签
    // 这仅适用于某些格式的 TIFF，不是可靠方法
    match try_read_tags_from_bytes(path) {
        Some(result) => return result,
        None => debug!("[GeoTIFF] 字节级标签解析未找到地理信息"),
    }

    (None, None, None, None, None, None)
}

/// 尝试使用 tiff crate 读取原生 TIFF 标签
fn try_read_geotiff_tags_native(
    path: &Path,
) -> Result<
    (
        Option<Bounds>,
        Option<String>,
        Option<f64>,
        Option<f64>,
        Option<f64>,
        Option<f64>,
    ),
    String,
> {
    use std::fs::File;
    use tiff::decoder::ifd::Value;
    use tiff::decoder::Decoder;
    use tiff::tags::Tag;

    let file = File::open(path).map_err(|e| format!("{:?}", e))?;
    let mut reader = BufReader::new(file);
    let mut decoder = Decoder::new(&mut reader).map_err(|e| format!("{:?}", e))?;

    let (img_width, img_height) = decoder.dimensions().map_err(|e| format!("{:?}", e))?;

    // 读取 ModelTiepointTag (33922) — GeoTIFF 私有标签。
    // 注意: tiff crate 的 Tag 枚举为这些标签定义了具名变体
    // (Tag::ModelTiepointTag / Tag::ModelPixelScaleTag), 解码器存入 IFD 时
    // 用的是具名变体, 必须用具名变体查询 (Tag::Unknown 永远查不到 → bug7)。
    let model_tiepoint = decoder
        .get_tag(Tag::ModelTiepointTag)
        .ok()
        .and_then(|v| match v {
            Value::Double(v) => Some(vec![v]),
            Value::List(items) => {
                let nums: Vec<f64> = items
                    .iter()
                    .filter_map(|item| match item {
                        Value::Double(d) => Some(*d),
                        _ => None,
                    })
                    .collect();
                if nums.len() >= 6 {
                    Some(nums)
                } else {
                    None
                }
            },
            _ => None,
        });

    // ModelPixelScaleTag (33550)
    let model_pixel_scale = decoder
        .get_tag(Tag::ModelPixelScaleTag)
        .ok()
        .and_then(|v| match v {
            Value::Double(v) => Some(vec![v]),
            Value::List(items) => {
                let nums: Vec<f64> = items
                    .iter()
                    .filter_map(|item| match item {
                        Value::Double(d) => Some(*d),
                        _ => None,
                    })
                    .collect();
                if nums.len() >= 3 {
                    Some(nums)
                } else {
                    None
                }
            },
            _ => None,
        });

    let bounds = match (&model_tiepoint, &model_pixel_scale) {
        (Some(tp), Some(ps)) if tp.len() >= 6 && ps.len() >= 3 => {
            let tie_x = tp[3];
            let tie_y = tp[4];
            let scale_x = ps[0];
            let scale_y = ps[1];
            let h = img_height as f64;
            let w = img_width as f64;

            Some(Bounds::new(
                tie_x,
                tie_y - h * scale_y,
                tie_x + w * scale_x,
                tie_y,
            ))
        },
        _ => None,
    };

    let pscale_x = model_pixel_scale
        .as_ref()
        .and_then(|ps| ps.first().copied());
    let pscale_y = model_pixel_scale.as_ref().and_then(|ps| ps.get(1).copied());
    let tie_x = model_tiepoint.as_ref().and_then(|tp| tp.get(3).copied());
    let tie_y = model_tiepoint.as_ref().and_then(|tp| tp.get(4).copied());

    Ok((bounds, None, pscale_x, pscale_y, tie_x, tie_y))
}

/// 回退方案：从原始字节中搜索 GeoTIFF 标签
fn try_read_tags_from_bytes(
    _path: &Path,
) -> Option<(
    Option<Bounds>,
    Option<String>,
    Option<f64>,
    Option<f64>,
    Option<f64>,
    Option<f64>,
)> {
    // 这是一个简化的回退实现
    // 实际生产环境中建议使用 proj / gdal 等专业库读取 GeoTIFF 标签
    None
}

/// 从 CoverageData 中裁剪指定区域的子图像
pub fn crop_coverage(data: &CoverageData, bounds: &Bounds) -> Option<RgbaImage> {
    if data.bounds.is_none() {
        // 无地理参考，返回全图
        return Some(data.rgba_image.clone());
    }

    let data_bounds = data.bounds.as_ref().unwrap();

    // 检查是否相交
    if bounds.minx >= data_bounds.maxx
        || bounds.maxx <= data_bounds.minx
        || bounds.miny >= data_bounds.maxy
        || bounds.maxy <= data_bounds.miny
    {
        return None; // 不相交
    }

    // 计算像素坐标
    let img_width = data.width as f64;
    let img_height = data.height as f64;

    let x_ratio = img_width / (data_bounds.maxx - data_bounds.minx);
    let y_ratio = img_height / (data_bounds.maxy - data_bounds.miny);

    let px = ((bounds.minx.max(data_bounds.minx) - data_bounds.minx) * x_ratio) as u32;
    let py = ((data_bounds.maxy - bounds.maxy.min(data_bounds.maxy)) * y_ratio) as u32;
    let pw =
        ((bounds.maxx.min(data_bounds.maxx) - bounds.minx.max(data_bounds.minx)) * x_ratio) as u32;
    let ph =
        ((bounds.maxy.min(data_bounds.maxy) - bounds.miny.max(data_bounds.miny)) * y_ratio) as u32;

    if pw == 0 || ph == 0 {
        return None;
    }

    let mut img = data.rgba_image.clone();
    Some(image::imageops::crop(&mut img, px, py, pw, ph).to_image())
}

/// 将 CoverageData 编码为指定格式的字节缓冲区
pub fn encode_coverage(data: &CoverageData, format: &str) -> Result<Vec<u8>, String> {
    let mut buf = Vec::new();

    match format {
        "image/png" | "png" => {
            data.rgba_image
                .write_to(&mut std::io::Cursor::new(&mut buf), ImageFormat::Png)
                .map_err(|e| format!("PNG 编码失败: {}", e))?;
        },
        "image/jpeg" | "image/jpg" | "jpeg" | "jpg" => {
            // JPEG 不支持透明度，先转为 RGB
            let rgb = DynamicImage::ImageRgba8(data.rgba_image.clone()).to_rgb8();
            rgb.write_to(&mut std::io::Cursor::new(&mut buf), ImageFormat::Jpeg)
                .map_err(|e| format!("JPEG 编码失败: {}", e))?;
        },
        "image/tiff" | "image/tif" | "tiff" | "tif" => {
            data.rgba_image
                .write_to(&mut std::io::Cursor::new(&mut buf), ImageFormat::Tiff)
                .map_err(|e| format!("TIFF 编码失败: {}", e))?;
        },
        _ => {
            // 默认输出 PNG
            data.rgba_image
                .write_to(&mut std::io::Cursor::new(&mut buf), ImageFormat::Png)
                .map_err(|e| format!("PNG 编码失败: {}", e))?;
        },
    }

    Ok(buf)
}

/// 判断文件是否为支持的栅格格式
pub fn is_supported_raster_format<P: AsRef<Path>>(path: P) -> bool {
    match path.as_ref().extension().and_then(|e| e.to_str()) {
        Some(ext) => matches!(
            ext.to_lowercase().as_str(),
            "tif" | "tiff" | "png" | "jpg" | "jpeg"
        ),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_supported_format() {
        assert!(is_supported_raster_format("test.tif"));
        assert!(is_supported_raster_format("test.tiff"));
        assert!(is_supported_raster_format("test.png"));
        assert!(!is_supported_raster_format("test.shp"));
        assert!(!is_supported_raster_format("test.txt"));
    }

    #[test]
    fn test_crop_coverage_no_geo() {
        let img = RgbaImage::new(100, 100);
        let data = CoverageData {
            name: "test".to_string(),
            width: 100,
            height: 100,
            band_count: 3,
            color_type: "RGB".to_string(),
            rgba_image: img,
            bounds: None,
            crs: None,
            pixel_scale_x: None,
            pixel_scale_y: None,
            tie_point_x: None,
            tie_point_y: None,
        };
        let bounds = Bounds::new(0.0, 0.0, 10.0, 10.0);
        let cropped = crop_coverage(&data, &bounds);
        assert!(cropped.is_some());
        assert_eq!(cropped.unwrap().dimensions(), (100, 100));
    }

    /// 生成带地理配准标签的 8x8 RGB GeoTIFF fixture (tiff encoder):
    /// ModelPixelScaleTag=[1,1,0], ModelTiepointTag=[0,0,0,0,8,0] → bounds (0,0,8,8)
    fn create_georef_tiff_fixture(dir: &std::path::Path) -> std::path::PathBuf {
        use tiff::encoder::*;
        use tiff::tags::Tag;

        let path = dir.join("geo.tif");
        let file = std::fs::File::create(&path).unwrap();
        let mut tiff = TiffEncoder::new(file).unwrap();
        let mut image_enc = tiff.new_image::<colortype::RGB8>(8, 8).unwrap();
        let pixel_scale: &[f64] = &[1.0, 1.0, 0.0];
        let tiepoint: &[f64] = &[0.0, 0.0, 0.0, 0.0, 8.0, 0.0];
        image_enc
            .encoder()
            .write_tag(Tag::ModelPixelScaleTag, pixel_scale)
            .unwrap();
        image_enc
            .encoder()
            .write_tag(Tag::ModelTiepointTag, tiepoint)
            .unwrap();
        let mut data = Vec::with_capacity(8 * 8 * 3);
        for _ in 0..64 {
            data.extend_from_slice(&[10, 20, 30]);
        }
        image_enc.write_data(&data).unwrap();
        path
    }

    /// bug7 守护: 真实 GeoTIFF 的地理配准标签 (ModelTiepointTag/ModelPixelScaleTag)
    /// 必须能通过 read_geotiff_metadata / read_geotiff 读到边界。此前用
    /// Tag::Unknown 查询永远失败 → bounds=None → WCS SUBSET 静默返回全图。
    #[test]
    fn test_read_geotiff_georef_tags() {
        let dir = std::env::temp_dir().join(format!("terrane-geotiff-geo-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = create_georef_tiff_fixture(&dir);

        // read_geotiff_metadata: 边界 + 尺寸
        let meta = read_geotiff_metadata(&path).unwrap();
        let b = meta.bounds.expect("GeoTIFF 应读到地理边界 (bug7 守护)");
        assert!(
            (b.minx - 0.0).abs() < 1e-6,
            "minx 应为 0.0, 实际: {}",
            b.minx
        );
        assert!(
            (b.miny - 0.0).abs() < 1e-6,
            "miny 应为 0.0, 实际: {}",
            b.miny
        );
        assert!(
            (b.maxx - 8.0).abs() < 1e-6,
            "maxx 应为 8.0, 实际: {}",
            b.maxx
        );
        assert!(
            (b.maxy - 8.0).abs() < 1e-6,
            "maxy 应为 8.0, 实际: {}",
            b.maxy
        );
        assert_eq!(meta.width, 8);
        assert_eq!(meta.height, 8);

        // read_geotiff: 完整覆盖数据也应带边界 + 像素比例
        let cov = read_geotiff(&path).unwrap();
        let cb = cov.bounds.expect("read_geotiff 也应读到边界");
        assert!((cb.minx - 0.0).abs() < 1e-6);
        assert!((cb.maxy - 8.0).abs() < 1e-6);
        assert!(cov.pixel_scale_x.is_some(), "应读到像素比例 X");
        assert_eq!(cov.pixel_scale_x.unwrap(), 1.0);
        assert_eq!(cov.tie_point_y.unwrap(), 8.0);

        // crop_coverage 应能按地理子集裁剪 (SUBSET 0..2 → 2x2)
        let img = read_geotiff(&path).unwrap();
        let cropped = crop_coverage(&img, &Bounds::new(0.0, 0.0, 2.0, 2.0)).unwrap();
        assert_eq!(cropped.dimensions(), (2, 2), "SUBSET(0,2) 应裁剪为 2x2");

        std::fs::remove_dir_all(&dir).ok();
    }
}
