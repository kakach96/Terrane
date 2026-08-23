use serde::{Deserialize, Serialize};

/// 命名空间 (Namespace)
///
/// 在 GeoServer 中，命名空间与工作空间一一对应，
/// 每个工作空间关联一个 XML 命名空间 URI。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Namespace {
    /// 命名空间前缀 (通常与工作空间名称一致)
    pub prefix: String,
    /// 命名空间 URI (例如 "http://geoserver.org/default")
    pub uri: String,
    /// 是否隔离 (isolated workspace)
    pub isolated: bool,
    /// 关联的工作空间名称
    pub workspace: Option<String>,
    pub created: String,
    pub modified: String,
}
