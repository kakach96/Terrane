use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use config::{Config, File};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GeoServerConfig {
    /// 服务端配置 (无配置文件/env-only 时使用默认值)
    #[serde(default)]
    pub server: ServerConfig,
    /// 元数据存储配置 (默认 sqlite) — 兼容旧节名 `[database]` (serde alias)
    #[serde(alias = "database", default)]
    pub metadata: MetadataConfig,
    /// 矢量数据存储配置 (可选; 图层要素数据) — 兼容旧节名 `[business]` (serde alias)
    #[serde(alias = "business", default)]
    pub vector: Option<VectorConfig>,
    /// 栅格数据存储配置 (可选; GeoTIFF/WorldImage/ArcGrid 等栅格文件)
    #[serde(default)]
    pub raster: Option<RasterConfig>,
    /// 缓存存储配置 (可选; 瓦片缓存 + 会话缓存) — 兼容旧节名 `[gwc]` (serde alias)
    #[serde(alias = "gwc", default)]
    pub cache: Option<CacheConfig>,
    /// 安全配置 (无配置时使用内置默认 JWT 密钥, 生产必须注入)
    #[serde(default)]
    pub security: SecurityConfig,
    /// 日志配置
    #[serde(default)]
    pub logging: LoggingConfig,
    /// 数据目录 (默认: "./data")
    #[serde(default = "default_data_dir")]
    pub data_dir: PathBuf,
    /// 内置工作空间配置
    #[serde(default)]
    pub workspaces: Vec<WorkspaceConfig>,
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
    /// 优雅关闭时等待在途请求完成的最大秒数 (默认: 30; 容器滚动更新/缩容时生效)
    #[serde(default = "default_shutdown_timeout")]
    pub shutdown_timeout_secs: u64,
}

/// 元数据存储配置 — 保存工作空间、数据源、图层、样式、权限、会话等配置元数据。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MetadataConfig {
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

/// 矢量数据存储配置 — 保存图层要素 (矢量业务数据)。
///
/// 未配置 `[vector]` 节时的默认规则 (见 [`GeoServerConfig::effective_vector`]):
/// - 元数据存储为 sqlite → `local` (本地目录, 默认 `<data_dir>/business`)
/// - 元数据存储为其他外部存储 (如 postgres) → `metadata` (复用元数据存储, 内置默认选项)
///
/// 兼容旧节名 `[business]` (serde alias, 声明在字段 `vector` 上)。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VectorConfig {
    /// 矢量存储后端类型: "local" | "metadata" | "postgres" (默认: "local")
    #[serde(default = "default_vector_kind")]
    pub kind: String,
    /// 本地目录 (kind = "local" 时生效; 默认: `<data_dir>/business`, 支持 NFS/对象存储挂载)
    #[serde(default)]
    pub dir: Option<PathBuf>,
    /// PostgreSQL 配置 (kind = "postgres", 或复用 postgres 元数据存储时生效)
    #[serde(default)]
    pub postgres: PostgresConfig,
}

/// 栅格数据存储配置 — 保存栅格文件 (GeoTIFF / WorldImage / ArcGrid)。
///
/// 未配置 `[raster]` 节时默认 `local` (本地目录, 默认 `<data_dir>/rasters`)。
/// 后续可扩展 `s3` / `minio` 等对象存储后端 (见 [`crate::store::raster`])。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RasterConfig {
    /// 栅格存储后端类型: "local" (默认) | 未来 "s3"/"minio"
    #[serde(default = "default_raster_kind")]
    pub kind: String,
    /// 本地目录 (kind = "local" 时生效; 默认: `<data_dir>/rasters`, 支持 NFS/对象存储挂载)
    #[serde(default)]
    pub dir: Option<PathBuf>,
}

