//! 存储层统一错误类型

use std::fmt;

/// 存储层错误 (Sqlite / Postgres 统一)
#[derive(Debug)]
pub enum StoreError {
    /// SQLite 后端错误
    Sqlite(rusqlite::Error),
    /// PostgreSQL 后端错误
    Postgres(Box<dyn std::error::Error + Send + Sync>),
    /// 其他存储错误 (如连接池/状态错误)
    Other(String),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Sqlite(e) => write!(f, "SQLite error: {}", e),
            StoreError::Postgres(e) => write!(f, "PostgreSQL error: {}", e),
            StoreError::Other(msg) => write!(f, "Store error: {}", msg),
        }
    }
}

impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            StoreError::Sqlite(e) => Some(e),
            StoreError::Postgres(e) => Some(e.as_ref()),
            StoreError::Other(_) => None,
        }
    }
}

impl From<rusqlite::Error> for StoreError {
    fn from(e: rusqlite::Error) -> Self {
        StoreError::Sqlite(e)
    }
}

impl From<tokio_postgres::Error> for StoreError {
    fn from(e: tokio_postgres::Error) -> Self {
        StoreError::Postgres(Box::new(e))
    }
}

impl From<deadpool_postgres::PoolError> for StoreError {
    fn from(e: deadpool_postgres::PoolError) -> Self {
        StoreError::Postgres(Box::new(e))
    }
}

impl From<deadpool_postgres::CreatePoolError> for StoreError {
    fn from(e: deadpool_postgres::CreatePoolError) -> Self {
        StoreError::Postgres(Box::new(e))
    }
}

impl From<StoreError> for crate::error::GeoServerError {
    fn from(e: StoreError) -> Self {
        crate::error::GeoServerError::InternalError(format!("{}", e))
    }
}

impl From<std::io::Error> for StoreError {
    fn from(e: std::io::Error) -> Self {
        StoreError::Postgres(Box::new(e))
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(e: serde_json::Error) -> Self {
        StoreError::Other(e.to_string())
    }
}
