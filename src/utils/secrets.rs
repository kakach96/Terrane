//! Credential management helpers — data source secrets via environment
//! interpolation (K8s Secrets style) and redaction (never logged).
//!
//! Implements the "credential management" cloud-native gap (see
//! `docs/IMPLEMENTATION_PLAN.md` §6.2 Phase 3): data source passwords /
//! secret keys are *not* stored in plaintext configs when avoidable — a
//! `${ENV_VAR}` reference is persisted instead and resolved from the process
//! environment at connection-build time, so secrets live in K8s Secrets /
//! injected env vars rather than in `terrane.toml` or the metadata store.
//!
//! [`redact`] masks secret-looking values so connection strings can be logged
//! safely (diagnostics must never echo passwords or keys).

use crate::models::DataSourceConnection;

/// Expand `${ENV_VAR}` references in a value from the process environment.
///
/// - `${VAR}` → value of `VAR` (empty when unset).
/// - A bare value without `${...}` is returned unchanged.
/// - Unknown variables expand to the empty string (fail-open, so a missing
///   secret surfaces as an empty credential at connect time, never a panic).
///
/// This is intentionally strict about the `${VAR}` form (no `$VAR` shorthand)
/// so ordinary values containing `$` are not accidentally rewritten.
pub fn resolve_secret(value: &str) -> String {
    if !value.contains("${") {
        return value.to_string();
    }
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        if let Some(end) = after.find('}') {
            let var = &after[..end];
            let resolved = std::env::var(var).unwrap_or_default();
            out.push_str(&resolved);
            rest = &after[end + 1..];
        } else {
            // Unterminated `${` — keep the literal text.
            out.push_str("${");
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

/// Resolve any `${ENV_VAR}` references in a connection's secret fields.
///
/// Mutates the connection in place; non-secret fields are left untouched.
/// Callers should resolve *before* building pools/clients so secrets never
/// round-trip through the metadata store as plaintext.
pub fn resolve_connection_secrets(conn: &mut DataSourceConnection) {
    if let Some(pw) = &conn.password {
        if pw.contains("${") {
            conn.password = Some(resolve_secret(pw));
        }
    }
    for field in [&mut conn.s3_access_key, &mut conn.s3_secret_key] {
        if let Some(v) = field {
            if v.contains("${") {
                *field = Some(resolve_secret(v));
            }
        }
    }
}

/// Mask a secret value for logs: keep the first 3 chars, replace the rest.
/// Empty / short secrets become `***`.
pub fn redact(secret: &str) -> String {
    let visible = secret.chars().take(3).collect::<String>();
    if visible.is_empty() {
        "***".to_string()
    } else {
        format!("{}***", visible)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_placeholder_unchanged() {
        assert_eq!(resolve_secret("plain-password"), "plain-password");
        assert_eq!(resolve_secret(""), "");
    }

    #[test]
    fn test_expand_env() {
        std::env::set_var("TERRANE_TEST_SECRET_A", "s3cr3t");
        assert_eq!(resolve_secret("${TERRANE_TEST_SECRET_A}"), "s3cr3t");
        assert_eq!(
            resolve_secret("pre-${TERRANE_TEST_SECRET_A}-post"),
            "pre-s3cr3t-post"
        );
        std::env::remove_var("TERRANE_TEST_SECRET_A");
    }

    #[test]
    fn test_missing_env_expands_empty() {
        std::env::remove_var("TERRANE_TEST_SECRET_MISSING");
        assert_eq!(resolve_secret("${TERRANE_TEST_SECRET_MISSING}"), "");
        assert_eq!(resolve_secret("a-${TERRANE_TEST_SECRET_MISSING}-b"), "a--b");
    }

    #[test]
    fn test_unterminated_placeholder_kept() {
        assert_eq!(resolve_secret("${VAR"), "${VAR");
    }

    #[test]
    fn test_resolve_connection_secrets() {
        std::env::set_var("TERRANE_TEST_DB_PW", "db-pass");
        let mut conn = DataSourceConnection {
            password: Some("${TERRANE_TEST_DB_PW}".to_string()),
            s3_secret_key: Some("${TERRANE_TEST_DB_PW}".to_string()),
            ..Default::default()
        };
        resolve_connection_secrets(&mut conn);
        assert_eq!(conn.password.as_deref(), Some("db-pass"));
        assert_eq!(conn.s3_secret_key.as_deref(), Some("db-pass"));
        std::env::remove_var("TERRANE_TEST_DB_PW");
    }

    #[test]
    fn test_redact() {
        assert_eq!(redact("super-secret-password"), "sup***");
        assert_eq!(redact(""), "***");
        assert_eq!(redact("ab"), "ab***");
    }
}
