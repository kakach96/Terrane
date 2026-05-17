use actix_web::web;

pub fn configure_routes(cfg: &mut web::ServiceConfig, api_context: &str) {
    cfg.service(
        web::scope("/wms")
            .route("", web::get().to(crate::handlers::handle_wms_request))
    )
    .service(
        web::scope("/wfs")
            .route("", web::get().to(crate::handlers::handle_wfs_request))
    )
    .service(
        web::scope("/wcs")
            .route("", web::get().to(crate::handlers::handle_wcs_request))
    )
    .service(
        web::scope(api_context)
            .route("/health", web::get().to(crate::handlers::health_check))
            .service(
                web::resource("/layers")
                    .route(web::get().to(crate::handlers::list_layers))
                    .route(web::post().to(crate::handlers::create_layer))
            )
            .service(
                web::resource("/layers/{layer}")
                    .route(web::get().to(crate::handlers::get_layer))
                    .route(web::put().to(crate::handlers::update_layer))
                    .route(web::delete().to(crate::handlers::delete_layer))
            )
            .service(
                web::resource("/layers/{layer}/preview")
                    .route(web::get().to(crate::handlers::preview_layer))
            )
            .service(
                web::resource("/layers/{layer}/features")
                    .route(web::get().to(crate::handlers::get_layer_features))
                    .route(web::post().to(crate::handlers::create_feature))
            )
            .service(
                web::resource("/layers/{layer}/features/{feature}")
                    .route(web::get().to(crate::handlers::get_feature))
                    .route(web::put().to(crate::handlers::update_feature))
                    .route(web::delete().to(crate::handlers::delete_feature))
            )
            .service(
                web::resource("/workspaces")
                    .route(web::get().to(crate::handlers::list_workspaces))
                    .route(web::post().to(crate::handlers::create_workspace))
            )
            .service(
                web::resource("/workspaces/{workspace}")
                    .route(web::get().to(crate::handlers::get_workspace))
                    .route(web::put().to(crate::handlers::update_workspace))
                    .route(web::delete().to(crate::handlers::delete_workspace))
            )
            .route("/server/status", web::get().to(crate::handlers::get_server_status))
            .route("/data/upload", web::post().to(crate::handlers::upload_geojson))
            .service(
                web::resource("/data-sources")
                    .route(web::get().to(crate::handlers::list_data_sources))
                    .route(web::post().to(crate::handlers::create_data_source))
            )
            .route("/data-sources/test", web::post().to(crate::handlers::test_connection))
            .route("/data-sources/{name}/tables", web::get().to(crate::handlers::get_data_source_tables))
            .service(
                web::resource("/data-sources/{name}")
                    .route(web::get().to(crate::handlers::get_data_source))
                    .route(web::put().to(crate::handlers::update_data_source))
                    .route(web::delete().to(crate::handlers::delete_data_source))
            )
            .route("/data-sources/{name}/test", web::post().to(crate::handlers::test_data_source_connection))
    );
}
