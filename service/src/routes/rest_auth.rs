//! Authentication & authorization REST endpoints, all scoped under
//! `api_context`.
//!
//! Contributes to the shared `api_context` scope (see `routes::mod`).

use actix_web::{web, Scope};

pub fn add_routes(scope: Scope) -> Scope {
    scope
        // 认证
        .route("/auth/login", web::post().to(crate::handlers::login))
        .route("/auth/logout", web::post().to(crate::handlers::logout))
        .route("/auth/verify", web::get().to(crate::handlers::verify))
        .route(
            "/auth/change-password",
            web::post().to(crate::handlers::change_password),
        )
        .route("/auth/users", web::get().to(crate::handlers::list_users))
        .route("/auth/users", web::post().to(crate::handlers::create_user))
        .route(
            "/auth/users/{username}",
            web::put().to(crate::handlers::update_user),
        )
        .route(
            "/auth/users/{username}",
            web::delete().to(crate::handlers::delete_user),
        )
        // 权限
        .service(
            web::resource("/permissions")
                .route(web::get().to(crate::handlers::list_permissions))
                .route(web::post().to(crate::handlers::create_permission)),
        )
        .route(
            "/permissions/{id}",
            web::delete().to(crate::handlers::delete_permission),
        )
        .route(
            "/permissions/check/{type}/{name}",
            web::get().to(crate::handlers::check_permission_handler),
        )
}
