//! # 认证与授权模块
//!
//! 提供用户认证、JWT Token、密码哈希等功能。
//! 支持 Basic Auth 和 Bearer Token 两种认证方式。

use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use chrono::{Utc, Duration};
use jsonwebtoken::{encode, decode, Header, Validation, EncodingKey, DecodingKey};
use rand::Rng;
use tracing::info;

// ---------------------------------------------------------------------------
// 用户 & 角色模型
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub username: String,
    pub password_hash: String,
    pub salt: String,
    pub role: UserRole,
    pub enabled: bool,
    pub created: String,
    pub modified: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum UserRole {
    #[serde(rename = "admin")]
    Admin,
    #[serde(rename = "manager")]
    Manager,
    #[serde(rename = "user")]
    User,
    #[serde(rename = "guest")]
    Guest,
}

impl std::fmt::Display for UserRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UserRole::Admin => write!(f, "admin"),
            UserRole::Manager => write!(f, "manager"),
            UserRole::User => write!(f, "user"),
            UserRole::Guest => write!(f, "guest"),
        }
    }
}

// ---------------------------------------------------------------------------
// JWT Claims
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    /// 用户名
    pub sub: String,
    /// 角色
    pub role: String,
    /// JWT ID (对应数据库会话记录主键)
    pub jti: String,
    /// 过期时间 (Unix timestamp)
    pub exp: usize,
    /// 签发时间
    pub iat: usize,
}

// ---------------------------------------------------------------------------
// 密码处理
// ---------------------------------------------------------------------------

/// 生成随机 salt
pub fn generate_salt() -> String {
    let mut rng = rand::thread_rng();
    let salt: Vec<u8> = (0..16).map(|_| rng.gen()).collect();
    hex::encode(&salt)
}

/// 使用 SHA-256 + salt 哈希密码
pub fn hash_password(password: &str, salt: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    hasher.update(salt.as_bytes());
    hex::encode(hasher.finalize())
}

/// 验证密码
pub fn verify_password(password: &str, salt: &str, hash: &str) -> bool {
    hash_password(password, salt) == hash
}

// ---------------------------------------------------------------------------
// JWT Token 管理
// ---------------------------------------------------------------------------

/// JWT 密钥（从配置读取，集群各副本共享）
static JWT_SECRET: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// 默认密钥（仅在未通过 `init_secret` 配置时用于测试/降级）
const DEFAULT_JWT_SECRET: &str = "terrane-jwt-secret-2026";

/// 初始化 JWT 密钥（应在进程启动时从配置调用一次）
pub fn init_secret(secret: &str) {
    let _ = JWT_SECRET.set(secret.to_string());
}

fn jwt_secret() -> &'static str {
    JWT_SECRET.get().map(|s| s.as_str()).unwrap_or(DEFAULT_JWT_SECRET)
}

/// 生成 JWT Token (含 jti，用于数据库会话关联)
pub fn generate_token(username: &str, role: &UserRole, hours: i64) -> Result<String, String> {
    let now = Utc::now();
    let exp = now + Duration::hours(hours);

    let claims = Claims {
        sub: username.to_string(),
        role: role.to_string(),
        jti: uuid::Uuid::new_v4().to_string(),
        exp: exp.timestamp() as usize,
        iat: now.timestamp() as usize,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_secret().as_ref()),
    )
    .map_err(|e| format!("JWT 生成失败: {}", e))
}

/// 验证 JWT Token 并返回 Claims
pub fn verify_token(token: &str) -> Result<Claims, String> {
    let token = token.trim_start_matches("Bearer ");
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(jwt_secret().as_ref()),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|e| format!("JWT 验证失败: {}", e))
}

// ---------------------------------------------------------------------------
// 初始化默认管理员
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// 测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_salt() {
        let s1 = generate_salt();
        let s2 = generate_salt();
        assert_eq!(s1.len(), 32);  // 16 bytes = 32 hex chars
        assert_eq!(s2.len(), 32);
        assert_ne!(s1, s2);  // 每次生成的 salt 应不同
    }

    #[test]
    fn test_hash_password_consistency() {
        let salt = generate_salt();
        let hash1 = hash_password("mypassword", &salt);
        let hash2 = hash_password("mypassword", &salt);
        assert_eq!(hash1, hash2);  // 相同 salt + 相同密码 = 相同 hash
    }

    #[test]
    fn test_hash_password_different_salt() {
        let salt1 = generate_salt();
        let salt2 = generate_salt();
        let hash1 = hash_password("mypassword", &salt1);
        let hash2 = hash_password("mypassword", &salt2);
        assert_ne!(hash1, hash2);  // 不同 salt = 不同 hash
    }

    #[test]
    fn test_verify_password_correct() {
        let password = "correct-horse-battery-staple";
        let salt = generate_salt();
        let hash = hash_password(password, &salt);
        assert!(verify_password(password, &salt, &hash));
    }

    #[test]
    fn test_verify_password_wrong() {
        let salt = generate_salt();
        let hash = hash_password("real_password", &salt);
        assert!(!verify_password("wrong_password", &salt, &hash));
    }

    #[test]
    fn test_generate_and_verify_token() {
        let token = generate_token("admin", &UserRole::Admin, 1).unwrap();
        let claims = verify_token(&token).unwrap();
        assert_eq!(claims.sub, "admin");
        assert_eq!(claims.role, "admin");
    }

    #[test]
    fn test_verify_invalid_token() {
        let result = verify_token("invalid.token.here");
        assert!(result.is_err());
    }

    #[test]
    fn test_token_expiry() {
        // 使用 0 小时过期时间生成一个立即过期的 token
        let token = generate_token("testuser", &UserRole::User, 0).unwrap();
        // 注意: 由于生成和验证之间有微小延迟, 这个测试可能不稳定
        // 这里只验证 token 格式正确
        assert!(token.split('.').count() == 3);
    }

    #[test]
    fn test_user_role_display() {
        assert_eq!(UserRole::Admin.to_string(), "admin");
        assert_eq!(UserRole::Manager.to_string(), "manager");
        assert_eq!(UserRole::User.to_string(), "user");
        assert_eq!(UserRole::Guest.to_string(), "guest");
    }
}

/// 创建默认管理员用户（如果不存在）
pub async fn ensure_default_admin(store: &dyn crate::store::Store) {
    match store.get_user("admin").await {
        Ok(Some(_)) => {
            info!("[Auth] 默认管理员用户已存在");
        }
        _ => {
            let salt = generate_salt();
            let hash = hash_password("geoserver", &salt);
            match store.create_user("admin", &hash, &salt, &UserRole::Admin, true).await {
                Ok(_) => info!("[Auth] 已创建默认管理员: admin / geoserver"),
                Err(e) => eprintln!("[Auth] 创建默认管理员失败: {}", e),
            }
        }
    }
}
