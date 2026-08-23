//! OGC API - Styles HTTP Handler
//!
//! Routes under `/ogc/styles`: landing page, `/conformance`, `/styles`
//! (list / create), `/styles/{styleId}` (get / replace / delete),
//! `/styles/{styleId}/metadata` and the collection linkage
//! `/collections` + `/collections/{id}/styles`. Write operations require a
//! valid Bearer token (JWT), consistent with the REST catalog API.

use crate::handlers::auth_handler::require_auth;
use crate::models::style::StyleFormat;
use crate::services::ogc_styles::{self, StyleSummary};
use crate::state::{AppState, StyleMeta};
use crate::store::StyleRecord;
use actix_web::{web, HttpRequest, HttpResponse};
use serde::Deserialize;

/// Service base URL (OGC API is served at the root path, not under the API
/// context).
fn base_url(state: &AppState) -> String {
    format!(
        "http://{}:{}",
        state.config.server.host, state.config.server.port
    )
}

fn json_response(value: serde_json::Value) -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/json")
        .body(serde_json::to_string(&value).unwrap_or_default())
}

fn not_found(what: &str) -> HttpResponse {
    HttpResponse::NotFound().json(serde_json::json!({
        "code": "NotFound",
        "description": format!("Resource not found: {}", what),
    }))
}

fn unauthorized(message: &str) -> HttpResponse {
    HttpResponse::Unauthorized().json(serde_json::json!({
        "code": "Unauthorized",
        "description": message,
    }))
}

/// Collect the style summaries from the in-memory catalog.
async fn style_summaries(state: &AppState) -> Vec<StyleSummary> {
    let styles = state.styles.read().await;
    let meta = state.styles_meta.read().await;
    let mut out: Vec<StyleSummary> = styles
        .keys()
        .map(|name| {
            let m = meta.get(name);
            StyleSummary {
                id: name.clone(),
                title: m.map(|m| m.title.clone()).unwrap_or_else(|| name.clone()),
                description: None,
                format: m.map(|m| m.format.clone()).unwrap_or(StyleFormat::SLD),
            }
        })
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

fn now_ts() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// `GET /ogc/styles` — landing page.
pub async fn handle_ogc_styles_landing(state: web::Data<AppState>) -> HttpResponse {
    json_response(ogc_styles::landing_page(&base_url(state.get_ref())))
}

/// `GET /ogc/styles/conformance`
pub async fn handle_ogc_styles_conformance() -> HttpResponse {
    json_response(ogc_styles::conformance())
}

/// `GET /ogc/styles/styles` — style list.
pub async fn handle_ogc_styles_list(state: web::Data<AppState>) -> HttpResponse {
    let summaries = style_summaries(state.get_ref()).await;
    json_response(ogc_styles::styles_list(
        &base_url(state.get_ref()),
        &summaries,
    ))
}

/// Request body of `POST /ogc/styles/styles`.
#[derive(Deserialize)]
pub struct CreateStyleBody {
    pub id: String,
    pub title: Option<String>,
    pub description: Option<String>,
    /// Style body in its native format (SLD XML / CSS / YSLD YAML / MBStyle
    /// JSON); the format is auto-detected from the content.
    pub content: String,
}

/// `POST /ogc/styles/styles` — create a style (requires auth).
pub async fn handle_ogc_styles_create(
    req: HttpRequest,
    body: web::Json<CreateStyleBody>,
    state: web::Data<AppState>,
) -> HttpResponse {
    if let Err(e) = require_auth(&req) {
        return unauthorized(&e.to_string());
    }
    let id = body.id.trim().to_string();
    if id.is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "code": "InvalidParameterValue",
            "description": "id must not be empty",
        }));
    }
    let format = crate::models::style::detect_style_format(&body.content);
    let title = body.title.clone().unwrap_or_else(|| id.clone());

    // In-memory catalog.
    state
        .styles
        .write()
        .await
        .insert(id.clone(), body.content.clone());
    state.styles_meta.write().await.insert(
        id.clone(),
        StyleMeta {
            title: title.clone(),
            is_builtin: false,
            format: format.clone(),
        },
    );

    // Persist to the metadata store.
    if let Some(store) = &state.store {
        let ts = now_ts();
        let _ = store
            .create_style(&StyleRecord {
                name: id.clone(),
                title: title.clone(),
                format: format.to_string(),
                is_builtin: false,
                content: body.content.clone(),
                created: ts.clone(),
                modified: ts,
            })
            .await;
    }

    let summary = StyleSummary {
        id: id.clone(),
        title,
        description: body.description.clone(),
        format,
    };
    HttpResponse::Created().json(ogc_styles::style_metadata(
        &base_url(state.get_ref()),
        &summary,
    ))
}

