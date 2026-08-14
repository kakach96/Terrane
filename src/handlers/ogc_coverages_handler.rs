//! OGC API - Coverages HTTP Handler
//!
//! Routes under `/ogc/coverages`: landing page, `/conformance`,
//! `/collections`, `/collections/{id}` and the `coverage` operation at
//! `/collections/{id}/coverage` (GeoTIFF default, PNG / JPEG via `?f=`),
//! delegating to the raster readers behind the WCS 2.0 GetCoverage pipeline.

use crate::models::{Bounds, DataSourceType};
use crate::services::ogc_coverages::{self, CoverageCollection};
use crate::state::AppState;
use actix_web::{web, HttpResponse};
use serde::Deserialize;

/// Service base URL (OGC API is served at the root path, not under the API
/// context).
fn base_url(state: &AppState) -> String {
    format!(
        "http://{}:{}",
        state.config.server.host, state.config.server.port
    )
}

fn json_response(value: serde_json::Value) -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/json")
        .body(serde_json::to_string(&value).unwrap_or_default())
}

fn not_found(what: &str) -> HttpResponse {
    HttpResponse::NotFound().json(serde_json::json!({
        "code": "NotFound",
        "description": format!("Resource not found: {}", what),
    }))
}

fn is_raster_type(ds_type: &DataSourceType) -> bool {
    matches!(
        ds_type,
        DataSourceType::Geotiff | DataSourceType::WorldImage | DataSourceType::ArcGrid
    )
}

/// Materialize a raster data source to a local file (supports local / s3).
///
/// WorldImage needs its world-file sibling, so it goes through
/// `materialize_dir`; GeoTIFF / ArcGrid are single files via
/// `materialize_file`.
async fn materialize_raster(
    conn: &crate::models::DataSourceConnection,
    ds_type: &DataSourceType,
) -> Result<Option<crate::store::file_resolver::MaterializedFile>, crate::error::GeoServerError> {
    match ds_type {
        DataSourceType::WorldImage => crate::store::materialize_dir(conn).await,
        _ => crate::store::materialize_file(conn).await,
    }
}

/// Discover every raster data source and read its real metadata into a
/// coverage collection (GeoTIFF / ArcGrid / WorldImage).
async fn discover_coverages(state: &AppState) -> Vec<CoverageCollection> {
    let mut coverages = Vec::new();

    let Some(store) = &state.store else {
        return coverages;
    };
    let Ok(ds_list) = store.get_all_data_sources().await else {
        return coverages;
    };

    for ds in &ds_list {
        if !is_raster_type(&ds.data_source_type) {
            continue;
        }
        let Some(conn) = &ds.connection else {
            continue;
        };
        let Some(file_path) = conn.file_path.as_ref() else {
            continue;
        };
        // Local files must exist; s3 objects are validated on read.
        let is_local = crate::store::storage_type(conn) == "local";
        if is_local && !std::path::Path::new(file_path).exists() {
            continue;
        }

        let mut cc = CoverageCollection {
            id: ds.name.clone(),
            title: ds.name.clone(),
            description: None,
            bbox: Bounds::new(-180.0, -90.0, 180.0, 90.0),
            srs: "EPSG:4326".to_string(),
            width: 0,
            height: 0,
            band_count: 0,
            file_type: match ds.data_source_type {
                DataSourceType::Geotiff => "GeoTIFF",
                DataSourceType::WorldImage => "WorldImage",
                DataSourceType::ArcGrid => "ArcGrid",
                _ => "Raster",
            }
            .to_string(),
        };

        match materialize_raster(conn, &ds.data_source_type).await {
            Ok(Some(materialized)) => {
                let path = materialized.path;
                match ds.data_source_type {
                    DataSourceType::Geotiff => {
                        if let Ok(meta) = crate::utils::geotiff::read_geotiff_metadata(&path) {
                            if let Some(b) = meta.bounds {
                                cc.bbox = b;
                            }
                            cc.width = meta.width;
                            cc.height = meta.height;
                            cc.band_count = meta.band_count;
                            cc.srs = meta.crs.clone().unwrap_or_else(|| "EPSG:4326".to_string());
                        }
                    },
                    DataSourceType::ArcGrid => {
                        if let Ok((bounds, width, height)) =
                            crate::utils::arcgrid::read_arcgrid_meta(&path)
                        {
                            cc.bbox = bounds;
                            cc.width = width;
                            cc.height = height;
                            cc.band_count = 1;
                        }
                    },
                    DataSourceType::WorldImage => {
                        if let Ok(meta) = crate::utils::worldimage::read_worldimage_meta(&path) {
                            cc.bbox = meta.bounds;
                            cc.width = meta.width;
                            cc.height = meta.height;
                            cc.band_count = 4;
                            cc.srs = meta.crs.clone().unwrap_or_else(|| "EPSG:4326".to_string());
                        }
                    },
                    _ => {},
                }
            },
            Ok(None) | Err(_) => {
                // Unreadable raster: keep the defaults so the collection is
                // still listed (like WCS GetCapabilities does).
            },
        }

        coverages.push(cc);
    }

    coverages
}

