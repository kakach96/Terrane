use actix_web::{HttpResponse, web};
use crate::services::wcs::{self, WcsRequest, WcsCapabilities, CoverageDescription};
use crate::state::AppState;
use crate::error::GeoServerError;
use quick_xml::se::to_string;

pub async fn handle_wcs_request(
    query: web::Query<Vec<(String, String)>>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let params = query.as_ref();
    let wcs_request = wcs::parse_wcs_request(params)?;
    
    match wcs_request.request {
        wcs::WcsOperation::GetCapabilities => handle_get_capabilities(&state, &wcs_request).await,
        wcs::WcsOperation::DescribeCoverage => handle_describe_coverage(&state, &wcs_request).await,
        wcs::WcsOperation::GetCoverage => handle_get_coverage(&state, &wcs_request).await,
    }
}

async fn handle_get_capabilities(state: &AppState, _request: &WcsRequest) -> Result<HttpResponse, GeoServerError> {
    let base_url = format!("http://{}:{}", state.config.server.host, state.config.server.port);
    let capabilities = WcsCapabilities::new(&base_url);
    
    let xml = to_string(&capabilities)
        .map_err(|e| GeoServerError::ServiceError(format!("Failed to serialize capabilities: {}", e)))?;
    
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
{}"#,
        xml
    );
    
    Ok(HttpResponse::Ok()
        .content_type("application/xml")
        .body(xml))
}

async fn handle_describe_coverage(_state: &AppState, request: &WcsRequest) -> Result<HttpResponse, GeoServerError> {
    let coverage_ids = request.coverage_id.as_ref()
        .ok_or_else(|| GeoServerError::BadRequest("COVERAGEID parameter is required".to_string()))?;
    
    let mut descriptions = Vec::new();
    
    for coverage_id in coverage_ids {
        let description = CoverageDescription::new(coverage_id);
        descriptions.push(description);
    }
    
    let xml = to_string(&descriptions)
        .map_err(|e| GeoServerError::ServiceError(format!("Failed to serialize descriptions: {}", e)))?;
    
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<wcs:CoverageDescriptions xmlns:wcs="http://www.opengis.net/wcs/2.0"
                          xmlns:gml="http://www.opengis.net/gml/3.2">
{}
</wcs:CoverageDescriptions>"#,
        xml
    );
    
    Ok(HttpResponse::Ok()
        .content_type("application/xml")
        .body(xml))
}

async fn handle_get_coverage(_state: &AppState, request: &WcsRequest) -> Result<HttpResponse, GeoServerError> {
    let coverage_ids = request.coverage_id.as_ref()
        .ok_or_else(|| GeoServerError::BadRequest("COVERAGEID parameter is required".to_string()))?;
    
    let coverage_id = coverage_ids.first()
        .ok_or_else(|| GeoServerError::BadRequest("At least one COVERAGEID is required".to_string()))?;
    
    let output_format = request.output_format.as_deref().unwrap_or("image/tiff");
    
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
    
    let mut buffer = Vec::new();
    
    match output_format {
        "image/tiff" | "image/tif" => {
            img.write_to(&mut std::io::Cursor::new(&mut buffer), image::ImageFormat::Tiff)
                .map_err(|e| GeoServerError::RenderingError(e.to_string()))?;
        }
        "image/png" => {
            img.write_to(&mut std::io::Cursor::new(&mut buffer), image::ImageFormat::Png)
                .map_err(|e| GeoServerError::RenderingError(e.to_string()))?;
        }
        "image/jpeg" | "image/jpg" => {
            img.write_to(&mut std::io::Cursor::new(&mut buffer), image::ImageFormat::Jpeg)
                .map_err(|e| GeoServerError::RenderingError(e.to_string()))?;
        }
        _ => {
            img.write_to(&mut std::io::Cursor::new(&mut buffer), image::ImageFormat::Tiff)
                .map_err(|e| GeoServerError::RenderingError(e.to_string()))?;
        }
    }
    
    let content_type = match output_format {
        "image/tiff" | "image/tif" => "image/tiff",
        "image/png" => "image/png",
        "image/jpeg" | "image/jpg" => "image/jpeg",
        _ => "image/tiff",
    };
    
    Ok(HttpResponse::Ok()
        .content_type(content_type)
        .append_header(("Content-Description", format!("Coverage: {}", coverage_id)))
        .body(buffer))
}
