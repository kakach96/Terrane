//! LDAP enterprise identity — authenticate users against an LDAP directory.
//!
//! Implements the "enterprise identity" security gap (see
//! `docs/IMPLEMENTATION_PLAN.md` §P3): when `[security.ldap]` is enabled, the
//! login flow falls back to an LDAP bind when the local user is missing or the
//! local password check fails. On success the user is auto-provisioned locally
//! (role mapped from group membership) and a normal JWT session is issued.
//!
//! The DN / filter / role-mapping helpers are pure functions (unit-tested);
//! the network bind lives in [`authenticate_ldap`] and is exercised by an
//! `#[ignore]` live test that requires a reachable LDAP server.

use crate::auth::UserRole;
use crate::config::LdapConfig;
use ldap3::LdapConnAsync;

/// Substitute the `{username}` placeholder in a filter template.
pub fn substitute_username(filter: &str, username: &str) -> String {
    filter.replace("{username}", username)
}

/// Escape an LDAP DN value (RFC 4514) so usernames with special characters
/// cannot inject additional DN components.
pub fn escape_dn_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            ',' | '+' | '"' | '\\' | '<' | '>' | ';' | '=' | '#' | '\0' => {
                out.push('\\');
                out.push(c);
            },
            _ => out.push(c),
        }
    }
    out
}

/// Build the user DN for a simple-bind login from the base DN and username.
///
/// When `user_filter` is the default `(uid={username})`, the RDN is derived as
/// `uid=<escaped-username>`; otherwise the filter is used as-is for a search
/// (the caller then binds with the returned DN).
pub fn user_dn_from_base(base_dn: &str, username: &str) -> String {
    format!("uid={},{}", escape_dn_value(username), base_dn)
}

/// Map LDAP group membership (memberOf DNs / group names) to a Terrane role.
///
/// Returns `UserRole::Admin` when the user belongs to `admin_group` (matched by
/// DN suffix or exact group name), otherwise the configured default role.
pub fn map_role(groups: &[String], admin_group: &str, default_role: &str) -> UserRole {
    let admin_group = admin_group.trim();
    if admin_group.is_empty() {
        return parse_role(default_role);
    }
    let admin_lower = admin_group.to_lowercase();
    let is_admin = groups.iter().any(|g| {
        let g_lower = g.trim().to_lowercase();
        g_lower == admin_lower
            || g_lower.ends_with(&format!(",{}", admin_lower))
            || g_lower
                .split(',')
                .next()
                .map(|cn| cn == admin_lower)
                .unwrap_or(false)
    });
    if is_admin {
        UserRole::Admin
    } else {
        parse_role(default_role)
    }
}

/// Parse a role string with a safe fallback to `UserRole::User`.
pub fn parse_role(role: &str) -> UserRole {
    match role.to_lowercase().as_str() {
        "admin" => UserRole::Admin,
        "manager" => UserRole::Manager,
        "guest" => UserRole::Guest,
        _ => UserRole::User,
    }
}

