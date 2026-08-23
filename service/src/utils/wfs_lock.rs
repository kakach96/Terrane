//! WFS feature-lock registry (in-process, in-memory).
//!
//! Implements the locking semantics behind the WFS `LockFeature` and
//! `GetFeatureWithLock` operations (WFS 1.1.0 / 2.0.0). WFS-T writes are not
//! implemented yet, so locks are purely coordination state (no persistence,
//! no writes): they guard concurrent consumers against editing the same
//! feature, and are lost on restart.
//!
//! Locks are keyed per layer and per feature id. Each lock records the
//! owning `lockId` (an opaque token returned to the client) and an optional
//! expiry. Expired locks are pruned lazily on access — no background task.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use uuid::Uuid;

/// A single locked feature.
#[derive(Debug, Clone)]
pub struct FeatureLock {
    /// Opaque token identifying the lock owner (the WFS `lockId`).
    pub lock_id: String,
    /// Absolute expiry instant; `None` = never expires.
    pub expires_at: Option<Instant>,
}

impl FeatureLock {
    fn is_expired(&self, now: Instant) -> bool {
        self.expires_at.map(|e| e <= now).unwrap_or(false)
    }
}

/// WFS lock registry: `layer -> (feature_id -> lock)`.
#[derive(Debug, Clone, Default)]
pub struct WfsLockRegistry {
    inner: Arc<RwLock<HashMap<String, HashMap<String, FeatureLock>>>>,
}

