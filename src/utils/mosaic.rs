//! ImageMosaic data source: a directory of raster granules served as one
//! coverage (GeoServer-style mosaic).
//!
//! Each supported raster file (GeoTIFF / WorldImage / ArcGrid / PNG / JPEG)
//! inside the mosaic directory is a *granule*. Granules may overlap or tile
//! adjacent areas; the mosaic's overall bounds is the union of all granule
//! bounds. WCS GetCoverage and WMS GetMap composite the granules that
//! intersect the requested subset (later granules draw on top).

use image::RgbaImage;
use std::path::{Path, PathBuf};

use crate::models::Bounds;

/// A single mosaic granule: file path + decoded RGBA image + bounds.
#[derive(Debug, Clone)]
pub struct MosaicGranule {
    pub path: PathBuf,
    pub image: RgbaImage,
    pub bounds: Bounds,
}

/// Scan a directory and return every supported raster file found (recursively
/// for one level; subdirectories are not descended into).
pub fn scan_raster_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && is_supported_raster(&path) {
            files.push(path);
        }
    }
    files.sort();
    files
}

/// Whether a file is a supported mosaic granule format.
pub fn is_supported_raster(path: &Path) -> bool {
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => matches!(
            ext.to_lowercase().as_str(),
            "tif" | "tiff" | "png" | "jpg" | "jpeg" | "asc" | "grd"
        ),
        None => false,
    }
}

/// Read a single granule file into an RGBA image + bounds.
///
/// Supported: GeoTIFF (georeferenced tags), WorldImage (world file),
/// ArcGrid (ESRI ASCII grid), plain PNG/JPEG (world file required).
fn read_granule(path: &Path) -> Option<(RgbaImage, Bounds)> {
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

/// Load the mosaic directory into granules (only those that decode cleanly).
pub fn load_mosaic(dir: &Path) -> Vec<MosaicGranule> {
    scan_raster_files(dir)
        .into_iter()
        .filter_map(|path| {
            read_granule(&path).map(|(image, bounds)| MosaicGranule {
                path,
                image,
                bounds,
            })
        })
        .collect()
}

/// Overall bounds of a mosaic (union of all granule bounds), if any.
pub fn mosaic_bounds(granules: &[MosaicGranule]) -> Option<Bounds> {
    let mut b: Option<Bounds> = None;
    for g in granules {
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
    b
}

/// Granules intersecting the target bounds.
pub fn granules_in_bounds<'a>(
    granules: &'a [MosaicGranule],
    target: &Bounds,
) -> Vec<&'a MosaicGranule> {
    granules
        .iter()
        .filter(|g| bounds_intersect(&g.bounds, target))
        .collect()
}

fn bounds_intersect(a: &Bounds, b: &Bounds) -> bool {
    a.minx < b.maxx && a.maxx > b.minx && a.miny < b.maxy && a.maxy > b.miny
}

/// Composite all granules intersecting `target` into a single RGBA image of
/// the given output size, mapping the target bounds to the pixel grid.
/// Granules later in the list draw on top. Returns None when nothing
/// intersects.
pub fn render_mosaic(
    granules: &[MosaicGranule],
    target: &Bounds,
    width: u32,
    height: u32,
) -> Option<RgbaImage> {
    let mut out = RgbaImage::new(width, height);
    let mut any = false;
    for g in granules_in_bounds(granules, target) {
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

/// Source-over composite of `src` onto `dst` (transparent pixels skipped).
fn composite(dst: &mut RgbaImage, src: &RgbaImage) {
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

    /// Create a georeferenced 4x4 GeoTIFF at the given origin (min corner),
    /// covering a 4x4 geographic unit.
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
    fn test_scan_and_load_mosaic() {
        let dir = std::env::temp_dir().join(format!("terrane-mosaic-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        make_geotiff(&dir, "a.tif", 0.0, 0.0);
        make_geotiff(&dir, "b.tif", 4.0, 0.0);
        // A non-raster file must be ignored.
        std::fs::write(dir.join("notes.txt"), "x").unwrap();

        let granules = load_mosaic(&dir);
        assert_eq!(granules.len(), 2, "only raster files are granules");

        let b = mosaic_bounds(&granules).expect("bounds");
        assert!((b.minx - 0.0).abs() < 1e-6);
        assert!((b.maxx - 8.0).abs() < 1e-6);
        assert!((b.maxy - 4.0).abs() < 1e-6);

        // Only the left granule intersects the left half.
        let left = granules_in_bounds(&granules, &Bounds::new(0.0, 0.0, 3.9, 3.9));
        assert_eq!(left.len(), 1);

        // Render the full mosaic: must produce an 8-unit-wide image.
        let img = render_mosaic(&granules, &b, 80, 40).expect("rendered");
        assert_eq!(img.dimensions(), (80, 40));
        // A pixel in the right half must be non-transparent (granule b).
        assert_ne!(img.get_pixel(60, 20).0[3], 0);

        std::fs::remove_dir_all(&dir).ok();
    }
}
