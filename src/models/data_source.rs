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

impl DataSource {
    /// 判断数据源是否为基于文件的类型（shapefile / geotiff）
    pub fn is_file_based(&self) -> bool {
        matches!(self.data_source_type, DataSourceType::Shapefile | DataSourceType::Geotiff)
    }

    /// 判断数据源是否为 PostGIS 数据库类型
    pub fn is_database(&self) -> bool {
        matches!(self.data_source_type, DataSourceType::Postgis)
    }
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

/// 数据源连接信息。
///
/// - 对于 PostGIS: 使用 host/port/database/schema/username/password
/// - 对于 Shapefile/GeoTIFF: 使用 file_path/file_storage_type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSourceConnection {
    // -- PostGIS 字段 (均为可选，以便文件型数据源留空) --
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub database: Option<String>,
    #[serde(default = "default_schema")]
    pub schema: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,

    // -- 文件型字段 --
    /// 文件路径 (shapefile .shp / geotiff .tif)
    #[serde(default)]
    pub file_path: Option<String>,
    /// 文件存储类型，例如 "local" / "s3" / "oss"
    #[serde(default = "default_file_storage")]
    pub file_storage_type: Option<String>,
}

fn default_schema() -> Option<String> {
    Some("public".to_string())
}

fn default_file_storage() -> Option<String> {
    Some("local".to_string())
}

impl DataSourceConnection {
    /// 快速创建一个文件型连接
    pub fn file(file_path: impl Into<String>) -> Self {
        DataSourceConnection {
            host: None,
            port: None,
            database: None,
            schema: Some("public".to_string()),
            username: None,
            password: None,
            file_path: Some(file_path.into()),
            file_storage_type: Some("local".to_string()),
        }
    }

    /// 快速创建一个 PostGIS 连接
    pub fn postgis(
        host: impl Into<String>,
        port: u16,
        database: impl Into<String>,
        username: impl Into<String>,
        password: Option<String>,
    ) -> Self {
        DataSourceConnection {
            host: Some(host.into()),
            port: Some(port),
            database: Some(database.into()),
            schema: Some("public".to_string()),
            username: Some(username.into()),
            password,
            file_path: None,
            file_storage_type: None,
        }
    }
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