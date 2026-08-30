//! OGC API - Processes HTTP Handler
//!
//! Routes under `/ogc/processes`: landing page, `/conformance`, `/processes`,
//! `/processes/{processId}`, and the synchronous job surface —
//! `GET/POST /jobs`, `GET/DELETE /jobs/{jobId}`, `GET /jobs/{jobId}/results`.
//! Execution reuses the pure-Rust WPS process engine (`services/wps.rs`).

use crate::error::TerraneError;
use crate::services::ogc_processes;
use crate::services::wps::{self, ResolvedInput};
use crate::state::AppState;
use actix_web::{web, HttpRequest, HttpResponse};
use serde_json::{json, Value};
use std::collections::HashMap;

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

/// Extract a Terrane layer name from an OGC API href, e.g.
/// `http://host/ogc/features/collections/world/items` → `world`.
fn layer_from_href(href: &str) -> Option<String> {
    let path = href.split('?').next()?;
    let idx = path.rfind("/collections/")?;
    let rest = &path[idx + "/collections/".len()..];
    let layer = rest.split('/').next()?;
    if layer.is_empty() {
        None
    } else {
        Some(layer.to_string())
    }
}

/// Resolve an OGC API - Processes input value to a WPS `ResolvedInput`:
/// `layer:<name>` / an OGC API href to a collection loads the layer (through
/// `query_layer_features`, which falls back to the vector store), a GeoJSON
/// feature collection (string or object) is parsed inline; numbers / booleans /
/// strings stay literals.
async fn resolve_input(state: &AppState, value: &Value) -> Result<ResolvedInput, TerraneError> {
    match value {
        Value::String(s) => {
            if let Some(layer) = s.strip_prefix("layer:") {
                let features =
                    crate::handlers::features::query_layer_features(state, layer, None, None, None)
                        .await
                        .map_err(|_| {
                            TerraneError::BadRequest(format!("Layer '{}' not found", layer))
                        })?;
                return Ok(ResolvedInput::Features(features));
            }
            if s.trim_start().starts_with('{') {
                let features =
                    wps::parse_feature_collection_json(s).map_err(TerraneError::BadRequest)?;
                return Ok(ResolvedInput::Features(features));
            }
            Ok(ResolvedInput::Literal(s.clone()))
        },
        Value::Object(obj) => {
            if obj.get("type").and_then(|t| t.as_str()) == Some("FeatureCollection") {
                let s = serde_json::to_string(value).unwrap_or_default();
                let features =
                    wps::parse_feature_collection_json(&s).map_err(TerraneError::BadRequest)?;
                return Ok(ResolvedInput::Features(features));
            }
            if let Some(href) = obj.get("href").and_then(|h| h.as_str()) {
                if let Some(layer) = layer_from_href(href) {
                    if let Ok(features) = crate::handlers::features::query_layer_features(
                        state, &layer, None, None, None,
                    )
                    .await
                    {
                        return Ok(ResolvedInput::Features(features));
                    }
                }
                return Err(TerraneError::BadRequest(format!(
                    "Unresolvable input href: {}",
                    href
                )));
            }
            Ok(ResolvedInput::Literal(
                serde_json::to_string(value).unwrap_or_default(),
            ))
        },
        Value::Number(n) => Ok(ResolvedInput::Literal(n.to_string())),
        Value::Bool(b) => Ok(ResolvedInput::Literal(b.to_string())),
        _ => Ok(ResolvedInput::Literal(value.to_string())),
    }
}

/// Execute a job (synchronous first surface): resolve inputs, run the built-in
/// process, persist the job and return `201 Created` with the status document.
async fn handle_execute(state: &AppState, base_url: &str, body: &str) -> HttpResponse {
    let req = match ogc_processes::parse_job_request(body) {
        Ok(r) => r,
        Err(e) => {
            return HttpResponse::BadRequest().json(json!({
                "code": "InvalidParameterValue",
                "description": e,
            }))
        },
    };
    let process_id = match req.process_id {
        Some(id) => id,
        None => {
            return HttpResponse::BadRequest().json(json!({
                "code": "MissingParameterValue",
                "description": "processID is required",
            }))
        },
    };
    let spec = match wps::find_process(&process_id) {
        Some(s) => s,
        None => {
            return HttpResponse::NotFound().json(json!({
                "code": "NotFound",
                "description": format!("Unknown process: {}", process_id),
            }))
        },
    };

    let mut resolved: HashMap<String, ResolvedInput> = HashMap::new();
    if let Some(inputs) = &req.inputs {
        for (name, value) in inputs {
            match resolve_input(state, value).await {
                Ok(input) => {
                    resolved.insert(name.clone(), input);
                },
                Err(e) => {
                    return HttpResponse::BadRequest().json(json!({
                        "code": "InvalidParameterValue",
                        "description": format!("input '{}': {}", name, e),
                    }))
                },
            }
        }
    }

    match wps::run_process(&spec, &resolved) {
        Ok(result) => {
            let output = match result.value {
                wps::OutputValue::GeoJson(v) => v,
                wps::OutputValue::Literal(s) => json!(s),
            };
            let job = ogc_processes::make_successful_job(&process_id, output);
            let job_id = job.job_id.clone();
            if let Ok(mut jobs) = state.ogc_jobs.lock() {
                jobs.insert(job_id, job.clone());
            }
            let doc = ogc_processes::job_status(base_url, &job);
            HttpResponse::Created()
                .content_type("application/json")
                .body(serde_json::to_string(&doc).unwrap_or_default())
        },
        Err(e) => HttpResponse::BadRequest().json(json!({
            "code": "NoApplicableCode",
            "description": e,
        })),
    }
}

