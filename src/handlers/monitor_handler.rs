//! 监控面板处理器
//!
//! 提供服务器请求统计、性能监控、审计日志查询。
//! 端点: GET /monitor/stats, GET /monitor/requests, GET /monitor/logs

use actix_web::{HttpRequest, HttpResponse, web};
use serde::Serialize;
use crate::state::{AppState, EndpointStats, RequestRecord};
use crate::error::GeoServerError;
use std::collections::HashMap;
use std::sync::atomic::Ordering;

/// 监控统计响应
#[derive(Debug, Serialize)]
pub struct MonitorStats {
    /// 服务器启动时间
    pub uptime_seconds: u64,
    /// 总请求数
    pub total_requests: u64,
    /// 总错误数
    pub total_errors: u64,
    /// 错误率 (%)
    pub error_rate: f64,
    /// 每秒请求数 (近 5 分钟平均)
    pub requests_per_second: f64,
    /// 各端点请求统计
    pub endpoints: HashMap<String, EndpointStats>,
    /// 各 HTTP 方法统计
    pub methods: HashMap<String, u64>,
    /// 各状态码统计
    pub status_codes: HashMap<u16, u64>,
    /// 系统信息
    pub system: SystemInfo,
}

#[derive(Debug, Serialize)]
pub struct SystemInfo {
    pub version: String,
    pub rust_version: String,
    pub os: String,
    pub hostname: String,
    pub cpu_cores: u32,
    pub memory_mb: u64,
}

/// 审计日志条目
#[derive(Debug, Clone, Serialize)]
pub struct AuditLogEntry {
    pub id: i64,
    pub timestamp: String,
    pub action: String,
    pub username: String,
    pub resource: Option<String>,
    pub detail: Option<String>,
}

/// 获取监控统计
pub async fn get_monitor_stats(
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let uptime = state.start_time.elapsed().as_secs();
    let total_reqs = state.request_count.load(Ordering::Relaxed);
    let total_errs = state.error_count.load(Ordering::Relaxed);
    let error_rate = if total_reqs > 0 {
        (total_errs as f64 / total_reqs as f64) * 100.0
    } else {
        0.0
    };

    // 收集端点统计
    let endpoints = state.endpoint_stats.read().await;
    let methods = state.method_stats.read().await;
    let status_codes = state.status_code_stats.read().await;

    // 计算 RPS (近5分钟)
    let recent_count = state.recent_request_count.load(Ordering::Relaxed);
    let rps = recent_count as f64 / 300.0; // 5分钟

    // 系统信息
    let system = SystemInfo {
        version: "1.0.0".to_string(),
        rust_version: format!("{}.{}.{}", 
            std::env::var("CARGO_PKG_RUST_VERSION").unwrap_or_else(|_| "1.75".to_string()),
            "", ""),
        os: std::env::consts::OS.to_string(),
        hostname: hostname(),
        cpu_cores: num_cpus() as u32,
        memory_mb: total_memory_mb(),
    };

    Ok(HttpResponse::Ok().json(MonitorStats {
        uptime_seconds: uptime,
        total_requests: total_reqs,
        total_errors: total_errs,
        error_rate,
        requests_per_second: rps,
        endpoints: endpoints.clone(),
        methods: methods.clone(),
        status_codes: status_codes.clone(),
        system,
    }))
}

/// 获取最近请求记录
pub async fn get_recent_requests(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let query = req.query_string();
    let limit: usize = query.split('&')
        .find(|p| p.starts_with("limit="))
        .and_then(|p| p.split('=').nth(1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(100);

    let records = state.request_log.read().await;
    let recent: Vec<RequestRecord> = records.iter().rev().take(limit).cloned().collect();

    Ok(HttpResponse::Ok().json(recent))
}

/// 获取审计日志
pub async fn get_audit_logs(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    use crate::store::sqlite_store::AuditLogRecord;

    if let Some(ref store) = state.store {
        let query = req.query_string();
        let limit: usize = query.split('&')
            .find(|p| p.starts_with("limit="))
            .and_then(|p| p.split('=').nth(1))
            .and_then(|v| v.parse().ok())
            .unwrap_or(100);
        let offset: usize = query.split('&')
            .find(|p| p.starts_with("offset="))
            .and_then(|p| p.split('=').nth(1))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);

        let logs = store.get_audit_logs(limit, offset).await
            .map_err(|e| GeoServerError::InternalError(format!("读取审计日志失败: {}", e)))?;

        let entries: Vec<AuditLogEntry> = logs.into_iter().map(|log| AuditLogEntry {
            id: log.id,
            timestamp: log.created_at,
            action: log.action,
            username: log.username,
            resource: log.resource,
            detail: log.detail,
        }).collect();

        Ok(HttpResponse::Ok().json(entries))
    } else {
        Ok(HttpResponse::Ok().json(Vec::<AuditLogEntry>::new()))
    }
}

/// 清除监控统计
pub async fn reset_monitor_stats(
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    state.request_count.store(0, Ordering::Relaxed);
    state.error_count.store(0, Ordering::Relaxed);
    state.recent_request_count.store(0, Ordering::Relaxed);
    state.endpoint_stats.write().await.clear();
    state.method_stats.write().await.clear();
    state.status_code_stats.write().await.clear();
    state.request_log.write().await.clear();

    Ok(HttpResponse::Ok().json(serde_json::json!({"status": "ok", "message": "监控统计已重置"})))
}

// ==================== 辅助函数 ====================

fn hostname() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "unknown".to_string())
}

fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

fn total_memory_mb() -> u64 {
    // 通过 sysinfo crate 获取系统内存
    use sysinfo::System;
    let mut sys = System::new();
    sys.refresh_memory();
    sys.total_memory() / 1024 / 1024
}
