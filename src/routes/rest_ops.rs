//! Operational REST endpoints, all scoped under `api_context`.
//!
//! Covers data upload, tile access & cache (including WMTS RESTful and
//! GeoWebCache-compatible TMS / WMS-C), vector tiles (MVT), monitoring and
//! backup/restore.
//!
//! Contributes to the shared `api_context` scope (see `routes::mod`).

use actix_web::{web, Scope};

pub fn add_routes(scope: Scope) -> Scope {
    scope
        .route(
            "/data/upload",
            web::post().to(crate::handlers::upload_geojson),
        )
        .route(
            "/data/upload/shapefile",
            web::post().to(crate::handlers::upload_handler::upload_shapefile),
        )
        .route(
            "/data/upload/geotiff",
            web::post().to(crate::handlers::upload_handler::upload_geotiff),
        )
        // 矢量瓦片 (MVT) — 必须在通用瓦片路由之前注册, 否则
        // /tiles/{layer}/{z}/{x}/{y} 会捕获 {y}=="0.pbf" (遮蔽 bug)。
        .route(
            "/tiles/{layer}/{z}/{x}/{y}.pbf",
            web::get().to(crate::handlers::handle_mvt_tile),
        )
        .route(
            "/tiles/{layer}/{z}/{x}/{y}",
            web::get().to(crate::handlers::get_tile),
        )
        .route(
            "/mvt/{layer}/{z}/{x}/{y}",
            web::get().to(crate::handlers::handle_mvt_tile),
        )
        // WMTS RESTful 瓦片模板: /wmts/{layer}/{tileMatrixSet}/{tileMatrix}/{tileCol}/{tileRow}
        .route(
            "/wmts/{layer}/{tileMatrixSet}/{tileMatrix}/{tileCol}/{tileRow}",
            web::get().to(crate::handlers::handle_wmts_rest_tile),
        )
        // GeoWebCache 兼容: TMS 1.0.0 (RESTful + KVP) 与 WMS-C 1.1.1
        .route(
            "/gwc/service/tms",
            web::get().to(crate::handlers::handle_tms_request),
        )
        .route(
            "/gwc/service/tms/1.0.0",
            web::get().to(crate::handlers::handle_tms_path),
        )
        .route(
            "/gwc/service/tms/1.0.0/{tail:.*}",
            web::get().to(crate::handlers::handle_tms_path),
        )
        .route(
            "/gwc/service/wms",
            web::get().to(crate::handlers::handle_wmsc_request),
        )
        .route(
            "/tiles/cache/clear/{layer}",
            web::delete().to(crate::handlers::clear_tile_cache),
        )
        .route(
            "/tiles/cache/stats",
            web::get().to(crate::handlers::get_tile_cache_stats),
        )
        // 系统信息 (GeoServer 兼容路径)
        .route(
            "/about/version",
            web::get().to(crate::handlers::about_version),
        )
        .route(
            "/about/system-status",
            web::get().to(crate::handlers::about_system_status),
        )
        // 数据目录资源管理
        .route("/resources", web::get().to(crate::handlers::list_resources))
        .route(
            "/resources",
            web::post().to(crate::handlers::upload_resource),
        )
        .route(
            "/resources",
            web::delete().to(crate::handlers::delete_resource),
        )
        // 监控
        .route(
            "/monitor/stats",
            web::get().to(crate::handlers::get_monitor_stats),
        )
        .route(
            "/monitor/requests",
            web::get().to(crate::handlers::get_recent_requests),
        )
        .route(
            "/monitor/logs",
            web::get().to(crate::handlers::get_audit_logs),
        )
        .route(
            "/monitor/reset",
            web::delete().to(crate::handlers::reset_monitor_stats),
        )
        // 备份/恢复
        .route(
            "/backup/export",
            web::get().to(crate::handlers::handle_export),
        )
        .route(
            "/backup/import",
            web::post().to(crate::handlers::handle_import),
        )
}
