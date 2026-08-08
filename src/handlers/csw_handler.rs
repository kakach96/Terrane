//! CSW (Catalog Service for the Web) 2.0.2 HTTP Handler
//!
//! Routes: `GET/POST /csw` (KVP), `POST /csw` (XML). Supports GetCapabilities,
//! DescribeRecord, GetRecords, GetRecordById and GetDomain, backed by the
//! Terrane layer catalog.

use crate::services::csw::{self, CswOperation};
use crate::state::AppState;
use actix_web::{web, HttpRequest, HttpResponse};

/// Service base URL (CSW is served at the root path, not under the API context).
fn base_url(state: &AppState) -> String {
    format!(
        "http://{}:{}",
        state.config.server.host, state.config.server.port
    )
}

fn xml_response(body: String) -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/xml")
        .body(body)
}

/// Run a parsed CSW operation and build the HTTP response.
async fn dispatch(state: web::Data<AppState>, operation: CswOperation) -> HttpResponse {
    let state = state.get_ref();
    match operation {
        CswOperation::GetCapabilities => {
            xml_response(csw::build_capabilities(&base_url(state)))
        },
        CswOperation::DescribeRecord {
            typenames,
            output_format: _,
        } => xml_response(csw::build_describe_record(&typenames)),
        CswOperation::GetRecords {
            typenames: _,
            query,
        } => {
            let layers = state.list_layers().await;
            xml_response(csw::build_get_records(&base_url(state), &layers, &query))
        },
        CswOperation::GetRecordById {
            ids,
            element_set,
            output_schema: _,
        } => {
            let layers = state.list_layers().await;
            xml_response(csw::build_get_record_by_id(
                &base_url(state),
                &layers,
                &ids,
                element_set,
            ))
        },
        CswOperation::GetDomain { parameter_name } => xml_response(csw::build_get_domain(
            parameter_name.as_deref().unwrap_or("GetRecordsResultType"),
        )),
    }
}

/// CSW GET entry point: `/csw?SERVICE=CSW&REQUEST=...`
pub async fn handle_csw_request(
    _req: HttpRequest,
    query: web::Query<Vec<(String, String)>>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let params: &[(String, String)] = query.as_ref();
    let operation = match csw::parse_csw_request(params) {
        Ok(op) => op,
        Err(e) => {
            return xml_response(csw::build_exception(
                "InvalidParameterValue",
                &e.to_string(),
            ))
        },
    };
    dispatch(state, operation).await
}

/// CSW POST entry point: `/csw` — XML request body or KVP form-encoded body.
pub async fn handle_csw_post_request(
    req: HttpRequest,
    body: String,
    state: web::Data<AppState>,
) -> HttpResponse {
    let content_type = req
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    // XML request body
    if content_type.contains("xml") || body.trim_start().starts_with('<') {
        let operation = match csw::parse_csw_post(&body) {
            Ok(op) => op,
            Err(e) => {
                return xml_response(csw::build_exception(
                    "InvalidParameterValue",
                    &e.to_string(),
                ))
            },
        };
        return dispatch(state, operation).await;
    }

    // Otherwise try KVP form-encoded body
    let params: Vec<(String, String)> = body
        .split('&')
        .filter_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            let k = parts.next()?.to_string();
            let v = parts.next().unwrap_or("").to_string();
            Some((k, v))
        })
        .collect();

    let operation = match csw::parse_csw_request(&params) {
        Ok(op) => op,
        Err(e) => {
            return xml_response(csw::build_exception(
                "InvalidParameterValue",
                &e.to_string(),
            ))
        },
    };
    dispatch(state, operation).await
}
