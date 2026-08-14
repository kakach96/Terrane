//! Resilience middleware for Terrane.
//!
//! Cloud-native hardening (see `docs/IMPLEMENTATION_PLAN.md` §6.1 "Resilience"):
//! - **Rate limiting** — a sliding-window per-client limiter that rejects
//!   over-limit requests with HTTP 429 (Too Many Requests). Keyed by client IP
//!   (falling back to `X-Forwarded-For`, then a shared `unknown` bucket for
//!   requests without a peer address, e.g. in tests). `max_requests == 0`
//!   disables the limiter (every request passes).
//! - **Request timeout** — cancels requests that exceed a configured deadline
//!   and answers HTTP 504 (Gateway Timeout), so a slow handler can never hold a
//!   worker slot forever. A zero duration disables the timeout.
//!
//! Both are opt-in via `[server]` config (`rate_limit_max_requests`,
//! `rate_limit_window_secs`, `request_timeout_secs`) and are applied in
//! `main.rs` around the whole app (static files included).
//!
//! The middlewares follow actix-web's own middleware pattern (`EitherBody`):
//! the inner service keeps its generic body `B` (the `Left` variant) while the
//! rejection / timeout responses use the default `Right` `BoxBody` variant, so
//! both branches share one response type.

use actix_web::body::EitherBody;
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::{Error, HttpResponse};
use std::collections::HashMap;
use std::future::{ready, Ready};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Sliding-window rate limiter keyed by client identifier.
///
/// The window is bucketed per whole second; each bucket holds the request count
/// of that second. `allow` prunes buckets older than the window, sums the rest
/// and increments the current bucket. Buckets for clients that stop requesting
/// are pruned lazily on their next request, so memory stays bounded by the
/// number of *active* clients.
#[derive(Debug)]
pub struct RateLimiter {
    max_requests: u64,
    window_secs: u64,
    clients: Mutex<HashMap<String, HashMap<u64, u64>>>,
}

impl RateLimiter {
    pub fn new(max_requests: u64, window_secs: u64) -> Self {
        RateLimiter {
            max_requests,
            window_secs: window_secs.max(1),
            clients: Mutex::new(HashMap::new()),
        }
    }

    /// Record one request for `key`. Returns `true` when the request is inside
    /// the limit (and was counted), `false` when it must be rejected. A
    /// `max_requests` of 0 disables the limiter entirely.
    pub fn allow(&self, key: &str) -> bool {
        if self.max_requests == 0 {
            return true;
        }
        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let window_start = now_secs.saturating_sub(self.window_secs.saturating_sub(1));

        let mut clients = self.clients.lock().unwrap();
        let buckets = clients.entry(key.to_string()).or_default();
        buckets.retain(|&sec, _| sec >= window_start);
        let count: u64 = buckets.values().sum();
        if count >= self.max_requests {
            return false;
        }
        *buckets.entry(now_secs).or_insert(0) += 1;
        true
    }

    /// Current request count for `key` within the window (used by tests).
    pub fn count(&self, key: &str) -> u64 {
        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let window_start = now_secs.saturating_sub(self.window_secs.saturating_sub(1));
        let clients = self.clients.lock().unwrap();
        clients
            .get(key)
            .map(|b| {
                b.iter()
                    .filter(|(&sec, _)| sec >= window_start)
                    .map(|(_, c)| c)
                    .sum()
            })
            .unwrap_or(0)
    }
}

/// Middleware factory: rejects over-limit requests with HTTP 429.
#[derive(Debug, Clone)]
pub struct RateLimit {
    limiter: Arc<RateLimiter>,
}

impl RateLimit {
    pub fn new(max_requests: u64, window_secs: u64) -> Self {
        RateLimit {
            limiter: Arc::new(RateLimiter::new(max_requests, window_secs)),
        }
    }
}

impl<S, B> Transform<S, ServiceRequest> for RateLimit
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Transform = RateLimitMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(RateLimitMiddleware {
            service,
            limiter: self.limiter.clone(),
        }))
    }
}

