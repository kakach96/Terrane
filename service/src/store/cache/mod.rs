//! Cache data store abstraction — tile cache + session cache.
//!
//! Backends are selected via [`crate::config::CacheConfig::kind`]:
//! - `local` — tile cache on local disk, session cache in memory (default)
//! - `redis` — shared tile + session cache on Redis (multi-replica deployments)

pub mod redis;
pub mod session;
pub mod tile;

pub use session::{build_session_cache, SessionCache};
pub use tile::{
    build_tile_cache_backend, RedisTileCacheBackend, TileCacheBackend, TileCacheKey, TileCacheStats,
};
