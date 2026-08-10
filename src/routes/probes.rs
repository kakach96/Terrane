//! Cloud-native probes & monitoring endpoints.
//!
//! Registered on fixed root paths, decoupled from `api_context` so container /
//! K8s probes can rely on them regardless of the API base path.

use actix_web::web;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/health/live", web::get().to(crate::handlers::health_live))
        .route(
            "/health/ready",
            web::get().to(crate::handlers::health_ready),
        )
        .route("/metrics", web::get().to(crate::handlers::get_metrics));
}