/// Look up a style summary by id (None when missing).
async fn find_style_summary(state: &AppState, id: &str) -> Option<StyleSummary> {
    let styles = state.styles.read().await;
    let meta = state.styles_meta.read().await;
    if !styles.contains_key(id) {
        return None;
    }
    let m = meta.get(id);
    Some(StyleSummary {
        id: id.to_string(),
        title: m.map(|m| m.title.clone()).unwrap_or_else(|| id.to_string()),
        description: None,
        format: m.map(|m| m.format.clone()).unwrap_or(StyleFormat::SLD),
    })
}

/// `GET /ogc/styles/styles/{styleId}` — style content in its native format.
pub async fn handle_ogc_styles_style(
    path: web::Path<String>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let id = path.into_inner();
    let summary = match find_style_summary(state.get_ref(), &id).await {
        Some(s) => s,
        None => return not_found(&id),
    };
    let content = {
        let styles = state.styles.read().await;
        match styles.get(&id) {
            Some(c) => c.clone(),
            None => return not_found(&id),
        }
    };
    HttpResponse::Ok()
        .content_type(ogc_styles::mime_for_format(&summary.format))
        .body(content)
}

/// `PUT /ogc/styles/styles/{styleId}` — replace style content (requires auth).
pub async fn handle_ogc_styles_put(
    req: HttpRequest,
    path: web::Path<String>,
    body: String,
    state: web::Data<AppState>,
) -> HttpResponse {
    if let Err(e) = require_auth(&req) {
        return unauthorized(&e.to_string());
    }
    let id = path.into_inner();
    if find_style_summary(state.get_ref(), &id).await.is_none() {
        return not_found(&id);
    }
    let format = crate::models::style::detect_style_format(&body);

    let mut styles = state.styles.write().await;
    styles.insert(id.clone(), body.clone());
    drop(styles);
    state
        .styles_meta
        .write()
        .await
        .entry(id.clone())
        .and_modify(|m| {
            m.format = format.clone();
        });

    if let Some(store) = &state.store {
        let _ = store
            .update_style(&id, None, Some(format.to_string()), Some(body), None)
            .await;
    }

    json_response(serde_json::json!({
        "id": id,
        "message": "Style updated",
    }))
}

/// `DELETE /ogc/styles/styles/{styleId}` — delete a style (requires auth).
pub async fn handle_ogc_styles_delete(
    req: HttpRequest,
    path: web::Path<String>,
    state: web::Data<AppState>,
) -> HttpResponse {
    if let Err(e) = require_auth(&req) {
        return unauthorized(&e.to_string());
    }
    let id = path.into_inner();
    if find_style_summary(state.get_ref(), &id).await.is_none() {
        return not_found(&id);
    }
    state.styles.write().await.remove(&id);
    state.styles_meta.write().await.remove(&id);
    if let Some(store) = &state.store {
        let _ = store.delete_style(&id).await;
    }
    HttpResponse::NoContent().finish()
}

/// `GET /ogc/styles/styles/{styleId}/metadata` — style metadata (JSON).
pub async fn handle_ogc_styles_metadata(
    path: web::Path<String>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let id = path.into_inner();
    match find_style_summary(state.get_ref(), &id).await {
        Some(summary) => json_response(ogc_styles::style_metadata(
            &base_url(state.get_ref()),
            &summary,
        )),
        None => not_found(&id),
    }
}

/// `GET /ogc/styles/collections` — layer collections.
pub async fn handle_ogc_styles_collections(state: web::Data<AppState>) -> HttpResponse {
    let layers = state.list_layers().await;
    json_response(ogc_styles::collections(&base_url(state.get_ref()), &layers))
}

/// `GET /ogc/styles/collections/{collectionId}/styles` — styles of a layer.
pub async fn handle_ogc_styles_collection_styles(
    path: web::Path<String>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let name = path.into_inner();
    let layer = match state.get_layer(&name).await {
        Some(l) => l,
        None => return not_found(&name),
    };
    let available = style_summaries(state.get_ref()).await;
    json_response(ogc_styles::collection_styles(
        &base_url(state.get_ref()),
        &layer,
        &available,
    ))
}