/// Find a single coverage collection by id.
async fn find_coverage(state: &AppState, id: &str) -> Option<CoverageCollection> {
    discover_coverages(state)
        .await
        .into_iter()
        .find(|c| c.id == id)
}

/// Read the full raster of a data source into an RGBA image + bounds.
async fn read_raster_image(
    state: &AppState,
    id: &str,
) -> Option<(image::RgbaImage, Option<Bounds>)> {
    let Some(store) = &state.store else {
        return None;
    };
    let Ok(Some(ds)) = store.get_data_source(id).await else {
        return None;
    };
    if !is_raster_type(&ds.data_source_type) {
        return None;
    }
    let conn = ds.connection.as_ref()?;
    let materialized = materialize_raster(conn, &ds.data_source_type)
        .await
        .ok()??;
    let path = materialized.path;
    match ds.data_source_type {
        DataSourceType::Geotiff => crate::utils::geotiff::read_geotiff(&path)
            .ok()
            .map(|cov| (cov.rgba_image, cov.bounds)),
        DataSourceType::WorldImage => crate::utils::worldimage::read_worldimage(&path)
            .ok()
            .map(|w| (w.rgba_image, Some(w.bounds))),
        DataSourceType::ArcGrid => crate::utils::arcgrid::read_arcgrid(&path)
            .ok()
            .map(|a| (a.rgba_image, Some(a.bounds))),
        _ => None,
    }
}

/// Crop an RGBA image to a geographic bounding box, given the coverage bounds.
///
/// Pixel row 0 is the top (north) edge, so the y-axis is flipped vs. the
/// geographic (north-up) bounds. Used for WorldImage / ArcGrid; GeoTIFF goes
/// through `geotiff::crop_coverage`.
fn crop_raster_by_bounds(
    img: &image::RgbaImage,
    cov: &Bounds,
    target: &Bounds,
) -> Option<image::RgbaImage> {
    if cov.maxx <= cov.minx || cov.maxy <= cov.miny {
        return None;
    }
    // No intersection.
    if target.minx >= cov.maxx
        || target.maxx <= cov.minx
        || target.miny >= cov.maxy
        || target.maxy <= cov.miny
    {
        return None;
    }
    let (w, h) = (img.width() as f64, img.height() as f64);
    let x_ratio = w / (cov.maxx - cov.minx);
    let y_ratio = h / (cov.maxy - cov.miny);
    let px = ((target.minx.max(cov.minx) - cov.minx) * x_ratio).floor() as u32;
    let py = ((cov.maxy - target.maxy.min(cov.maxy)) * y_ratio).floor() as u32;
    let pw = ((target.maxx.min(cov.maxx) - target.minx.max(cov.minx)) * x_ratio).ceil() as u32;
    let ph = ((target.maxy.min(cov.maxy) - target.miny.max(cov.miny)) * y_ratio).ceil() as u32;
    if pw == 0 || ph == 0 {
        return None;
    }
    let pw = pw.min(img.width() - px);
    let ph = ph.min(img.height() - py);
    Some(image::imageops::crop_imm(img, px, py, pw, ph).to_image())
}

/// Query parameters of the `coverage` operation.
#[derive(Deserialize)]
pub struct CoverageQuery {
    pub bbox: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub f: Option<String>,
}

/// `GET /ogc/coverages` — landing page.
pub async fn handle_ogc_coverages_landing(state: web::Data<AppState>) -> HttpResponse {
    json_response(ogc_coverages::landing_page(&base_url(state.get_ref())))
}

