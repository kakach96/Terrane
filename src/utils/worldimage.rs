//! # WorldImage 解析器
//!
//! WorldImage 格式: 图片文件 (.png/.jpg/.tif) + 同名世界文件 (.pgw/.jgw/.tfw)
//!
//! 世界文件包含 6 行仿射变换参数:
//! ```text
//! A = pixel width (x-scale)
//! D = y-rotation
//! B = x-rotation
//! E = pixel height (y-scale, 通常为负值)
//! C = top-left x
//! F = top-left y
//! ```

use std::path::{Path, PathBuf};
use image::RgbaImage;
use tracing::info;

use crate::models::Bounds;

/// WorldImage 覆盖数据
#[derive(Debug, Clone)]
pub struct WorldImageData {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub rgba_image: RgbaImage,
    pub bounds: Bounds,
    pub pixel_scale_x: f64,
    pub pixel_scale_y: f64,
    pub tie_point_x: f64,
    pub tie_point_y: f64,
}

/// WorldImage 元数据
#[derive(Debug, Clone)]
pub struct WorldImageMeta {
    pub width: u32,
    pub height: u32,
    pub bounds: Bounds,
    pub crs: Option<String>,
    pub file_size_bytes: u64,
}

/// 世界文件扩展名与图片扩展名的映射
fn world_file_ext(image_path: &Path) -> Option<&'static str> {
    match image_path.extension()?.to_str()?.to_lowercase().as_str() {
        "png" => Some("pgw"),
        "jpg" | "jpeg" => Some("jgw"),
        "tif" | "tiff" => Some("tfw"),
        "bmp" => Some("bpw"),
        "gif" => Some("gfw"),
        _ => None,
    }
}

/// 解析世界文件
fn parse_world_file(path: &Path) -> Result<(f64, f64, f64, f64, f64, f64), String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("无法读取世界文件 '{:?}': {}", path, e))?;

    let values: Vec<f64> = content
        .lines()
        .filter_map(|line| line.trim().parse::<f64>().ok())
        .collect();

    if values.len() < 6 {
        return Err(format!("世界文件格式错误: 需要 6 个数值, 找到 {}", values.len()));
    }

    Ok((values[0], values[1], values[2], values[3], values[4], values[5]))
}

/// 查找世界文件（尝试多种扩展名）
fn find_world_file(image_path: &Path) -> Option<PathBuf> {
    let stem = image_path.with_extension("");

    // 优先使用标准世界文件扩展名
    if let Some(wld_ext) = world_file_ext(image_path) {
        let wld_path = stem.with_extension(wld_ext);
        if wld_path.exists() {
            return Some(wld_path);
        }
    }

    // 回退: 尝试 .wld 扩展名 (某些 GIS 软件使用)
    let wld_path = stem.with_extension("wld");
    if wld_path.exists() {
        return Some(wld_path);
    }

    None
}

/// 读取 WorldImage 文件
pub fn read_worldimage<P: AsRef<Path>>(path: P) -> Result<WorldImageData, String> {
    let path = path.as_ref();
    info!("[WorldImage] 开始读取: {:?}", path);

    let name = path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("worldimage")
        .to_string();

    // 读取图片
    let img = image::open(path)
        .map_err(|e| format!("无法读取图片 '{:?}': {}", path, e))?;

    let (width, height) = (img.width(), img.height());
    let rgba = img.to_rgba8();

    // 读取世界文件
    let wld_path = find_world_file(path)
        .ok_or_else(|| format!("找不到世界文件 (尝试了 .pgw/.jgw/.tfw/.bpw/.gfw/.wld) 对于 '{:?}'", path))?;

    let (_a, _d, _b, e, c, f) = parse_world_file(&wld_path)?;

    // 计算边界
    let pixel_scale_x = _a.abs();
    let pixel_scale_y = e.abs();
    let minx = c - pixel_scale_x / 2.0;  // 左上角像素中心 → 左边界
    let maxx = minx + width as f64 * pixel_scale_x;
    let maxy = f + pixel_scale_y / 2.0;  // 左上角像素中心 → 上边界
    let miny = maxy - height as f64 * pixel_scale_y;

    let bounds = Bounds::new(minx, miny, maxx, maxy);

    info!("[WorldImage] 读取完成: {}x{}, 边界={:?}, 世界文件={:?}",
          width, height, bounds, wld_path);

    Ok(WorldImageData {
        name,
        width,
        height,
        rgba_image: rgba,
        bounds,
        pixel_scale_x,
        pixel_scale_y,
        tie_point_x: c,
        tie_point_y: f,
    })
}

