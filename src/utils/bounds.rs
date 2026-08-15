//! 图层边界自动计算工具
//!
//! 支持从 Shapefile、GeoTIFF、PostGIS 数据源自动计算边界

use crate::error::GeoServerError;
use crate::models::{Bounds, CoordinateReferenceSystem, DataSource, DataSourceType};
use tracing::info;

/// 图层边界计算结果
#[derive(Debug, Clone)]
pub struct ComputedBounds {
    pub bounds: Bounds,
    pub crs: CoordinateReferenceSystem,
}

/// 从数据源自动计算图层边界
///
/// 根据数据源类型选择不同的计算方式：
/// - Shapefile → 解析 .shp 文件头
/// - GeoTIFF → 读取 GeoTIFF 地理标签
/// - PostGIS → 查询 ST_Extent()（需外部传入 pg_pool）
pub async fn compute_layer_bounds(
    ds: &DataSource,
    native_name: Option<&str>,
    pg_pool: Option<&deadpool_postgres::Pool>,
) -> Result<Option<ComputedBounds>, GeoServerError> {
    match ds.data_source_type {
        DataSourceType::Shapefile => compute_shapefile_bounds(ds),
        DataSourceType::Geotiff => compute_geotiff_bounds(ds),
        DataSourceType::Postgis => {
            if let (Some(name), Some(pool)) = (native_name, pg_pool) {
                compute_postgis_bounds(ds, name, pool).await
            } else {
                Ok(None)
            }
        },
        DataSourceType::Mysql => Ok(None), // MySQL 边界经要素查询自动计算
        DataSourceType::Geopackage => compute_geopackage_bounds(ds),
        DataSourceType::GeoJson => compute_geojson_bounds(ds),
        DataSourceType::WorldImage => compute_worldimage_bounds(ds),
        DataSourceType::CascadedWms => Ok(None),
        DataSourceType::ArcGrid => compute_arcgrid_bounds(ds),
        DataSourceType::ImageMosaic => compute_mosaic_bounds(ds),
        DataSourceType::ImagePyramid => compute_pyramid_bounds(ds),
        DataSourceType::Redis => Ok(None),
        DataSourceType::Metadata => Ok(None),
    }
}

/// 从 ImagePyramid 目录计算边界 (聚合所有层级 granule 边界)
fn compute_pyramid_bounds(ds: &DataSource) -> Result<Option<ComputedBounds>, GeoServerError> {
    let file_path = ds
        .connection
        .as_ref()
        .and_then(|c| c.file_path.as_ref())
        .ok_or_else(|| GeoServerError::BadRequest("ImagePyramid 数据源缺少目录路径".to_string()))?;

    info!("[Bounds] 从 ImagePyramid 计算边界: {}", file_path);

    let dir = std::path::Path::new(file_path);
    if !dir.is_dir() {
        info!("[Bounds] ImagePyramid 目录不存在: {}", file_path);
        return Ok(None);
    }
    let levels = crate::utils::pyramid::load_pyramid(dir);
    match crate::utils::pyramid::pyramid_bounds(&levels) {
        Some(bounds) => {
            let crs = CoordinateReferenceSystem::EPSG4326;
            info!(
                "[Bounds] ImagePyramid 边界: {:?}, 层级数: {}",
                bounds,
                levels.len()
            );
            Ok(Some(ComputedBounds { bounds, crs }))
        },
        None => {
            info!("[Bounds] ImagePyramid 无有效层级, 返回 None");
            Ok(None)
        },
    }
}

/// 从 ImageMosaic 目录计算边界 (聚合所有 granule 边界)
fn compute_mosaic_bounds(ds: &DataSource) -> Result<Option<ComputedBounds>, GeoServerError> {
    let file_path = ds
        .connection
        .as_ref()
        .and_then(|c| c.file_path.as_ref())
        .ok_or_else(|| GeoServerError::BadRequest("ImageMosaic 数据源缺少目录路径".to_string()))?;

    info!("[Bounds] 从 ImageMosaic 计算边界: {}", file_path);

    let dir = std::path::Path::new(file_path);
    if !dir.is_dir() {
        info!("[Bounds] ImageMosaic 目录不存在: {}", file_path);
        return Ok(None);
    }
    let granules = crate::utils::mosaic::load_mosaic(dir);
    match crate::utils::mosaic::mosaic_bounds(&granules) {
        Some(bounds) => {
            let crs = CoordinateReferenceSystem::EPSG4326;
            info!(
                "[Bounds] ImageMosaic 边界: {:?}, granule 数: {}",
                bounds,
                granules.len()
            );
            Ok(Some(ComputedBounds { bounds, crs }))
        },
        None => {
            info!("[Bounds] ImageMosaic 无有效 granule, 返回 None");
            Ok(None)
        },
    }
}

