//! ImagePyramid data source: a pyramid of raster granules served as one
//! coverage (GeoServer-style image pyramid).
//!
//! Directory layout: numeric subdirectories `0/`, `1/`, `2/`, … each holding
//! raster granules (GeoTIFF / WorldImage / ArcGrid / PNG / JPEG). Level `0` is
//! the highest resolution; each higher level halves the resolution. An
//! optional `properties` file may list `levels=N` (fallback: count numeric
//! subdirectories). WCS GetCoverage / WMS GetMap pick the level whose ground
//! resolution best matches the requested output resolution, then composite
//! the granules of that level intersecting the target bounds.

use std::path::{Path, PathBuf};

use crate::models::Bounds;

/// A single pyramid granule: level index + decoded RGBA image + bounds.
#[derive(Debug, Clone)]
pub struct PyramidGranule {
    pub level: u32,
    pub path: PathBuf,
    pub image: image::RgbaImage,
    pub bounds: Bounds,
}

/// A pyramid level: index + its granules.
#[derive(Debug, Clone)]
pub struct PyramidLevel {
    pub level: u32,
    pub granules: Vec<PyramidGranule>,
}

/// Read the `levels` count from an optional `properties` file
/// (`levels=N`), falling back to the number of numeric subdirectories.
pub fn level_count(dir: &Path) -> u32 {
    let props = dir.join("properties");
    if let Ok(content) = std::fs::read_to_string(&props) {
        for line in content.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("levels") {
                if let Some(v) = rest.split('=').nth(1) {
                    if let Ok(n) = v.trim().parse::<u32>() {
                        if n > 0 {
                            return n;
                        }
                    }
                }
            }
        }
    }
    // Fallback: count numeric subdirectories.
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .filter(|e| e.path().is_dir() && e.file_name().to_string_lossy().parse::<u32>().is_ok())
        .count() as u32
}

/// Scan a pyramid directory into levels (only granules that decode cleanly).
/// Levels are sorted ascending (0 = highest resolution first).
pub fn load_pyramid(dir: &Path) -> Vec<PyramidLevel> {
    let count = level_count(dir);
    let mut levels = Vec::new();
    for level in 0..count {
        let level_dir = dir.join(level.to_string());
        let mut granules = Vec::new();
        for path in crate::utils::mosaic::scan_raster_files(&level_dir) {
            if let Some((image, bounds)) = read_granule_file(&path) {
                granules.push(PyramidGranule {
                    level,
                    path,
                    image,
                    bounds,
                });
            }
        }
        if !granules.is_empty() {
            levels.push(PyramidLevel { level, granules });
        }
    }
    levels
}

/// Read a single granule file into an RGBA image + bounds.
fn read_granule_file(path: &Path) -> Option<(image::RgbaImage, Bounds)> {
    let ext = path.extension().and_then(|e| e.to_str())?.to_lowercase();
    match ext.as_str() {
        "tif" | "tiff" => {
            let cov = crate::utils::geotiff::read_geotiff(path).ok()?;
            Some((cov.rgba_image, cov.bounds?))
        },
        "asc" | "grd" => {
            let ag = crate::utils::arcgrid::read_arcgrid(path).ok()?;
            Some((ag.rgba_image, ag.bounds))
        },
        "png" | "jpg" | "jpeg" => {
            let wim = crate::utils::worldimage::read_worldimage(path).ok()?;
            Some((wim.rgba_image, wim.bounds))
        },
        _ => None,
    }
}

/// Overall bounds of a pyramid (union of all granules across all levels).
pub fn pyramid_bounds(levels: &[PyramidLevel]) -> Option<Bounds> {
    let mut b: Option<Bounds> = None;
    for lvl in levels {
        for g in &lvl.granules {
            match b {
                None => b = Some(g.bounds.clone()),
                Some(ref mut cur) => {
                    cur.minx = cur.minx.min(g.bounds.minx);
                    cur.miny = cur.miny.min(g.bounds.miny);
                    cur.maxx = cur.maxx.max(g.bounds.maxx);
                    cur.maxy = cur.maxy.max(g.bounds.maxy);
                },
            }
        }
    }
    b
}

/// Estimate the ground resolution (geo-units/pixel) of a level from its first
/// granule's bounds and pixel size.
fn level_resolution(level: &PyramidLevel) -> Option<f64> {
    let g = level.granules.first()?;
    let w = g.image.width().max(1) as f64;
    let h = g.image.height().max(1) as f64;
    let rx = (g.bounds.maxx - g.bounds.minx) / w;
    let ry = (g.bounds.maxy - g.bounds.miny) / h;
    Some(rx.max(ry).max(f64::MIN_POSITIVE))
}

