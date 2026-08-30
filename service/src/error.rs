use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TerraneError {
    #[error("Data not found: {0}")]
    NotFound(String),

    #[error("Invalid request: {0}")]
    BadRequest(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Not implemented: {0}")]
    NotImplemented(String),

    #[error("Internal error: {0}")]
    InternalError(String),

    #[error("Service error: {0}")]
    ServiceError(String),

    #[error("Projection error: {0}")]
    ProjectionError(String),

    #[error("Rendering error: {0}")]
    RenderingError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerdeError(#[from] serde_json::Error),

    #[error("Image processing error: {0}")]
    ImageError(#[from] image::ImageError),

    /// Localized, user-facing error with a stable machine-readable `code`
    /// (used to look up localized text on the client / via Accept-Language).
    #[error("{message}")]
    Localized {
        code: &'static str,
        status: StatusCode,
        message: String,
    },
}

impl TerraneError {
    /// Build a localized error with a stable code and explicit HTTP status.
    pub fn localized(code: &'static str, status: StatusCode, message: impl Into<String>) -> Self {
        TerraneError::Localized {
            code,
            status,
            message: message.into(),
        }
    }
}

impl ResponseError for TerraneError {
    fn error_response(&self) -> HttpResponse {
        match self {
            TerraneError::NotFound(msg) => HttpResponse::NotFound().json(serde_json::json!({
                "error": "Not Found",
                "code": "NOT_FOUND",
                "message": msg
            })),
            TerraneError::BadRequest(msg) => HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Bad Request",
                "code": "BAD_REQUEST",
                "message": msg
            })),
            TerraneError::Conflict(msg) => HttpResponse::Conflict().json(serde_json::json!({
                "error": "Conflict",
                "code": "CONFLICT",
                "message": msg
            })),
            TerraneError::NotImplemented(msg) => {
                HttpResponse::NotImplemented().json(serde_json::json!({
                    "error": "Not Implemented",
                    "code": "NOT_IMPLEMENTED",
                    "message": msg
                }))
            },
            TerraneError::ServiceError(msg) => {
                HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": "Service Error",
                    "code": "SERVICE_ERROR",
                    "message": msg
                }))
            },
            TerraneError::ProjectionError(msg) => {
                HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": "Projection Error",
                    "code": "PROJECTION_ERROR",
                    "message": msg
                }))
            },
            TerraneError::RenderingError(msg) => {
                HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": "Rendering Error",
                    "code": "RENDERING_ERROR",
                    "message": msg
                }))
            },
            TerraneError::ConfigError(msg) => {
                HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": "Configuration Error",
                    "code": "CONFIG_ERROR",
                    "message": msg
                }))
            },
            TerraneError::IoError(msg) => {
                HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": "IO Error",
                    "code": "IO_ERROR",
                    "message": msg.to_string()
                }))
            },
            TerraneError::SerdeError(msg) => {
                HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": "Serialization Error",
                    "code": "SERIALIZATION_ERROR",
                    "message": msg.to_string()
                }))
            },
            TerraneError::ImageError(msg) => {
                HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": "Image Processing Error",
                    "code": "IMAGE_ERROR",
                    "message": msg.to_string()
                }))
            },
            TerraneError::Localized {
                code,
                status,
                message,
            } => HttpResponse::build(*status).json(serde_json::json!({
                "error": status.canonical_reason().unwrap_or("Error"),
                "code": code,
                "message": message
            })),
            _ => HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Internal Server Error",
                "code": "INTERNAL_ERROR",
                "message": self.to_string()
            })),
        }
    }
}
