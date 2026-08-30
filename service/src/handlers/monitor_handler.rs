//! 监控面板处理器
//!
//! 提供服务器请求统计、性能监控、审计日志查询。
//! 端点: GET /monitor/stats, GET /monitor/requests, GET /monitor/logs

use crate::error::TerraneError;
use crate::state::{AppState, EndpointStats, RequestRecord};
use actix_web::{web, HttpRequest, HttpResponse};
use serde::Serialize;
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
pub async fn get_monitor_stats(state: web::Data<AppState>) -> Result<HttpResponse, TerraneError> {
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
        rust_version: format!(
            "{}.{}.{}",
            std::env::var("CARGO_PKG_RUST_VERSION").unwrap_or_else(|_| "1.75".to_string()),
            "",
            ""
        ),
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
) -> Result<HttpResponse, TerraneError> {
    let query = req.query_string();
    let limit: usize = query
        .split('&')
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
) -> Result<HttpResponse, TerraneError> {
    if let Some(ref store) = state.store {
        let query = req.query_string();
        let limit: usize = query
            .split('&')
            .find(|p| p.starts_with("limit="))
            .and_then(|p| p.split('=').nth(1))
            .and_then(|v| v.parse().ok())
            .unwrap_or(100);
        let offset: usize = query
            .split('&')
            .find(|p| p.starts_with("offset="))
            .and_then(|p| p.split('=').nth(1))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);

        let logs = store
            .get_audit_logs(limit, offset)
            .await
            .map_err(|e| TerraneError::InternalError(format!("读取审计日志失败: {}", e)))?;

        let entries: Vec<AuditLogEntry> = logs
            .into_iter()
            .map(|log| AuditLogEntry {
                id: log.id,
                timestamp: log.created_at,
                action: log.action,
                username: log.username,
                resource: log.resource,
                detail: log.detail,
            })
            .collect();

        Ok(HttpResponse::Ok().json(entries))
    } else {
        Ok(HttpResponse::Ok().json(Vec::<AuditLogEntry>::new()))
    }
}

/// 清除监控统计
pub async fn reset_monitor_stats(state: web::Data<AppState>) -> Result<HttpResponse, TerraneError> {
    state.request_count.store(0, Ordering::Relaxed);
    state.error_count.store(0, Ordering::Relaxed);
    state.recent_request_count.store(0, Ordering::Relaxed);
    state.endpoint_stats.write().await.clear();
    state.method_stats.write().await.clear();
    state.status_code_stats.write().await.clear();
    state.request_log.write().await.clear();

    Ok(HttpResponse::Ok().json(serde_json::json!({"status": "ok", "message": "监控统计已重置"})))
}

// ==================== Prometheus /metrics ====================

/// 追加一行带 HELP/TYPE 声明的 Prometheus 指标。
fn push_metric(
    out: &mut String,
    name: &str,
    help: &str,
    kind: &str,
    value: impl std::fmt::Display,
) {
    out.push_str("# HELP ");
    out.push_str(name);
    out.push(' ');
    out.push_str(help);
    out.push('\n');
    out.push_str("# TYPE ");
    out.push_str(name);
    out.push(' ');
    out.push_str(kind);
    out.push('\n');
    out.push_str(name);
    out.push(' ');
    out.push_str(&value.to_string());
    out.push('\n');
}

/// 追加一行带 label 的 Prometheus 指标 (label 值做转义)。
fn push_metric_labeled(
    out: &mut String,
    name: &str,
    labels: &[(&str, &str)],
    value: impl std::fmt::Display,
) {
    out.push_str(name);
    out.push('{');
    for (i, (k, v)) in labels.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(k);
        out.push_str("=\"");
        for c in v.chars() {
            match c {
                '\\' => out.push_str("\\\\"),
                '"' => out.push_str("\\\""),
                '\n' => out.push_str("\\n"),
                c => out.push(c),
            }
        }
        out.push('"');
    }
    out.push_str("} ");
    out.push_str(&value.to_string());
    out.push('\n');
}

