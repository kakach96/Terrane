//! # 级联服务 (Cascaded Services)
//!
//! 允许将外部 WMS/WMTS 服务作为数据源使用。
//! 请求会透传到上游服务器，响应直接返回给客户端。
//!
//! ## 韧性 (Resilience, 见 `docs/IMPLEMENTATION_PLAN.md` §6.1)
//!
//! 级联请求具备两层韧性保护, 经 `[server]` 配置 (`cascaded_max_retries` /
//! `cascaded_retry_base_ms` / `cascaded_circuit_threshold` /
//! `cascaded_circuit_reset_secs`) 控制:
//!
//! - **重试 + 指数退避** — 瞬时故障 (连接失败 / 超时 / HTTP 429 / 5xx) 自动重试,
//!   退避延迟为 `retry_base_ms * 2^(attempt-1)`, 最多 `max_retries` 次。
//! - **熔断器** — 按上游 GetMap URL 独立统计连续失败: 达到阈值后打开熔断
//!   (快速失败, 不再请求上游), 经过 `circuit_reset_secs` 后进入半开状态允许
//!   一个试探请求, 成功则关闭, 失败则重新打开。

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tracing::{info, warn};

/// 级联 WMS 韧性参数 — 重试部分 (来自 `[server]` 配置)。
///
/// 熔断器参数 (threshold / reset) 不在此结构内: 熔断器实例由
/// `AppState.cascaded_circuits` 持有并按上游 URL 隔离, 本结构只携带每次
/// `fetch_cascaded_map` 调用需要的重试参数。
#[derive(Debug, Clone, Copy)]
pub struct CascadedResilience {
    /// 瞬时故障最大重试次数 (0 = 不重试)
    pub max_retries: u32,
    /// 重试退避基准毫秒 (指数退避: base * 2^(attempt-1))
    pub retry_base_ms: u64,
}

impl Default for CascadedResilience {
    fn default() -> Self {
        CascadedResilience {
            max_retries: 2,
            retry_base_ms: 200,
        }
    }
}

/// 级联 WMS 配置
#[derive(Debug, Clone)]
pub struct CascadedWmsConfig {
    /// 上游 WMS GetMap URL
    pub get_map_url: String,
    /// 上游 WMS GetCapabilities URL
    pub capabilities_url: Option<String>,
    /// 上游 WMS GetFeatureInfo URL
    pub feature_info_url: Option<String>,
    /// 上游图层名称
    pub remote_layer: String,
    /// 超时秒数
    pub timeout_secs: u64,
    /// 额外自定义参数
    pub extra_params: HashMap<String, String>,
}

/// 从 DataSourceConnection 中提取级联 WMS 配置
pub fn extract_cascaded_config(
    connection: &crate::models::DataSourceConnection,
) -> Option<CascadedWmsConfig> {
    let host = connection.host.as_ref()?;
    let port = connection.port.unwrap_or(80);
    let path = connection.database.as_deref().unwrap_or("/wms");
    let remote_layer = connection.schema.as_deref().unwrap_or("layer");

    let scheme = if port == 443 { "https" } else { "http" };
    let base_url = format!("{}://{}:{}{}", scheme, host, port, path);

    Some(CascadedWmsConfig {
        get_map_url: format!("{}?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetMap", base_url),
        capabilities_url: Some(format!("{}?SERVICE=WMS&REQUEST=GetCapabilities", base_url)),
        feature_info_url: Some(format!(
            "{}?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetFeatureInfo",
            base_url
        )),
        remote_layer: remote_layer.to_string(),
        timeout_secs: 10,
        extra_params: HashMap::new(),
    })
}

/// 对查询串参数值做百分号编码 (空格/引号/逗号/括号/& 等)
fn url_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len() * 3);
    for b in value.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            },
            b' ' => out.push_str("%20"),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

/// 单次上游请求的错误, 区分瞬时 (可重试) 与永久 (不可重试) 故障。
#[derive(Debug)]
enum CascadedError {
    /// 瞬时故障 — 连接失败 / 超时 / HTTP 429 / 5xx
    Transient(String),
    /// 永久故障 — 客户端错误等, 重试无意义
    Fatal(String),
}

