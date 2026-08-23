//! Session cache abstraction — fast path for session lookups.
//!
//! The metadata store remains the source of truth for sessions; the cache is a
//! write-through fast layer. The local backend keeps sessions in memory with a
//! TTL. (Redis-backed session cache intentionally not implemented — session
//! management stays on simple JWT + metadata store, see `docs/ROADMAP.md`.)

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::config::CacheConfig;
use crate::store::SessionRecord;
use crate::store::StoreError;

/// Session cache abstraction.
#[async_trait]
pub trait SessionCache: Send + Sync {
    /// Look up a session by its JWT id (`jti`).
    async fn get(&self, jti: &str) -> Option<SessionRecord>;
    /// Cache (or refresh) a session.
    async fn set(&self, session: SessionRecord) -> Result<(), StoreError>;
    /// Invalidate a single session.
    async fn remove(&self, jti: &str) -> Result<(), StoreError>;
    /// Invalidate all sessions for a user.
    async fn remove_user(&self, username: &str) -> Result<(), StoreError>;
}

/// A cached session entry with its insertion time (for TTL eviction).
struct Entry {
    session: SessionRecord,
    cached_at: Instant,
}

/// In-memory session cache with a TTL (write-through; the metadata store is
/// the source of truth).
pub struct LocalSessionCache {
    inner: RwLock<HashMap<String, Entry>>,
    ttl: std::time::Duration,
}

impl LocalSessionCache {
    pub fn new(ttl_secs: u64) -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
            ttl: std::time::Duration::from_secs(ttl_secs.max(1)),
        }
    }
}

#[async_trait]
impl SessionCache for LocalSessionCache {
    async fn get(&self, jti: &str) -> Option<SessionRecord> {
        let mut inner = self.inner.write().await;
        match inner.get(jti) {
            Some(e) if e.cached_at.elapsed() <= self.ttl => Some(e.session.clone()),
            Some(_) => {
                // Expired -> evict and treat as a miss.
                inner.remove(jti);
                None
            },
            None => None,
        }
    }

    async fn set(&self, session: SessionRecord) -> Result<(), StoreError> {
        self.inner.write().await.insert(
            session.jti.clone(),
            Entry {
                session,
                cached_at: Instant::now(),
            },
        );
        Ok(())
    }

    async fn remove(&self, jti: &str) -> Result<(), StoreError> {
        self.inner.write().await.remove(jti);
        Ok(())
    }

    async fn remove_user(&self, username: &str) -> Result<(), StoreError> {
        let mut inner = self.inner.write().await;
        inner.retain(|_, e| e.session.username != username);
        Ok(())
    }
}

/// Build the session cache. Only the `local` backend exists (simple JWT mode);
/// session management does not use Redis.
pub fn build_session_cache(config: &CacheConfig) -> Option<Arc<dyn SessionCache>> {
    Some(Arc::new(LocalSessionCache::new(config.session_ttl_secs)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_session(jti: &str, username: &str) -> SessionRecord {
        SessionRecord {
            jti: jti.to_string(),
            username: username.to_string(),
            role: "admin".to_string(),
            issued_at: "2026-01-01T00:00:00Z".to_string(),
            expires_at: "2026-01-02T00:00:00Z".to_string(),
            last_seen_at: "2026-01-01T00:00:00Z".to_string(),
            revoked: false,
            user_agent: None,
            ip_address: None,
        }
    }

    #[actix_rt::test]
    async fn test_set_get_remove() {
        let cache = LocalSessionCache::new(300);
        cache.set(sample_session("jti-1", "alice")).await.unwrap();

        let got = cache.get("jti-1").await;
        assert!(got.is_some(), "已 set 的会话应能 get 到");
        assert_eq!(got.unwrap().username, "alice");

        cache.remove("jti-1").await.unwrap();
        assert!(cache.get("jti-1").await.is_none(), "remove 后应 miss");
    }

    #[actix_rt::test]
    async fn test_miss_returns_none() {
        let cache = LocalSessionCache::new(300);
        assert!(cache.get("unknown-jti").await.is_none());
    }

    #[actix_rt::test]
    async fn test_remove_user_invalidates_all() {
        let cache = LocalSessionCache::new(300);
        cache.set(sample_session("jti-a", "alice")).await.unwrap();
        cache.set(sample_session("jti-b", "alice")).await.unwrap();
        cache.set(sample_session("jti-c", "bob")).await.unwrap();

        cache.remove_user("alice").await.unwrap();

        assert!(cache.get("jti-a").await.is_none());
        assert!(cache.get("jti-b").await.is_none());
        assert!(cache.get("jti-c").await.is_some(), "其他用户的会话应保留");
    }

    #[actix_rt::test]
    async fn test_ttl_expiry() {
        let cache = LocalSessionCache::new(1); // 1 秒 TTL
        cache.set(sample_session("jti-ttl", "carol")).await.unwrap();
        assert!(cache.get("jti-ttl").await.is_some());

        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
        assert!(cache.get("jti-ttl").await.is_none(), "TTL 过期后应 miss");
    }
}
