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

/// JWT 密钥（从配置读取或使用默认）
const JWT_SECRET: &str = "rust-geoserver-jwt-secret-2026";

/// 生成 JWT Token
pub fn generate_token(username: &str, role: &UserRole, hours: i64) -> Result<String, String> {
    let now = Utc::now();
    let exp = now + Duration::hours(hours);

    let claims = Claims {
        sub: username.to_string(),
        role: role.to_string(),
        exp: exp.timestamp() as usize,
        iat: now.timestamp() as usize,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(JWT_SECRET.as_ref()),
    )
    .map_err(|e| format!("JWT 生成失败: {}", e))
}

/// 验证 JWT Token 并返回 Claims
pub fn verify_token(token: &str) -> Result<Claims, String> {
    let token = token.trim_start_matches("Bearer ");
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(JWT_SECRET.as_ref()),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|e| format!("JWT 验证失败: {}", e))
}

// ---------------------------------------------------------------------------
// 初始化默认管理员
// ---------------------------------------------------------------------------

/// 创建默认管理员用户（如果不存在）
pub async fn ensure_default_admin(store: &crate::store::SqliteStore) {
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
