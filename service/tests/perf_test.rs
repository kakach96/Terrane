//! HTTP-level performance harness (opt-in, `#[ignore]`).
//!
//! Boots a real HTTP server (in-memory SQLite, temp data dir) seeded with a
//! 400-point GeoJSON layer, then drives concurrent load against representative
//! endpoints and reports throughput + latency percentiles (p50/p95/p99).
//!
//! Run:
//!
//! ```bash
//! cargo test --test perf_test -- --ignored --nocapture
//! ```
//!
//! Tunables (environment variables):
//! - `PERF_REQUESTS`     — measured requests per scenario (default 200)
//! - `PERF_CONCURRENCY`  — concurrent client tasks (default 8)
//! - `PERF_WARMUP`       — warmup requests per scenario before measuring (default 20)
//!
//! The harness asserts only on request success rate; latency numbers are
//! informational (hardware-dependent). Micro-benchmarks for in-process hot
//! paths live in `benches/core_paths.rs` (`cargo bench`).

use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use terrane::models::Bounds;
use terrane::utils::tile_grid;

const LAYER: &str = "perf_points";
const FEATURE_COUNT: usize = 400;
// France bbox the seeded points are spread over (EPSG:4326).
const BBOX: Bounds = Bounds {
    minx: 2.0,
    miny: 48.5,
    maxx: 3.0,
    maxy: 48.9,
};

fn env_or(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|v| *v > 0)
        .unwrap_or(default)
}

fn perf_config() -> terrane::config::GeoServerConfig {
    let mut config = terrane::config::GeoServerConfig::default();
    config.server.host = "127.0.0.1".to_string();
    config.server.port = 0;
    config.metadata.sqlite_path = ":memory:".into();

    // Keep all file writes inside a per-run temp directory.
    let tmp = std::env::temp_dir().join(format!("terrane-perf-{}", std::process::id()));
    config.data_dir = tmp.clone();
    config.cache = terrane::config::CacheConfig {
        kind: "local".to_string(),
        cache_dir: tmp.join("gwc"),
        meta_dir: tmp.join("gwc-meta"),
        expire_after_secs: 0,
        max_tiles: 0,
        layer_quota_bytes: 0,
        enabled: false,
        default_gridset: "EPSG:4326".to_string(),
        session_ttl_secs: 300,
    };
    config
}

struct ScenarioStats {
    name: String,
    requests: usize,
    failures: usize,
    mean_ms: f64,
    p50_ms: f64,
    p95_ms: f64,
    p99_ms: f64,
    max_ms: f64,
    rps: f64,
}

async fn spawn_server(state: terrane::state::AppState) -> std::net::SocketAddr {
    let state = actix_web::web::Data::new(state);
    let (tx, rx) = std::sync::mpsc::channel::<std::net::SocketAddr>();

    std::thread::spawn(move || {
        actix_web::rt::System::new().block_on(async move {
            let server = actix_web::HttpServer::new(move || {
                actix_web::App::new()
                    .app_data(state.clone())
                    .configure(|svc| terrane::routes::configure_routes(svc, "/geoserver"))
            })
            .workers(2)
            .bind(("127.0.0.1", 0))
            .expect("failed to bind perf-test server");

            let addr = server
                .addrs()
                .first()
                .copied()
                .expect("server has no bound address");
            let running = server.run();
            tx.send(addr).expect("receiver dropped");
            let _ = running.await;
        });
    });

    rx.recv_timeout(Duration::from_secs(30))
        .expect("perf-test server did not start in time")
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((p / 100.0) * sorted.len() as f64).ceil() as usize;
    sorted[(idx.min(sorted.len())) - 1]
}

/// Seed data source + published layer over HTTP (mirrors the upload flow used
/// by the REST integration tests).
async fn seed_layer(base: &str, client: &reqwest::Client) {
    let features: Vec<serde_json::Value> = (0..FEATURE_COUNT)
        .map(|i| {
            let lon = BBOX.minx + (i % 200) as f64 * 0.005;
            let lat = BBOX.miny + (i / 200) as f64 * 0.002;
            json!({
                "id": format!("perf-{i}"),
                "type": "Feature",
                "geometry": { "type": "Point", "coordinates": [lon, lat] },
                "properties": { "name": format!("place-{i}"), "population": i * 100, "layer_name": LAYER }
            })
        })
        .collect();

    let resp = client
        .post(format!("{base}/geoserver/data/upload"))
        .json(&json!({
            "type": "FeatureCollection",
            "total_count": FEATURE_COUNT,
            "features": features,
        }))
        .send()
        .await
        .expect("upload request failed");
    assert!(
        resp.status().is_success(),
        "GeoJSON upload failed: {}",
        resp.status()
    );

    let resp = client
        .post(format!("{base}/geoserver/layers"))
        .json(&json!({
            "name": LAYER,
            "title": "Perf Points",
            "workspace": "default",
            "store": LAYER,
            "native_name": LAYER,
            "srs": "EPSG:4326",
            "minx": BBOX.minx, "miny": BBOX.miny, "maxx": BBOX.maxx, "maxy": BBOX.maxy,
        }))
        .send()
        .await
        .expect("layer create failed");
    assert!(
        resp.status().is_success(),
        "layer publish failed: {}",
        resp.status()
    );
}

