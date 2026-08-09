//! OGC API - Processes integration tests.
//!
//! Covers the OGC API - Processes surface (OGC 18-062): landing page,
//! /conformance, /processes, /processes/{processId} and the synchronous job
//! surface (POST /jobs, GET /jobs, GET /jobs/{jobId}, GET /jobs/{jobId}/results,
//! DELETE /jobs/{jobId}).

#[macro_use]
mod common;

use actix_web::test;

#[actix_rt::test]
async fn test_ogc_processes_landing() {
    let app = build_test_app!();

    let req = test::TestRequest::get().uri("/ogc/processes").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "landing 应返回 200, 实际: {}",
        resp.status()
    );
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["title"], "Terrane");
    let rels: Vec<&str> = body["links"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["rel"].as_str().unwrap())
        .collect();
    assert!(rels.contains(&"self"));
    assert!(rels.contains(&"conformance"));
    assert!(rels.contains(&"processes"));
}

#[actix_rt::test]
async fn test_ogc_processes_conformance() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/ogc/processes/conformance")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: serde_json::Value = test::read_body_json(resp).await;
    let conforms = body["conformsTo"].as_array().unwrap();
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

#[actix_rt::test]
async fn test_ogc_processes_list() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/ogc/processes/processes")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = test::read_body_json(resp).await;
    let processes = body["processes"].as_array().unwrap();
    let ids: Vec<&str> = processes
        .iter()
        .map(|p| p["id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&"vec:Centroid"));
    assert!(ids.contains(&"vec:Buffer"));
    assert!(ids.contains(&"gs:Bounds"));
}

#[actix_rt::test]
async fn test_ogc_processes_description() {
    let app = build_test_app!();

    let req = test::TestRequest::get()
        .uri("/ogc/processes/processes/vec:Buffer")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["id"], "vec:Buffer");
    assert_eq!(body["version"], "1.0.0");
    assert!(body["inputs"]["features"].is_object());
    assert!(body["outputs"]["result"].is_object());

    // 未知 process → 404
    let req = test::TestRequest::get()
        .uri("/ogc/processes/processes/vec:DoesNotExist")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::NOT_FOUND);
}

#[actix_rt::test]
async fn test_ogc_processes_execute_centroid() {
    let app = build_test_app!();

    // 执行 vec:Centroid, 输入为本地图层引用 layer:world (空发布)
    let req = test::TestRequest::post()
        .uri("/ogc/processes/jobs")
        .set_json(&serde_json::json!({
            "processID": "vec:Centroid",
            "inputs": { "features": "layer:world" }
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        actix_web::http::StatusCode::CREATED,
        "同步执行应返回 201, 实际: {}",
        resp.status()
    );

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["processID"], "vec:Centroid");
    assert_eq!(body["status"], "successful");
    let job_id = body["jobID"].as_str().unwrap().to_string();
    let rels: Vec<&str> = body["links"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["rel"].as_str().unwrap())
        .collect();
    assert!(rels.contains(&"self"));
    assert!(rels.contains(&"results"));

    // 获取任务状态
    let req = test::TestRequest::get()
        .uri(&format!("/ogc/processes/jobs/{}", job_id))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let status: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(status["jobID"], job_id);
    assert_eq!(status["status"], "successful");

    // 获取任务结果 → GeoJSON FeatureCollection
    let req = test::TestRequest::get()
        .uri(&format!("/ogc/processes/jobs/{}/results", job_id))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let results: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(results["result"]["type"], "FeatureCollection");
}

#[actix_rt::test]
async fn test_ogc_processes_execute_buffer() {
    let app = build_test_app!();

    let req = test::TestRequest::post()
        .uri("/ogc/processes/jobs")
        .set_json(&serde_json::json!({
            "processID": "vec:Buffer",
            "inputs": {
                "features": { "type": "FeatureCollection", "features": [
                    { "type": "Feature", "id": "a", "properties": {}, "geometry": { "type": "Point", "coordinates": [0.0, 0.0] } }
                ]},
                "distance": 10
            }
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::CREATED);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["status"], "successful");
    let job_id = body["jobID"].as_str().unwrap().to_string();

    let req = test::TestRequest::get()
        .uri(&format!("/ogc/processes/jobs/{}/results", job_id))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let results: serde_json::Value = test::read_body_json(resp).await;
    let features = results["result"]["features"].as_array().unwrap();
    assert_eq!(features.len(), 1);
    // buffer 几何应产出多边形 (面)
    assert_eq!(features[0]["geometry"]["type"], "Polygon");
}

#[actix_rt::test]
async fn test_ogc_processes_jobs_list() {
    let app = build_test_app!();

    // 先执行一个任务 (world 空发布)
    let req = test::TestRequest::post()
        .uri("/ogc/processes/jobs")
        .set_json(&serde_json::json!({
            "processID": "gs:Bounds",
            "inputs": { "features": "layer:world" }
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::CREATED);

    let req = test::TestRequest::get()
        .uri("/ogc/processes/jobs")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = test::read_body_json(resp).await;
    let jobs = body["jobs"].as_array().unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0]["processID"], "gs:Bounds");
    assert_eq!(jobs[0]["status"], "successful");
}

#[actix_rt::test]
async fn test_ogc_processes_execute_errors() {
    let app = build_test_app!();

    // 未知 process → 404
    let req = test::TestRequest::post()
        .uri("/ogc/processes/jobs")
        .set_json(&serde_json::json!({ "processID": "vec:DoesNotExist", "inputs": {} }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::NOT_FOUND);

    // 非法 body → 400
    let req = test::TestRequest::post()
        .uri("/ogc/processes/jobs")
        .set_payload("not json")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::BAD_REQUEST);

    // 未知任务 → 404
    let req = test::TestRequest::get()
        .uri("/ogc/processes/jobs/unknown-job")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::NOT_FOUND);

    // 未知任务 results → 404
    let req = test::TestRequest::get()
        .uri("/ogc/processes/jobs/unknown-job/results")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), actix_web::http::StatusCode::NOT_FOUND);
}

#[actix_rt::test]
async fn test_ogc_processes_cancel_successful_conflict() {
    let app = build_test_app!();

    // 同步执行的任务立即 success, 取消应返回 409 (不能取消已完成任务)
    let req = test::TestRequest::post()
        .uri("/ogc/processes/jobs")
        .set_json(&serde_json::json!({
            "processID": "vec:Centroid",
            "inputs": { "features": "layer:world" }
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    let job_id = body["jobID"].as_str().unwrap().to_string();

    let req = test::TestRequest::delete()
        .uri(&format!("/ogc/processes/jobs/{}", job_id))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        actix_web::http::StatusCode::CONFLICT,
        "已完成任务不可取消, 应返回 409"
    );
}
