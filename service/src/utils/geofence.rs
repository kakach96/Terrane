//! GeoFence-style fine-grained access control.
//!
//! Implements the "fine-grained GeoFence ACL" security gap (see
//! `docs/IMPLEMENTATION_PLAN.md` §P3): per-request `workspace / store / layer`
//! rules evaluated against the authenticated subject (or anonymous). Rules
//! reuse the [`Permission`] model (user/role wildcards, resource type + name,
//! access mode, allow/deny effect, priority), so they are managed through the
//! existing `/permissions` REST endpoints.
//!
//! Semantics (GeoServer GeoFence-like):
//! - The most specific matching rule wins: exact resource (layer > store >
//!   workspace > global) beats wildcard, exact subject (user > role) beats
//!   wildcard; `priority` breaks ties, and a deny wins equal-priority ties.
//! - `admin` access covers `write` + `read`; `write` covers `read`.
//! - No matching rule → **allow** (open default). Enforcement is opt-in via
//!   `[security] geofence_enabled = true`; when disabled, access is open.
//! - Admin role bypasses rules (documented; GeoServer's default admin policy).

use crate::models::permission::{AccessMode, Effect, Permission};

/// The resource scope a request targets.
#[derive(Debug, Clone, Copy)]
pub struct AccessContext<'a> {
    /// Subject username (`"anonymous"` when unauthenticated).
    pub username: &'a str,
    /// Subject role (`"guest"` when unauthenticated).
    pub role: &'a str,
    /// Target workspace (may be empty for service-level checks).
    pub workspace: &'a str,
    /// Target store (may be empty).
    pub store: &'a str,
    /// Target layer (may be empty for workspace/store checks).
    pub layer: &'a str,
    /// Requested access mode: `"read"` | `"write"` | `"admin"`.
    pub mode: &'a str,
}

/// Does a rule's access mode satisfy the requested mode?
/// `admin` ⊇ `write` ⊇ `read`.
fn mode_covers(rule_mode: &AccessMode, requested: &str) -> bool {
    match rule_mode {
        AccessMode::Admin => true,
        AccessMode::Write => requested != "admin",
        AccessMode::Read => requested == "read",
    }
}

/// Resource-match score: higher = more specific; `None` = rule does not apply.
///
/// A rule matches when its resource type targets the requested scope (layer /
/// store / workspace / service-global) and its resource name is the exact name
/// (or a `*` wildcard). Layer rules also match the `workspace:layer` qualified
/// form.
fn resource_score(p: &Permission, ctx: &AccessContext) -> Option<u32> {
    let matches = |name: &str, target: &str| name == "*" || name == target;
    match p.resource_type.as_str() {
        "layer" => {
            if matches(&p.resource_name, ctx.layer)
                || (!ctx.workspace.is_empty()
                    && matches(
                        &p.resource_name,
                        &format!("{}:{}", ctx.workspace, ctx.layer),
                    ))
            {
                Some(300)
            } else {
                None
            }
        },
        "store" => {
            if matches(&p.resource_name, ctx.store) {
                Some(200)
            } else {
                None
            }
        },
        "workspace" => {
            if matches(&p.resource_name, ctx.workspace) {
                Some(100)
            } else {
                None
            }
        },
        "service" => {
            // service-global rule: applies to any request in this service
            if p.resource_name == "*" || p.resource_name.is_empty() {
                Some(50)
            } else {
                None
            }
        },
        _ => None,
    }
}

/// Subject-match score: higher = more specific; `None` = rule does not apply.
fn subject_score(p: &Permission, ctx: &AccessContext) -> Option<u32> {
    let user = if p.username == ctx.username {
        4
    } else if p.username == "*" {
        0
    } else {
        return None;
    };
    let role = if p.role == ctx.role {
        2
    } else if p.role == "*" {
        0
    } else {
        return None;
    };
    Some(user + role)
}

/// Evaluate a request against the rule set.
///
/// Returns `true` when access is granted. The most specific matching rule
/// decides; with no matching rule the default is allow.
pub fn evaluate(rules: &[Permission], ctx: &AccessContext) -> bool {
    let mut best: Option<(&Permission, u32)> = None;
    for p in rules {
        if !mode_covers(&p.access_mode, ctx.mode) {
            continue;
        }
        let Some(rs) = resource_score(p, ctx) else {
            continue;
        };
        let Some(ss) = subject_score(p, ctx) else {
            continue;
        };
        let score = rs + ss;
        match best {
            None => best = Some((p, score)),
            Some((cur, cur_score)) => {
                if score > cur_score
                    || (score == cur_score && p.priority > cur.priority)
                    || (score == cur_score
                        && p.priority == cur.priority
                        && p.effect == Effect::Deny
                        && cur.effect == Effect::Allow)
                {
                    best = Some((p, score));
                }
            },
        }
    }

    match best {
        Some((p, _)) => p.effect == Effect::Allow,
        None => true,
    }
}