/// Resolve the client identifier for rate limiting.
fn client_key(req: &ServiceRequest) -> String {
    if let Some(addr) = req.peer_addr() {
        return addr.ip().to_string();
    }
    req.headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or("").trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

pub struct RateLimitMiddleware<S> {
    service: S,
    limiter: Arc<RateLimiter>,
}

impl<S, B> Service<ServiceRequest> for RateLimitMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future =
        Pin<Box<dyn std::future::Future<Output = Result<ServiceResponse<EitherBody<B>>, Error>>>>;

    fn poll_ready(&self, ctx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(ctx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        if self.limiter.allow(&client_key(&req)) {
            let fut = self.service.call(req);
            Box::pin(async move { Ok(fut.await?.map_into_left_body()) })
        } else {
            tracing::warn!("Rate limit exceeded for client '{}'", client_key(&req));
            let resp = HttpResponse::TooManyRequests().json(serde_json::json!({
                "code": "RATE_LIMITED",
                "description": "Too many requests",
            }));
            Box::pin(async move { Ok(req.into_response(resp).map_into_right_body()) })
        }
    }
}

/// Middleware factory: cancels requests exceeding the deadline with HTTP 504.
/// A zero duration disables the timeout (plain pass-through).
#[derive(Debug, Clone, Copy)]
pub struct RequestTimeout {
    timeout: Duration,
}

impl RequestTimeout {
    pub fn new(timeout: Duration) -> Self {
        RequestTimeout { timeout }
    }
}

impl<S, B> Transform<S, ServiceRequest> for RequestTimeout
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Transform = RequestTimeoutMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(RequestTimeoutMiddleware {
            service,
            timeout: self.timeout,
        }))
    }
}

pub struct RequestTimeoutMiddleware<S> {
    service: S,
    timeout: Duration,
}

impl<S, B> Service<ServiceRequest> for RequestTimeoutMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future =
        Pin<Box<dyn std::future::Future<Output = Result<ServiceResponse<EitherBody<B>>, Error>>>>;

    fn poll_ready(&self, ctx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(ctx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let timeout = self.timeout;

        // Disabled: plain pass-through.
        if timeout.is_zero() {
            let fut = self.service.call(req);
            return Box::pin(async move { Ok(fut.await?.map_into_left_body()) });
        }

        // `ServiceRequest` is not `Clone` (and cloning its `HttpRequest` head
        // would break the route match-info), so the 504 is returned as an
        // error response instead of rebuilding a `ServiceResponse`.
        let fut = self.service.call(req);
        Box::pin(async move {
            match tokio::time::timeout(timeout, fut).await {
                Ok(res) => Ok(res?.map_into_left_body()),
                Err(_) => {
                    tracing::warn!("Request timed out after {:?}", timeout);
                    let resp = HttpResponse::GatewayTimeout().json(serde_json::json!({
                        "code": "REQUEST_TIMEOUT",
                        "description": format!("Request exceeded the {:?} deadline", timeout),
                    }));
                    Err(
                        actix_web::error::InternalError::from_response("request timed out", resp)
                            .into(),
                    )
                },
            }
        })
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration as StdDuration;

    #[test]
    fn test_allow_within_limit() {
        let limiter = RateLimiter::new(3, 60);
        assert!(limiter.allow("a"));
        assert!(limiter.allow("a"));
        assert!(limiter.allow("a"));
        assert!(!limiter.allow("a"), "第 4 个请求应被拒绝");
        assert_eq!(limiter.count("a"), 3);
    }

    #[test]
    fn test_disabled_limiter_always_allows() {
        let limiter = RateLimiter::new(0, 60);
        for _ in 0..10 {
            assert!(limiter.allow("a"));
        }
        assert_eq!(limiter.count("a"), 0, "禁用时不计数");
    }

    #[test]
    fn test_clients_are_independent() {
        let limiter = RateLimiter::new(1, 60);
        assert!(limiter.allow("a"));
        assert!(!limiter.allow("a"));
        assert!(limiter.allow("b"), "不同客户端应独立计数");
    }

    #[test]
    fn test_window_slides() {
        // 窗口 1 秒, 每窗口 1 个请求
        let limiter = RateLimiter::new(1, 1);
        assert!(limiter.allow("a"));
        assert!(!limiter.allow("a"), "窗口内第 2 个请求应被拒绝");
        thread::sleep(StdDuration::from_millis(1100));
        assert!(limiter.allow("a"), "窗口滑动后应恢复");
    }

    #[test]
    fn test_count_after_prune() {
        let limiter = RateLimiter::new(5, 1);
        assert!(limiter.allow("a"));
        thread::sleep(StdDuration::from_millis(1100));
        // 旧桶已被 prune, count 回到 0
        assert_eq!(limiter.count("a"), 0);
        assert!(limiter.allow("a"));
    }
}