/// 从 GeoJSON 文件计算边界 (遍历要素坐标)
fn compute_geojson_bounds(ds: &DataSource) -> Result<Option<ComputedBounds>, GeoServerError> {
    let conn = ds
        .connection
        .as_ref()
        .ok_or_else(|| GeoServerError::BadRequest("GeoJSON 数据源缺少连接信息".to_string()))?;
    let file_path = conn
        .file_path
        .as_ref()
        .ok_or_else(|| GeoServerError::BadRequest("GeoJSON 数据源缺少文件路径".to_string()))?;

    info!("[Bounds] 从 GeoJSON 计算边界: {}", file_path);

    let raw = match std::fs::read_to_string(file_path) {
        Ok(s) => s,
        Err(e) => {
            info!("[Bounds] GeoJSON 读取失败: {}", e);
            return Ok(None);
        },
    };
    let root: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            info!("[Bounds] GeoJSON 解析失败: {}", e);
            return Ok(None);
        },
    };

    let mut minx = f64::MAX;
    let mut miny = f64::MAX;
    let mut maxx = f64::MIN;
    let mut maxy = f64::MIN;
    let mut found = false;

    if let Some(features) = root.get("features").and_then(|v| v.as_array()) {
        for f in features {
            if let Some(geom) = f.get("geometry") {
                collect_geojson_coords(geom.get("coordinates"), &mut |x, y| {
                    minx = minx.min(x);
                    miny = miny.min(y);
                    maxx = maxx.max(x);
                    maxy = maxy.max(y);
                    found = true;
                });
            }
        }
    }

    if !found {
        info!("[Bounds] GeoJSON 无有效坐标, 返回 None");
        return Ok(None);
    }

    let bounds = Bounds::new(minx, miny, maxx, maxy);
    info!("[Bounds] GeoJSON 边界: {:?}", bounds);
    Ok(Some(ComputedBounds {
        bounds,
        crs: CoordinateReferenceSystem::EPSG4326,
    }))
}

/// 递归收集 GeoJSON 坐标数组中的所有 (x, y) 坐标点
fn collect_geojson_coords(arr: Option<&serde_json::Value>, visit: &mut impl FnMut(f64, f64)) {
    let Some(arr) = arr else { return };
    if let serde_json::Value::Array(items) = arr {
        if items.iter().all(|i| i.is_number()) {
            // 坐标点 [x, y, ...]
            if items.len() >= 2 {
                let x = items[0].as_f64().unwrap_or(0.0);
                let y = items[1].as_f64().unwrap_or(0.0);
                visit(x, y);
            }
        } else {
            for item in items {
                collect_geojson_coords(Some(item), visit);
            }
        }
    }
}

/// 从 Shapefile 计算边界
fn compute_shapefile_bounds(ds: &DataSource) -> Result<Option<ComputedBounds>, GeoServerError> {
    let file_path = ds
        .connection
        .as_ref()
        .and_then(|c| c.file_path.as_ref())
        .ok_or_else(|| GeoServerError::BadRequest("Shapefile 数据源缺少文件路径".to_string()))?;

    info!("[Bounds] 从 Shapefile 计算边界: {}", file_path);

    match crate::utils::shapefile::read_shapefile(file_path) {
        Ok(result) => {
            let crs = result.crs.unwrap_or(CoordinateReferenceSystem::EPSG4326);
            info!(
                "[Bounds] Shapefile 边界: {:?}, CRS: {:?}",
                result.bounds, crs
            );
            Ok(Some(ComputedBounds {
                bounds: result.bounds,
                crs,
            }))
        },
        Err(e) => {
            info!("[Bounds] Shapefile 读取失败(将使用默认边界): {}", e);
            Ok(None)
        },
    }
}

