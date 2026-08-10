//! GeoServer-style unified OWS dispatcher: `/{api_context}/ows`.
//!
//! Dispatches to the individual OGC service handlers based on the `service`
//! KVP parameter (see `handlers::ows_handler`). GET supports
//! WMS/WFS/WCS/WPS/CSW; POST supports WFS/WPS/CSW.
//!
//! All `api_context`-scoped routes share one `web::Scope` (see `routes::mod`)
//! so submodules append routes via chained builders instead of registering
//! their own scope at the same prefix.

use actix_web::{web, Scope};

pub fn add_routes(scope: Scope) -> Scope {
    scope.service(
        web::resource("/ows")
            .route(web::get().to(crate::handlers::handle_ows_request))
            .route(web::post().to(crate::handlers::handle_ows_post_request)),
    )
}
