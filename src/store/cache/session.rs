//! Session cache abstraction — fast path for session lookups.
//!
//! The metadata store remains the source of truth for sessions; the cache is a
//! write-through fast layer. The local backend keeps sessions in memory with a
//! TTL. Future backend: Redis (shared across replicas).

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
            }
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

/// Build the session cache selected by [`CacheConfig::kind`].
///
/// Future: `"redis"` -> a Redis-backed [`SessionCache`].
pub fn build_session_cache(config: &CacheConfig) -> Option<Arc<dyn SessionCache>> {
    Some(Arc::new(LocalSessionCache::new(config.session_ttl_secs)))
}
