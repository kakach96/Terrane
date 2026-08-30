//! Shared tile-rendering pipeline used by the tile protocol surfaces
//! (WMTS / TMS / WMS-C).
//!
//! All three surfaces resolve a `(layer, gridset, z, col, row)` request to the
//! same PNG bytes via the in-memory metadata store and the local vector store,
//! and share the GeoWebCache-style tile cache when it is enabled.

use crate::error::TerraneError;
use crate::handlers::features;
use crate::models::Bounds;
use crate::state::AppState;
use crate::utils::rendering::{MapRenderer, RenderFormat, RenderOptions};
use crate::utils::tile_grid;
use actix_web::{HttpRequest, HttpResponse};

/// FNV-1a 64-bit content hash → ETag for tile bytes.
pub fn tile_etag(data: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in data {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    }
    format!("\"{:016x}\"", hash)
}

/// Current time as an HTTP `Last-Modified` stamp.
pub fn tile_last_modified() -> String {
    chrono::Utc::now()
        .format("%a, %d %b %Y %H:%M:%S GMT")
        .to_string()
}

/// Build a `304 Not Modified` response when the request carries a matching
/// `If-None-Match` / `If-Modified-Since`; otherwise `None` (serve normally).
///
/// `hit` marks the tile-cache result for the `X-Tile-Cache` header.
pub fn conditional_tile_response(
    req: &HttpRequest,
    data: &[u8],
    hit: bool,
) -> Option<HttpResponse> {
    let etag = tile_etag(data);
    let last_modified = tile_last_modified();

    let not_modified = |req: &HttpRequest, etag: &str, last_modified: &str| -> bool {
        // If-None-Match: exact match (or "*") → 304.
        if let Some(inm) = req
            .headers()
            .get("If-None-Match")
            .and_then(|v| v.to_str().ok())
        {
            let inm = inm.trim();
            if inm == "*"
                || inm.trim_matches('"') == etag.trim_matches('"')
                || inm
                    .split(',')
                    .any(|t| t.trim().trim_matches('"') == etag.trim_matches('"'))
            {
                return true;
            }
        }
        // If-Modified-Since: equal to the current Last-Modified stamp → 304.
        if let Some(ims) = req
            .headers()
            .get("If-Modified-Since")
            .and_then(|v| v.to_str().ok())
        {
            if ims.trim() == last_modified {
                return true;
            }
        }
        false
    };

    if not_modified(req, &etag, &last_modified) {
        return Some(
            HttpResponse::NotModified()
                .insert_header(("X-Tile-Cache", if hit { "HIT" } else { "MISS" }))
                .insert_header(("ETag", etag.clone()))
                .insert_header(("Last-Modified", last_modified.clone()))
                .finish(),
        );
    }
    None
}

/// Output format for a rendered tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileFormat {
    Png,
    Jpeg,
}

impl TileFormat {
    /// Map a file extension (TMS) to a tile format (defaults to PNG).
    pub fn from_extension(ext: &str) -> TileFormat {
        match ext {
            "jpeg" | "jpg" => TileFormat::Jpeg,
            _ => TileFormat::Png,
        }
    }

    /// Map a WMS output format (MIME) to a tile format (defaults to PNG).
    pub fn from_mime(mime: &str) -> TileFormat {
        match mime {
            m if m.contains("jpeg") || m.contains("jpg") => TileFormat::Jpeg,
            _ => TileFormat::Png,
        }
    }

    pub fn mime(&self) -> &'static str {
        match self {
            TileFormat::Png => "image/png",
            TileFormat::Jpeg => "image/jpeg",
        }
    }
}

