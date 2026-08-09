//! # OGC API - Processes implementation
//!
//! First OGC API - Processes surface for Terrane (OGC 18-062), served at
//! `/ogc/processes` (JSON). The reference GeoServer at :18080 does not ship
//! the OGC API extension, so this follows the OGC API - Processes schema
//! directly.
//!
//! Resources: landing page, `/conformance`, `/processes`,
//! `/processes/{processId}`, and a **synchronous** job surface —
//! `POST /processes/jobs` executes a built-in process immediately and returns
//! `201` with a status document, then `GET /jobs/{jobId}` / `GET /jobs/{jobId}/
//! results` / `DELETE /jobs/{jobId}` complete the job lifecycle. The built-in
//! processes are the same pure-Rust ones as the WPS 1.0.0 surface
//! (`vec:Centroid` / `vec:Buffer` / `gs:Bounds`), so both surfaces speak the
//! same processing engine.

use crate::services::wps::{self, WpsDataInputKind, WpsOutputKind};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;

/// OGC API - Processes media type.
pub const PROCESSES_MIME: &str = "application/json";
/// The process version advertised in descriptions.
pub const PROCESS_VERSION: &str = "1.0.0";
fn link(href: &str, rel: &str, type_: &str, title: &str) -> Value {
    json!({
        "href": href,
        "rel": rel,
        "type": type_,
        "title": title,
    })
}

/// Build the OGC API - Processes landing page (`GET /ogc/processes`).
pub fn landing_page(base_url: &str) -> Value {
    json!({
        "title": "Terrane",
        "description": "Cloud-native spatial data server powered by Rust — OGC API Processes",
        "links": [
            link(&format!("{}/ogc/processes", base_url), "self", "application/json", "This document"),
            link(&format!("{}/ogc/processes/conformance", base_url), "conformance", "application/json", "OGC API conformance classes"),
            link(&format!("{}/ogc/processes/processes", base_url), "processes", "application/json", "Processes"),
        ],
    })
}

/// Build the OGC API - Processes conformance declaration
/// (`GET /ogc/processes/conformance`).
pub fn conformance() -> Value {
    json!({
        "conformsTo": [
            "http://www.opengis.net/spec/ogcapi-processes-1/1.0/conf/core",
            "http://www.opengis.net/spec/ogcapi-processes-1/1.0/conf/ogc-process-description",
            "http://www.opengis.net/spec/ogcapi-processes-1/1.0/conf/json",
        ]
    })
}

fn input_schema(kind: WpsDataInputKind) -> Value {
    match kind {
        WpsDataInputKind::ComplexData => {
            json!({"type": "object", "contentMediaType": "application/geo+json"})
        },
        WpsDataInputKind::LiteralDouble => json!({"type": "number"}),
        WpsDataInputKind::LiteralString => json!({"type": "string"}),
    }
}

fn output_schema(kind: WpsOutputKind) -> Value {
    match kind {
        WpsOutputKind::ComplexData => {
            json!({"type": "object", "contentMediaType": "application/geo+json"})
        },
        WpsOutputKind::Literal => json!({"type": ["number", "string"]}),
    }
}

/// A JSON process summary (used in the `/processes` list).
fn process_summary(spec: &wps::ProcessSpec) -> Value {
    json!({
        "id": spec.identifier,
        "title": spec.title,
        "description": spec.abstract_text,
        "version": PROCESS_VERSION,
        "jobControlOptions": ["sync-execute"],
        "outputTransmission": ["value"],
    })
}

/// Build the `/processes` list document.
pub fn process_list(base_url: &str) -> Value {
    let processes: Vec<Value> = wps::builtin_processes()
        .iter()
        .map(process_summary)
        .collect();
    json!({
        "processes": processes,
        "links": [
            link(&format!("{}/ogc/processes/processes", base_url), "self", "application/json", "Processes"),
        ],
    })
}

/// Build a full process description (`GET /ogc/processes/{processId}`).
/// Returns `None` for unknown process ids.
pub fn process_description(process_id: &str) -> Option<Value> {
    let spec = wps::find_process(process_id)?;
    let mut inputs = serde_json::Map::new();
    for i in &spec.inputs {
        inputs.insert(
            i.identifier.to_string(),
            json!({
                "title": i.title,
                "minOccurs": i.min_occurs,
                "maxOccurs": i.max_occurs,
                "schema": input_schema(i.kind),
            }),
        );
    }
    let mut outputs = serde_json::Map::new();
    for o in &spec.outputs {
        outputs.insert(
            o.identifier.to_string(),
            json!({
                "title": o.title,
                "schema": output_schema(o.kind),
            }),
        );
    }
    Some(json!({
        "id": spec.identifier,
        "title": spec.title,
        "description": spec.abstract_text,
        "version": PROCESS_VERSION,
        "jobControlOptions": ["sync-execute"],
        "outputTransmission": ["value"],
        "inputs": inputs,
        "outputs": outputs,
    }))
}

// ---------------------------------------------------------------------------
// Job model
// ---------------------------------------------------------------------------

/// OGC API - Processes job status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OgcJobStatus {
    Accepted,
    Running,
    Successful,
    Failed,
    Dismissed,
}

/// An in-memory processing job (first surface: synchronous execution).
#[derive(Debug, Clone)]
pub struct OgcJob {
    pub job_id: String,
    pub process_id: String,
    pub status: OgcJobStatus,
    pub created: String,
    pub message: Option<String>,
    pub result: Option<Value>,
}

/// Generate a unique job id (`uuid` v4).
pub fn new_job_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

/// Create a new job in the `Successful` state.
pub fn make_successful_job(process_id: &str, result: Value) -> OgcJob {
    OgcJob {
        job_id: new_job_id(),
        process_id: process_id.to_string(),
        status: OgcJobStatus::Successful,
        created: now_iso(),
        message: Some("Process completed".to_string()),
        result: Some(result),
    }
}

