//! Lightweight server-side localization for user-facing REST messages.
//!
//! The frontend is the primary localization authority (it maps the stable
//! `code` returned in error responses to its own ngx-translate catalog).
//! This module lets the backend additionally render localized `message`
//! text for a small set of purely user-facing messages (auth/login), driven
//! by the request `Accept-Language` header. Unknown keys fall back to the
//! key itself.

use actix_web::http::header::HeaderMap;

/// Supported server locales.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    /// Chinese (zh-CN).
    Zh,
    /// English (en-US).
    En,
}

/// Parse the preferred locale from the `Accept-Language` header.
/// Any value starting with `zh` maps to Chinese; everything else defaults
/// to English.
pub fn from_accept_language(headers: &HeaderMap) -> Lang {
    let value = headers
        .get(actix_web::http::header::ACCEPT_LANGUAGE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if value.to_ascii_lowercase().starts_with("zh") {
        Lang::Zh
    } else {
        Lang::En
    }
}

/// Translate a message key with positional args (`{}` placeholders) into the
/// given locale. Falls back to returning `key` when unknown.
pub fn tr(lang: Lang, key: &str, args: &[&str]) -> String {
    let template = match (lang, key) {
        // Login / auth
        (Lang::Zh, "login.invalid_credentials") => "用户名或密码错误",
        (Lang::Zh, "login.disabled") => "账号已被禁用",
        (Lang::Zh, "login.db_unavailable") => "数据库不可用",
        (Lang::Zh, "login.failed") => "登录失败",
        (Lang::Zh, "login.success") => "登录成功",
        (Lang::Zh, "logout.success") => "已退出登录",
        (Lang::En, "login.invalid_credentials") => "Invalid username or password",
        (Lang::En, "login.disabled") => "Account disabled",
        (Lang::En, "login.db_unavailable") => "Database unavailable",
        (Lang::En, "login.failed") => "Login failed",
        (Lang::En, "login.success") => "Login successful",
        (Lang::En, "logout.success") => "Signed out",
        _ => key,
    };
    render(template, args)
}

/// Replace `{}` placeholders with the supplied positional args in order.
fn render(template: &str, args: &[&str]) -> String {
    let mut out = template.to_string();
    for arg in args {
        if let Some(pos) = out.find("{}") {
            out = format!("{}{}{}", &out[..pos], arg, &out[pos + 2..]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::http::header::{HeaderMap, HeaderValue};

    #[test]
    fn zh_accept_language() {
        let mut headers = HeaderMap::new();
        headers.insert(
            actix_web::http::header::ACCEPT_LANGUAGE,
            HeaderValue::from_static("zh-CN,zh;q=0.9"),
        );
        assert_eq!(from_accept_language(&headers), Lang::Zh);
    }

    #[test]
    fn en_default_when_missing() {
        let headers = HeaderMap::new();
        assert_eq!(from_accept_language(&headers), Lang::En);
    }

    #[test]
    fn tr_zh_and_en() {
        assert_eq!(tr(Lang::Zh, "login.disabled", &[]), "账号已被禁用");
        assert_eq!(tr(Lang::En, "login.disabled", &[]), "Account disabled");
        assert_eq!(tr(Lang::Zh, "login.success", &[]), "登录成功");
    }

    #[test]
    fn tr_unknown_key_falls_back() {
        assert_eq!(tr(Lang::En, "unknown.key", &[]), "unknown.key");
    }
}