/// Render a single tile for `(layer, gridset, z, col, row)` where `row` is
/// top-down (WMTS / slippy convention). Returns the encoded bytes and whether
/// they came from the tile cache (`true` = HIT) or were freshly rendered
/// (`false` = MISS). Only PNG output participates in the cache (the cache key
/// is format-less, so JPEG renders are always fresh).
pub async fn render_tile_bytes(
    state: &AppState,
    layer: &str,
    gridset: &str,
    z: u32,
    col: u32,
    row: u32,
    format: TileFormat,
) -> Result<(Vec<u8>, bool), TerraneError> {
    let gridset = tile_grid::canonical_gridset(gridset);

    // 0. Resolve the per-layer cache backend (layer.cache_store → Redis data
    //    source, otherwise the default local cache).
    let layer_obj = {
        let layers = state.layers.read().await;
        layers.iter().find(|l| l.name == layer).cloned()
    };
    let cache = match &layer_obj {
        Some(l) => state.tile_cache_for(l).await,
        None => state.tile_cache.clone(),
    };

    // 1. Try the tile cache first (PNG only).
    if format == TileFormat::Png {
        if let Some(ref cache) = cache {
            if let Some(cached) = cache.get(layer, &gridset, z, col, row).await {
                return Ok((cached, true));
            }
        }
    }

    // 2. Compute the tile bounds from the shared grid math.
    let bounds = tile_grid::tile_bounds(&gridset, z, col, row).ok_or_else(|| {
        TerraneError::BadRequest(format!(
            "Tile index z={} x={} y={} is out of range for gridset {}",
            z, col, row, gridset
        ))
    })?;

    let tile_size = 256u32;
    let options = RenderOptions {
        width: tile_size,
        height: tile_size,
        transparent: true,
        bg_color: None,
        format: RenderFormat::PNG,
    };

    let renderer = MapRenderer::new(options, bounds.clone());
    let layers_lock = state.layers.read().await;
    let styles_lock = state.styles.read().await;
    let meta_lock = state.styles_meta.read().await;
    let mut render_items = Vec::new();
    let mut raster: Option<Option<(image::RgbaImage, Bounds)>> = None;

    if let Some(layer_obj) = layers_lock.iter().find(|l| l.name == layer) {
        use crate::handlers::style_handler::{
            calculate_tile_scale_denom, get_style_rules, reproject_geometry_helper,
        };
        use crate::utils::sld_parser;

        // 栅格图层 (GeoTIFF / WorldImage / ArcGrid / ImageMosaic): 加载栅格并绘制到瓦片。
        let data_source = if let Some(store) = &state.store {
            store.get_data_source(&layer_obj.store).await.ok().flatten()
        } else {
            None
        };
        if let Some(ds) = &data_source {
            if matches!(
                ds.data_source_type,
                crate::models::DataSourceType::Geotiff
                    | crate::models::DataSourceType::WorldImage
                    | crate::models::DataSourceType::ArcGrid
                    | crate::models::DataSourceType::ImageMosaic
                    | crate::models::DataSourceType::ImagePyramid
            ) {
                if let Some(conn) = &ds.connection {
                    let materialized = match ds.data_source_type {
                        crate::models::DataSourceType::WorldImage
                        | crate::models::DataSourceType::ImageMosaic
                        | crate::models::DataSourceType::ImagePyramid => {
                            crate::store::materialize_dir(conn).await.ok().flatten()
                        },
                        _ => crate::store::materialize_file(conn).await.ok().flatten(),
                    };
                    if let Some(m) = materialized {
                        let img_bounds = match ds.data_source_type {
                            crate::models::DataSourceType::Geotiff => {
                                crate::utils::geotiff::read_geotiff(&m.path)
                                    .ok()
                                    .map(|cov| (cov.rgba_image, cov.bounds))
                            },
                            crate::models::DataSourceType::WorldImage => {
                                crate::utils::worldimage::read_worldimage(&m.path)
                                    .ok()
                                    .map(|w| (w.rgba_image, Some(w.bounds)))
                            },
                            crate::models::DataSourceType::ArcGrid => {
                                crate::utils::arcgrid::read_arcgrid(&m.path)
                                    .ok()
                                    .map(|a| (a.rgba_image, Some(a.bounds)))
                            },
                            crate::models::DataSourceType::ImageMosaic => {
                                let granules = crate::utils::mosaic::load_mosaic(&m.path);
                                let b = crate::utils::mosaic::mosaic_bounds(&granules);
                                match b {
                                    Some(bb) => crate::utils::mosaic::render_mosaic(
                                        &granules, &bb, 1024, 1024,
                                    )
                                    .map(|img| (img, Some(bb))),
                                    None => None,
                                }
                            },
                            crate::models::DataSourceType::ImagePyramid => {
                                let levels = crate::utils::pyramid::load_pyramid(&m.path);
                                let b = crate::utils::pyramid::pyramid_bounds(&levels);
                                match b {
                                    Some(bb) => {
                                        let lvl = crate::utils::pyramid::select_level(
                                            &levels,
                                            f64::MIN_POSITIVE,
                                        );
                                        lvl.and_then(|l| {
                                            crate::utils::pyramid::render_level(l, &bb, 1024, 1024)
                                                .map(|img| (img, Some(bb)))
                                        })
                                    },
                                    None => None,
                                }
                            },
                            _ => None,
                        };
                        if let Some((img, Some(b))) = img_bounds {
                            raster = Some(Some((img, b)));
                        }
                    }
                }
                drop(layers_lock);
                drop(styles_lock);
                drop(meta_lock);
                return finish_tile(
                    state,
                    &cache,
                    layer,
                    &gridset,
                    z,
                    col,
                    row,
                    format,
                    renderer,
                    bounds,
                    raster,
                    Vec::new(),
                )
                .await;
            }
        }

        let layer_crs = layer_obj.srs.to_epsg();
        let needs_reproject = layer_crs != "EPSG:4326";
        let rules = get_style_rules(&styles_lock, &meta_lock, layer_obj);

        let features = features::query_layer_features(state, layer, None, None, None)
            .await
            .unwrap_or_default();
        let scale_denom = calculate_tile_scale_denom(z);
        for feature in &features {
            let geom = if needs_reproject {
                reproject_geometry_helper(&feature.geometry, &layer_crs, "EPSG:4326")
            } else {
                feature.geometry.clone()
            };
            let style = sld_parser::resolve_style(&rules, feature, Some(scale_denom));
            render_items.push((geom, style));
        }
    }
    drop(layers_lock);
    drop(styles_lock);
    drop(meta_lock);

    finish_tile(
        state,
        &cache,
        layer,
        &gridset,
        z,
        col,
        row,
        format,
        renderer,
        bounds,
        raster,
        render_items,
    )
    .await
}

