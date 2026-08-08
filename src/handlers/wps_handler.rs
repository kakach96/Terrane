//! WPS (Web Processing Service) 1.0.0 HTTP Handler
//!
//! Routes: `GET/POST /wps` (KVP), `POST /wps` (XML Execute). Supports
//! GetCapabilities, DescribeProcess and Execute for the built-in processes.

use crate::error::GeoServerError;
use crate::services::wps::{self, ResolvedInput, WpsOperation};
use crate::state::AppState;
use actix_web::{web, HttpRequest, HttpResponse};
use std::collections::HashMap;

/// Service base URL (WPS is served at the root path, not under the API context).
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

/// Resolve an input value to features: `layer:<name>` loads the layer, a JSON
/// feature collection is parsed inline; otherwise it stays a literal.
async fn resolve_input(state: &AppState, value: &str) -> Result<ResolvedInput, GeoServerError> {
    if let Some(layer) = value.strip_prefix("layer:") {
        let features = state
            .get_layer_features(layer)
            .await
            .ok_or_else(|| GeoServerError::BadRequest(format!("Layer '{}' not found", layer)))?;
        return Ok(ResolvedInput::Features(features));
    }
    if value.trim_start().starts_with('{') {
        let features =
            wps::parse_feature_collection_json(value).map_err(GeoServerError::BadRequest)?;
        return Ok(ResolvedInput::Features(features));
    }
    Ok(ResolvedInput::Literal(value.to_string()))
}

/// Run an Execute request (already parsed) and build the HTTP response.
async fn handle_execute(
    state: &AppState,
    identifier: &str,
    input_pairs: Vec<(String, String)>,
    response_raw: bool,
) -> HttpResponse {
    let spec = match wps::find_process(identifier) {
        Some(s) => s,
        None => {
            return xml_response(wps::build_exception(
                "InvalidParameterValue",
                &format!("Unknown process: {}", identifier),
            ))
        },
    };

    let mut resolved: HashMap<String, ResolvedInput> = HashMap::new();
    for (name, value) in input_pairs {
        match resolve_input(state, &value).await {
            Ok(input) => {
                resolved.insert(name, input);
            },
            Err(e) => {
                return xml_response(wps::build_exception("NoApplicableCode", &e.to_string()));
            },
        }
    }

    match wps::run_process(&spec, &resolved) {
        Ok(result) => {
            if response_raw {
                match &result.value {
                    wps::OutputValue::GeoJson(v) => HttpResponse::Ok()
                        .content_type("application/json")
                        .body(serde_json::to_string(v).unwrap_or_default()),
                    wps::OutputValue::Literal(s) => HttpResponse::Ok()
                        .content_type("text/plain")
                        .body(s.clone()),
                }
            } else {
                xml_response(wps::build_execute_response(
                    &base_url(state),
                    &spec,
                    &result,
                ))
            }
        },
        Err(e) => xml_response(wps::build_exception("NoApplicableCode", &e)),
    }
}

/// WPS GET entry point: `/wps?SERVICE=WPS&REQUEST=...`
pub async fn handle_wps_request(
    _req: HttpRequest,
    query: web::Query<Vec<(String, String)>>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let params: &[(String, String)] = query.as_ref();
    let operation = match wps::parse_wps_request(params) {
        Ok(op) => op,
        Err(e) => {
            return xml_response(wps::build_exception(
                "InvalidParameterValue",
                &e.to_string(),
            ))
        },
    };

    match operation {
        WpsOperation::GetCapabilities => {
            xml_response(wps::build_capabilities(&base_url(state.get_ref())))
        },
        WpsOperation::DescribeProcess { identifiers } => {
            match wps::build_process_descriptions(&identifiers) {
                Ok(xml) => xml_response(xml),
                Err(e) => xml_response(wps::build_exception(
                    "InvalidParameterValue",
                    &e.to_string(),
                )),
            }
        },
        WpsOperation::Execute {
            identifier,
            data_inputs,
            response_raw,
            output_id: _output_id,
        } => handle_execute(state.get_ref(), &identifier, data_inputs, response_raw).await,
    }
}

/// WPS POST entry point: `/wps` — XML Execute body or KVP form-encoded body.
pub async fn handle_wps_post_request(
    req: HttpRequest,
    body: String,
    state: web::Data<AppState>,
) -> HttpResponse {
    let content_type = req
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    // XML Execute request
    if content_type.contains("xml") || body.contains("<wps:Execute") || body.contains("Execute") {
        let parsed = match wps::parse_execute_xml(&body) {
            Ok(p) => p,
            Err(e) => {
                return xml_response(wps::build_exception("InvalidParameterValue", &e));
            },
        };

        let mut input_pairs: Vec<(String, String)> = Vec::new();
        for input in parsed.inputs {
            let value = if let Some(lit) = input.literal {
                lit
            } else if let Some(complex) = input.complex_data {
                complex
            } else if let Some(reference) = input.reference {
                // Terrane extension: `layer:<name>` references resolve locally.
                reference
            } else {
                continue;
            };
            input_pairs.push((input.identifier, value));
        }

        return handle_execute(
            state.get_ref(),
            &parsed.identifier,
            input_pairs,
            parsed.response_raw,
        )
        .await;
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

    let operation = match wps::parse_wps_request(&params) {
        Ok(op) => op,
        Err(e) => {
            return xml_response(wps::build_exception(
                "InvalidParameterValue",
                &e.to_string(),
            ))
        },
    };

    match operation {
        WpsOperation::GetCapabilities => {
            xml_response(wps::build_capabilities(&base_url(state.get_ref())))
        },
        WpsOperation::DescribeProcess { identifiers } => {
            match wps::build_process_descriptions(&identifiers) {
                Ok(xml) => xml_response(xml),
                Err(e) => xml_response(wps::build_exception(
                    "InvalidParameterValue",
                    &e.to_string(),
                )),
            }
        },
        WpsOperation::Execute {
            identifier,
            data_inputs,
            response_raw,
            output_id: _output_id,
        } => handle_execute(state.get_ref(), &identifier, data_inputs, response_raw).await,
    }
}