/// Select the best level for a target ground resolution: the coarsest level
/// whose resolution is still at least as fine as `target_resolution` (avoids
/// loading overly fine detail for coarse requests). If no level is fine
/// enough, falls back to the finest available level.
pub fn select_level(levels: &[PyramidLevel], target_resolution: f64) -> Option<&PyramidLevel> {
    if levels.is_empty() {
        return None;
    }
    // Walk from the coarsest level (highest index) toward the finest: pick the
    // first level whose resolution meets the target (coarsest adequate).
    for lvl in levels.iter().rev() {
        if let Some(res) = level_resolution(lvl) {
            if res <= target_resolution * 1.0001 {
                return Some(lvl);
            }
        }
    }
    // No level is fine enough → use the finest.
    levels.first()
}

/// Composite the granules of a single level intersecting `target` into an
/// RGBA image of the given output size. Returns None when nothing intersects.
pub fn render_level(
    level: &PyramidLevel,
    target: &Bounds,
    width: u32,
    height: u32,
) -> Option<image::RgbaImage> {
    let mut out = image::RgbaImage::new(width, height);
    let mut any = false;
    for g in &level.granules {
        if let Some(tile) = crate::handlers::wms_handler::render_raster_to_map(
            &g.image, &g.bounds, target, width, height,
        ) {
            composite(&mut out, &tile);
            any = true;
        }
    }
    if any {
        Some(out)
    } else {
        None
    }
}

/// Composite `src` onto `dst` (source-over, transparent pixels skipped).
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a georeferenced 4x4 GeoTIFF granule at (origin_x, origin_y).
    fn make_geotiff(dir: &Path, name: &str, origin_x: f64, origin_y: f64) -> PathBuf {
        use tiff::encoder::*;
        use tiff::tags::Tag;
        let path = dir.join(name);
        let file = std::fs::File::create(&path).unwrap();
        let mut tiff = TiffEncoder::new(file).unwrap();
        let mut enc = tiff.new_image::<colortype::RGB8>(4, 4).unwrap();
        let pixel_scale: &[f64] = &[1.0, 1.0, 0.0];
        let tiepoint: &[f64] = &[0.0, 0.0, 0.0, origin_x, origin_y + 4.0, 0.0];
        enc.encoder()
            .write_tag(Tag::ModelPixelScaleTag, pixel_scale)
            .unwrap();
        enc.encoder()
            .write_tag(Tag::ModelTiepointTag, tiepoint)
            .unwrap();
        let data: Vec<u8> = (0..4 * 4 * 3).map(|i| (i % 251) as u8).collect();
        enc.write_data(&data).unwrap();
        path
    }

    #[test]
    fn test_load_pyramid_and_select_level() {
        let dir = std::env::temp_dir().join(format!("terrane-pyramid-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("0")).unwrap();
        std::fs::create_dir_all(dir.join("1")).unwrap();
        // Level 0: 4px covering 4 units → resolution 1.0.
        make_geotiff(&dir.join("0"), "a.tif", 0.0, 0.0);
        // Level 1: 4px covering 8 units → resolution 2.0 (coarser).
        make_geotiff(&dir.join("1"), "b.tif", 0.0, 0.0);
        // Override level 1's tie point so its bounds span 8 units.
        let path = dir.join("1").join("b.tif");
        {
            use tiff::encoder::*;
            use tiff::tags::Tag;
            let file = std::fs::File::create(&path).unwrap();
            let mut tiff = TiffEncoder::new(file).unwrap();
            let mut enc = tiff.new_image::<colortype::RGB8>(4, 4).unwrap();
            let pixel_scale: &[f64] = &[2.0, 2.0, 0.0];
            let tiepoint: &[f64] = &[0.0, 0.0, 0.0, 0.0, 8.0, 0.0];
            enc.encoder()
                .write_tag(Tag::ModelPixelScaleTag, pixel_scale)
                .unwrap();
            enc.encoder()
                .write_tag(Tag::ModelTiepointTag, tiepoint)
                .unwrap();
            let data: Vec<u8> = (0..4 * 4 * 3).map(|i| (i % 251) as u8).collect();
            enc.write_data(&data).unwrap();
        }
        // properties file with explicit level count.
        std::fs::write(dir.join("properties"), "levels=2\n").unwrap();

        let levels = load_pyramid(&dir);
        assert_eq!(levels.len(), 2, "two levels");
        assert_eq!(levels[0].level, 0);
        assert_eq!(levels[1].level, 1);

        let b = pyramid_bounds(&levels).expect("bounds");
        assert!((b.minx - 0.0).abs() < 1e-6);
        assert!((b.maxx - 8.0).abs() < 1e-6);

        // Target resolution coarser than level 0 → picks level 1.
        let picked = select_level(&levels, 2.0).expect("level");
        assert_eq!(picked.level, 1);
        // Finer target → picks level 0.
        let picked = select_level(&levels, 0.5).expect("level");
        assert_eq!(picked.level, 0);

        let img = render_level(&levels[0], &b, 40, 40).expect("rendered");
        assert_eq!(img.dimensions(), (40, 40));
        // World (2, 2) is inside level 0's [0,4] extent → pixel (10, 30).
        assert_ne!(
            img.get_pixel(10, 30).0[3],
            0,
            "level 0 pixel must be opaque"
        );

        std::fs::remove_dir_all(&dir).ok();
    }
}
