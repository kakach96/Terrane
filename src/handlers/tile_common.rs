//! Shared tile-rendering pipeline used by the tile protocol surfaces
//! (WMTS / TMS / WMS-C).
//!
//! All three surfaces resolve a `(layer, gridset, z, col, row)` request to the
//! same PNG bytes via the in-memory metadata store and the local vector store,
//! and share the GeoWebCache-style tile cache when it is enabled.

use crate::error::GeoServerError;
use crate::state::AppState;
use crate::utils::tile_grid;

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
) -> Result<(Vec<u8>, bool), GeoServerError> {
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
        GeoServerError::BadRequest(format!(
            "Tile index z={} x={} y={} is out of range for gridset {}",
            z, col, row, gridset
        ))
    })?;

    use crate::handlers::features;
    use crate::utils::rendering::{MapRenderer, RenderFormat, RenderOptions};

    let tile_size = 256u32;
    let options = RenderOptions {
        width: tile_size,
        height: tile_size,
        transparent: true,
        bg_color: None,
        format: RenderFormat::PNG,
    };

    let renderer = MapRenderer::new(options, bounds);
    let layers_lock = state.layers.read().await;
    let styles_lock = state.styles.read().await;
    let meta_lock = state.styles_meta.read().await;
    let mut render_items = Vec::new();

    if let Some(layer_obj) = layers_lock.iter().find(|l| l.name == layer) {
        use crate::handlers::style_handler::{
            calculate_tile_scale_denom, get_style_rules, reproject_geometry_helper,
        };
        use crate::utils::sld_parser;

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

    let img = renderer.render(render_items);
    let mut buffer = std::io::Cursor::new(Vec::new());
    match format {
        TileFormat::Png => img
            .write_to(&mut buffer, image::ImageFormat::Png)
            .map_err(|e| GeoServerError::RenderingError(e.to_string()))?,
        TileFormat::Jpeg => image::DynamicImage::ImageRgba8(img)
            .to_rgb8()
            .write_to(&mut buffer, image::ImageFormat::Jpeg)
            .map_err(|e| GeoServerError::RenderingError(e.to_string()))?,
    }
    let tile_data = buffer.into_inner();

    // 3. Populate the tile cache (PNG only).
    if format == TileFormat::Png {
        if let Some(ref cache) = cache {
            cache.put(layer, &gridset, z, col, row, &tile_data).await;
        }
    }

    Ok((tile_data, false))
}