/// 向外部 WMS 请求瓦片/地图图像 (单次尝试, 无重试)。
///
/// `extra_params` 为请求级透传的厂商参数 (如 CQL_FILTER / TIME / ELEVATION),
/// 会百分号编码后追加到上游 URL。
///
/// 返回 (image_bytes, content_type)
#[allow(clippy::too_many_arguments)] // signature mirrors the upstream WMS GetMap query parameters
async fn fetch_cascaded_map_once(
    config: &CascadedWmsConfig,
    bbox: &str,
    width: u32,
    height: u32,
    format: &str,
    srs: &str,
    style: Option<&str>,
    transparent: bool,
    extra_params: &HashMap<String, String>,
) -> Result<(Vec<u8>, String), CascadedError> {
    let mut url = format!(
        "{}&LAYERS={}&BBOX={}&WIDTH={}&HEIGHT={}&FORMAT={}&SRS={}&TRANSPARENT={}",
        config.get_map_url,
        config.remote_layer,
        bbox,
        width,
        height,
        format,
        srs,
        if transparent { "TRUE" } else { "FALSE" },
    );

    if let Some(style_name) = style {
        url.push_str(&format!("&STYLES={}", style_name));
    }

    // 添加数据源静态自定义参数
    for (key, value) in &config.extra_params {
        url.push_str(&format!("&{}={}", key, url_encode(value)));
    }

    // 添加请求级透传厂商参数 (CQL_FILTER / TIME / ELEVATION 等)
    for (key, value) in extra_params {
        if !value.trim().is_empty() {
            url.push_str(&format!("&{}={}", key, url_encode(value)));
        }
    }

    info!("[Cascaded] 请求外部 WMS: {}", url);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(config.timeout_secs))
        .build()
        .map_err(|e| CascadedError::Fatal(format!("HTTP 客户端创建失败: {}", e)))?;

    let response = client.get(&url).send().await.map_err(|e| {
        // 连接失败 / 超时属于瞬时故障, 可重试
        if e.is_timeout() || e.is_connect() {
            CascadedError::Transient(format!("外部 WMS 请求失败: {}", e))
        } else {
            CascadedError::Fatal(format!("外部 WMS 请求失败: {}", e))
        }
    })?;

    let status = response.status();
    if !status.is_success() {
        // 429 / 5xx 为瞬时故障 (上游过载/暂时不可用), 4xx 为永久故障
        let code = status.as_u16();
        let msg = format!("外部 WMS 返回错误状态: {}", status);
        if code == 429 || code >= 500 {
            return Err(CascadedError::Transient(msg));
        }
        return Err(CascadedError::Fatal(msg));
    }

    let content_type = response
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("image/png")
        .to_string();

    let bytes = response
        .bytes()
        .await
        .map_err(|e| CascadedError::Transient(format!("读取响应失败: {}", e)))?
        .to_vec();

    info!(
        "[Cascaded] 外部 WMS 响应: {} bytes, Content-Type: {}",
        bytes.len(),
        content_type
    );
    Ok((bytes, content_type))
}

/// 向外部 WMS 请求瓦片/地图图像 (带韧性: 重试 + 指数退避 + 熔断器)。
///
/// 返回 (image_bytes, content_type); 重试耗尽或熔断打开时返回错误信息。
#[allow(clippy::too_many_arguments)] // signature mirrors the upstream WMS GetMap query parameters
pub async fn fetch_cascaded_map(
    config: &CascadedWmsConfig,
    resilience: &CascadedResilience,
    circuits: Option<&CascadedCircuits>,
    bbox: &str,
    width: u32,
    height: u32,
    format: &str,
    srs: &str,
    style: Option<&str>,
    transparent: bool,
    extra_params: &HashMap<String, String>,
) -> Result<(Vec<u8>, String), String> {
    let upstream = config.get_map_url.clone();

    // 熔断器检查: 打开时快速失败, 不请求上游
    if let Some(c) = circuits {
        if !c.allow(&upstream) {
            warn!("[Cascaded] 熔断打开, 拒绝上游请求: {}", upstream);
            return Err(format!("上游熔断打开 (circuit open): {}", upstream));
        }
    }

    let mut attempt = 0u32;
    loop {
        match fetch_cascaded_map_once(
            config,
            bbox,
            width,
            height,
            format,
            srs,
            style,
            transparent,
            extra_params,
        )
        .await
        {
            Ok(v) => {
                if let Some(c) = circuits {
                    c.record(&upstream, true);
                }
                return Ok(v);
            },
            Err(CascadedError::Fatal(msg)) => {
                if let Some(c) = circuits {
                    c.record(&upstream, false);
                }
                return Err(msg);
            },
            Err(CascadedError::Transient(msg)) => {
                if attempt >= resilience.max_retries {
                    if let Some(c) = circuits {
                        c.record(&upstream, false);
                    }
                    warn!("[Cascaded] 上游重试耗尽: {}", msg);
                    return Err(msg);
                }
                attempt += 1;
                let delay_ms = backoff_delay_ms(resilience.retry_base_ms, attempt);
                warn!(
                    "[Cascaded] 瞬时故障 (第 {} 次重试, {}ms): {}",
                    attempt, delay_ms, msg
                );
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            },
        }
    }
}