/// `GET /ogc/coverages/conformance`
pub async fn handle_ogc_coverages_conformance() -> HttpResponse {
    json_response(ogc_coverages::conformance())
}

/// `GET /ogc/coverages/collections` — coverage collections (one per raster
/// data source).
pub async fn handle_ogc_coverages_collections(state: web::Data<AppState>) -> HttpResponse {
    let coverages = discover_coverages(state.get_ref()).await;
    json_response(ogc_coverages::collections(
        &base_url(state.get_ref()),
        &coverages,
    ))
}

/// `GET /ogc/coverages/collections/{collection}`
pub async fn handle_ogc_coverages_collection(
    path: web::Path<String>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let id = path.into_inner();
    match find_coverage(state.get_ref(), &id).await {
        Some(cc) => json_response(ogc_coverages::collection(&base_url(state.get_ref()), &cc)),
        None => not_found(&id),
    }
}

/// `GET /ogc/coverages/collections/{collection}/coverage`
///
/// Returns the coverage raster (GeoTIFF default, PNG / JPEG via `?f=`).
/// Optional `bbox` (minx,miny,maxx,maxy) crops the raster to the geographic
/// extent; optional `width` / `height` rescale the output.
pub async fn handle_ogc_coverages_coverage(
    path: web::Path<String>,
    query: web::Query<CoverageQuery>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let id = path.into_inner();
    if find_coverage(state.get_ref(), &id).await.is_none() {
        return not_found(&id);
    }

    let Some((mut img, cov_bounds)) = read_raster_image(state.get_ref(), &id).await else {
        return HttpResponse::InternalServerError().json(serde_json::json!({
            "code": "NoSuchCoverage",
            "description": format!("Coverage '{}' could not be read", id),
        }));
    };

    // Optional spatial crop.
    if let Some(bbox_str) = query.bbox.as_deref() {
        match ogc_coverages::parse_bbox(bbox_str) {
            Some(target) => {
                let cropped = if let Some(cov) = &cov_bounds {
                    crop_raster_by_bounds(&img, cov, &target)
                } else {
                    None
                };
                if let Some(c) = cropped {
                    img = c;
                } else {
                    return HttpResponse::BadRequest().json(serde_json::json!({
                        "code": "InvalidParameterValue",
                        "description": "bbox does not intersect the coverage extent",
                    }));
                }
            },
            None => {
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "code": "InvalidParameterValue",
                    "description": "bbox parameter must be minx,miny,maxx,maxy",
                }))
            },
        }
    }

    // Optional resize.
    if let (Some(w), Some(h)) = (query.width, query.height) {
        if w > 0 && h > 0 && (w != img.width() || h != img.height()) {
            img = image::imageops::resize(&img, w, h, image::imageops::FilterType::Lanczos3);
        }
    }

    // Encode.
    let format = match query.f.as_deref() {
        Some(f) if f.to_lowercase().contains("jpeg") || f.to_lowercase() == "jpg" => {
            ogc_coverages::COVERAGE_JPEG_MIME
        },
        Some(f) if f.to_lowercase().contains("png") => ogc_coverages::COVERAGE_PNG_MIME,
        _ => ogc_coverages::COVERAGE_TIFF_MIME,
    };
    let (content_type, image_format) = match format {
        ogc_coverages::COVERAGE_JPEG_MIME => ("image/jpeg", image::ImageFormat::Jpeg),
        ogc_coverages::COVERAGE_PNG_MIME => ("image/png", image::ImageFormat::Png),
        _ => ("image/tiff", image::ImageFormat::Tiff),
    };

    let mut buffer = Vec::new();
    // JPEG / TIFF do not carry an alpha channel.
    let encode_res =
        if image_format == image::ImageFormat::Jpeg || image_format == image::ImageFormat::Tiff {
            let rgb = image::DynamicImage::ImageRgba8(img).to_rgb8();
            rgb.write_to(&mut std::io::Cursor::new(&mut buffer), image_format)
        } else {
            img.write_to(&mut std::io::Cursor::new(&mut buffer), image_format)
        };
    if let Err(e) = encode_res {
        return HttpResponse::InternalServerError().json(serde_json::json!({
            "code": "EncodingError",
            "description": format!("Failed to encode coverage: {}", e),
        }));
    }

    HttpResponse::Ok()
        .content_type(content_type)
        .append_header(("Content-Description", format!("Coverage: {}", id)))
        .body(buffer)
}
