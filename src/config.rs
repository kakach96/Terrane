use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use config::{Config, File};
use crate::utils::tile_cache::GwcConfig;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GeoServerConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub logging: LoggingConfig,
    pub data_dir: PathBuf,
    pub workspaces: Vec<WorkspaceConfig>,
    /// GeoWebCache 瓦片缓存配置
    #[serde(default)]
    pub gwc: Option<GwcConfig>,
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
    /// SQLite 数据库文件路径
    #[serde(default = "default_sqlite_path")]
    pub sqlite_path: PathBuf,
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
                sqlite_path: default_sqlite_path(),
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
        let config = Config::builder()
            .add_source(File::with_name(path))
            .build()?;

        config.try_deserialize()
    }
}