/// 指数退避延迟 (毫秒): base * 2^(attempt-1)
fn backoff_delay_ms(base_ms: u64, attempt: u32) -> u64 {
    base_ms.saturating_mul(1u64 << attempt.saturating_sub(1).min(30))
}

// ---------------------------------------------------------------------------
// 熔断器 (per-upstream circuit breaker)
// ---------------------------------------------------------------------------

/// 熔断器状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// 关闭 — 请求正常放行
    Closed,
    /// 打开 — 快速失败, 不请求上游
    Open,
    /// 半开 — 允许一个试探请求, 成功则关闭, 失败则重新打开
    HalfOpen,
}

/// 单个上游的熔断器。
#[derive(Debug)]
pub struct CircuitBreaker {
    /// 连续失败阈值
    threshold: u32,
    /// 打开后重置为半开的等待时长
    reset_after: Duration,
    /// 当前状态
    state: CircuitState,
    /// 连续失败计数
    consecutive_failures: u32,
    /// 打开时刻 (用于计算重置窗口)
    opened_at: Option<Instant>,
}

impl CircuitBreaker {
    pub fn new(threshold: u32, reset_after: Duration) -> Self {
        CircuitBreaker {
            threshold: threshold.max(1),
            reset_after,
            state: CircuitState::Closed,
            consecutive_failures: 0,
            opened_at: None,
        }
    }

    /// 是否允许发起请求; 打开超过重置窗口后自动转入半开并放行试探请求。
    pub fn allow(&mut self) -> bool {
        match self.state {
            CircuitState::Closed | CircuitState::HalfOpen => true,
            CircuitState::Open => {
                let should_probe = self
                    .opened_at
                    .map(|t| t.elapsed() >= self.reset_after)
                    .unwrap_or(true);
                if should_probe {
                    self.state = CircuitState::HalfOpen;
                    true
                } else {
                    false
                }
            },
        }
    }

    /// 记录一次成功: 关闭熔断, 清零连续失败。
    pub fn record_success(&mut self) {
        self.state = CircuitState::Closed;
        self.consecutive_failures = 0;
        self.opened_at = None;
    }

    /// 记录一次失败: 半开试探失败立即重新打开; 关闭状态连续失败达到阈值打开。
    pub fn record_failure(&mut self) {
        self.consecutive_failures += 1;
        match self.state {
            CircuitState::HalfOpen => {
                self.state = CircuitState::Open;
                self.opened_at = Some(Instant::now());
            },
            CircuitState::Closed => {
                if self.consecutive_failures >= self.threshold {
                    self.state = CircuitState::Open;
                    self.opened_at = Some(Instant::now());
                }
            },
            CircuitState::Open => {
                // 已打开, 保持打开 (重置窗口从首次打开起算)
                self.opened_at = Some(self.opened_at.unwrap_or_else(Instant::now));
            },
        }
    }

    pub fn state(&self) -> CircuitState {
        self.state
    }
}

/// 熔断器注册表: 按上游 URL 分别统计, 避免一个上游故障拖垮所有级联图层。
#[derive(Debug)]
pub struct CascadedCircuits {
    inner: Mutex<HashMap<String, CircuitBreaker>>,
    threshold: u32,
    reset_after: Duration,
}

impl CascadedCircuits {
    pub fn new(threshold: u32, reset_after: Duration) -> Self {
        CascadedCircuits {
            inner: Mutex::new(HashMap::new()),
            threshold,
            reset_after,
        }
    }

    /// 检查上游熔断是否放行; 阈值 0 表示禁用熔断 (总是放行)。
    pub fn allow(&self, upstream: &str) -> bool {
        if self.threshold == 0 {
            return true;
        }
        let mut inner = self.inner.lock().unwrap();
        let breaker = inner
            .entry(upstream.to_string())
            .or_insert_with(|| CircuitBreaker::new(self.threshold, self.reset_after));
        breaker.allow()
    }

    /// 记录上游请求结果。
    pub fn record(&self, upstream: &str, success: bool) {
        if self.threshold == 0 {
            return;
        }
        let mut inner = self.inner.lock().unwrap();
        if let Some(breaker) = inner.get_mut(upstream) {
            if success {
                breaker.record_success();
            } else {
                breaker.record_failure();
            }
        }
    }

