//! # 级联服务 (Cascaded Services)
//!
//! 允许将外部 WMS/WMTS 服务作为数据源使用。
//! 请求会透传到上游服务器，响应直接返回给客户端。

use std::collections::HashMap;
use tracing::info;

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

/// 向外部 WMS 请求瓦片/地图图像
///
/// `extra_params` 为请求级透传的厂商参数 (如 CQL_FILTER / TIME / ELEVATION),
/// 会百分号编码后追加到上游 URL。
///
/// 返回 (image_bytes, content_type)
pub async fn fetch_cascaded_map(
    config: &CascadedWmsConfig,
    bbox: &str,
    width: u32,
    height: u32,
    format: &str,
    srs: &str,
    style: Option<&str>,
    transparent: bool,
    extra_params: &HashMap<String, String>,
) -> Result<(Vec<u8>, String), String> {
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
        .map_err(|e| format!("HTTP 客户端创建失败: {}", e))?;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("外部 WMS 请求失败: {}", e))?;

    let status = response.status();
    if !status.is_success() {
        return Err(format!("外部 WMS 返回错误状态: {}", status));
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
        .map_err(|e| format!("读取响应失败: {}", e))?
        .to_vec();

    info!(
        "[Cascaded] 外部 WMS 响应: {} bytes, Content-Type: {}",
        bytes.len(),
        content_type
    );
    Ok((bytes, content_type))
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
        }
    }

    #[test]
    fn test_extract_cascaded_config_defaults() {
        let conn = conn_with(
            Some("example.com"),
            Some(8080),
            Some("/geoserver/wms"),
            Some("remote_layer"),
        );
        let cfg = extract_cascaded_config(&conn).expect("应能提取级联配置");

        assert!(cfg
            .get_map_url
            .contains("http://example.com:8080/geoserver/wms"));
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
}
