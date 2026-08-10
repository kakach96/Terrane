//! 认证处理器 — 登录/登出/用户管理

use super::rest_handler::ApiResponse;
use crate::auth::{
    generate_salt, generate_token, hash_password, verify_password, verify_token, UserRole,
};
use crate::error::GeoServerError;
use crate::i18n;
use crate::state::AppState;
use actix_web::http::StatusCode;
use actix_web::{web, HttpRequest, HttpResponse};
use serde::Deserialize;
use tracing::info;

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    pub old_password: String,
    pub new_password: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub role: Option<String>,
}

fn req_ip(req: &HttpRequest) -> Option<String> {
    req.peer_addr().map(|a| a.to_string())
}

fn req_user_agent(req: &HttpRequest) -> Option<String> {
    req.headers()
        .get("User-Agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

/// 登录 — 返回 JWT Token 并在数据库中记录会话
pub async fn login(
    body: web::Json<LoginRequest>,
    state: web::Data<AppState>,
    req: HttpRequest,
) -> Result<HttpResponse, GeoServerError> {
    let ip = req_ip(&req);
    let lang = i18n::from_accept_language(&req.headers());

    if let Some(store) = &state.store {
        match store.get_user(&body.username).await {
            Ok(Some(user)) => {
                if !user.enabled {
                    let _ = store
                        .audit_log(
                            &body.username,
                            "LOGIN_FAILED",
                            None,
                            Some("账号已禁用"),
                            ip.as_deref(),
                        )
                        .await;
                    return Err(GeoServerError::localized(
                        "LOGIN_DISABLED",
                        StatusCode::BAD_REQUEST,
                        i18n::tr(lang, "login.disabled", &[]),
                    ));
                }

                if verify_password(&body.password, &user.salt, &user.password_hash) {
                    let token = generate_token(&user.username, &user.role, 24)
                        .map_err(|e| GeoServerError::InternalError(e))?;

                    // 记录会话到数据库 (支持登出/吊销) 并写入会话缓存
                    if let Ok(claims) = verify_token(&token) {
                        let now = chrono::Utc::now();
                        let now_str = now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
                        let expires_at = (now + chrono::Duration::hours(24))
                            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
                        let session = crate::store::SessionRecord {
                            jti: claims.jti.clone(),
                            username: user.username.clone(),
                            role: user.role.to_string(),
                            issued_at: now_str.clone(),
                            expires_at,
                            last_seen_at: now_str,
                            revoked: false,
                            user_agent: req_user_agent(&req),
                            ip_address: ip.clone(),
                        };
                        let _ = store.create_session(&session).await;
                        if let Some(cache) = &state.session_cache {
                            let _ = cache.set(session.clone()).await;
                        }
                        let _ = store.cleanup_expired_sessions().await;
                    }

                    let _ = store
                        .audit_log(
                            &body.username,
                            "LOGIN",
                            None,
                            Some("登录成功"),
                            ip.as_deref(),
                        )
                        .await;

                    info!(
                        "[Auth] 用户 '{}' 登录成功 (角色: {})",
                        user.username, user.role
                    );
                    Ok(
                        HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
                            "token": token,
                            "username": user.username,
                            "role": user.role.to_string(),
                            "message": i18n::tr(lang, "login.success", &[]),
                        }))),
                    )
                } else {
                    let _ = store
                        .audit_log(
                            &body.username,
                            "LOGIN_FAILED",
                            None,
                            Some("密码错误"),
                            ip.as_deref(),
                        )
                        .await;
                    Err(GeoServerError::localized(
                        "LOGIN_INVALID_CREDENTIALS",
                        StatusCode::BAD_REQUEST,
                        i18n::tr(lang, "login.invalid_credentials", &[]),
                    ))
                }
            },
            Ok(None) => Err(GeoServerError::localized(
                "LOGIN_INVALID_CREDENTIALS",
                StatusCode::BAD_REQUEST,
                i18n::tr(lang, "login.invalid_credentials", &[]),
            )),
            Err(e) => {
                eprintln!("[Auth] 查询用户失败: {}", e);
                Err(GeoServerError::localized(
                    "LOGIN_FAILED",
                    StatusCode::INTERNAL_SERVER_ERROR,
                    i18n::tr(lang, "login.failed", &[]),
                ))
            },
        }
    } else {
        Err(GeoServerError::localized(
            "LOGIN_DB_UNAVAILABLE",
            StatusCode::SERVICE_UNAVAILABLE,
            i18n::tr(lang, "login.db_unavailable", &[]),
        ))
    }
}

/// 登出 — 吊销数据库中的会话
pub async fn logout(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let lang = i18n::from_accept_language(&req.headers());
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| GeoServerError::BadRequest("未登录".to_string()))?;
    let claims = verify_token(auth_header)
        .map_err(|_| GeoServerError::BadRequest("Token 无效".to_string()))?;

    if let Some(store) = &state.store {
        let _ = store.delete_session(&claims.jti).await;
        if let Some(cache) = &state.session_cache {
            let _ = cache.remove(&claims.jti).await;
        }
        let _ = store
            .audit_log(
                &claims.sub,
                "LOGOUT",
                None,
                Some("登出"),
                req_ip(&req).as_deref(),
            )
            .await;
    }

    Ok(
        HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
            "message": i18n::tr(lang, "logout.success", &[]),
        }))),
    )
}