impl WfsLockRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Drop expired locks for `layer` (lazy prune on access).
    fn prune(&self, layer: &str, now: Instant) {
        if let Ok(mut map) = self.inner.write() {
            if let Some(locks) = map.get_mut(layer) {
                locks.retain(|_, l| !l.is_expired(now));
            }
        }
    }

    /// Try to lock `fids` of `layer`.
    ///
    /// - `lock_all` (WFS `lockAction=ALL`, the default): any already-locked
    ///   feature aborts the whole request and `Err(conflicts)` is returned.
    /// - `lock_all = false` (`lockAction=SOME`): only the free features are
    ///   locked; the locked ones are reported via the skipped list.
    ///
    /// On success returns `(lock_id, locked_fids, skipped_fids)` where the
    /// skipped list is non-empty only under `lockAction=SOME` or when
    /// `fids` was empty.
    pub fn acquire(
        &self,
        layer: &str,
        fids: &[String],
        ttl: Option<Duration>,
        lock_all: bool,
    ) -> Result<(String, Vec<String>, Vec<String>), Vec<String>> {
        let now = Instant::now();
        self.prune(layer, now);

        let mut map = self.inner.write().unwrap();
        let locks = map.entry(layer.to_string()).or_default();

        // Determine which features are already locked by someone else.
        let conflicts: Vec<String> = fids
            .iter()
            .filter(|fid| locks.get(*fid).map(|l| !l.is_expired(now)).unwrap_or(false))
            .cloned()
            .collect();

        if lock_all && !conflicts.is_empty() {
            return Err(conflicts);
        }

        let lock_id = Uuid::new_v4().to_string();
        let expires_at = ttl.map(|d| now + d);
        let mut locked = Vec::new();
        let mut skipped = Vec::new();
        for fid in fids {
            if locks.get(fid).map(|l| !l.is_expired(now)).unwrap_or(false) {
                skipped.push(fid.clone());
                continue;
            }
            locks.insert(
                fid.clone(),
                FeatureLock {
                    lock_id: lock_id.clone(),
                    expires_at,
                },
            );
            locked.push(fid.clone());
        }
        Ok((lock_id, locked, skipped))
    }

    /// Extend the expiry of every lock owned by `lock_id` on `layer`.
    /// Returns `false` if the lock id holds nothing on that layer.
    pub fn renew(&self, layer: &str, lock_id: &str, ttl: Option<Duration>) -> bool {
        let now = Instant::now();
        self.prune(layer, now);
        let mut map = self.inner.write().unwrap();
        let renewed = match map.get_mut(layer) {
            Some(locks) => {
                let mut any = false;
                for lock in locks.values_mut() {
                    if lock.lock_id == lock_id {
                        lock.expires_at = ttl.map(|d| now + d);
                        any = true;
                    }
                }
                any
            },
            None => false,
        };
        renewed
    }

    /// Release every lock owned by `lock_id` on `layer`; returns the
    /// released feature ids.
    pub fn release(&self, layer: &str, lock_id: &str) -> Vec<String> {
        let mut map = self.inner.write().unwrap();
        let mut released = Vec::new();
        if let Some(locks) = map.get_mut(layer) {
            locks.retain(|fid, lock| {
                if lock.lock_id == lock_id {
                    released.push(fid.clone());
                    false
                } else {
                    true
                }
            });
        }
        released
    }

    /// Feature ids of `layer` currently locked by `lock_id`.
    pub fn locked_features(&self, layer: &str, lock_id: &str) -> Vec<String> {
        let now = Instant::now();
        self.prune(layer, now);
        let map = self.inner.read().unwrap();
        match map.get(layer) {
            Some(locks) => locks
                .iter()
                .filter(|(_, l)| l.lock_id == lock_id && !l.is_expired(now))
                .map(|(fid, _)| fid.clone())
                .collect(),
            None => Vec::new(),
        }
    }

    /// Whether `fid` on `layer` is currently locked (by anyone).
    pub fn is_locked(&self, layer: &str, fid: &str) -> bool {
        let now = Instant::now();
        self.prune(layer, now);
        let map = self.inner.read().unwrap();
        map.get(layer)
            .and_then(|locks| locks.get(fid))
            .map(|l| !l.is_expired(now))
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ttl(secs: u64) -> Option<Duration> {
        Some(Duration::from_secs(secs))
    }

    #[test]
    fn test_acquire_and_owned_lock() {
        let reg = WfsLockRegistry::new();
        let (lock_id, locked, skipped) = reg
            .acquire("world", &["f1".into(), "f2".into()], ttl(60), true)
            .expect("first acquire should succeed");
        assert_eq!(locked, vec!["f1".to_string(), "f2".to_string()]);
        assert!(skipped.is_empty());
        assert!(reg.is_locked("world", "f1"));
        assert!(reg.is_locked("world", "f2"));
        assert_eq!(reg.locked_features("world", &lock_id).len(), 2);
    }

    #[test]
    fn test_lock_all_conflict_aborts() {
        let reg = WfsLockRegistry::new();
        reg.acquire("world", &["f1".into()], ttl(60), true).unwrap();
        let err = reg
            .acquire("world", &["f1".into(), "f2".into()], ttl(60), true)
            .expect_err("lockAction=ALL must abort on conflict");
        assert_eq!(err, vec!["f1".to_string()]);
        // Nothing was locked by the failed request.
        assert!(!reg.is_locked("world", "f2"));
    }

    #[test]
    fn test_lock_some_skips_conflicts() {
        let reg = WfsLockRegistry::new();
        reg.acquire("world", &["f1".into()], ttl(60), true).unwrap();
        let (lock_id, locked, skipped) = reg
            .acquire("world", &["f1".into(), "f2".into()], ttl(60), false)
            .expect("lockAction=SOME must not fail");
        assert_eq!(locked, vec!["f2".to_string()]);
        assert_eq!(skipped, vec!["f1".to_string()]);
        assert!(reg.is_locked("world", "f2"));
        assert_eq!(
            reg.locked_features("world", &lock_id),
            vec!["f2".to_string()]
        );
    }

    #[test]
    fn test_expiry_auto_release() {
        let reg = WfsLockRegistry::new();
        reg.acquire("world", &["f1".into()], ttl(60), true).unwrap();
        assert!(reg.is_locked("world", "f1"));
        // Acquire a lock with an already-elapsed TTL; it must auto-expire.
        let elapsed = Some(Duration::from_millis(1));
        reg.acquire("world", &["f2".into()], elapsed, true).unwrap();
        std::thread::sleep(Duration::from_millis(5));
        assert!(!reg.is_locked("world", "f2"));
        assert!(reg.is_locked("world", "f1"), "f1 (60s TTL) still held");
        // Once expired, the same feature can be re-acquired.
        reg.acquire("world", &["f2".into()], elapsed, true).unwrap();
        std::thread::sleep(Duration::from_millis(5));
        assert!(!reg.is_locked("world", "f2"));
    }

    #[test]
    fn test_never_expires_when_ttl_none() {
        let reg = WfsLockRegistry::new();
        reg.acquire("world", &["f1".into()], None, true).unwrap();
        assert!(reg.is_locked("world", "f1"));
        reg.renew("world", "nonexistent", None);
        assert!(reg.is_locked("world", "f1"), "ttl=None must never expire");
    }

    #[test]
    fn test_renew_and_release() {
        let reg = WfsLockRegistry::new();
        let (lock_id, locked, _) = reg.acquire("world", &["f1".into()], ttl(60), true).unwrap();
        assert_eq!(locked.len(), 1);
        assert!(reg.renew("world", &lock_id, ttl(120)));
        assert!(!reg.renew("world", "unknown-lock", ttl(120)));

        let released = reg.release("world", &lock_id);
        assert_eq!(released, vec!["f1".to_string()]);
        assert!(!reg.is_locked("world", "f1"));
        // Releasing again is a no-op.
        assert!(reg.release("world", &lock_id).is_empty());
    }

    #[test]
    fn test_locks_are_per_layer() {
        let reg = WfsLockRegistry::new();
        reg.acquire("world", &["f1".into()], ttl(60), true).unwrap();
        // The same feature id on another layer is independent.
        reg.acquire("cities", &["f1".into()], ttl(60), true)
            .expect("different layer must not conflict");
        assert!(reg.is_locked("world", "f1"));
        assert!(reg.is_locked("cities", "f1"));
    }
}