/// 共享瓦片渲染收尾: 栅格底图 + 矢量叠加 → 编码 → 缓存。
#[allow(clippy::too_many_arguments)] // tile pipeline wiring
async fn finish_tile(
    state: &AppState,
    cache: &Option<crate::utils::tile_cache::TileCache>,
    layer: &str,
    gridset: &str,
    z: u32,
    col: u32,
    row: u32,
    format: TileFormat,
    renderer: MapRenderer,
    bounds: Bounds,
    raster: Option<Option<(image::RgbaImage, Bounds)>>,
    render_items: Vec<(
        crate::models::GeoJsonGeometry,
        crate::utils::rendering::Style,
    )>,
) -> Result<(Vec<u8>, bool), TerraneError> {
    let mut img = image::RgbaImage::new(256, 256);

    // 栅格底图 (与 WMS GetMap 相同的裁剪/缩放逻辑)。
    if let Some(Some((raster_img, raster_bounds))) = raster {
        if let Some(tile) =
            super::wms_handler::render_raster_to_map(&raster_img, &raster_bounds, &bounds, 256, 256)
        {
            composite(&mut img, &tile);
        }
    }

    // 矢量要素 + 标签。
    let vector = renderer.render(render_items);
    composite(&mut img, &vector);

    let mut buffer = std::io::Cursor::new(Vec::new());
    match format {
        TileFormat::Png => img
            .write_to(&mut buffer, image::ImageFormat::Png)
            .map_err(|e| TerraneError::RenderingError(e.to_string()))?,
        TileFormat::Jpeg => image::DynamicImage::ImageRgba8(img)
            .to_rgb8()
            .write_to(&mut buffer, image::ImageFormat::Jpeg)
            .map_err(|e| TerraneError::RenderingError(e.to_string()))?,
    }
    let tile_data = buffer.into_inner();

    // 3. Populate the tile cache (PNG only).
    if format == TileFormat::Png {
        if let Some(ref cache) = cache {
            cache.put(layer, gridset, z, col, row, &tile_data).await;
        }
    }

    let _ = state;
    Ok((tile_data, false))
}

/// Source-over composite `src` onto `dst` (transparent pixels skipped).
fn composite(dst: &mut image::RgbaImage, src: &image::RgbaImage) {
    for (y, row) in src.rows().enumerate() {
        for (x, px) in row.enumerate() {
            let fg = px.0;
            let a = fg[3] as f32 / 255.0;
            if a <= 0.0 {
                continue;
            }
            if a >= 1.0 {
                dst.put_pixel(x as u32, y as u32, image::Rgba(fg));
                continue;
            }
            let bg = dst.get_pixel(x as u32, y as u32).0;
            let b = bg[3] as f32 / 255.0;
            let out_a = a + b * (1.0 - a);
            let blend = |f: u8, g: u8| -> u8 {
                ((f as f32 * a + g as f32 * b * (1.0 - a)) / out_a).round() as u8
            };
            dst.put_pixel(
                x as u32,
                y as u32,
                image::Rgba([
                    blend(fg[0], bg[0]),
                    blend(fg[1], bg[1]),
                    blend(fg[2], bg[2]),
                    (out_a * 255.0).round() as u8,
                ]),
            );
        }
    }
}