async fn run_scenario(
    client: &reqwest::Client,
    base: &str,
    name: &str,
    path: &str,
    total: usize,
    concurrency: usize,
    warmup: usize,
) -> ScenarioStats {
    let url = format!("{base}{path}");

    // Warmup (connection pools, lazy caches, first-render allocations).
    for _ in 0..warmup {
        client
            .get(&url)
            .send()
            .await
            .expect("warmup request failed");
    }

    let cursor = Arc::new(AtomicUsize::new(0));
    let latencies: Arc<std::sync::Mutex<Vec<f64>>> =
        Arc::new(std::sync::Mutex::new(Vec::with_capacity(total)));
    let failures = Arc::new(AtomicUsize::new(0));
    let started = Instant::now();

    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..concurrency {
        let client = client.clone();
        let url = url.clone();
        let cursor = cursor.clone();
        let latencies = latencies.clone();
        let failures = failures.clone();
        tasks.spawn(async move {
            loop {
                if cursor.fetch_add(1, Ordering::Relaxed) >= total {
                    break;
                }
                let begin = Instant::now();
                match client.get(&url).send().await {
                    Ok(resp) => {
                        if !resp.status().is_success() {
                            failures.fetch_add(1, Ordering::Relaxed);
                        }
                        let _ = resp.bytes().await;
                    },
                    Err(_) => {
                        failures.fetch_add(1, Ordering::Relaxed);
                    },
                }
                latencies
                    .lock()
                    .expect("latency mutex poisoned")
                    .push(begin.elapsed().as_secs_f64() * 1000.0);
            }
        });
    }
    while tasks.join_next().await.is_some() {}
    let elapsed = started.elapsed();

    let mut samples = latencies.lock().expect("latency mutex poisoned").clone();
    samples.sort_by(|a, b| a.total_cmp(b));

    let count = samples.len();
    let sum: f64 = samples.iter().sum();
    ScenarioStats {
        name: name.to_string(),
        requests: count,
        failures: failures.load(Ordering::Relaxed),
        mean_ms: if count > 0 { sum / count as f64 } else { 0.0 },
        p50_ms: percentile(&samples, 50.0),
        p95_ms: percentile(&samples, 95.0),
        p99_ms: percentile(&samples, 99.0),
        max_ms: samples.last().copied().unwrap_or(0.0),
        rps: count as f64 / elapsed.as_secs_f64(),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "performance harness — run explicitly: cargo test --test perf_test -- --ignored --nocapture"]
async fn http_load_benchmark() {
    let requests = env_or("PERF_REQUESTS", 200);
    let concurrency = env_or("PERF_CONCURRENCY", 8);
    let warmup = env_or("PERF_WARMUP", 20);

    let addr = spawn_server(terrane::state::AppState::new(perf_config()).await).await;
    let base = format!("http://{addr}");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .pool_max_idle_per_host(64)
        .build()
        .expect("client build");

    seed_layer(&base, &client).await;

    // One tile covering the seeded points at zoom 3 (top-down/slippy row).
    let (col, row) = tile_grid::tile_for_bbox("EPSG:4326", 3, &BBOX).expect("tile index");
    let scenarios: Vec<(&str, String)> = vec![
        ("rest_layers_list", "/geoserver/layers".to_string()),
        (
            "wms_getcapabilities_130",
            "/wms?SERVICE=WMS&VERSION=1.3.0&REQUEST=GetCapabilities".to_string(),
        ),
        (
            "wms_getmap_png_256",
            format!(
                "/wms?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetMap&LAYERS={LAYER}\
                 &BBOX=-180,-90,180,90&WIDTH=256&HEIGHT=256&SRS=EPSG:4326&FORMAT=image/png"
            ),
        ),
        (
            "wfs_getfeature_json",
            format!(
                "/wfs?SERVICE=WFS&REQUEST=GetFeature&TYPENAME={LAYER}\
                 &OUTPUTFORMAT=application/json"
            ),
        ),
        (
            "tile_png_z3",
            format!("/geoserver/tiles/{LAYER}/3/{col}/{row}"),
        ),
    ];

    println!(
        "\nTerrane HTTP performance harness — {requests} req/scenario, \
         {concurrency} concurrent clients, {FEATURE_COUNT} seeded features"
    );
    println!(
        "{:<26}{:>9}{:>8}{:>10}{:>10}{:>10}{:>10}{:>10}{:>10}",
        "scenario", "reqs", "fail", "mean(ms)", "p50(ms)", "p95(ms)", "p99(ms)", "max(ms)", "req/s"
    );

    for (name, path) in &scenarios {
        let stats = run_scenario(&client, &base, name, path, requests, concurrency, warmup).await;
        println!(
            "{:<26}{:>9}{:>8}{:>10.2}{:>10.2}{:>10.2}{:>10.2}{:>10.2}{:>10.1}",
            stats.name,
            stats.requests,
            stats.failures,
            stats.mean_ms,
            stats.p50_ms,
            stats.p95_ms,
            stats.p99_ms,
            stats.max_ms,
            stats.rps
        );
        assert_eq!(stats.failures, 0, "scenario '{name}' had failed requests");
    }

    // Best-effort cleanup of the temp data dir.
    let _ = std::fs::remove_dir_all(
        std::env::temp_dir().join(format!("terrane-perf-{}", std::process::id())),
    );
}