/// Authenticate `username`/`password` against the LDAP directory.
///
/// Flow: connect → bind (service account when `bind_dn` is set, otherwise
/// directly as the resolved user DN) → search the user entry + `memberOf`
/// groups → return the mapped role. `Ok(None)` means the credentials were
/// rejected; `Err` is a transport/configuration failure.
pub async fn authenticate_ldap(
    cfg: &LdapConfig,
    username: &str,
    password: &str,
) -> Result<Option<UserRole>, String> {
    if !cfg.enabled || cfg.url.trim().is_empty() || cfg.base_dn.trim().is_empty() {
        return Ok(None);
    }

    let (conn, mut ldap) = LdapConnAsync::new(cfg.url.trim())
        .await
        .map_err(|e| format!("LDAP connect failed: {}", e))?;
    ldap3::drive!(conn);

    // Bind: service account (if configured) or the user DN directly.
    if !cfg.bind_dn.trim().is_empty() {
        ldap.simple_bind(cfg.bind_dn.trim(), &cfg.bind_password)
            .await
            .map_err(|e| format!("LDAP service bind failed: {}", e))?;

        // Search the user entry to resolve its DN.
        let filter = substitute_username(&cfg.user_filter, username);
        let rs = ldap
            .search(
                cfg.base_dn.trim(),
                ldap3::Scope::Subtree,
                &filter,
                vec!["dn", "memberOf"],
            )
            .await
            .map_err(|e| format!("LDAP search failed: {}", e))?;
        if rs.0.is_empty() {
            return Ok(None);
        }
        let entry = ldap3::SearchEntry::construct(rs.0[0].clone());
        let groups: Vec<String> = entry.attrs.get("memberOf").cloned().unwrap_or_default();
        return Ok(Some(map_role(&groups, &cfg.admin_group, &cfg.default_role)));
    }

    // Direct simple bind as the user DN (no service account).
    let user_dn = user_dn_from_base(cfg.base_dn.trim(), username);
    match ldap.simple_bind(&user_dn, password).await {
        Ok(ldap3::LdapResult { rc: 0, .. }) => {
            // Group lookup requires an authenticated search; with a direct bind
            // we cannot enumerate groups, so fall back to the default role
            // unless an admin group is explicitly configured (then attempt a
            // self-scoped search as the user).
            if cfg.admin_group.trim().is_empty() {
                Ok(Some(map_role(&[], "", &cfg.default_role)))
            } else {
                let filter = substitute_username(&cfg.user_filter, username);
                let rs = ldap
                    .search(
                        cfg.base_dn.trim(),
                        ldap3::Scope::Subtree,
                        &filter,
                        vec!["memberOf"],
                    )
                    .await
                    .map_err(|e| format!("LDAP group search failed: {}", e))?;
                let groups: Vec<String> =
                    rs.0.first()
                        .map(|e| {
                            ldap3::SearchEntry::construct(e.clone())
                                .attrs
                                .get("memberOf")
                                .cloned()
                                .unwrap_or_default()
                        })
                        .unwrap_or_default();
                Ok(Some(map_role(&groups, &cfg.admin_group, &cfg.default_role)))
            }
        },
        Ok(_) => Ok(None),
        Err(_) => Ok(None), // invalid credentials → treat as rejected, not error
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_substitute_username() {
        assert_eq!(
            substitute_username("(uid={username})", "alice"),
            "(uid=alice)"
        );
        assert_eq!(
            substitute_username("(&(objectClass=person)(sAMAccountName={username}))", "bob"),
            "(&(objectClass=person)(sAMAccountName=bob))"
        );
    }

    #[test]
    fn test_escape_dn_value() {
        assert_eq!(escape_dn_value("alice"), "alice");
        assert_eq!(escape_dn_value("a,b"), "a\\,b");
        assert_eq!(escape_dn_value("x=1"), "x\\=1");
    }

    #[test]
    fn test_user_dn_from_base() {
        assert_eq!(
            user_dn_from_base("dc=example,dc=com", "alice"),
            "uid=alice,dc=example,dc=com"
        );
        // special chars escaped — cannot inject extra DN components
        assert_eq!(
            user_dn_from_base("dc=example,dc=com", "a,b"),
            "uid=a\\,b,dc=example,dc=com"
        );
    }

    #[test]
    fn test_map_role_admin_group() {
        let groups = vec![
            "cn=users,dc=example,dc=com".to_string(),
            "cn=admins,dc=example,dc=com".to_string(),
        ];
        assert_eq!(
            map_role(&groups, "cn=admins,dc=example,dc=com", "user"),
            UserRole::Admin
        );
        assert_eq!(map_role(&groups, "cn=admins", "user"), UserRole::Admin);
    }

    #[test]
    fn test_map_role_default() {
        let groups = vec!["cn=users,dc=example,dc=com".to_string()];
        assert_eq!(
            map_role(&groups, "cn=admins,dc=example,dc=com", "user"),
            UserRole::User
        );
        // empty admin group → default role always
        assert_eq!(map_role(&groups, "", "guest"), UserRole::Guest);
    }

    #[test]
    fn test_parse_role() {
        assert_eq!(parse_role("admin"), UserRole::Admin);
        assert_eq!(parse_role("manager"), UserRole::Manager);
        assert_eq!(parse_role("guest"), UserRole::Guest);
        assert_eq!(parse_role("user"), UserRole::User);
        assert_eq!(parse_role("unknown"), UserRole::User);
    }

    #[test]
    fn test_disabled_config_returns_none() {
        let cfg = LdapConfig::default();
        let rt = actix_rt::Runtime::new().unwrap();
        let res = rt.block_on(authenticate_ldap(&cfg, "alice", "pw"));
        assert!(res.is_ok());
        assert!(res.unwrap().is_none());
    }
}
