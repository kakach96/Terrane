//! OWS (OGC Web Services) unified dispatcher: `/ows?service=...`
//!
//! Mirrors GeoServer's `/ows` endpoint: it dispatches to the individual OGC
//! service handlers based on the `service` KVP parameter.
//!
//! - GET supports WMS, WFS, WCS, WPS, CSW.
//! - POST supports WFS (KVP form body), WPS (XML Execute) and CSW (XML body),
//!   i.e. the services that expose a POST handler.
//!
//! When the `service` parameter is missing or unsupported an OWS
//! `ExceptionReport` is returned, following the OWS Common 1.1 convention.

use crate::state::AppState;
use actix_web::{
    web::{self, Query},
    HttpRequest, HttpResponse, ResponseError,
};

/// Look up a case-insensitive query parameter.
fn query_param<'a>(params: &'a [(String, String)], key: &str) -> Option<&'a str> {
    params
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .map(|(_, v)| v.as_str())
}

/// Build an OWS `ExceptionReport` XML response (OWS Common 1.1).
fn ows_exception(exception_code: &str, locator: &str, text: &str) -> HttpResponse {
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<ows:ExceptionReport xmlns:ows="http://www.opengis.net/ows/1.1"
                     xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
                     xsi:schemaLocation="http://www.opengis.net/ows/1.1 http://schemas.opengis.net/ows/1.1.0/owsExceptionReport.xsd"
                     version="1.1.0">
  <ows:Exception exceptionCode="{code}" locator="{locator}">
    <ows:ExceptionText>{text}</ows:ExceptionText>
  </ows:Exception>
</ows:ExceptionReport>"#,
        code = exception_code,
        locator = locator,
        text = text
    );
    HttpResponse::Ok().content_type("application/xml").body(xml)
}

/// `GET /ows` — dispatch to the service handler named by the `service` param.
pub async fn handle_ows_request(
    req: HttpRequest,
    query: Query<Vec<(String, String)>>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let service = query_param(query.as_ref(), "service").map(|s| s.to_ascii_uppercase());
    match service.as_deref() {
        Some("WMS") => crate::handlers::handle_wms_request(req, query, state).await,
        Some("WFS") => match crate::handlers::handle_wfs_request(query, state).await {
            Ok(resp) => resp,
            Err(e) => e.error_response(),
        },
        Some("WCS") => match crate::handlers::handle_wcs_request(query, state).await {
            Ok(resp) => resp,
            Err(e) => e.error_response(),
        },
        Some("WPS") => crate::handlers::handle_wps_request(req, query, state).await,
        Some("CSW") => crate::handlers::handle_csw_request(req, query, state).await,
        Some(other) => ows_exception(
            "InvalidParameterValue",
            "service",
            &format!("Unsupported service: {}", other),
        ),
        None => ows_exception(
            "MissingParameterValue",
            "service",
            "No service parameter specified",
        ),
    }
}

/// `POST /ows` — dispatch based on the `service` query param, falling back to
/// sniffing the request body when it is absent.
pub async fn handle_ows_post_request(
    req: HttpRequest,
    body: String,
    state: web::Data<AppState>,
) -> HttpResponse {
    // Parse the query string so a `service` param on `/ows?service=WFS` is honored.
    let params: Vec<(String, String)> =
        Query::<Vec<(String, String)>>::from_query(req.query_string())
            .map(|q| q.into_inner())
            .unwrap_or_default();
    let service = query_param(&params, "service").map(|s| s.to_ascii_uppercase());

    // When no explicit `service` param, sniff the body to infer the target
    // service. `contains` (rather than `starts_with`) so an XML declaration or
    // leading whitespace does not hide the root element's namespace prefix.
    let service = match service {
        Some(s) => Some(s),
        None => {
            if body.contains("<wps:") {
                Some("WPS".to_string())
            } else if body.contains("<csw:") {
                Some("CSW".to_string())
            } else if body.contains("<wfs:") {
                Some("WFS".to_string())
            } else if body.contains('=') {
                // KVP form-encoded body → WFS
                Some("WFS".to_string())
            } else {
                None
            }
        },
    };

    match service.as_deref() {
        Some("WFS") => match crate::handlers::handle_wfs_post_request(req, body, state).await {
            Ok(resp) => resp,
            Err(e) => e.error_response(),
        },
        Some("WPS") => crate::handlers::handle_wps_post_request(req, body, state).await,
        Some("CSW") => crate::handlers::handle_csw_post_request(req, body, state).await,
        Some(other) => ows_exception(
            "InvalidParameterValue",
            "service",
            &format!("Unsupported service: {}", other),
        ),
        None => ows_exception(
            "MissingParameterValue",
            "service",
            "No service parameter specified",
        ),
    }
}
