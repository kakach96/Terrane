//! OGC API - Common endpoints (root scope).
//!
//! Covers the OGC API - Features, Tiles, Maps and Processes building blocks,
//! each registered under its own `/ogc/<building-block>` scope.

use actix_web::web;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/ogc/features")
            .route("", web::get().to(crate::handlers::handle_ogc_landing))
            .route("/conformance", web::get().to(crate::handlers::handle_ogc_conformance))
            .route("/collections", web::get().to(crate::handlers::handle_ogc_collections))
            .route("/collections/{collection}", web::get().to(crate::handlers::handle_ogc_collection))
            .route("/collections/{collection}/items", web::get().to(crate::handlers::handle_ogc_items))
            .route(
                "/collections/{collection}/items/{feature}",
                web::get().to(crate::handlers::handle_ogc_item),
            ),
    )
    .service(
        web::scope("/ogc/tiles")
            .route("", web::get().to(crate::handlers::handle_ogc_tiles_landing))
            .route("/conformance", web::get().to(crate::handlers::handle_ogc_tiles_conformance))
            .route("/tileMatrixSets", web::get().to(crate::handlers::handle_ogc_tiles_tile_matrix_sets))
            .route(
                "/tileMatrixSets/{id}",
                web::get().to(crate::handlers::handle_ogc_tiles_tile_matrix_set),
            )
            .route("/collections", web::get().to(crate::handlers::handle_ogc_tiles_collections))
            .route(
                "/collections/{collection}/tiles",
                web::get().to(crate::handlers::handle_ogc_tiles_collection),
            )
            .route(
                "/collections/{collection}/tiles/{tileMatrixSetId}/{tileMatrix}/{tileRow}/{tileCol}",
                web::get().to(crate::handlers::handle_ogc_tile),
            ),
    )
    .service(
        web::scope("/ogc/coverages")
            .route("", web::get().to(crate::handlers::handle_ogc_coverages_landing))
            .route("/conformance", web::get().to(crate::handlers::handle_ogc_coverages_conformance))
            .route("/collections", web::get().to(crate::handlers::handle_ogc_coverages_collections))
            .route(
                "/collections/{collection}",
                web::get().to(crate::handlers::handle_ogc_coverages_collection),
            )
            .route(
                "/collections/{collection}/coverage",
                web::get().to(crate::handlers::handle_ogc_coverages_coverage),
            ),
    )
    .service(
        web::scope("/ogc/maps")
            .route("", web::get().to(crate::handlers::handle_ogc_maps_landing))
            .route("/conformance", web::get().to(crate::handlers::handle_ogc_maps_conformance))
            .route("/collections", web::get().to(crate::handlers::handle_ogc_maps_collections))
            .route(
                "/collections/{collection}",
                web::get().to(crate::handlers::handle_ogc_maps_collection),
            )
            .route(
                "/collections/{collection}/styles",
                web::get().to(crate::handlers::handle_ogc_maps_styles),
            )
            .route(
                "/collections/{collection}/map",
                web::get().to(crate::handlers::handle_ogc_maps_map),
            ),
    )
    .service(
        web::scope("/ogc/styles")
            .route("", web::get().to(crate::handlers::handle_ogc_styles_landing))
            .route("/conformance", web::get().to(crate::handlers::handle_ogc_styles_conformance))
            .route("/styles", web::get().to(crate::handlers::handle_ogc_styles_list))
            .route("/styles", web::post().to(crate::handlers::handle_ogc_styles_create))
            .route(
                "/styles/{styleId}",
                web::get().to(crate::handlers::handle_ogc_styles_style),
            )
            .route(
                "/styles/{styleId}",
                web::put().to(crate::handlers::handle_ogc_styles_put),
            )
            .route(
                "/styles/{styleId}",
                web::delete().to(crate::handlers::handle_ogc_styles_delete),
            )
            .route(
                "/styles/{styleId}/metadata",
                web::get().to(crate::handlers::handle_ogc_styles_metadata),
            )
            .route("/collections", web::get().to(crate::handlers::handle_ogc_styles_collections))
            .route(
                "/collections/{collectionId}/styles",
                web::get().to(crate::handlers::handle_ogc_styles_collection_styles),
            ),
    )
    .service(
        web::scope("/ogc/processes")
            .route("", web::get().to(crate::handlers::handle_ogc_processes_landing))
            .route("/conformance", web::get().to(crate::handlers::handle_ogc_processes_conformance))
            .route("/processes", web::get().to(crate::handlers::handle_ogc_processes_processes))
            .route(
                "/processes/{processId}",
                web::get().to(crate::handlers::handle_ogc_processes_process),
            )
            .route("/jobs", web::get().to(crate::handlers::handle_ogc_processes_jobs))
            .route("/jobs", web::post().to(crate::handlers::handle_ogc_processes_execute))
            .route(
                "/jobs/{jobId}",
                web::get().to(crate::handlers::handle_ogc_processes_job),
            )
            .route(
                "/jobs/{jobId}",
                web::delete().to(crate::handlers::handle_ogc_processes_job_cancel),
            )
            .route(
                "/jobs/{jobId}/results",
                web::get().to(crate::handlers::handle_ogc_processes_job_results),
            ),
    );
}
