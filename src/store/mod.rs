pub mod cache;
pub mod error;
pub mod file_store;
pub mod postgres_store;
pub mod sqlite_store;
pub mod types;

pub use cache::{build_session_cache, SessionCache};
pub use error::StoreError;
pub use file_store::{FileStore, LocalFileStore};
pub use postgres_store::PostgresStore;
pub use sqlite_store::SqliteStore;
pub use types::{
    AuditLogRecord, Layer, LayerGroupRecord, NamespaceRecord, SessionRecord, StyleRecord, Workspace,
};

use crate::auth::{User, UserRole};
use crate::handlers::CreateWorkspaceRequest;
use crate::models::permission::Permission;
use crate::models::sql_view::SqlView;
use crate::models::{DataSource, DataSourceConnection, DataSourceType};
use async_trait::async_trait;

/// 存储抽象层 — SqliteStore 与 PostgresStore 共同实现。
///
/// 集群部署时使用 PostgresStore，本地开发默认 SqliteStore。
#[async_trait]
pub trait Store: Send + Sync {
    // ---- 工作空间 ----
    async fn get_workspace(&self, name: &str) -> Result<Option<Workspace>, StoreError>;
    async fn get_all_workspaces(&self) -> Result<Vec<Workspace>, StoreError>;
    async fn create_workspace(
        &self,
        request: &CreateWorkspaceRequest,
    ) -> Result<Workspace, StoreError>;
    async fn update_workspace(
        &self,
        name: &str,
        title: Option<String>,
        description: Option<String>,
        enabled: Option<bool>,
    ) -> Result<(), StoreError>;
    async fn delete_workspace(&self, name: &str) -> Result<(), StoreError>;

    // ---- 命名空间 ----
    async fn get_namespace(&self, prefix: &str) -> Result<Option<NamespaceRecord>, StoreError>;
    async fn get_all_namespaces(&self) -> Result<Vec<NamespaceRecord>, StoreError>;
    async fn create_namespace(
        &self,
        prefix: &str,
        uri: &str,
        workspace: Option<&str>,
        isolated: bool,
    ) -> Result<NamespaceRecord, StoreError>;
    async fn update_namespace(
        &self,
        prefix: &str,
        uri: Option<String>,
        isolated: Option<bool>,
        workspace: Option<String>,
    ) -> Result<(), StoreError>;
    async fn delete_namespace(&self, prefix: &str) -> Result<(), StoreError>;

    // ---- 数据源 ----
    async fn get_data_source(&self, name: &str) -> Result<Option<DataSource>, StoreError>;
    async fn get_all_data_sources(&self) -> Result<Vec<DataSource>, StoreError>;
    async fn create_data_source(
        &self,
        name: &str,
        data_source_type: &DataSourceType,
        workspace: Option<String>,
        enabled: bool,
        connection: &DataSourceConnection,
    ) -> Result<DataSource, StoreError>;
    async fn update_data_source(
        &self,
        name: &str,
        data_source_type: Option<DataSourceType>,
        workspace: Option<String>,
        enabled: Option<bool>,
        connection: Option<DataSourceConnection>,
    ) -> Result<(), StoreError>;
    async fn delete_data_source(&self, name: &str) -> Result<(), StoreError>;

    // ---- 图层 ----
    async fn get_layer(&self, name: &str) -> Result<Option<Layer>, StoreError>;
    async fn get_all_layers(&self) -> Result<Vec<Layer>, StoreError>;
    async fn create_layer(&self, layer: &Layer) -> Result<Layer, StoreError>;
    async fn update_layer(
        &self,
        name: &str,
        title: Option<String>,
        abstract_text: Option<String>,
        native_name: Option<String>,
        enabled: Option<bool>,
    ) -> Result<(), StoreError>;
    async fn delete_layer(&self, name: &str) -> Result<(), StoreError>;

    // ---- SQL 视图 ----
    async fn get_sql_view(&self, name: &str) -> Result<Option<SqlView>, StoreError>;
    async fn get_all_sql_views(&self) -> Result<Vec<SqlView>, StoreError>;
    async fn create_sql_view(&self, view: &SqlView) -> Result<(), StoreError>;
    async fn update_sql_view(
        &self,
        name: &str,
        sql: Option<String>,
        geometry_column: Option<String>,
        geometry_type: Option<String>,
        crs: Option<String>,
        parameters: Option<Vec<crate::models::sql_view::SqlViewParameter>>,
        description: Option<String>,
    ) -> Result<(), StoreError>;
    async fn delete_sql_view(&self, name: &str) -> Result<(), StoreError>;

    // ---- 用户 ----
    async fn get_user(&self, username: &str) -> Result<Option<User>, StoreError>;
    async fn get_all_users(&self) -> Result<Vec<User>, StoreError>;
    async fn create_user(
        &self,
        username: &str,
        password_hash: &str,
        salt: &str,
        role: &UserRole,
        enabled: bool,
    ) -> Result<(), StoreError>;
    async fn update_user(
        &self,
        username: &str,
        role: Option<&UserRole>,
        enabled: Option<bool>,
    ) -> Result<(), StoreError>;
    async fn delete_user(&self, username: &str) -> Result<(), StoreError>;

    // ---- 审计日志 ----
    async fn audit_log(
        &self,
        username: &str,
        action: &str,
        resource: Option<&str>,
        detail: Option<&str>,
        ip_address: Option<&str>,
    ) -> Result<(), StoreError>;
    async fn get_audit_logs(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<AuditLogRecord>, StoreError>;

    // ---- 权限 ----
    async fn get_permissions(&self) -> Result<Vec<Permission>, StoreError>;
    async fn create_permission(&self, p: &Permission) -> Result<i64, StoreError>;
    async fn delete_permission(&self, id: i64) -> Result<(), StoreError>;
    async fn check_permission(
        &self,
        username: &str,
        role: &str,
        resource_type: &str,
        resource_name: &str,
        required_mode: &str,
    ) -> Result<bool, StoreError>;

    // ---- 样式 ----
    async fn get_all_styles(&self) -> Result<Vec<StyleRecord>, StoreError>;
    async fn get_style(&self, name: &str) -> Result<Option<StyleRecord>, StoreError>;
    async fn create_style(&self, style: &StyleRecord) -> Result<(), StoreError>;
    async fn update_style(
        &self,
        name: &str,
        title: Option<String>,
        format: Option<String>,
        content: Option<String>,
        is_builtin: Option<bool>,
    ) -> Result<(), StoreError>;
    async fn delete_style(&self, name: &str) -> Result<(), StoreError>;

    // ---- 图层组 ----
    async fn get_all_layer_groups(&self) -> Result<Vec<LayerGroupRecord>, StoreError>;
    async fn get_layer_group(&self, name: &str) -> Result<Option<LayerGroupRecord>, StoreError>;
    async fn create_layer_group(&self, group: &LayerGroupRecord) -> Result<(), StoreError>;
    async fn delete_layer_group(&self, name: &str) -> Result<(), StoreError>;

    // ---- 会话 ----
    async fn create_session(&self, session: &SessionRecord) -> Result<(), StoreError>;
    async fn get_session(&self, jti: &str) -> Result<Option<SessionRecord>, StoreError>;
    async fn delete_session(&self, jti: &str) -> Result<(), StoreError>;
    async fn delete_user_sessions(&self, username: &str) -> Result<(), StoreError>;
    async fn cleanup_expired_sessions(&self) -> Result<usize, StoreError>;
}
