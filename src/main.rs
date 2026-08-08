#![allow(dead_code)]

use actix_cors::Cors;
use actix_files::Files;
use actix_web::{middleware, web, App, HttpResponse, HttpServer};
use chrono::{Datelike, Local, Timelike};
use clap::Parser;
use tokio::fs;
use tokio::signal;
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::time::FormatTime;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod auth;
mod backup;
mod config;
mod error;
mod handlers;
mod models;
mod routes;
mod services;
mod state;
mod store;
mod utils;

use config::GeoServerConfig;
use state::AppState;

struct FriendlyTimeFormat;

impl FormatTime for FriendlyTimeFormat {
    fn format_time(&self, w: &mut Writer<'_>) -> std::fmt::Result {
        let now = Local::now();
        write!(
            w,
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
            now.year(),
            now.month(),
            now.day(),
            now.hour(),
            now.minute(),
            now.second(),
            now.nanosecond() / 1_000_000
        )
    }
}

#[derive(Parser, Debug)]
#[command(name = "terrane")]
#[command(about = "A high-performance geospatial data server implemented in Rust", long_about = None)]
struct Args {
    #[arg(long, default_value = "terrane")]
    config: String,

    #[arg(long)]
    host: Option<String>,

    #[arg(short, long)]
    port: Option<u16>,
}

fn init_tracing(default_level: &str) {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| default_level.into()),
        ))
        .with(
            tracing_subscriber::fmt::layer()
                .with_timer(FriendlyTimeFormat)
                .with_level(true)
                .with_target(true)
                .with_thread_ids(true)
                .with_thread_names(true),
        )
        .init();
}

fn load_config(config_path: &str) -> GeoServerConfig {
    // terrane.toml (可选) + GEOSERVER__ 环境变量双源加载，env 覆盖文件
    GeoServerConfig::load_from_file(config_path).unwrap_or_else(|e| {
        // 注意: load_config 在 init_tracing 之前调用, tracing::warn! 不生效,
        // 因此必须同时输出到 stderr, 否则配置错误会被静默吞掉
        let msg = format!(
            "Failed to load config '{}': {}. Using defaults.",
            config_path, e
        );
        eprintln!("[config] WARNING: {}", msg);
        tracing::warn!("{}", msg);
        GeoServerConfig::default()
    })
}

fn print_startup_info(host: &str, port: u16, api_context: &str) {
    tracing::info!("Starting Terrane on {}:{}", host, port);
    tracing::info!("WMS endpoint: http://{}:{}/wms", host, port);
    tracing::info!("WFS endpoint: http://{}:{}/wfs", host, port);
    tracing::info!("WCS endpoint: http://{}:{}/wcs", host, port);
    tracing::info!("REST API: http://{}:{}{}", host, port, api_context);
}

async fn serve_index() -> HttpResponse {
    match fs::read_to_string("./static/index.html").await {
        Ok(content) => HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(content),
        Err(_) => HttpResponse::NotFound().body("Not Found"),
    }
}

/// 监听 Ctrl+C (SIGINT) 与 SIGTERM, 触发优雅关闭以排空在途请求。
/// 容器平台 (K8s/Docker) 向进程发送 SIGTERM 进行滚动更新/缩容, 需要优雅排空。
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => { tracing::info!("Received Ctrl+C / SIGINT, draining in-flight requests..."); }
        _ = terminate => { tracing::info!("Received SIGTERM, draining in-flight requests..."); }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let args = Args::parse();
    let config = load_config(&args.config);

    init_tracing(&config.logging.level);

    let host = args.host.unwrap_or(config.server.host.clone());
    let port = args.port.unwrap_or(config.server.port);
    let api_context = config.server.api_context.clone();
    let static_dir = config.server.static_dir.clone();
    let static_dir_str = static_dir.to_string_lossy().to_string();

    // 初始化 JWT 密钥 (集群各副本必须共享相同密钥)
    auth::init_secret(&config.security.jwt_secret);

    print_startup_info(&host, port, &api_context);
    tracing::info!("Metadata store backend: {}", config.metadata.kind);

    let app_state = web::Data::new(AppState::new(config).await);

    let cors_config = app_state.config.cors.clone();
    let shutdown_timeout = app_state.config.server.shutdown_timeout_secs;

    HttpServer::new(move || {
        // CORS 中间件
        let cors_middleware = if cors_config.enabled {
            let has_specific_origins = cors_config.allowed_origins.iter().any(|o| o != "*");
            let mut cors = if has_specific_origins {
                let mut c = Cors::default();
                for origin in &cors_config.allowed_origins {
                    c = c.allowed_origin(origin);
                }
                for method in &cors_config.allowed_methods {
                    c = c.allowed_methods(vec![method.as_str()]);
                }
                for header in &cors_config.allowed_headers {
                    c = c.allowed_header(header);
                }
                c
            } else {
                Cors::permissive()
            };
            cors = cors.max_age(Some(cors_config.max_age as usize));
            if cors_config.allow_credentials {
                cors = cors.supports_credentials();
            }

            cors
        } else {
            Cors::default()
        };

        App::new()
            .app_data(app_state.clone())
            .wrap(cors_middleware)
            .wrap(middleware::Logger::default())
            .wrap(middleware::Compress::default())
            .configure(|svc| routes::configure_routes(svc, &api_context))
            .service(
                Files::new("/", &static_dir_str)
                    .index_file("index.html")
                    .use_etag(false)
                    .use_last_modified(false),
            )
            .default_service(web::route().to(serve_index))
    })
    .bind((host.as_str(), port))?
    .shutdown_signal(shutdown_signal())
    .shutdown_timeout(shutdown_timeout)
    .run()
    .await
}
