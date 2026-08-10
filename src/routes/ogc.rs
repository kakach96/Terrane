//! OGC KVP service endpoints (root scope, no `api_context` prefix).
//!
//! Each service is exposed under its own path (`/wms`, `/wfs`, ...) and, for
//! POST-capable services, both GET and POST are registered. This mirrors
//! GeoServer's individual service endpoints; the unified `/ows` dispatcher
//! lives in `routes::ows`.

use actix_web::web;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/wms").route("", web::get().to(crate::handlers::handle_wms_request)))
        .service(web::scope("/wmts").route("", web::get().to(crate::handlers::handle_wmts_request)))
        .service(
            web::scope("/wfs")
                .route("", web::get().to(crate::handlers::handle_wfs_request))
                .route("", web::post().to(crate::handlers::handle_wfs_post_request)),
        )
        .service(web::scope("/wcs").route("", web::get().to(crate::handlers::handle_wcs_request)))
        .service(
            web::scope("/wps")
                .route("", web::get().to(crate::handlers::handle_wps_request))
                .route("", web::post().to(crate::handlers::handle_wps_post_request)),
        )
        .service(
            web::scope("/csw")
                .route("", web::get().to(crate::handlers::handle_csw_request))
                .route("", web::post().to(crate::handlers::handle_csw_post_request)),
        );
}