/// 缓存存储配置 — 瓦片缓存 + 会话缓存。
///
/// 未配置 `[cache]` 节时默认 `local` (瓦片缓存落盘 `<data_dir>/gwc`, 会话缓存内存写穿透)。
/// 后续可扩展 `redis` 等后端 (见 [`crate::store::cache`])。
/// 兼容旧节名 `[gwc]` (serde alias, 声明在字段 `cache` 上)。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CacheConfig {
    /// 缓存后端类型: "local" (默认) | 未来 "redis"
    #[serde(default = "default_cache_kind")]
    pub kind: String,
    /// 瓦片缓存根目录 (默认: `<data_dir>/gwc`)
    pub cache_dir: PathBuf,
    /// 缓存元数据目录 (默认: `<data_dir>/gwc/meta`)
    pub meta_dir: PathBuf,
    /// 瓦片过期时间 (秒, 0=永不过期)
    #[serde(default = "default_expire")]
    pub expire_after_secs: u64,
    /// 最大缓存瓦片数 (0=无限制)
    #[serde(default)]
    pub max_tiles: u64,
    /// 是否启用缓存
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// 默认 gridset
    #[serde(default = "default_gridset")]
    pub default_gridset: String,
    /// 会话缓存 TTL (秒; local 内存会话缓存生效)
    #[serde(default = "default_session_ttl")]
    pub session_ttl_secs: u64,
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
    /// PostgreSQL 实例 (数据库名)
    #[serde(default = "default_db_instance")]
    pub instance: String,
    /// PostgreSQL 模式 (表所在 schema; 默认: "public")
    #[serde(default = "default_pg_schema")]
    pub schema: String,
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

fn default_vector_kind() -> String {
    "local".to_string()
}

fn default_raster_kind() -> String {
    "local".to_string()
}

fn default_cache_kind() -> String {
    "local".to_string()
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

fn default_db_instance() -> String {
    "geoserver".to_string()
}

fn default_pg_schema() -> String {
    "public".to_string()
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
    "geoferris-jwt-secret-2026".to_string()
}

fn default_expire() -> u64 { 86400 }
fn default_enabled() -> bool { true }
fn default_gridset() -> String { "EPSG:4326".to_string() }
fn default_session_ttl() -> u64 { 86400 }

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
            host: "127.0.0.1".to_string(),
            port: 8080,
            api_context: "/geoserver".to_string(),
            static_dir: default_static_dir(),
            connect_timeout_secs: default_connect_timeout(),
            shutdown_timeout_secs: default_shutdown_timeout(),
        }
    }
}

impl Default for MetadataConfig {
    fn default() -> Self {
        MetadataConfig {
            kind: default_db_kind(),
            sqlite_path: default_sqlite_path(),
            postgres: PostgresConfig::default(),
        }
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        SecurityConfig {
            jwt_secret: default_jwt_secret(),
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        LoggingConfig {
            level: default_log_level(),
        }
    }
}

impl Default for PostgresConfig {
    fn default() -> Self {
        PostgresConfig {
            host: default_db_host(),
            port: default_db_port(),
            instance: default_db_instance(),
            schema: default_pg_schema(),
            user: default_db_user(),
            password: default_db_password(),
            pool_size: default_db_pool_size(),
        }
    }
}

impl Default for RasterConfig {
    fn default() -> Self {
        RasterConfig {
            kind: default_raster_kind(),
            dir: None,
        }
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        CacheConfig {
            kind: default_cache_kind(),
            cache_dir: PathBuf::from("./data/gwc"),
            meta_dir: PathBuf::from("./data/gwc/meta"),
            expire_after_secs: default_expire(),
            max_tiles: 100_000,
            enabled: default_enabled(),
            default_gridset: default_gridset(),
            session_ttl_secs: default_session_ttl(),
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

fn default_shutdown_timeout() -> u64 {
    30
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
                shutdown_timeout_secs: default_shutdown_timeout(),
            },
            metadata: MetadataConfig {
                kind: default_db_kind(),
                sqlite_path: default_sqlite_path(),
                postgres: PostgresConfig::default(),
            },
            vector: None,
            raster: None,
            cache: Some(CacheConfig {
                cache_dir: PathBuf::from("./data/gwc"),
                meta_dir: PathBuf::from("./data/gwc/meta"),
                ..Default::default()
            }),
            security: SecurityConfig {
                jwt_secret: default_jwt_secret(),
            },
            logging: LoggingConfig {
                level: default_log_level(),
            },
            data_dir: PathBuf::from("./data"),
            workspaces: vec![],
            cors: CorsConfig::default(),
        }
    }
}

impl GeoServerConfig {
    /// 解析生效的矢量数据存储配置 (应用默认规则)。
    ///
    /// - 显式配置了 `[vector]` → 直接使用 (dir 为空时回退 `<data_dir>/business`)
    /// - 未配置:
    ///   - 元数据存储非 sqlite (外部存储) → 复用元数据存储 (kind = "metadata", 内置默认选项)
    ///   - 元数据存储为 sqlite → 本地目录 (kind = "local", 默认 `<data_dir>/business`)
    pub fn effective_vector(&self) -> VectorConfig {
        let mut vc = self.vector.clone().unwrap_or_else(|| {
            if self.metadata.kind != "sqlite" {
                VectorConfig {
                    kind: "metadata".to_string(),
                    dir: None,
                    postgres: PostgresConfig::default(),
                }
            } else {
                VectorConfig {
                    kind: "local".to_string(),
                    dir: None,
                    postgres: PostgresConfig::default(),
                }
            }
        });
        if vc.dir.is_none() {
            vc.dir = Some(self.data_dir.join("business"));
        }
        vc
    }

