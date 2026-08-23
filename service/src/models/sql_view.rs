use serde::{Deserialize, Serialize};

/// SQL 视图定义
///
/// 在 GeoServer 中，SQL View 允许用户定义参数化 SQL 查询，
/// 将其作为虚拟图层发布，支持动态参数替换。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqlView {
    /// 视图名称 (同时也是虚拟图层名)
    pub name: String,
    /// SQL 查询语句（支持 `%param_name%` 参数占位符）
    pub sql: String,
    /// 所属工作空间
    pub workspace: String,
    /// 数据源名称（需为已注册的 PostGIS 数据源）
    pub store: String,
    /// 空间列名
    pub geometry_column: String,
    /// 几何类型
    pub geometry_type: String,
    /// 坐标系
    pub crs: String,
    /// 参数定义
    pub parameters: Vec<SqlViewParameter>,
    /// 描述
    pub description: Option<String>,
    pub created: String,
    pub modified: String,
}

/// SQL 视图参数定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqlViewParameter {
    /// 参数名（不含 `%` 包裹符）
    pub name: String,
    /// 默认值
    pub default_value: String,
    /// 正则验证表达式（可选）
    pub regex_validator: Option<String>,
}