/// Build the job status document (`GET /jobs/{jobId}`).
pub fn job_status(base_url: &str, job: &OgcJob) -> Value {
    let status_str = match job.status {
        OgcJobStatus::Accepted => "accepted",
        OgcJobStatus::Running => "running",
        OgcJobStatus::Successful => "successful",
        OgcJobStatus::Failed => "failed",
        OgcJobStatus::Dismissed => "dismissed",
    };
    let mut doc = json!({
        "jobID": job.job_id,
        "processID": job.process_id,
        "type": "process",
        "status": status_str,
        "created": job.created,
        "links": [
            link(&format!("{}/ogc/processes/jobs/{}", base_url, job.job_id), "self", "application/json", "Status"),
        ],
    });
    if let Some(msg) = &job.message {
        doc["message"] = json!(msg);
    }
    if job.status == OgcJobStatus::Successful {
        doc["links"].as_array_mut().unwrap().push(link(
            &format!("{}/ogc/processes/jobs/{}/results", base_url, job.job_id),
            "results",
            "application/json",
            "Results",
        ));
    }
    doc
}

/// Build the job results document (`GET /jobs/{jobId}/results`).
pub fn job_results(job: &OgcJob) -> Value {
    match &job.result {
        Some(v) => json!({ "result": v }),
        None => json!({}),
    }
}

// ---------------------------------------------------------------------------
// Job request parsing
// ---------------------------------------------------------------------------

/// A parsed OGC API - Processes job request body.
#[derive(Debug, Clone, Deserialize)]
pub struct JobRequest {
    #[serde(rename = "processID")]
    pub process_id: Option<String>,
    pub inputs: Option<HashMap<String, Value>>,
    #[allow(dead_code)]
    pub outputs: Option<Value>,
    #[allow(dead_code)]
    pub mode: Option<String>,
}

/// Parse a job request JSON body.
pub fn parse_job_request(body: &str) -> Result<JobRequest, String> {
    serde_json::from_str(body).map_err(|e| format!("invalid job request JSON: {}", e))
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_landing_structure() {
        let v = landing_page("http://localhost:8080");
        assert_eq!(v["title"], "Terrane");
        let rels: Vec<&str> = v["links"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l["rel"].as_str().unwrap())
            .collect();
        assert!(rels.contains(&"self"));
        assert!(rels.contains(&"conformance"));
        assert!(rels.contains(&"processes"));
    }

    #[test]
    fn test_conformance_classes() {
        let v = conformance();
        let conforms = v["conformsTo"].as_array().unwrap();
        assert!(conforms
            .iter()
            .any(|x| x.as_str().unwrap().contains("core")));
        assert!(conforms
            .iter()
            .any(|x| x.as_str().unwrap().contains("ogc-process-description")));
        assert!(conforms
            .iter()
            .any(|x| x.as_str().unwrap().contains("json")));
    }

    #[test]
    fn test_process_list_has_builtins() {
        let v = process_list("http://localhost:8080");
        let processes = v["processes"].as_array().unwrap();
        let ids: Vec<&str> = processes
            .iter()
            .map(|p| p["id"].as_str().unwrap())
            .collect();
        assert!(ids.contains(&"vec:Centroid"));
        assert!(ids.contains(&"vec:Buffer"));
        assert!(ids.contains(&"gs:Bounds"));
        let first = &processes[0];
        assert_eq!(first["jobControlOptions"][0], "sync-execute");
        assert_eq!(first["outputTransmission"][0], "value");
    }

    #[test]
    fn test_process_description_buffer() {
        let v = process_description("vec:Buffer").unwrap();
        assert_eq!(v["id"], "vec:Buffer");
        assert_eq!(v["version"], "1.0.0");
        assert_eq!(v["inputs"]["features"]["minOccurs"], 1);
        assert_eq!(v["inputs"]["distance"]["schema"]["type"], "number");
        assert_eq!(
            v["inputs"]["features"]["schema"]["contentMediaType"],
            "application/geo+json"
        );
        assert!(v["outputs"]["result"].is_object());
    }

    #[test]
    fn test_process_description_unknown_is_none() {
        assert!(process_description("vec:DoesNotExist").is_none());
    }

    #[test]
    fn test_job_status_document() {
        let job = make_successful_job(
            "vec:Centroid",
            json!({"type": "FeatureCollection", "features": []}),
        );
        let v = job_status("http://localhost:8080", &job);
        assert_eq!(v["jobID"], job.job_id);
        assert_eq!(v["processID"], "vec:Centroid");
        assert_eq!(v["status"], "successful");
        let rels: Vec<&str> = v["links"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l["rel"].as_str().unwrap())
            .collect();
        assert!(rels.contains(&"self"));
        assert!(rels.contains(&"results"));
    }

    #[test]
    fn test_job_results_document() {
        let job = make_successful_job(
            "gs:Bounds",
            json!({"type": "FeatureCollection", "features": []}),
        );
        let v = job_results(&job);
        assert!(v["result"]["type"] == "FeatureCollection");
    }

    #[test]
    fn test_parse_job_request() {
        let body = r#"{
            "processID": "vec:Buffer",
            "inputs": {
                "features": "layer:world",
                "distance": 100
            }
        }"#;
        let req = parse_job_request(body).unwrap();
        assert_eq!(req.process_id.as_deref(), Some("vec:Buffer"));
        let inputs = req.inputs.unwrap();
        assert_eq!(inputs["features"], "layer:world");
        assert_eq!(inputs["distance"], 100.0);
    }

    #[test]
    fn test_parse_job_request_invalid() {
        assert!(parse_job_request("not json").is_err());
    }
}
