use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use config::{Config, File};
use crate::utils::tile_cache::GwcConfig;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GeoServerConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub security: SecurityConfig,
    pub logging: LoggingConfig,
    /// 数据目录 (默认: "./data")
    #[serde(default = "default_data_dir")]
    pub data_dir: PathBuf,
    pub workspaces: Vec<WorkspaceConfig>,
    /// GeoWebCache 瓦片缓存配置
    #[serde(default)]
    pub gwc: Option<GwcConfig>,
    /// CORS 配置
    #[serde(default)]
    pub cors: CorsConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub api_context: String,
    /// 静态文件目录
    #[serde(default = "default_static_dir")]
    pub static_dir: PathBuf,
    /// PostgreSQL 连接超时秒数
    #[serde(default = "default_connect_timeout")]
    pub connect_timeout_secs: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DatabaseConfig {
    /// 存储后端类型: "sqlite" | "postgres" (默认: "sqlite")
    #[serde(default = "default_db_kind")]
    pub kind: String,
    /// SQLite 数据库文件路径
    #[serde(default = "default_sqlite_path")]
    pub sqlite_path: PathBuf,
    /// PostgreSQL 配置 (kind = "postgres" 时生效)
    #[serde(default)]
    pub postgres: PostgresConfig,
}

/// PostgreSQL 连接配置 (仅 kind = "postgres" 时生效)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PostgresConfig {
    /// PostgreSQL 主机
    #[serde(default = "default_db_host")]
    pub host: String,
    /// PostgreSQL 端口
    #[serde(default = "default_db_port")]
    pub port: u16,
    /// PostgreSQL 数据库名
    #[serde(default = "default_db_name")]
    pub name: String,
    /// PostgreSQL 用户名
    #[serde(default = "default_db_user")]
    pub user: String,
    /// PostgreSQL 密码
    #[serde(default = "default_db_password")]
    pub password: String,
    /// PostgreSQL 连接池大小
    #[serde(default = "default_db_pool_size")]
    pub pool_size: u32,
}

/// 安全配置 (集群部署时各副本必须共享相同密钥)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SecurityConfig {
    /// JWT 签名密钥 (建议通过 GEOSERVER__SECURITY__JWT_SECRET 环境变量注入)
    #[serde(default = "default_jwt_secret")]
    pub jwt_secret: String,
}

fn default_db_kind() -> String {
    "sqlite".to_string()
}

fn default_data_dir() -> PathBuf {
    PathBuf::from("./data")
}

fn default_db_host() -> String {
    "127.0.0.1".to_string()
}

fn default_db_port() -> u16 {
    5432
}

fn default_db_name() -> String {
    "geoserver".to_string()
}

fn default_db_user() -> String {
    "postgres".to_string()
}

fn default_db_password() -> String {
    "".to_string()
}

fn default_db_pool_size() -> u32 {
    10
}

fn default_jwt_secret() -> String {
    "rust-geoserver-jwt-secret-2026".to_string()
}