/// 从 GeoTIFF 计算边界
fn compute_geotiff_bounds(ds: &DataSource) -> Result<Option<ComputedBounds>, GeoServerError> {
    let file_path = ds
        .connection
        .as_ref()
        .and_then(|c| c.file_path.as_ref())
        .ok_or_else(|| GeoServerError::BadRequest("GeoTIFF 数据源缺少文件路径".to_string()))?;

    info!("[Bounds] 从 GeoTIFF 计算边界: {}", file_path);

    match crate::utils::geotiff::read_geotiff(file_path) {
        Ok(coverage) => {
            if let Some(bounds) = coverage.bounds {
                let crs = coverage
                    .crs
                    .map(|c| CoordinateReferenceSystem::from_epsg(&c))
                    .unwrap_or(CoordinateReferenceSystem::EPSG4326);
                info!("[Bounds] GeoTIFF 边界: {:?}, CRS: {:?}", bounds, crs);
                Ok(Some(ComputedBounds { bounds, crs }))
            } else {
                info!("[Bounds] GeoTIFF 无地理标签，使用默认边界");
                Ok(None)
            }
        },
        Err(e) => {
            info!("[Bounds] GeoTIFF 读取失败(将使用默认边界): {}", e);
            Ok(None)
        },
    }
}

/// 从 PostGIS 查询 ST_Extent() 计算边界
async fn compute_postgis_bounds(
    ds: &DataSource,
    native_name: &str,
    pool: &deadpool_postgres::Pool,
) -> Result<Option<ComputedBounds>, GeoServerError> {
    let conn = match ds.connection.as_ref() {
        Some(c) => c,
        None => return Ok(None),
    };

    let client = match pool.get().await {
        Ok(c) => c,
        Err(_) => return Ok(None),
    };

    let schema = conn
        .schema
        .as_deref()
        .map(|s| {
            if s.is_empty() || s == "public" {
                "public"
            } else {
                s
            }
        })
        .unwrap_or("public");

    // 获取几何列名
    let geom_col = get_postgis_geom_column(&client, schema, native_name).await;

    let sql = if let Some(ref col) = geom_col {
        format!(
            "SELECT ST_Extent({}) as extent, ST_SRID({}) as srid FROM \"{}\".\"{}\"",
            col, col, schema, native_name
        )
    } else {
        return Ok(None);
    };

    match client.query_one(&sql, &[]).await {
        Ok(row) => {
            let extent: Option<String> = row.get(0);
            let srid: Option<i32> = row.get(1);

            if let Some(ext_str) = extent {
                // ST_Extent 返回 "BOX(minx miny, maxx maxy)"
                if let Some(bounds) = parse_postgis_extent(&ext_str) {
                    let crs = srid
                        .map(|s| CoordinateReferenceSystem::from_epsg(&format!("EPSG:{}", s)))
                        .unwrap_or(CoordinateReferenceSystem::EPSG4326);
                    info!("[Bounds] PostGIS 边界: {:?}, SRID: {:?}", bounds, srid);
                    return Ok(Some(ComputedBounds { bounds, crs }));
                }
            }
            Ok(None)
        },
        Err(e) => {
            info!("[Bounds] PostGIS ST_Extent 查询失败: {}", e);
            Ok(None)
        },
    }
}

/// 解析 PostGIS ST_Extent 返回的字符串
/// 格式: "BOX(minx miny, maxx maxy)"
fn parse_postgis_extent(ext: &str) -> Option<Bounds> {
    let ext = ext.trim();
    if !ext.starts_with("BOX(") || !ext.ends_with(')') {
        return None;
    }
    let inner = &ext[4..ext.len() - 1];
    let parts: Vec<&str> = inner.split(',').collect();
    if parts.len() != 2 {
        return None;
    }
    let min: Vec<f64> = parts[0]
        .split_whitespace()
        .filter_map(|s| s.parse().ok())
        .collect();
    let max: Vec<f64> = parts[1]
        .split_whitespace()
        .filter_map(|s| s.parse().ok())
        .collect();
    if min.len() >= 2 && max.len() >= 2 {
        Some(Bounds::new(min[0], min[1], max[0], max[1]))
    } else {
        None
    }
}