    /// 解析生效的栅格存储配置 (默认 `local`, 目录 `<data_dir>/rasters`)。
    pub fn effective_raster(&self) -> RasterConfig {
        let mut rc = self.raster.clone().unwrap_or_default();
        if rc.dir.is_none() {
            rc.dir = Some(self.data_dir.join("rasters"));
        }
        rc
    }

    /// 解析生效的缓存存储配置。
    ///
    /// - 未配置 `[cache]` → 默认 `local` 缓存, 瓦片目录锚定到 `<data_dir>/gwc`
    /// - 显式配置 → 按原样使用 (目录字段相对路径保持 CWD 语义, 与 `effective_vector` 一致)
    pub fn effective_cache(&self) -> CacheConfig {
        match self.cache.clone() {
            Some(c) => c,
            None => CacheConfig {
                cache_dir: self.data_dir.join("gwc"),
                meta_dir: self.data_dir.join("gwc").join("meta"),
                ..Default::default()
            },
        }
    }

    pub fn load() -> Result<Self, config::ConfigError> {
        let config = Config::builder()
            .add_source(File::with_name("geoferris").required(false))
            .add_source(config::Environment::with_prefix("GEOSERVER").separator("__"))
            .build()?;

        config.try_deserialize()
    }

    pub fn load_from_file(path: &str) -> Result<Self, config::ConfigError> {
        let mut builder = Config::builder();

        // 候选配置文件: 先 CWD 下的 geoferris.toml, 再回退到可执行文件所在目录
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

#[cfg(test)]
mod tests {
    use super::*;
    use config::{Config, File, FileFormat};

    fn parse_toml(content: &str) -> GeoServerConfig {
        Config::builder()
            .add_source(File::from_str(content, FileFormat::Toml))
            .build()
            .unwrap()
            .try_deserialize()
            .unwrap()
    }

    #[test]
    fn test_new_sections_parse() {
        let cfg = parse_toml(
            r#"
            [vector]
            kind = "local"
            [raster]
            kind = "local"
            [cache]
            kind = "local"
            cache_dir = "./tmp/gwc"
            meta_dir = "./tmp/gwc/meta"
            "#,
        );
        assert_eq!(cfg.vector.unwrap().kind, "local");
        assert_eq!(cfg.raster.unwrap().kind, "local");
        let c = cfg.cache.unwrap();
        assert_eq!(c.kind, "local");
        assert_eq!(c.cache_dir, PathBuf::from("./tmp/gwc"));
        assert_eq!(c.meta_dir, PathBuf::from("./tmp/gwc/meta"));
    }

    #[test]
    fn test_legacy_sections_alias() {
        // 旧节名 [business] / [gwc] 通过 serde alias 映射到 vector / cache
        let cfg = parse_toml(
            r#"
            [business]
            kind = "metadata"
            [gwc]
            cache_dir = "./data/gwc"
            meta_dir = "./data/gwc/meta"
            "#,
        );
        assert_eq!(cfg.vector.unwrap().kind, "metadata");
        let c = cfg.cache.unwrap();
        assert_eq!(c.cache_dir, PathBuf::from("./data/gwc"));
    }

    #[test]
    fn test_effective_defaults() {
        let cfg = GeoServerConfig::default();
        assert_eq!(cfg.effective_vector().kind, "local");
        assert_eq!(cfg.effective_vector().dir.unwrap(), PathBuf::from("./data/business"));

        let r = cfg.effective_raster();
        assert_eq!(r.kind, "local");
        assert_eq!(r.dir.unwrap(), PathBuf::from("./data/rasters"));

        let c = cfg.effective_cache();
        assert_eq!(c.kind, "local");
        assert_eq!(c.cache_dir, PathBuf::from("./data/gwc"));
        assert_eq!(c.meta_dir, PathBuf::from("./data/gwc/meta"));
        assert_eq!(c.session_ttl_secs, 86400);
    }
}