/// Prometheus `/metrics` 端点 — 纯 Rust 手工生成 Prometheus 文本格式 (零外部依赖)。
///
/// 暴露指标: 请求/错误计数与速率、方法/状态码/端点分布、瓦片缓存命中率、
/// PostgreSQL 连接池水位、系统资源 (CPU/内存)。供 Prometheus 抓取 / K8s HPA。
pub async fn get_metrics(state: web::Data<AppState>) -> Result<HttpResponse, TerraneError> {
    let mut out = String::with_capacity(4096);

    let uptime = state.start_time.elapsed().as_secs();
    let total_reqs = state.request_count.load(Ordering::Relaxed);
    let total_errs = state.error_count.load(Ordering::Relaxed);
    let error_rate = if total_reqs > 0 {
        (total_errs as f64 / total_reqs as f64) * 100.0
    } else {
        0.0
    };
    let rps = state.recent_request_count.load(Ordering::Relaxed) as f64 / 300.0;

    // ---- 基础计数 ----
    push_metric(
        &mut out,
        "terrane_uptime_seconds",
        "Server uptime in seconds",
        "gauge",
        uptime,
    );
    push_metric(
        &mut out,
        "terrane_requests_total",
        "Total number of HTTP requests handled",
        "counter",
        total_reqs,
    );
    push_metric(
        &mut out,
        "terrane_errors_total",
        "Total number of HTTP requests that ended in error",
        "counter",
        total_errs,
    );
    push_metric(
        &mut out,
        "terrane_error_rate_percent",
        "Percentage of requests that ended in error",
        "gauge",
        format!("{:.4}", error_rate),
    );
    push_metric(
        &mut out,
        "terrane_requests_per_second",
        "Requests per second averaged over the last 5 minutes",
        "gauge",
        format!("{:.4}", rps),
    );

    // ---- 方法 / 状态码 / 端点分布 ----
    {
        let methods = state.method_stats.read().await;
        for (m, c) in methods.iter() {
            push_metric_labeled(
                &mut out,
                "terrane_method_requests_total",
                &[("method", m)],
                *c,
            );
        }
    }
    {
        let status_codes = state.status_code_stats.read().await;
        for (s, c) in status_codes.iter() {
            push_metric_labeled(
                &mut out,
                "terrane_http_status_total",
                &[("status", &s.to_string())],
                *c,
            );
        }
    }
    {
        let endpoints = state.endpoint_stats.read().await;
        for (ep, stats) in endpoints.iter() {
            push_metric_labeled(
                &mut out,
                "terrane_endpoint_requests_total",
                &[("endpoint", ep)],
                stats.count,
            );
            push_metric_labeled(
                &mut out,
                "terrane_endpoint_errors_total",
                &[("endpoint", ep)],
                stats.error_count,
            );
            push_metric_labeled(
                &mut out,
                "terrane_endpoint_duration_avg_ms",
                &[("endpoint", ep)],
                format!("{:.4}", stats.avg_duration_ms),
            );
        }
    }

    // ---- 瓦片缓存 (GeoWebCache) ----
    if let Some(ref cache) = state.tile_cache {
        let st = cache.stats();
        push_metric(
            &mut out,
            "terrane_tile_cache_hits_total",
            "Number of tile cache hits",
            "counter",
            st.hits,
        );
        push_metric(
            &mut out,
            "terrane_tile_cache_misses_total",
            "Number of tile cache misses",
            "counter",
            st.misses,
        );
        push_metric(
            &mut out,
            "terrane_tile_cache_hit_rate",
            "Tile cache hit rate (0..1)",
            "gauge",
            format!("{:.4}", cache.hit_rate()),
        );
    }

    // ---- PostgreSQL 连接池水位 (deadpool) ----
    {
        let pools = state.pg_pools.lock().unwrap();
        push_metric(
            &mut out,
            "terrane_pg_pool_count",
            "Number of cached PostgreSQL connection pools",
            "gauge",
            pools.len(),
        );
        for (name, pool) in pools.iter() {
            let st = pool.status();
            push_metric_labeled(&mut out, "terrane_pg_pool_size", &[("pool", name)], st.size);
            push_metric_labeled(
                &mut out,
                "terrane_pg_pool_available",
                &[("pool", name)],
                st.available,
            );
            push_metric_labeled(
                &mut out,
                "terrane_pg_pool_max",
                &[("pool", name)],
                st.max_size,
            );
        }
    }

    // ---- 系统资源 ----
    {
        let mut sys = sysinfo::System::new();
        sys.refresh_memory();
        push_metric(
            &mut out,
            "terrane_system_cpu_cores",
            "Number of logical CPU cores",
            "gauge",
            num_cpus(),
        );
        push_metric(
            &mut out,
            "terrane_system_memory_total_bytes",
            "Total system memory in bytes",
            "gauge",
            sys.total_memory(),
        );
        push_metric(
            &mut out,
            "terrane_system_memory_used_bytes",
            "Used system memory in bytes",
            "gauge",
            sys.used_memory(),
        );
        let used_percent = if sys.total_memory() > 0 {
            format!(
                "{:.2}",
                (sys.used_memory() as f64 / sys.total_memory() as f64) * 100.0
            )
        } else {
            "0".to_string()
        };
        push_metric(
            &mut out,
            "terrane_system_memory_used_percent",
            "Used system memory percent",
            "gauge",
            used_percent,
        );
    }

    Ok(HttpResponse::Ok()
        .insert_header(("Content-Type", "text/plain; version=0.0.4; charset=utf-8"))
        .body(out))
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