    /// 当前熔断状态 (用于测试/监控)。
    pub fn state(&self, upstream: &str) -> Option<CircuitState> {
        let inner = self.inner.lock().unwrap();
        inner.get(upstream).map(|b| b.state())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::DataSourceConnection;

    fn conn_with(
        host: Option<&str>,
        port: Option<u16>,
        database: Option<&str>,
        schema: Option<&str>,
    ) -> DataSourceConnection {
        DataSourceConnection {
            host: host.map(String::from),
            port,
            database: database.map(String::from),
            schema: schema.map(String::from),
            username: None,
            password: None,
            file_path: None,
            file_storage_type: Some("local".to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn test_extract_cascaded_config_defaults() {
        let conn = conn_with(
            Some("example.com"),
            Some(8080),
            Some("/terrane/wms"),
            Some("remote_layer"),
        );
        let cfg = extract_cascaded_config(&conn).expect("应能提取级联配置");

        assert!(cfg
            .get_map_url
            .contains("http://example.com:8080/terrane/wms"));
        assert!(cfg.get_map_url.contains("REQUEST=GetMap"));
        assert!(cfg
            .capabilities_url
            .as_deref()
            .unwrap()
            .contains("REQUEST=GetCapabilities"));
        assert_eq!(cfg.remote_layer, "remote_layer");
        assert_eq!(cfg.timeout_secs, 10);
    }

    #[test]
    fn test_extract_cascaded_config_https() {
        let conn = conn_with(Some("secure.example.com"), Some(443), None, Some("world"));
        let cfg = extract_cascaded_config(&conn).unwrap();
        assert!(cfg
            .get_map_url
            .starts_with("https://secure.example.com:443/wms"));
        assert_eq!(cfg.remote_layer, "world");
    }

    #[test]
    fn test_extract_cascaded_config_default_port() {
        // 未指定端口时默认 80, database 缺省 /wms
        let conn = conn_with(Some("plain.example.com"), None, None, None);
        let cfg = extract_cascaded_config(&conn).unwrap();
        assert!(cfg
            .get_map_url
            .starts_with("http://plain.example.com:80/wms"));
        assert_eq!(cfg.remote_layer, "layer");
    }

    #[test]
    fn test_extract_cascaded_config_missing_host() {
        let conn = conn_with(None, None, None, None);
        assert!(extract_cascaded_config(&conn).is_none());
    }

    #[test]
    fn test_backoff_delay_is_exponential() {
        assert_eq!(backoff_delay_ms(200, 1), 200);
        assert_eq!(backoff_delay_ms(200, 2), 400);
        assert_eq!(backoff_delay_ms(200, 3), 800);
        assert_eq!(backoff_delay_ms(100, 4), 800);
        // 不发生溢出
        assert_eq!(backoff_delay_ms(u64::MAX, 100), u64::MAX);
    }

    #[test]
    fn test_circuit_breaker_trips_after_threshold() {
        let mut cb = CircuitBreaker::new(3, Duration::from_secs(60));
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.allow());

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed, "失败未达阈值不应打开");
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open, "达到阈值应打开");
        assert!(!cb.allow(), "打开时应快速失败");
    }

    #[test]
    fn test_circuit_breaker_half_open_recovers_on_success() {
        let mut cb = CircuitBreaker::new(2, Duration::from_millis(50));
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        // 重置窗口未到: 仍拒绝
        assert!(!cb.allow());

        std::thread::sleep(Duration::from_millis(60));
        // 窗口过后: 半开放行一个试探请求
        assert!(cb.allow(), "重置后应允许试探请求");
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed, "试探成功应关闭熔断");
        assert!(cb.allow());
    }

    #[test]
    fn test_circuit_breaker_half_open_failure_reopens() {
        let mut cb = CircuitBreaker::new(1, Duration::from_millis(30));
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        std::thread::sleep(Duration::from_millis(40));
        assert!(cb.allow(), "重置后应允许试探请求");
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open, "试探失败应立即重新打开");
        assert!(!cb.allow());
    }

    #[test]
    fn test_circuit_registry_disabled_when_threshold_zero() {
        let circuits = CascadedCircuits::new(0, Duration::from_secs(60));
        assert!(circuits.allow("http://upstream/wms"), "阈值 0 应禁用熔断");
        circuits.record("http://upstream/wms", false);
        assert!(circuits.allow("http://upstream/wms"));
        assert!(circuits.state("http://upstream/wms").is_none());
    }

    #[test]
    fn test_circuit_registry_per_upstream_isolation() {
        let circuits = CascadedCircuits::new(1, Duration::from_secs(60));
        assert!(circuits.allow("http://a/wms"));
        circuits.record("http://a/wms", false);
        assert!(!circuits.allow("http://a/wms"), "上游 a 应打开");
        assert!(
            circuits.allow("http://b/wms"),
            "上游 b 不应受影响 (按上游隔离)"
        );
    }
}
