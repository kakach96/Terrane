//! 存储层共享类型 (SqliteStore 与 PostgresStore 共用)

/// 数据库层工作空间记录
#[derive(Debug, Clone)]
pub struct Workspace {
    pub name: String,
    pub title: String,
    pub enabled: bool,
    pub layer_count: i32,
    pub description: String,
    pub created: String,
    pub modified: String,
}

/// 数据库层命名空间记录
#[derive(Debug, Clone)]
pub struct NamespaceRecord {
    pub prefix: String,
    pub uri: String,
    pub isolated: bool,
    pub workspace: Option<String>,
    pub created: String,
    pub modified: String,
}

/// 审计日志记录
#[derive(Debug, Clone)]
pub struct AuditLogRecord {
    pub id: i64,
    pub username: String,
    pub action: String,
    pub resource: Option<String>,
    pub detail: Option<String>,
    pub ip_address: Option<String>,
    pub created_at: String,
}

/// 数据库层图层记录
#[derive(Debug, Clone)]
pub struct Layer {
    pub name: String,
    pub title: String,
    pub workspace: String,
    pub store: String,
    pub srs: String,
    pub abstract_text: Option<String>,
    pub native_name: Option<String>,
    pub enabled: bool,
    pub minx: f64,
    pub miny: f64,
    pub maxx: f64,
    pub maxy: f64,
    pub created: String,
    pub modified: String,
    /// 瓦片缓存后端数据源名称 (type = "redis"); 为空 = 默认内存/本地缓存
    pub cache_store: Option<String>,
}

/// 样式记录
#[derive(Debug, Clone)]
pub struct StyleRecord {
    pub name: String,
    pub title: String,
    /// 样式格式: SLD / CSS / YSLD / MBStyle
    pub format: String,
    pub is_builtin: bool,
    pub content: String,
    pub created: String,
    pub modified: String,
}

/// 图层组记录 (layers/styles 序列化为 JSON 存储)
#[derive(Debug, Clone)]
pub struct LayerGroupRecord {
    pub name: String,
    pub title: String,
    pub layers: Vec<String>,
    pub styles: Vec<Option<String>>,
    pub created: String,
    pub modified: String,
}

/// 会话记录 (JWT jti 关联)
#[derive(Debug, Clone)]
pub struct SessionRecord {
    pub jti: String,
    pub username: String,
    pub role: String,
    pub issued_at: String,
    pub expires_at: String,
    pub last_seen_at: String,
    pub revoked: bool,
    pub user_agent: Option<String>,
    pub ip_address: Option<String>,
}
