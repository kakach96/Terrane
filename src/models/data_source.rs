use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSource {
    pub name: String,
    pub data_source_type: DataSourceType,
    pub workspace: Option<String>,
    pub enabled: bool,
    pub connection: Option<DataSourceConnection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DataSourceType {
    Postgis,
    Shapefile,
    Geotiff,
}

impl std::fmt::Display for DataSourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DataSourceType::Postgis => write!(f, "postgis"),
            DataSourceType::Shapefile => write!(f, "shapefile"),
            DataSourceType::Geotiff => write!(f, "geotiff"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSourceConnection {
    pub host: String,
    pub port: u16,
    pub database: String,
    #[serde(default = "default_schema")]
    pub schema: String,
    pub username: String,
    pub password: Option<String>,
}

fn default_schema() -> String {
    "public".to_string()
}

#[derive(Debug, Deserialize)]
pub struct CreateDataSourceRequest {
    pub name: String,
    #[serde(rename = "type")]
    pub data_source_type: DataSourceType,
    pub workspace: Option<String>,
    pub enabled: Option<bool>,
    pub connection: DataSourceConnection,
}

#[derive(Debug, Deserialize)]
pub struct UpdateDataSourceRequest {
    #[serde(rename = "type")]
    pub data_source_type: Option<DataSourceType>,
    pub workspace: Option<String>,
    pub enabled: Option<bool>,
    pub connection: Option<DataSourceConnection>,
}

#[derive(Debug, Serialize)]
pub struct ConnectionTestResult {
    pub success: bool,
    pub message: Option<String>,
}