//! # ArcGrid (ESRI ASCII Grid) 解析器
//!
//! ESRI ASCII Grid 格式是一种简单的文本栅格格式。
//! 文件以 ASCII 文本存储栅格数据，包含头信息和数据矩阵。
//!
//! ## 格式
//! ```text
//! ncols        100
//! nrows        100
//! xllcorner    100.0
//! yllcorner    50.0
//! cellsize     0.5
//! NODATA_value -9999
//! [行数据，每行 ncols 个值]
//! ```

use std::path::Path;
use image::{RgbaImage, Rgba};
use tracing::info;

use crate::models::Bounds;

/// ArcGrid 覆盖数据
#[derive(Debug, Clone)]
pub struct ArcGridData {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub rgba_image: RgbaImage,
    pub bounds: Bounds,
    pub cell_size: f64,
    pub nodata_value: f64,
    pub min_value: f64,
    pub max_value: f64,
}

/// 读取 ArcGrid 文件
pub fn read_arcgrid<P: AsRef<Path>>(path: P) -> Result<ArcGridData, String> {
    let path = path.as_ref();
    info!("[ArcGrid] 开始读取: {:?}", path);

    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("无法读取 ArcGrid 文件 '{:?}': {}", path, e))?;

    let name = path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("arcgrid")
        .to_string();

    // 解析头信息
    let lines: Vec<&str> = content.lines().collect();
    if lines.len() < 6 {
        return Err("ArcGrid 文件头信息不完整（至少需要 6 行）".to_string());
    }

    let mut ncols = 0u32;
    let mut nrows = 0u32;
    let mut xllcorner = 0.0f64;
    let mut yllcorner = 0.0f64;
    let mut cellsize = 1.0f64;
    let mut nodata = -9999.0f64;
    let mut header_lines = 0;

    for (i, line) in lines.iter().enumerate() {
        let line = line.trim();
        if line.is_empty() { continue; }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 { continue; }

        let key = parts[0].to_lowercase();
        let val = parts[1];

        match key.as_str() {
            "ncols" => {
                ncols = val.parse().map_err(|_| "无效的 ncols".to_string())?;
                header_lines = i + 1;
            }
            "nrows" => {
                nrows = val.parse().map_err(|_| "无效的 nrows".to_string())?;
            }
            "xllcorner" | "xllcenter" => {
                xllcorner = val.parse().map_err(|_| "无效的 xllcorner".to_string())?;
            }
            "yllcorner" | "yllcenter" => {
                yllcorner = val.parse().map_err(|_| "无效的 yllcorner".to_string())?;
            }
            "cellsize" => {
                cellsize = val.parse().map_err(|_| "无效的 cellsize".to_string())?;
            }
            "nodata_value" => {
                nodata = val.parse().unwrap_or(-9999.0);
            }
            _ => {}
        }
    }

    if ncols == 0 || nrows == 0 {
        return Err("ArcGrid 文件缺少 ncols 或 nrows 定义".to_string());
    }

    if cellsize <= 0.0 {
        return Err("ArcGrid cellsize 必须 > 0".to_string());
    }

    // 计算边界
    let minx = xllcorner;
    let maxx = xllcorner + ncols as f64 * cellsize;
    let miny = yllcorner;
    let maxy = yllcorner + nrows as f64 * cellsize;
    let bounds = Bounds::new(minx, miny, maxx, maxy);

    // 读取数据行
    let data_lines: Vec<&str> = lines[header_lines..].iter()
        .filter(|l| !l.trim().is_empty())
        .copied()
        .collect();

    let mut values = Vec::with_capacity((nrows * ncols) as usize);
    let mut min_value = f64::MAX;
    let mut max_value = f64::MIN;

    for line in &data_lines {
        for token in line.split_whitespace() {
            if let Ok(v) = token.parse::<f64>() {
                if (v - nodata).abs() > f64::EPSILON {
                    min_value = min_value.min(v);
                    max_value = max_value.max(v);
                }
                values.push(v);
            }
        }
    }

    if values.len() != (nrows * ncols) as usize {
        return Err(format!(
            "数据点数不匹配: 期望 {} 个, 实际 {} 个",
            nrows * ncols, values.len()
        ));
    }

    // 渲染为 RGBA 图像（单波段灰度）
    let mut img = RgbaImage::new(ncols, nrows);
    let range = if (max_value - min_value).abs() > f64::EPSILON {
        max_value - min_value
    } else {
        1.0
    };

    for y in 0..nrows {
        for x in 0..ncols {
            let idx = (y * ncols + x) as usize;
            let val = values[idx];

            if (val - nodata).abs() < f64::EPSILON {
                img.put_pixel(x, y, Rgba([0, 0, 0, 0])); // 透明 = NODATA
            } else {
                let normalized = ((val - min_value) / range * 255.0) as u8;
                img.put_pixel(x, y, Rgba([normalized, normalized, normalized, 255]));
            }
        }
    }

    info!("[ArcGrid] 读取完成: {}x{}, 值范围 [{}, {}], 边界={:?}",
          ncols, nrows, min_value, max_value, bounds);

    Ok(ArcGridData {
        name,
        width: ncols,
        height: nrows,
        rgba_image: img,
        bounds,
        cell_size: cellsize,
        nodata_value: nodata,
        min_value,
        max_value,
    })
}

/// 读取 ArcGrid 元数据（轻量，不读全量数据）
pub fn read_arcgrid_meta<P: AsRef<Path>>(path: P) -> Result<(Bounds, u32, u32), String> {
    let path = path.as_ref();
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("无法读取 ArcGrid '{:?}': {}", path, e))?;

    let mut ncols = 0u32;
    let mut nrows = 0u32;
    let mut xllcorner = 0.0f64;
    let mut yllcorner = 0.0f64;
    let mut cellsize = 1.0f64;

    for line in content.lines() {
        let parts: Vec<&str> = line.trim().split_whitespace().collect();
        if parts.len() < 2 { continue; }
        match parts[0].to_lowercase().as_str() {
            "ncols" => { ncols = parts[1].parse().unwrap_or(0); }
            "nrows" => { nrows = parts[1].parse().unwrap_or(0); }
            "xllcorner" | "xllcenter" => { xllcorner = parts[1].parse().unwrap_or(0.0); }
            "yllcorner" | "yllcenter" => { yllcorner = parts[1].parse().unwrap_or(0.0); }
            "cellsize" => { cellsize = parts[1].parse().unwrap_or(1.0); }
            _ => { if parts[0].chars().all(|c| c.is_digit(10) || c == '.' || c == '-') { break; } }
        }
    }

    let minx = xllcorner;
    let maxx = xllcorner + ncols as f64 * cellsize;
    let miny = yllcorner;
    let maxy = yllcorner + nrows as f64 * cellsize;
    let bounds = Bounds::new(minx, miny, maxx, maxy);

    Ok((bounds, ncols, nrows))
}
