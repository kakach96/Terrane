//! Route registration.
//!
//! The route table is split into focused submodules so that each `configure`
//! function stays small and readable:
//!
//! - `probes`: cloud-native probes & metrics (fixed root paths, decoupled from
//!   `api_context` so container / K8s probes can rely on them).
//! - `ogc`: OGC KVP services (`/wms`, `/wmts`, `/wfs`, `/wcs`, `/wps`, `/csw`).
//! - `ogc_api`: OGC API - Common endpoints (`/ogc/features`, `/ogc/tiles`,
//!   `/ogc/maps`, `/ogc/processes`).
//! - `ows`: GeoServer-style unified dispatcher (`/{api_context}/ows`).
//! - `rest_catalog`: catalog management REST endpoints under `api_context`.
//! - `rest_ops`: operational endpoints (tiles, gwc, monitor, backup, upload).
//! - `rest_auth`: authentication & authorization endpoints.

pub mod ogc;
pub mod ogc_api;
pub mod ows;
pub mod probes;
pub mod rest_auth;
pub mod rest_catalog;
pub mod rest_ops;

use actix_web::web;

/// Register the full route table.
///
/// Kept as a single entry point with the same signature as before the split,
/// so `main.rs` and the integration tests do not need to change.
pub fn configure_routes(cfg: &mut web::ServiceConfig, api_context: &str) {
    probes::configure(cfg);
    ogc::configure(cfg);
    ogc_api::configure(cfg);

    // All `api_context`-scoped routes share ONE scope so they can never shadow
    // each other; each submodule appends its routes via a chained builder.
    let scope = web::scope(api_context);
    let scope = ows::add_routes(scope);
    let scope = rest_catalog::add_routes(scope);
    let scope = rest_ops::add_routes(scope);
    let scope = rest_auth::add_routes(scope);
    cfg.service(scope);
}
