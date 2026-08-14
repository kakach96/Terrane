//! Resilience middleware integration tests.
//!
//! Covers the cloud-native hardening middleware (see
//! `docs/IMPLEMENTATION_PLAN.md` §6.1): the sliding-window rate limiter
//! (HTTP 429) and the request timeout (HTTP 504). The middleware is opt-in
//! via `[server]` config in `main.rs`.
//!
//! The rate-limit tests exercise the middleware directly on a small app.
//! The timeout tests run a real `HttpServer` on an ephemeral port and issue
//! real HTTP requests: the timeout middleware cancels the inner future and
//! answers with an error response that actix renders as HTTP 504, which
//! `test::call_service` cannot observe (it panics on service errors).

use actix_web::{test, web, App, HttpResponse};
use std::time::Duration;

/// Spawn a real HTTP server with the given middleware + routes, wait until it
/// accepts connections, and return the base URL.
async fn spawn_timeout_server(
    timeout: Duration,
    path: &'static str,
    body: &'static str,
    sleep_ms: u64,
) -> String {
    let http_server = actix_web::HttpServer::new(move || {
        App::new()
            .wrap(terrane::middleware::RequestTimeout::new(timeout))
            .route(
                path,
                web::get().to(move || {
                    let body = body.to_string();
                    async move {
                        if sleep_ms > 0 {
                            tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
                        }
                        HttpResponse::Ok().body(body)
                    }
                }),
            )
    })
    .bind("127.0.0.1:0")
    .expect("bind ephemeral port");
    let addr = http_server.addrs()[0];
    let server = http_server.run();
    tokio::spawn(server);

    // Wait until the server accepts connections.
    let client = reqwest::Client::new();
    let probe = format!("http://{}/__probe", addr);
    for _ in 0..100 {
        if client.get(&probe).send().await.is_ok() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    format!("http://{}", addr)
}

#[actix_rt::test]
async fn test_rate_limit_429() {
    let app = test::init_service(
        App::new()
            .wrap(terrane::middleware::RateLimit::new(2, 60))
            .route(
                "/ping",
                web::get().to(|| async { HttpResponse::Ok().body("pong") }),
            ),
    )
    .await;

    // 前 2 个请求放行
    for _ in 0..2 {
        let req = test::TestRequest::get().uri("/ping").to_request();
        let resp = test::call_service(&app, req).await;
        assert!(
            resp.status().is_success(),
            "窗口内前 2 个请求应成功, 实际: {}",
            resp.status()
        );
    }

    // 第 3 个请求 -> 429
    let req = test::TestRequest::get().uri("/ping").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        actix_web::http::StatusCode::TOO_MANY_REQUESTS,
        "超限请求应返回 429"
    );
}

#[actix_rt::test]
async fn test_rate_limit_per_client_ip() {
    let app = test::init_service(
        App::new()
            .wrap(terrane::middleware::RateLimit::new(1, 60))
            .route(
                "/ping",
                web::get().to(|| async { HttpResponse::Ok().body("pong") }),
            ),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/ping")
        .insert_header(("X-Forwarded-For", "10.0.0.1"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    // 同一客户端再次请求 -> 429
    let req = test::TestRequest::get()
        .uri("/ping")
        .insert_header(("X-Forwarded-For", "10.0.0.1"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        actix_web::http::StatusCode::TOO_MANY_REQUESTS
    );

    // 不同客户端 -> 放行
    let req = test::TestRequest::get()
        .uri("/ping")
        .insert_header(("X-Forwarded-For", "10.0.0.2"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
}

#[actix_rt::test]
async fn test_request_timeout_504() {
    let base = spawn_timeout_server(Duration::from_millis(100), "/slow", "slow", 500).await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/slow", base))
        .send()
        .await
        .expect("request should complete");
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::GATEWAY_TIMEOUT,
        "超时请求应返回 504"
    );
}

#[actix_rt::test]
async fn test_request_timeout_fast_requests_pass() {
    let base = spawn_timeout_server(Duration::from_secs(5), "/fast", "fast", 0).await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/fast", base))
        .send()
        .await
        .expect("request should complete");
    assert!(
        resp.status().is_success(),
        "未超时请求应成功, 实际: {}",
        resp.status()
    );
    let body = resp.text().await.unwrap();
    assert_eq!(body, "fast");
}