/// 验证 Token 并返回当前用户信息 (校验数据库会话有效性)
pub async fn verify(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| GeoServerError::BadRequest("缺少 Authorization 头".to_string()))?;

    let claims = verify_token(auth_header)
        .map_err(|e| GeoServerError::BadRequest(format!("Token 验证失败: {}", e)))?;

    // 校验会话 (未记录 = 已过期/已登出)
    // 优先查会话缓存, 未命中回退元数据存储并回填缓存
    if let Some(store) = &state.store {
        let session_opt = if let Some(cache) = &state.session_cache {
            if let Some(s) = cache.get(&claims.jti).await {
                Some(s)
            } else {
                match store.get_session(&claims.jti).await {
                    Ok(Some(s)) => {
                        let _ = cache.set(s.clone()).await;
                        Some(s)
                    },
                    Ok(None) => None,
                    Err(_) => None,
                }
            }
        } else {
            match store.get_session(&claims.jti).await {
                Ok(s) => s,
                Err(_) => None,
            }
        };

        match session_opt {
            Some(session) => {
                if session.revoked {
                    return Err(GeoServerError::BadRequest("会话已失效".to_string()));
                }
            },
            None => return Err(GeoServerError::BadRequest("会话不存在或已过期".to_string())),
        }
    }

    Ok(
        HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
            "username": claims.sub,
            "role": claims.role,
            "authenticated": true,
        }))),
    )
}

/// 修改密码
pub async fn change_password(
    body: web::Json<ChangePasswordRequest>,
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| GeoServerError::BadRequest("未登录".to_string()))?;
    let claims = verify_token(auth_header)
        .map_err(|_| GeoServerError::BadRequest("Token 无效".to_string()))?;

    if let Some(store) = &state.store {
        match store.get_user(&claims.sub).await {
            Ok(Some(user)) => {
                if !verify_password(&body.old_password, &user.salt, &user.password_hash) {
                    return Err(GeoServerError::BadRequest("原密码错误".to_string()));
                }
                let new_salt = generate_salt();
                let new_hash = hash_password(&body.new_password, &new_salt);
                // 简化: 直接更新密码哈希
                let _ = store
                    .create_user(
                        &user.username,
                        &new_hash,
                        &new_salt,
                        &user.role,
                        user.enabled,
                    )
                    .await;
                // 删除旧记录 - 简化处理直接使用 delete + create
                let _ = store.delete_user(&user.username).await;
                let _ = store
                    .create_user(
                        &user.username,
                        &new_hash,
                        &new_salt,
                        &user.role,
                        user.enabled,
                    )
                    .await;

                Ok(
                    HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
                        "message": "密码修改成功",
                    }))),
                )
            },
            _ => Err(GeoServerError::BadRequest("用户不存在".to_string())),
        }
    } else {
        Err(GeoServerError::InternalError("数据库不可用".to_string()))
    }
}

/// 列出所有用户 (仅 admin)
pub async fn list_users(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    check_admin(&req)?;

    if let Some(store) = &state.store {
        match store.get_all_users().await {
            Ok(users) => {
                let result: Vec<_> = users
                    .iter()
                    .map(|u| {
                        serde_json::json!({
                            "username": u.username,
                            "role": u.role.to_string(),
                            "enabled": u.enabled,
                            "created": u.created,
                            "modified": u.modified,
                        })
                    })
                    .collect();
                Ok(HttpResponse::Ok().json(ApiResponse::success(result)))
            },
            Err(e) => Err(GeoServerError::InternalError(format!(
                "查询用户失败: {}",
                e
            ))),
        }
    } else {
        Err(GeoServerError::InternalError("数据库不可用".to_string()))
    }
}

/// 创建用户 (仅 admin)
pub async fn create_user(
    body: web::Json<CreateUserRequest>,
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    check_admin(&req)?;

    if let Some(store) = &state.store {
        if let Ok(Some(_)) = store.get_user(&body.username).await {
            return Err(GeoServerError::Conflict(format!(
                "用户 '{}' 已存在",
                body.username
            )));
        }
        let role = match body.role.as_deref() {
            Some("admin") => UserRole::Admin,
            Some("manager") => UserRole::Manager,
            Some("guest") => UserRole::Guest,
            _ => UserRole::User,
        };
        let salt = generate_salt();
        let hash = hash_password(&body.password, &salt);
        store
            .create_user(&body.username, &hash, &salt, &role, true)
            .await
            .map_err(|e| GeoServerError::InternalError(format!("创建用户失败: {}", e)))?;

        Ok(
            HttpResponse::Created().json(ApiResponse::success(serde_json::json!({
                "username": body.username,
                "role": role.to_string(),
                "message": "用户创建成功",
            }))),
        )
    } else {
        Err(GeoServerError::InternalError("数据库不可用".to_string()))
    }
}

/// 删除用户 (仅 admin)
pub async fn delete_user(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    check_admin(&req)?;
    let username = req.match_info().get("username").unwrap_or("");

    if let Some(store) = &state.store {
        store
            .delete_user(username)
            .await
            .map_err(|e| GeoServerError::InternalError(format!("删除失败: {}", e)))?;
        Ok(
            HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
                "message": format!("用户 '{}' 已删除", username),
            }))),
        )
    } else {
        Err(GeoServerError::InternalError("数据库不可用".to_string()))
    }
}

/// 检查是否为管理员
fn check_admin(req: &HttpRequest) -> Result<(), GeoServerError> {
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| GeoServerError::BadRequest("未登录".to_string()))?;
    let claims = verify_token(auth_header)
        .map_err(|_| GeoServerError::BadRequest("Token 无效".to_string()))?;
    if claims.role != "admin" {
        return Err(GeoServerError::BadRequest("需要管理员权限".to_string()));
    }
    Ok(())
}

/// 检查认证的辅助函数（供其他 handler 使用）
pub fn require_auth(req: &HttpRequest) -> Result<crate::auth::Claims, GeoServerError> {
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| GeoServerError::BadRequest("未登录".to_string()))?;
    verify_token(auth_header).map_err(|e| GeoServerError::BadRequest(format!("认证失败: {}", e)))
}
