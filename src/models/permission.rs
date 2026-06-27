use serde::{Deserialize, Serialize};

/// 权限条目 — 控制谁可以访问哪些资源
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Permission {
    pub id: Option<i64>,
    /// 用户名 (或 "*" 表示所有用户)
    pub username: String,
    /// 角色名 (或 "*" 表示所有角色)
    pub role: String,
    /// 资源类型: layer / workspace / service
    pub resource_type: String,
    /// 资源名称 (或 "*" 表示所有)
    pub resource_name: String,
    /// 访问模式: read / write / admin
    pub access_mode: AccessMode,
    /// 是否允许 (true=允许, false=拒绝)
    pub effect: Effect,
    /// 优先级 (数值越大优先级越高)
    pub priority: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AccessMode {
    Read,
    Write,
    Admin,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Effect {
    Allow,
    Deny,
}

impl std::str::FromStr for AccessMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "read" => Ok(AccessMode::Read),
            "write" => Ok(AccessMode::Write),
            "admin" => Ok(AccessMode::Admin),
            _ => Err(format!("未知的访问模式: {}", s)),
        }
    }
}

impl std::fmt::Display for AccessMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AccessMode::Read => write!(f, "read"),
            AccessMode::Write => write!(f, "write"),
            AccessMode::Admin => write!(f, "admin"),
        }
    }
}

impl std::str::FromStr for Effect {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "allow" => Ok(Effect::Allow),
            "deny" => Ok(Effect::Deny),
            _ => Err(format!("未知的效应: {}", s)),
        }
    }
}

impl std::fmt::Display for Effect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Effect::Allow => write!(f, "allow"),
            Effect::Deny => write!(f, "deny"),
        }
    }
}
