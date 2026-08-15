use serde::{Deserialize, Serialize};

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
    /// 判断数据源是否为基于文件的类型（shapefile / geotiff / geopackage / geojson）
    pub fn is_file_based(&self) -> bool {
        matches!(
            self.data_source_type,
            DataSourceType::Shapefile
                | DataSourceType::Geotiff
                | DataSourceType::Geopackage
                | DataSourceType::WorldImage
                | DataSourceType::ArcGrid
                | DataSourceType::GeoJson
                | DataSourceType::ImageMosaic
                | DataSourceType::ImagePyramid
        )
    }

    /// 判断数据源是否为 PostGIS 数据库类型
    pub fn is_database(&self) -> bool {
        matches!(self.data_source_type, DataSourceType::Postgis)
    }

    /// 判断数据源是否为栅格类型 (WCS / WMS 栅格渲染路径)
    pub fn is_raster(&self) -> bool {
        matches!(
            self.data_source_type,
            DataSourceType::Geotiff
                | DataSourceType::WorldImage
                | DataSourceType::ArcGrid
                | DataSourceType::ImageMosaic
                | DataSourceType::ImagePyramid
        )
    }
}

/// 内置 metadata 数据源名称（业务数据复用元数据存储时的内置默认选项）
pub const METADATA_DATA_SOURCE: &str = "metadata";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DataSourceType {
    Postgis,
    Shapefile,
    Geotiff,
    Geopackage,
    WorldImage,
    /// 级联 WMS — 公共名与 Display/前端/备份一致 (serde 的 lowercase 会把
    /// `CascadedWms` 变成 `cascadedwms`, 这里显式覆盖为 `cascaded_wms`)
    #[serde(rename = "cascaded_wms")]
    CascadedWms,
    /// Redis 缓存数据源 — 供切片图层选择使用 (host/port/database/password 字段;
    /// 持久化于元数据, 图层 `cache_store` 引用其名称)
    #[serde(rename = "redis")]
    Redis,
    /// ImageMosaic — 栅格目录马赛克数据源 (目录下多个 GeoTIFF/WorldImage/
    /// ArcGrid/PNG/JPEG 作为 granule, 整体作为一个覆盖发布)
    #[serde(rename = "image_mosaic")]
    ImageMosaic,
    /// ImagePyramid — 金字塔影像数据源 (目录下数字子目录 0/1/2/… 各含一层
    /// granule, 按请求分辨率选择最合适层级)
    #[serde(rename = "image_pyramid")]
    ImagePyramid,
    ArcGrid,
    /// GeoJSON 文件数据源 (存储位置由 connection.file_storage_type 决定: local/s3/oss)
    #[serde(rename = "geojson")]
    GeoJson,
    /// 元数据存储复用（业务数据复用元数据存储时的内置默认数据源）
    Metadata,
}

impl std::fmt::Display for DataSourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DataSourceType::Postgis => write!(f, "postgis"),
            DataSourceType::Shapefile => write!(f, "shapefile"),
            DataSourceType::Geotiff => write!(f, "geotiff"),
            DataSourceType::Geopackage => write!(f, "geopackage"),
            DataSourceType::WorldImage => write!(f, "worldimage"),
            DataSourceType::CascadedWms => write!(f, "cascaded_wms"),
            DataSourceType::Redis => write!(f, "redis"),
            DataSourceType::ImageMosaic => write!(f, "image_mosaic"),
            DataSourceType::ImagePyramid => write!(f, "image_pyramid"),
            DataSourceType::ArcGrid => write!(f, "arcgrid"),
            DataSourceType::GeoJson => write!(f, "geojson"),
            DataSourceType::Metadata => write!(f, "metadata"),
        }
    }
}

/// 数据源连接信息。
///
/// - 对于 PostGIS: 使用 host/port/database/schema/username/password
/// - 对于 Shapefile/GeoTIFF: 使用 file_path/file_storage_type
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
    /// 文件路径 (shapefile .shp / geotiff .tif / geojson .geojson)
    #[serde(default)]
    pub file_path: Option<String>,
    /// 文件存储类型: "local" (服务数据目录) / "s3" / "oss" (对象存储)
    #[serde(default = "default_file_storage")]
    pub file_storage_type: Option<String>,

    // -- S3 对象存储字段 (file_storage_type = "s3" 时生效) --
    /// S3 endpoint URL (例如 MinIO: http://localhost:9000; AWS 可留空走区域默认端点)
    #[serde(default)]
    pub s3_endpoint: Option<String>,
    /// S3 region (默认 "us-east-1")
    #[serde(default)]
    pub s3_region: Option<String>,
    /// S3 bucket 名称
    #[serde(default)]
    pub s3_bucket: Option<String>,
    /// S3 access key
    #[serde(default)]
    pub s3_access_key: Option<String>,
    /// S3 secret key
    #[serde(default)]
    pub s3_secret_key: Option<String>,
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
            schema: Some("public".to_string()),
            file_path: Some(file_path.into()),
            file_storage_type: Some("local".to_string()),
            ..Default::default()
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
            ..Default::default()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_source_type_serde_roundtrip() {
        // 公共名称 (Display / 前端 / 备份) 与 serde 序列化/反序列化必须一致
        let cases: &[(&str, DataSourceType)] = &[
            ("postgis", DataSourceType::Postgis),
            ("shapefile", DataSourceType::Shapefile),
            ("geotiff", DataSourceType::Geotiff),
            ("geopackage", DataSourceType::Geopackage),
            ("worldimage", DataSourceType::WorldImage),
            ("cascaded_wms", DataSourceType::CascadedWms),
            ("redis", DataSourceType::Redis),
            ("image_mosaic", DataSourceType::ImageMosaic),
            ("image_pyramid", DataSourceType::ImagePyramid),
            ("arcgrid", DataSourceType::ArcGrid),
            ("geojson", DataSourceType::GeoJson),
            ("metadata", DataSourceType::Metadata),
        ];
        for (name, expected) in cases {
            // serde 反序列化
            let parsed: DataSourceType = serde_json::from_str(&format!("\"{}\"", name))
                .unwrap_or_else(|e| panic!("type '{}' 应能反序列化: {}", name, e));
            assert_eq!(&parsed, expected, "type '{}' 反序列化结果不符", name);
            // serde 序列化
            let serialized = serde_json::to_string(expected).unwrap();
            assert_eq!(
                serialized,
                format!("\"{}\"", name),
                "type '{}' 序列化结果不符",
                name
            );
        }
    }

    #[test]
    fn test_data_source_connection_postgis() {
        let conn = DataSourceConnection::postgis(
            "localhost".to_string(),
            5432,
            "geoserver".to_string(),
            "user".to_string(),
            Some("pass".to_string()),
        );
        assert_eq!(conn.host.as_deref(), Some("localhost"));
        assert_eq!(conn.port, Some(5432));
        assert_eq!(conn.database.as_deref(), Some("geoserver"));
        assert_eq!(conn.schema.as_deref(), Some("public"));
        assert_eq!(conn.username.as_deref(), Some("user"));
        assert_eq!(conn.password.as_deref(), Some("pass"));
        assert!(conn.file_path.is_none());
    }
}
