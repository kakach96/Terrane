use actix_web::{HttpResponse, ResponseError};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum GeoServerError {
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
}

impl ResponseError for GeoServerError {
    fn error_response(&self) -> HttpResponse {
        match self {
            GeoServerError::NotFound(msg) => HttpResponse::NotFound().json(serde_json::json!({
                "error": "Not Found",
                "message": msg
            })),
            GeoServerError::BadRequest(msg) => HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Bad Request",
                "message": msg
            })),
            GeoServerError::NotImplemented(msg) => {
                HttpResponse::NotImplemented().json(serde_json::json!({
                    "error": "Not Implemented",
                    "message": msg
                }))
            },
            _ => HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Internal Server Error",
                "message": self.to_string()
            })),
        }
    }
}