/// 读取 WorldImage 元数据（轻量）
pub fn read_worldimage_meta<P: AsRef<Path>>(path: P) -> Result<WorldImageMeta, String> {
    let path = path.as_ref();
    let img = image::open(path)
        .map_err(|e| format!("无法读取图片 '{:?}': {}", path, e))?;

    let wld_path = find_world_file(path)
        .ok_or_else(|| format!("找不到世界文件对于 '{:?}'", path))?;

    let (_a, _d, _b, e, c, f) = parse_world_file(&wld_path)?;

    let pixel_scale_x = _a.abs();
    let pixel_scale_y = e.abs();
    let width = img.width();
    let height = img.height();
    let minx = c - pixel_scale_x / 2.0;
    let maxx = minx + width as f64 * pixel_scale_x;
    let maxy = f + pixel_scale_y / 2.0;
    let miny = maxy - height as f64 * pixel_scale_y;
    let bounds = Bounds::new(minx, miny, maxx, maxy);

    let file_size = std::fs::metadata(path)
        .map(|m| m.len())
        .unwrap_or(0);

    Ok(WorldImageMeta {
        width,
        height,
        bounds,
        crs: Some("EPSG:4326".to_string()),
        file_size_bytes: file_size,
    })
}

/// 按边界裁剪 WorldImage
pub fn crop_worldimage(data: &WorldImageData, bounds: &Bounds) -> Option<RgbaImage> {
    let px = data.pixel_scale_x;
    let py = data.pixel_scale_y;

    // 计算像素坐标
    let x1 = ((bounds.minx - data.bounds.minx) / px) as i64;
    let y1 = ((data.bounds.maxy - bounds.maxy) / py) as i64;
    let x2 = ((bounds.maxx - data.bounds.minx) / px) as i64;
    let y2 = ((data.bounds.maxy - bounds.miny) / py) as i64;

    let x = x1.max(0) as u32;
    let y = y1.max(0) as u32;
    let w = (x2 - x1 as i64).max(0).min((data.width - x) as i64) as u32;
    let h = (y2 - y1 as i64).max(0).min((data.height - y) as i64) as u32;

    if w == 0 || h == 0 {
        return None;
    }

    Some(image::imageops::crop(&mut data.rgba_image.clone(), x, y, w, h).to_image())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("terrane-wld-{}-{}", tag, std::process::id()))
    }

    fn write_fixture(tag: &str) -> PathBuf {
        let dir = temp_dir(tag);
        std::fs::create_dir_all(&dir).unwrap();
        let img_path = dir.join("ortho.png");
        let img = RgbaImage::from_pixel(2, 2, image::Rgba([10, 20, 30, 255]));
        img.save(&img_path).unwrap();
        let wld_path = dir.join("ortho.pgw");
        // A=0.5 D=0 B=0 E=-0.5 C=100 F=200
        std::fs::write(&wld_path, "0.5\n0.0\n0.0\n-0.5\n100.0\n200.0\n").unwrap();
        img_path
    }

    #[test]
    fn test_world_file_ext_mapping() {
        assert_eq!(world_file_ext(Path::new("a.png")), Some("pgw"));
        assert_eq!(world_file_ext(Path::new("a.jpg")), Some("jgw"));
        assert_eq!(world_file_ext(Path::new("a.tif")), Some("tfw"));
        assert_eq!(world_file_ext(Path::new("a.bmp")), Some("bpw"));
        assert_eq!(world_file_ext(Path::new("a.txt")), None);
    }

    #[test]
    fn test_read_worldimage_meta() {
        let img_path = write_fixture("meta");
        let meta = read_worldimage_meta(&img_path).unwrap();

        assert_eq!(meta.width, 2);
        assert_eq!(meta.height, 2);
        // minx = 100 - 0.25 = 99.75; maxx = 99.75 + 2*0.5 = 100.75
        assert!((meta.bounds.minx - 99.75).abs() < 1e-6);
        assert!((meta.bounds.maxx - 100.75).abs() < 1e-6);
        // maxy = 200 + 0.25 = 200.25; miny = 200.25 - 2*0.5 = 199.25
        assert!((meta.bounds.maxy - 200.25).abs() < 1e-6);
        assert!((meta.bounds.miny - 199.25).abs() < 1e-6);
        assert_eq!(meta.crs.as_deref(), Some("EPSG:4326"));

        std::fs::remove_dir_all(img_path.parent().unwrap()).ok();
    }

    #[test]
    fn test_crop_worldimage() {
        let img_path = write_fixture("crop");
        let data = read_worldimage(&img_path).unwrap();
        // 裁剪左下角 1x1 (像素坐标 x=0, y=1)
        let crop = crop_worldimage(&data, &Bounds::new(99.75, 199.25, 100.25, 199.75)).unwrap();
        assert_eq!(crop.width(), 1);
        assert_eq!(crop.height(), 1);
        std::fs::remove_dir_all(img_path.parent().unwrap()).ok();
    }

    #[test]
    fn test_missing_world_file() {
        let dir = temp_dir("nowld");
        std::fs::create_dir_all(&dir).unwrap();
        let img_path = dir.join("orphan.png");
        RgbaImage::new(1, 1).save(&img_path).unwrap();
        assert!(read_worldimage(&img_path).is_err(), "缺世界文件应报错");
        std::fs::remove_dir_all(&dir).ok();
    }
}
