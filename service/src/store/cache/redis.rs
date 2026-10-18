//! Shared Redis connection helper for cache backends.
//!
//! Wraps a lazily-created [`redis::aio::ConnectionManager`] (auto-reconnecting)
//! behind an `Arc<OnceCell>`, so tile / session cache backends share one
//! connection per process and reconnect automatically after Redis restarts.

use std::sync::Arc;

use redis::aio::ConnectionManager;
use tokio::sync::OnceCell;

use crate::store::StoreError;

/// A lazily-initialized, auto-reconnecting Redis connection.
#[derive(Clone)]
pub struct RedisConn {
    url: String,
    inner: Arc<OnceCell<ConnectionManager>>,
}

impl RedisConn {
    pub fn new(url: &str) -> Self {
        RedisConn {
            url: url.to_string(),
            inner: Arc::new(OnceCell::new()),
        }
    }

    /// Get (and lazily establish) the connection manager.
    ///
    /// Connection establishment is bounded by a timeout so an unreachable
    /// Redis (e.g. during an outage) cannot stall tile rendering for long
    /// (the connection manager itself retries with backoff).
    pub async fn conn(&self) -> Result<ConnectionManager, StoreError> {
        if let Some(cm) = self.inner.get() {
            return Ok(cm.clone());
        }
        let client = redis::Client::open(self.url.as_str())
            .map_err(|e| StoreError::Other(format!("Redis client error: {}", e)))?;
        let connect = ConnectionManager::new(client);
        let cm = tokio::time::timeout(std::time::Duration::from_secs(5), connect)
            .await
            .map_err(|_| StoreError::Other("Redis connection timed out".to_string()))?
            .map_err(|e| StoreError::Other(format!("Redis connection error: {}", e)))?;
        // Race-safe: another task may have initialized it already.
        let _ = self.inner.set(cm.clone());
        Ok(cm)
    }

    /// Ping the server to verify connectivity (used by `init` / tests).
    pub async fn ping(&self) -> Result<(), StoreError> {
        let mut conn = self.conn().await?;
        redis::cmd("PING")
            .query_async::<String>(&mut conn)
            .await
            .map_err(|e| StoreError::Other(format!("Redis PING failed: {}", e)))?;
        Ok(())
    }
}

/// Process-wide registry of shared [`RedisConn`] instances keyed by URL.
///
/// Repeated connectivity pings (e.g. the monitor health page refreshing every
/// 10s) reuse the auto-reconnecting connection manager instead of opening a
/// new socket per ping.
static SHARED_CONNS: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<String, RedisConn>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

/// Ping Redis through the shared connection for `url` (bounded by a 3s
/// timeout so an unreachable server cannot stall the caller, e.g. the
/// monitor page refresh loop).
pub async fn ping_shared(url: &str) -> Result<(), StoreError> {
    let conn = {
        let mut conns = SHARED_CONNS.lock().unwrap();
        conns
            .entry(url.to_string())
            .or_insert_with(|| RedisConn::new(url))
            .clone()
    };
    tokio::time::timeout(std::time::Duration::from_secs(3), conn.ping())
        .await
        .map_err(|_| StoreError::Other("Redis PING timed out".to_string()))?
}

/// Build a Redis connection URL from a `type = "redis"` data source connection.
///
/// `host`/`port`/`database` (DB index, default `0`)/`username`/`password` map
/// to `redis://[username:password@]host:port/db`.
pub fn redis_url_from_connection(conn: &crate::models::DataSourceConnection) -> Option<String> {
    let mut resolved = conn.clone();
    crate::utils::secrets::resolve_connection_secrets(&mut resolved);
    let conn = &resolved;

    let host = conn.host.as_deref()?.to_string();
    let port = conn.port.unwrap_or(6379);
    let db = conn.database.clone().unwrap_or_else(|| "0".to_string());

    let creds = match (&conn.username, &conn.password) {
        (Some(u), Some(p)) if !u.is_empty() && !p.is_empty() => format!("{}:{}@", u, p),
        (_, Some(p)) if !p.is_empty() => format!(":{}@", p),
        _ => String::new(),
    };
    Some(format!("redis://{}{}:{}/{}", creds, host, port, db))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::DataSourceConnection;

    #[test]
    fn test_redis_conn_is_clone() {
        let a = RedisConn::new("redis://127.0.0.1:6379");
        let b = a.clone();
        assert_eq!(a.url, b.url);
    }

    #[test]
    fn test_redis_url_from_connection_defaults() {
        let conn = DataSourceConnection {
            host: Some("cache.internal".to_string()),
            port: None,
            ..Default::default()
        };
        let url = redis_url_from_connection(&conn).unwrap();
        assert_eq!(url, "redis://cache.internal:6379/0");
    }

    #[test]
    fn test_redis_url_from_connection_full() {
        let conn = DataSourceConnection {
            host: Some("cache.internal".to_string()),
            port: Some(6380),
            database: Some("3".to_string()),
            username: Some("app".to_string()),
            password: Some("secret".to_string()),
            ..Default::default()
        };
        let url = redis_url_from_connection(&conn).unwrap();
        assert_eq!(url, "redis://app:secret@cache.internal:6380/3");
    }

    #[test]
    fn test_redis_url_from_connection_password_only() {
        let conn = DataSourceConnection {
            host: Some("cache.internal".to_string()),
            port: Some(6379),
            password: Some("secret".to_string()),
            ..Default::default()
        };
        let url = redis_url_from_connection(&conn).unwrap();
        assert_eq!(url, "redis://:secret@cache.internal:6379/0");
    }

    #[test]
    fn test_redis_url_from_connection_missing_host() {
        let conn = DataSourceConnection::default();
        assert!(redis_url_from_connection(&conn).is_none());
    }
}