/// `GET /ogc/processes` — landing page.
pub async fn handle_ogc_processes_landing(state: web::Data<AppState>) -> HttpResponse {
    json_response(ogc_processes::landing_page(&base_url(state.get_ref())))
}

/// `GET /ogc/processes/conformance`
pub async fn handle_ogc_processes_conformance() -> HttpResponse {
    json_response(ogc_processes::conformance())
}

/// `GET /ogc/processes/processes` — process list.
pub async fn handle_ogc_processes_processes(state: web::Data<AppState>) -> HttpResponse {
    json_response(ogc_processes::process_list(&base_url(state.get_ref())))
}

/// `GET /ogc/processes/processes/{processId}`
pub async fn handle_ogc_processes_process(
    path: web::Path<String>,
    _state: web::Data<AppState>,
) -> HttpResponse {
    let id = path.into_inner();
    match ogc_processes::process_description(&id) {
        Some(v) => json_response(v),
        None => not_found(&id),
    }
}

/// `GET /ogc/processes/jobs` — list stored jobs (status documents).
pub async fn handle_ogc_processes_jobs(
    _req: HttpRequest,
    state: web::Data<AppState>,
) -> HttpResponse {
    let jobs = state
        .ogc_jobs
        .lock()
        .map(|j| j.values().cloned().collect::<Vec<_>>());
    let items = match jobs {
        Ok(jobs) => jobs
            .iter()
            .map(|j| ogc_processes::job_status(&base_url(state.get_ref()), j))
            .collect::<Vec<_>>(),
        Err(_) => Vec::new(),
    };
    json_response(json!({
        "jobs": items,
        "links": [{
            "href": format!("{}/ogc/processes/jobs", base_url(state.get_ref())),
            "rel": "self",
            "type": "application/json",
            "title": "Jobs",
        }],
    }))
}

/// `POST /ogc/processes/jobs` — execute a process (sync, returns 201).
pub async fn handle_ogc_processes_execute(
    body: String,
    state: web::Data<AppState>,
) -> HttpResponse {
    handle_execute(state.get_ref(), &base_url(state.get_ref()), &body).await
}

/// `GET /ogc/processes/jobs/{jobId}` — job status.
pub async fn handle_ogc_processes_job(
    path: web::Path<String>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let job_id = path.into_inner();
    let job = state
        .ogc_jobs
        .lock()
        .ok()
        .and_then(|jobs| jobs.get(&job_id).cloned());
    match job {
        Some(job) => json_response(ogc_processes::job_status(&base_url(state.get_ref()), &job)),
        None => not_found(&job_id),
    }
}

/// `GET /ogc/processes/jobs/{jobId}/results`
pub async fn handle_ogc_processes_job_results(
    path: web::Path<String>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let job_id = path.into_inner();
    let job = state
        .ogc_jobs
        .lock()
        .ok()
        .and_then(|jobs| jobs.get(&job_id).cloned());
    match job {
        Some(job) if job.status == ogc_processes::OgcJobStatus::Successful => {
            json_response(ogc_processes::job_results(&job))
        },
        Some(_) => HttpResponse::Conflict().json(json!({
            "code": "JobNotFinished",
            "description": "The job has not finished successfully yet",
        })),
        None => not_found(&job_id),
    }
}

/// `DELETE /ogc/processes/jobs/{jobId}` — cancel a pending job.
pub async fn handle_ogc_processes_job_cancel(
    path: web::Path<String>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let job_id = path.into_inner();
    let mut jobs = match state.ogc_jobs.lock() {
        Ok(j) => j,
        Err(_) => {
            return HttpResponse::InternalServerError().json(json!({
                "code": "NoApplicableCode",
                "description": "job store unavailable",
            }))
        },
    };
    let job = match jobs.get_mut(&job_id) {
        Some(j) => j,
        None => return not_found(&job_id),
    };
    if job.status == ogc_processes::OgcJobStatus::Successful {
        return HttpResponse::Conflict().json(json!({
            "code": "JobAlreadyFinished",
            "description": "The job already finished and cannot be dismissed",
        }));
    }
    job.status = ogc_processes::OgcJobStatus::Dismissed;
    job.message = Some("Job dismissed".to_string());
    json_response(ogc_processes::job_status(&base_url(state.get_ref()), job))
}