impl Default for PostgresConfig {
    fn default() -> Self {
        PostgresConfig {
            host: default_db_host(),
            port: default_db_port(),
            name: default_db_name(),
            user: default_db_user(),
            password: default_db_password(),
            pool_size: default_db_pool_size(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoggingConfig {
    /// 日志级别 (trace/debug/info/warn/error)
    #[serde(default = "default_log_level")]
    pub level: String,
}

fn default_static_dir() -> PathBuf {
    PathBuf::from("./static")
}

fn default_connect_timeout() -> u64 {
    10
}

fn default_sqlite_path() -> PathBuf {
    PathBuf::from("geoserver.sqlite")
}

fn default_log_level() -> String {
    "info".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WorkspaceConfig {
    pub name: String,
    pub uri: String,
    pub stores: Vec<StoreConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StoreConfig {
    pub name: String,
    pub store_type: String,
    pub path: String,
    pub layers: Vec<LayerConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LayerConfig {
    pub name: String,
    pub title: String,
    #[serde(rename = "abstract")]
    pub abstract_text: String,
    pub srs: String,
    pub bounds: BoundsConfig,
    pub style: Option<String>,
}

/// CORS 配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CorsConfig {
    /// 是否启用 CORS (默认: true)
    #[serde(default = "default_cors_enabled")]
    pub enabled: bool,
    /// 允许的来源 (默认: ["*"])
    #[serde(default = "default_cors_origins")]
    pub allowed_origins: Vec<String>,
    /// 允许的方法 (默认: ["GET", "POST", "PUT", "DELETE", "OPTIONS"])
    #[serde(default = "default_cors_methods")]
    pub allowed_methods: Vec<String>,
    /// 允许的头 (默认: ["*"])
    #[serde(default = "default_cors_headers")]
    pub allowed_headers: Vec<String>,
    /// 是否允许凭据 (默认: true)
    #[serde(default = "default_cors_credentials")]
    pub allow_credentials: bool,
    /// 预检请求缓存时间 (秒, 默认: 3600)
    #[serde(default = "default_cors_max_age")]
    pub max_age: u64,
}

fn default_cors_enabled() -> bool { true }
fn default_cors_origins() -> Vec<String> { vec!["*".to_string()] }
fn default_cors_methods() -> Vec<String> { vec!["GET".to_string(), "POST".to_string(), "PUT".to_string(), "DELETE".to_string(), "OPTIONS".to_string(), "PATCH".to_string()] }
fn default_cors_headers() -> Vec<String> { vec!["*".to_string()] }
fn default_cors_credentials() -> bool { true }
fn default_cors_max_age() -> u64 { 3600 }

impl Default for CorsConfig {
    fn default() -> Self {
        CorsConfig {
            enabled: true,
            allowed_origins: vec!["*".to_string()],
            allowed_methods: vec![
                "GET".to_string(), "POST".to_string(), "PUT".to_string(),
                "DELETE".to_string(), "OPTIONS".to_string(), "PATCH".to_string()
            ],
            allowed_headers: vec!["*".to_string()],
            allow_credentials: true,
            max_age: 3600,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BoundsConfig {
    pub minx: f64,
    pub miny: f64,
    pub maxx: f64,
    pub maxy: f64,
}

impl Default for GeoServerConfig {
    fn default() -> Self {
        GeoServerConfig {
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 8080,
                api_context: "/geoserver".to_string(),
                static_dir: default_static_dir(),
                connect_timeout_secs: default_connect_timeout(),
            },
            database: DatabaseConfig {
                kind: default_db_kind(),
                sqlite_path: default_sqlite_path(),
                postgres: PostgresConfig::default(),
            },
            security: SecurityConfig {
                jwt_secret: default_jwt_secret(),
            },
            logging: LoggingConfig {
                level: default_log_level(),
            },
            data_dir: PathBuf::from("./data"),
            workspaces: vec![],
            gwc: Some(GwcConfig {
                cache_dir: PathBuf::from("./data/gwc"),
                meta_dir: PathBuf::from("./data/gwc/meta"),
                ..Default::default()
            }),
            cors: CorsConfig::default(),
        }
    }
}

impl GeoServerConfig {
    pub fn load() -> Result<Self, config::ConfigError> {
        let config = Config::builder()
            .add_source(File::with_name("geoserver").required(false))
            .add_source(config::Environment::with_prefix("GEOSERVER").separator("__"))
            .build()?;

        config.try_deserialize()
    }

    pub fn load_from_file(path: &str) -> Result<Self, config::ConfigError> {
        let mut builder = Config::builder();

        // 候选配置文件: 先 CWD 下的 geoserver.toml, 再回退到可执行文件所在目录
        let mut loaded = false;
        for file in config_file_candidates(path) {
            if file.exists() {
                builder = builder.add_source(File::with_name(&file.to_string_lossy()));
                loaded = true;
                break;
            }
        }
        if !loaded {
            builder = builder.add_source(File::with_name(path).required(false));
        }

        builder
            .add_source(config::Environment::with_prefix("GEOSERVER").separator("__"))
            .build()?
            .try_deserialize()
    }
}

/// 生成配置文件的候选路径 (含 .toml 扩展名):
/// 依次为 CWD 下的 path 及可执行文件目录下的同名文件。
fn config_file_candidates(path: &str) -> Vec<PathBuf> {
    let with_toml = |p: &PathBuf| -> PathBuf {
        let mut f = p.clone();
        if f.extension().is_none() {
            f.set_extension("toml");
        }
        f
    };

    let mut candidates = Vec::new();
    candidates.push(with_toml(&PathBuf::from(path)));

    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            if let Some(name) = PathBuf::from(path).file_name() {
                candidates.push(with_toml(&exe_dir.join(name)));
            }
        }
    }
    candidates
}
