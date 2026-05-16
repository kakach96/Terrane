use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use config::{Config, File};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GeoServerConfig {
    pub server: ServerConfig,
    pub data_dir: PathBuf,
    pub workspaces: Vec<WorkspaceConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub api_context: String,
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
            },
            data_dir: PathBuf::from("./data"),
            workspaces: vec![],
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
