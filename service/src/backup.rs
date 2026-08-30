//! # 备份与恢复
//!
//! 支持将 Terrane 全部配置导出为 JSON 文件，
//! 以及从 JSON 文件恢复配置。
//!
//! ## 备份内容
//! - 工作空间 (Workspaces)
//! - 数据源 (DataSources)
//! - 图层 (Layers)
//! - 样式 (Styles + SLD)
//! - 图层组 (LayerGroups)
//! - 命名空间 (Namespaces)
//! - SQL 视图 (SqlViews)
//! - 权限 (Permissions)
//! - 用户 (Users)

use crate::auth::UserRole;
use crate::models::permission::{AccessMode, Effect, Permission};
use crate::models::DataSourceType;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tracing::info;

/// 完整备份数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerraneBackup {
    /// 备份版本
    pub version: String,
    /// 创建时间
    pub created_at: String,
    /// 服务器信息
    pub server_info: serde_json::Value,
    /// 工作空间
    pub workspaces: Vec<WorkspaceBackup>,
    /// 数据源
    pub data_sources: Vec<DataSourceBackup>,
    /// 图层 (in-memory)
    pub layers: Vec<LayerBackup>,
    /// 样式 (SLD)
    pub styles: Vec<StyleBackup>,
    /// 图层组
    pub layer_groups: Vec<LayerGroupBackup>,
    /// 命名空间
    pub namespaces: Vec<NamespaceBackup>,
    /// SQL 视图
    pub sql_views: Vec<SqlViewBackup>,
    /// 权限
    pub permissions: Vec<PermissionBackup>,
    /// 用户
    pub users: Vec<UserBackup>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceBackup {
    pub name: String,
    pub title: String,
    pub description: String,
    pub enabled: bool,
    pub layer_count: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSourceBackup {
    pub name: String,
    pub data_source_type: String,
    pub workspace: Option<String>,
    pub enabled: bool,
    pub connection: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerBackup {
    pub name: String,
    pub title: String,
    pub workspace: String,
    pub store: String,
    pub native_name: Option<String>,
    pub srs: String,
    pub abstract_text: Option<String>,
    pub enabled: bool,
    pub minx: f64,
    pub miny: f64,
    pub maxx: f64,
    pub maxy: f64,
    /// 瓦片缓存后端数据源名称 (type = "redis"); 为空 = 默认内存/本地缓存
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_store: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleBackup {
    pub name: String,
    pub title: String,
    pub content: String,
    pub format: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerGroupBackup {
    pub name: String,
    pub title: String,
    pub layers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamespaceBackup {
    pub prefix: String,
    pub uri: String,
    pub isolated: bool,
    pub workspace: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqlViewBackup {
    pub name: String,
    pub sql: String,
    pub workspace: String,
    pub store: String,
    pub geometry_column: String,
    pub geometry_type: String,
    pub crs: String,
    pub parameters: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionBackup {
    pub username: String,
    pub role: String,
    pub resource_type: String,
    pub resource_name: String,
    pub access_mode: String,
    pub effect: String,
    pub priority: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserBackup {
    pub username: String,
    pub password_hash: String,
    pub salt: String,
    pub role: String,
    pub enabled: bool,
}

// ---------------------------------------------------------------------------
// 导出
// ---------------------------------------------------------------------------

/// 导出 Terrane 全部配置为备份对象
pub async fn export_backup(state: &crate::state::AppState) -> Result<TerraneBackup, String> {
    let store = state.store.as_ref().ok_or("数据库不可用，无法备份")?;

    info!("[Backup] 开始导出配置");

    // 工作空间
    let ws_records = store
        .get_all_workspaces()
        .await
        .map_err(|e| format!("读取工作空间失败: {}", e))?;
    let workspaces: Vec<WorkspaceBackup> = ws_records
        .into_iter()
        .map(|w| WorkspaceBackup {
            name: w.name,
            title: w.title,
            description: w.description,
            enabled: w.enabled,
            layer_count: w.layer_count,
        })
        .collect();

    // 数据源
    let ds_records = store
        .get_all_data_sources()
        .await
        .map_err(|e| format!("读取数据源失败: {}", e))?;
    let data_sources: Vec<DataSourceBackup> = ds_records
        .into_iter()
        .map(|ds| DataSourceBackup {
            name: ds.name,
            data_source_type: ds.data_source_type.to_string(),
            workspace: ds.workspace,
            enabled: ds.enabled,
            connection: serde_json::to_value(&ds.connection).unwrap_or_default(),
        })
        .collect();

    // 图层 (从 SQLite)
    let layer_records = store
        .get_all_layers()
        .await
        .map_err(|e| format!("读取图层失败: {}", e))?;
    let layers: Vec<LayerBackup> = layer_records
        .into_iter()
        .map(|l| LayerBackup {
            name: l.name,
            title: l.title,
            workspace: l.workspace,
            store: l.store,
            native_name: l.native_name,
            srs: l.srs,
            abstract_text: l.abstract_text,
            enabled: l.enabled,
            minx: l.minx,
            miny: l.miny,
            maxx: l.maxx,
            maxy: l.maxy,
            cache_store: l.cache_store,
        })
        .collect();

    // 样式
    let styles_lock = state.styles.read().await;
    let styles_meta = state.styles_meta.read().await;
    let style_list: Vec<StyleBackup> = styles_lock
        .iter()
        .map(|(name, content)| {
            let title = styles_meta
                .get(name)
                .map(|m| m.title.clone())
                .unwrap_or_default();
            let format = styles_meta.get(name).map(|m| m.format.to_string());
            StyleBackup {
                name: name.clone(),
                title,
                content: content.clone(),
                format,
            }
        })
        .collect();
    let styles = style_list;
    drop(styles_lock);
    drop(styles_meta);

    // 图层组
    let groups_lock = state.layer_groups.read().await;
    let layer_groups: Vec<LayerGroupBackup> = groups_lock
        .iter()
        .map(|g| LayerGroupBackup {
            name: g.name.clone(),
            title: g.title.clone(),
            layers: g.layers.clone(),
        })
        .collect();
    drop(groups_lock);

    // 命名空间
    let ns_records = store
        .get_all_namespaces()
        .await
        .map_err(|e| format!("读取命名空间失败: {}", e))?;
    let namespaces: Vec<NamespaceBackup> = ns_records
        .into_iter()
        .map(|ns| NamespaceBackup {
            prefix: ns.prefix,
            uri: ns.uri,
            isolated: ns.isolated,
            workspace: ns.workspace,
        })
        .collect();

    // SQL 视图
    let sv_records = store
        .get_all_sql_views()
        .await
        .map_err(|e| format!("读取 SQL 视图失败: {}", e))?;
    let sql_views: Vec<SqlViewBackup> = sv_records
        .into_iter()
        .map(|v| SqlViewBackup {
            name: v.name,
            sql: v.sql,
            workspace: v.workspace,
            store: v.store,
            geometry_column: v.geometry_column,
            geometry_type: v.geometry_type,
            crs: v.crs,
            parameters: serde_json::to_string(&v.parameters).unwrap_or_default(),
            description: v.description,
        })
        .collect();

    // 权限
    let perm_records = store
        .get_permissions()
        .await
        .map_err(|e| format!("读取权限失败: {}", e))?;
    let permissions: Vec<PermissionBackup> = perm_records
        .into_iter()
        .map(|p| PermissionBackup {
            username: p.username,
            role: p.role,
            resource_type: p.resource_type,
            resource_name: p.resource_name,
            access_mode: p.access_mode.to_string(),
            effect: p.effect.to_string(),
            priority: p.priority,
        })
        .collect();

    // 用户
    let user_records = store
        .get_all_users()
        .await
        .map_err(|e| format!("读取用户失败: {}", e))?;
    let users: Vec<UserBackup> = user_records
        .into_iter()
        .map(|u| UserBackup {
            username: u.username,
            password_hash: u.password_hash,
            salt: u.salt,
            role: u.role.to_string(),
            enabled: u.enabled,
        })
        .collect();

    info!(
        "[Backup] 导出完成: {} 个工作空间, {} 个数据源, {} 个图层, {} 个样式",
        workspaces.len(),
        data_sources.len(),
        layers.len(),
        styles.len()
    );

    Ok(TerraneBackup {
        version: "1.0".to_string(),
        created_at: Utc::now().to_rfc3339(),
        server_info: serde_json::json!({
            "version": env!("CARGO_PKG_VERSION"),
            "name": "terrane",
        }),
        workspaces,
        data_sources,
        layers,
        styles,
        layer_groups,
        namespaces,
        sql_views,
        permissions,
        users,
    })
}

// ---------------------------------------------------------------------------
// 导入
// ---------------------------------------------------------------------------

/// 从备份对象恢复配置
pub async fn import_backup(
    state: &crate::state::AppState,
    backup: &TerraneBackup,
) -> Result<ImportReport, String> {
    let store = state.store.as_ref().ok_or("数据库不可用，无法恢复")?;

    let mut report = ImportReport::new();

    info!("[Backup] 开始导入配置 (版本: {})", backup.version);

    // 1. 导入工作空间
    for ws in &backup.workspaces {
        let req = crate::handlers::CreateWorkspaceRequest {
            name: ws.name.clone(),
            title: Some(ws.title.clone()),
            description: Some(ws.description.clone()),
        };
        match store.create_workspace(&req).await {
            Ok(_) => report.workspaces_imported += 1,
            Err(e) => report.errors.push(format!("工作空间 '{}': {}", ws.name, e)),
        }
    }

    // 2. 导入数据源
    for ds in &backup.data_sources {
        let conn_val: crate::models::DataSourceConnection =
            match serde_json::from_value(ds.connection.clone()) {
                Ok(c) => c,
                Err(_) => {
                    report
                        .errors
                        .push(format!("数据源 '{}': 连接信息解析失败", ds.name));
                    continue;
                },
            };
        let ds_type = match ds.data_source_type.as_str() {
            "shapefile" => DataSourceType::Shapefile,
            "geotiff" => DataSourceType::Geotiff,
            "geopackage" => DataSourceType::Geopackage,
            "worldimage" => DataSourceType::WorldImage,
            "cascaded_wms" => DataSourceType::CascadedWms,
            "redis" => DataSourceType::Redis,
            "arcgrid" => DataSourceType::ArcGrid,
            _ => DataSourceType::Postgis,
        };
        match store
            .create_data_source(
                &ds.name,
                &ds_type,
                ds.workspace.clone(),
                ds.enabled,
                &conn_val,
            )
            .await
        {
            Ok(_) => report.data_sources_imported += 1,
            Err(e) => report.errors.push(format!("数据源 '{}': {}", ds.name, e)),
        }
    }

    // 3. 导入图层
    for l in &backup.layers {
        let layer = crate::store::Layer {
            name: l.name.clone(),
            title: l.title.clone(),
            workspace: l.workspace.clone(),
            store: l.store.clone(),
            srs: l.srs.clone(),
            abstract_text: l.abstract_text.clone(),
            native_name: l.native_name.clone(),
            enabled: l.enabled,
            minx: l.minx,
            miny: l.miny,
            maxx: l.maxx,
            maxy: l.maxy,
            cache_store: l.cache_store.clone(),
            created: String::new(),
            modified: String::new(),
        };
        match store.create_layer(&layer).await {
            Ok(_) => {
                report.layers_imported += 1;
                state.layers.write().await.push(crate::models::Layer::new(
                    l.name.clone(),
                    l.title.clone(),
                    l.workspace.clone(),
                    l.store.clone(),
                    crate::models::CoordinateReferenceSystem::from_epsg(&l.srs),
                ));
            },
            Err(e) => report.errors.push(format!("图层 '{}': {}", l.name, e)),
        }
    }

    // 4. 导入样式
    for st in &backup.styles {
        state
            .styles
            .write()
            .await
            .insert(st.name.clone(), st.content.clone());
        let format = st
            .format
            .as_deref()
            .and_then(|f| match f {
                "CSS" => Some(crate::models::style::StyleFormat::CSS),
                "YSLD" => Some(crate::models::style::StyleFormat::YSLD),
                "MBStyle" => Some(crate::models::style::StyleFormat::MBStyle),
                _ => None,
            })
            .unwrap_or_else(|| crate::models::style::detect_style_format(&st.content));
        state.styles_meta.write().await.insert(
            st.name.clone(),
            crate::state::StyleMeta {
                title: st.title.clone(),
                is_builtin: false,
                format: format.clone(),
            },
        );
        if let Some(s) = state.store.as_ref() {
            let ts = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
            let _ = s
                .create_style(&crate::store::StyleRecord {
                    name: st.name.clone(),
                    title: st.title.clone(),
                    format: format.to_string(),
                    is_builtin: false,
                    content: st.content.clone(),
                    created: ts.clone(),
                    modified: ts,
                })
                .await;
        }
        report.styles_imported += 1;
    }

    // 5. 导入命名空间
    for ns in &backup.namespaces {
        match store
            .create_namespace(&ns.prefix, &ns.uri, ns.workspace.as_deref(), ns.isolated)
            .await
        {
            Ok(_) => report.namespaces_imported += 1,
            Err(e) => report
                .errors
                .push(format!("命名空间 '{}': {}", ns.prefix, e)),
        }
    }

    // 6. 导入图层组
    for lg in &backup.layer_groups {
        state
            .layer_groups
            .write()
            .await
            .push(crate::models::layer::LayerGroup {
                name: lg.name.clone(),
                title: lg.title.clone(),
                layers: lg.layers.clone(),
                styles: vec![],
            });
        if let Some(s) = state.store.as_ref() {
            let ts = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
            let _ = s
                .create_layer_group(&crate::store::LayerGroupRecord {
                    name: lg.name.clone(),
                    title: lg.title.clone(),
                    layers: lg.layers.clone(),
                    styles: vec![],
                    created: ts.clone(),
                    modified: ts,
                })
                .await;
        }
        report.layer_groups_imported += 1;
    }

    // 7. 导入 SQL 视图
    for sv in &backup.sql_views {
        let params: Vec<crate::models::sql_view::SqlViewParameter> =
            serde_json::from_str(&sv.parameters).unwrap_or_default();
        let view = crate::models::sql_view::SqlView {
            name: sv.name.clone(),
            sql: sv.sql.clone(),
            workspace: sv.workspace.clone(),
            store: sv.store.clone(),
            geometry_column: sv.geometry_column.clone(),
            geometry_type: sv.geometry_type.clone(),
            crs: sv.crs.clone(),
            parameters: params,
            description: sv.description.clone(),
            created: Utc::now().to_rfc3339(),
            modified: Utc::now().to_rfc3339(),
        };
        match store.create_sql_view(&view).await {
            Ok(_) => report.sql_views_imported += 1,
            Err(e) => report.errors.push(format!("SQL 视图 '{}': {}", sv.name, e)),
        }
    }

    // 8. 导入权限
    for p in &backup.permissions {
        let perm = Permission {
            id: None,
            username: p.username.clone(),
            role: p.role.clone(),
            resource_type: p.resource_type.clone(),
            resource_name: p.resource_name.clone(),
            access_mode: match p.access_mode.as_str() {
                "write" => AccessMode::Write,
                "admin" => AccessMode::Admin,
                _ => AccessMode::Read,
            },
            effect: match p.effect.as_str() {
                "deny" => Effect::Deny,
                _ => Effect::Allow,
            },
            priority: p.priority,
        };
        match store.create_permission(&perm).await {
            Ok(_) => report.permissions_imported += 1,
            Err(e) => report.errors.push(format!("权限: {}", e)),
        }
    }

    // 9. 导入用户
    for u in &backup.users {
        let role = match u.role.as_str() {
            "admin" => UserRole::Admin,
            "manager" => UserRole::Manager,
            "guest" => UserRole::Guest,
            _ => UserRole::User,
        };
        match store
            .create_user(&u.username, &u.password_hash, &u.salt, &role, u.enabled)
            .await
        {
            Ok(_) => report.users_imported += 1,
            Err(e) => report.errors.push(format!("用户 '{}': {}", u.username, e)),
        }
    }

    info!("[Backup] 导入完成: {}", report.summary());
    Ok(report)
}

/// 导入报告
#[derive(Debug, Clone, Serialize)]
pub struct ImportReport {
    pub workspaces_imported: usize,
    pub data_sources_imported: usize,
    pub layers_imported: usize,
    pub styles_imported: usize,
    pub layer_groups_imported: usize,
    pub namespaces_imported: usize,
    pub sql_views_imported: usize,
    pub permissions_imported: usize,
    pub users_imported: usize,
    pub errors: Vec<String>,
}

impl ImportReport {
    pub fn new() -> Self {
        ImportReport {
            workspaces_imported: 0,
            data_sources_imported: 0,
            layers_imported: 0,
            styles_imported: 0,
            layer_groups_imported: 0,
            namespaces_imported: 0,
            sql_views_imported: 0,
            permissions_imported: 0,
            users_imported: 0,
            errors: vec![],
        }
    }
    pub fn summary(&self) -> String {
        format!(
            "工作空间={}, 数据源={}, 图层={}, 样式={}, 图层组={}, 命名空间={}, SQL视图={}, 权限={}, 用户={} (错误={})",
            self.workspaces_imported, self.data_sources_imported,
            self.layers_imported, self.styles_imported, self.layer_groups_imported,
            self.namespaces_imported, self.sql_views_imported,
            self.permissions_imported, self.users_imported,
            self.errors.len(),
        )
    }
}

impl Default for ImportReport {
    fn default() -> Self {
        Self::new()
    }
}