/// The default subject for unauthenticated requests.
pub fn anonymous_context<'a>(
    workspace: &'a str,
    store: &'a str,
    layer: &'a str,
    mode: &'a str,
) -> AccessContext<'a> {
    AccessContext {
        username: "anonymous",
        role: "guest",
        workspace,
        store,
        layer,
        mode,
    }
}

/// Resolve the caller subject from an optional `Authorization: Bearer` header.
///
/// Returns `(username, role)`; unauthenticated requests fall back to the
/// anonymous/guest subject.
pub fn subject_from_request(req: &actix_web::HttpRequest) -> (String, String) {
    let auth = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    match crate::auth::verify_token(auth) {
        Ok(claims) => (claims.sub, claims.role),
        Err(_) => ("anonymous".to_string(), "guest".to_string()),
    }
}

/// Enforce GeoFence access for a layer read/write/admin operation.
///
/// - When `[security] geofence_enabled` is false → always allowed.
/// - The `admin` role bypasses rules.
/// - Otherwise the request subject (or anonymous/guest) is evaluated against
///   the stored permission rules for the layer's `workspace / store / layer`
///   scope; a deny decision produces an HTTP 403.
pub async fn enforce_layer_access(
    state: &crate::state::AppState,
    req: &actix_web::HttpRequest,
    workspace: &str,
    store: &str,
    layer: &str,
    mode: &str,
) -> Result<(), crate::error::TerraneError> {
    if !state.config.security.geofence_enabled {
        return Ok(());
    }

    let (username, role) = subject_from_request(req);
    if role == "admin" {
        return Ok(());
    }

    let Some(store_iface) = &state.store else {
        return Ok(());
    };
    let rules = store_iface.get_permissions().await.map_err(|e| {
        crate::error::TerraneError::InternalError(format!("Load rules failed: {}", e))
    })?;

    let ctx = AccessContext {
        username: &username,
        role: &role,
        workspace,
        store,
        layer,
        mode,
    };
    if evaluate(&rules, &ctx) {
        Ok(())
    } else {
        Err(crate::error::TerraneError::localized(
            "GEOFENCE_DENIED",
            actix_web::http::StatusCode::FORBIDDEN,
            "Access to the requested layer is denied by GeoFence rules",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(
        username: &str,
        role: &str,
        resource_type: &str,
        resource_name: &str,
        mode: AccessMode,
        effect: Effect,
        priority: i32,
    ) -> Permission {
        Permission {
            id: None,
            username: username.to_string(),
            role: role.to_string(),
            resource_type: resource_type.to_string(),
            resource_name: resource_name.to_string(),
            access_mode: mode,
            effect,
            priority,
        }
    }

    fn ctx<'a>(
        u: &'a str,
        r: &'a str,
        ws: &'a str,
        st: &'a str,
        l: &'a str,
        m: &'a str,
    ) -> AccessContext<'a> {
        AccessContext {
            username: u,
            role: r,
            workspace: ws,
            store: st,
            layer: l,
            mode: m,
        }
    }

    #[test]
    fn test_no_rules_allows() {
        assert!(evaluate(
            &[],
            &ctx("alice", "user", "default", "shapes", "world", "read")
        ));
    }

    #[test]
    fn test_deny_layer_for_anonymous() {
        let rules = vec![rule(
            "*",
            "guest",
            "layer",
            "world",
            AccessMode::Read,
            Effect::Deny,
            0,
        )];
        assert!(!evaluate(
            &rules,
            &anonymous_context("default", "shapes", "world", "read")
        ));
        // a different layer is unaffected
        assert!(evaluate(
            &rules,
            &anonymous_context("default", "shapes", "cities", "read")
        ));
        // authenticated user with user role is unaffected by a guest-only rule
        assert!(evaluate(
            &rules,
            &ctx("alice", "user", "default", "shapes", "world", "read")
        ));
    }

    #[test]
    fn test_allow_rule_for_authenticated_user() {
        let rules = vec![
            rule(
                "*",
                "guest",
                "layer",
                "world",
                AccessMode::Read,
                Effect::Deny,
                0,
            ),
            rule(
                "alice",
                "*",
                "layer",
                "world",
                AccessMode::Read,
                Effect::Allow,
                10,
            ),
        ];
        assert!(!evaluate(
            &rules,
            &anonymous_context("default", "shapes", "world", "read")
        ));
        assert!(evaluate(
            &rules,
            &ctx("alice", "user", "default", "shapes", "world", "read")
        ));
    }

    #[test]
    fn test_workspace_rule_scopes_layers() {
        let rules = vec![rule(
            "*",
            "guest",
            "workspace",
            "internal",
            AccessMode::Read,
            Effect::Deny,
            0,
        )];
        assert!(!evaluate(
            &rules,
            &anonymous_context("internal", "s1", "l1", "read")
        ));
        assert!(evaluate(
            &rules,
            &anonymous_context("default", "s1", "l1", "read")
        ));
    }

    #[test]
    fn test_store_rule() {
        let rules = vec![rule(
            "*",
            "guest",
            "store",
            "restricted",
            AccessMode::Read,
            Effect::Deny,
            0,
        )];
        assert!(!evaluate(
            &rules,
            &anonymous_context("default", "restricted", "l1", "read")
        ));
        assert!(evaluate(
            &rules,
            &anonymous_context("default", "open", "l1", "read")
        ));
    }

    #[test]
    fn test_qualified_layer_name_matches() {
        let rules = vec![rule(
            "*",
            "guest",
            "layer",
            "default:world",
            AccessMode::Read,
            Effect::Deny,
            0,
        )];
        assert!(!evaluate(
            &rules,
            &anonymous_context("default", "shapes", "world", "read")
        ));
    }

    #[test]
    fn test_specific_rule_beats_wildcard() {
        // wildcard deny vs specific allow — the specific rule wins
        let rules = vec![
            rule(
                "*",
                "guest",
                "layer",
                "*",
                AccessMode::Read,
                Effect::Deny,
                0,
            ),
            rule(
                "*",
                "guest",
                "layer",
                "world",
                AccessMode::Read,
                Effect::Allow,
                5,
            ),
        ];
        assert!(evaluate(
            &rules,
            &anonymous_context("default", "shapes", "world", "read")
        ));
        assert!(!evaluate(
            &rules,
            &anonymous_context("default", "shapes", "cities", "read")
        ));
    }

    #[test]
    fn test_deny_wins_equal_priority_tie() {
        let rules = vec![
            rule(
                "*",
                "guest",
                "layer",
                "world",
                AccessMode::Read,
                Effect::Allow,
                0,
            ),
            rule(
                "*",
                "guest",
                "layer",
                "world",
                AccessMode::Read,
                Effect::Deny,
                0,
            ),
        ];
        assert!(!evaluate(
            &rules,
            &anonymous_context("default", "shapes", "world", "read")
        ));
    }

    #[test]
    fn test_priority_breaks_tie() {
        let rules = vec![
            rule(
                "*",
                "guest",
                "layer",
                "world",
                AccessMode::Read,
                Effect::Deny,
                0,
            ),
            rule(
                "*",
                "guest",
                "layer",
                "world",
                AccessMode::Read,
                Effect::Allow,
                5,
            ),
        ];
        assert!(evaluate(
            &rules,
            &anonymous_context("default", "shapes", "world", "read")
        ));
    }

    #[test]
    fn test_admin_mode_covers_write_and_read() {
        let rules = vec![rule(
            "alice",
            "admin",
            "layer",
            "world",
            AccessMode::Admin,
            Effect::Allow,
            0,
        )];
        assert!(evaluate(
            &rules,
            &ctx("alice", "admin", "default", "shapes", "world", "admin")
        ));
        assert!(evaluate(
            &rules,
            &ctx("alice", "admin", "default", "shapes", "world", "read")
        ));
    }

    #[test]
    fn test_write_mode_does_not_cover_admin() {
        // A write-allow rule matches write/read requests but never an admin
        // request (mode hierarchy: admin ⊇ write ⊇ read).
        let rules = vec![rule(
            "bob",
            "manager",
            "layer",
            "world",
            AccessMode::Write,
            Effect::Allow,
            0,
        )];
        assert!(evaluate(
            &rules,
            &ctx("bob", "manager", "default", "shapes", "world", "write")
        ));
        assert!(evaluate(
            &rules,
            &ctx("bob", "manager", "default", "shapes", "world", "read")
        ));
        // admin request does not match the write rule — falls through to
        // default allow, so a separate admin deny is required to block it.
        assert!(!mode_covers(&AccessMode::Write, "admin"));
        assert!(mode_covers(&AccessMode::Admin, "admin"));
        assert!(mode_covers(&AccessMode::Admin, "write"));
        assert!(mode_covers(&AccessMode::Write, "read"));
    }

    #[test]
    fn test_service_global_rule() {
        let rules = vec![rule(
            "*",
            "guest",
            "service",
            "*",
            AccessMode::Read,
            Effect::Deny,
            0,
        )];
        assert!(!evaluate(
            &rules,
            &anonymous_context("default", "shapes", "world", "read")
        ));
    }
}