/// 从 GeoPackage 计算边界
fn compute_geopackage_bounds(ds: &DataSource) -> Result<Option<ComputedBounds>, GeoServerError> {
    let file_path = ds
        .connection
        .as_ref()
        .and_then(|c| c.file_path.as_ref())
        .ok_or_else(|| GeoServerError::BadRequest("GeoPackage 数据源缺少文件路径".to_string()))?;

    info!("[Bounds] 从 GeoPackage 计算边界: {}", file_path);

    let layers = match crate::utils::geopackage::read_geopackage_layers(file_path) {
        Ok(l) => l,
        Err(e) => {
            info!("[Bounds] GeoPackage 读取失败: {}", e);
            return Ok(None);
        },
    };

    if layers.is_empty() {
        info!("[Bounds] GeoPackage 中没有图层");
        return Ok(None);
    }

    // 读取第一个图层的数据以获取边界
    match crate::utils::geopackage::read_geopackage_layer_features(
        file_path,
        &layers[0].table_name,
        Some(1000),
    ) {
        Ok(result) => {
            let crs = CoordinateReferenceSystem::from_epsg(&result.crs);
            info!(
                "[Bounds] GeoPackage 边界: {:?}, CRS: {}",
                result.bounds, result.crs
            );
            Ok(Some(ComputedBounds {
                bounds: result.bounds,
                crs,
            }))
        },
        Err(e) => {
            info!("[Bounds] GeoPackage 要素读取失败: {}", e);
            Ok(None)
        },
    }
}

/// 从 WorldImage 计算边界
fn compute_worldimage_bounds(ds: &DataSource) -> Result<Option<ComputedBounds>, GeoServerError> {
    let file_path = ds
        .connection
        .as_ref()
        .and_then(|c| c.file_path.as_ref())
        .ok_or_else(|| GeoServerError::BadRequest("WorldImage 数据源缺少文件路径".to_string()))?;

    info!("[Bounds] 从 WorldImage 计算边界: {}", file_path);

    match crate::utils::worldimage::read_worldimage_meta(file_path) {
        Ok(meta) => {
            let crs = CoordinateReferenceSystem::from_epsg("EPSG:4326");
            info!("[Bounds] WorldImage 边界: {:?}", meta.bounds);
            Ok(Some(ComputedBounds {
                bounds: meta.bounds,
                crs,
            }))
        },
        Err(e) => {
            info!("[Bounds] WorldImage 读取失败: {}", e);
            Ok(None)
        },
    }
}

/// 从 ArcGrid 计算边界
fn compute_arcgrid_bounds(ds: &DataSource) -> Result<Option<ComputedBounds>, GeoServerError> {
    let file_path = ds
        .connection
        .as_ref()
        .and_then(|c| c.file_path.as_ref())
        .ok_or_else(|| GeoServerError::BadRequest("ArcGrid 数据源缺少文件路径".to_string()))?;

    info!("[Bounds] 从 ArcGrid 计算边界: {}", file_path);

    match crate::utils::arcgrid::read_arcgrid_meta(file_path) {
        Ok((bounds, _width, _height)) => {
            let crs = CoordinateReferenceSystem::EPSG4326;
            info!("[Bounds] ArcGrid 边界: {:?}", bounds);
            Ok(Some(ComputedBounds { bounds, crs }))
        },
        Err(e) => {
            info!("[Bounds] ArcGrid 读取失败: {}", e);
            Ok(None)
        },
    }
}

/// 获取 PostGIS 表的几何列名
async fn get_postgis_geom_column(
    client: &deadpool_postgres::Client,
    schema: &str,
    table: &str,
) -> Option<String> {
    let sql = "SELECT f_geometry_column FROM geometry_columns WHERE f_table_schema = $1 AND f_table_name = $2".to_string();
    match client.query_opt(&sql, &[&schema, &table]).await {
        Ok(Some(row)) => row.get::<_, String>(0).into(),
        _ => {
            // 回退：查询所有 geometry 类型的列
            let sql = "SELECT column_name FROM information_schema.columns
                 WHERE table_schema = $1 AND table_name = $2
                 AND udt_name IN ('geometry', 'geography')"
                .to_string();
            match client.query_opt(&sql, &[&schema, &table]).await {
                Ok(Some(row)) => row.get::<_, String>(0).into(),
                _ => Some("geom".to_string()),
            }
        },
    }
}
