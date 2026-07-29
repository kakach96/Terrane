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
        feature_info_url: Some(format!("{}?SERVICE=WMS&VERSION=1.1.1&REQUEST=GetFeatureInfo", base_url)),
        remote_layer: remote_layer.to_string(),
        timeout_secs: 10,
        extra_params: HashMap::new(),
    })
}

/// 向外部 WMS 请求瓦片/地图图像
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

    // 添加额外参数
    for (key, value) in &config.extra_params {
        url.push_str(&format!("&{}={}", key, value));
    }

    info!("[Cascaded] 请求外部 WMS: {}", url);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(config.timeout_secs))
        .build()
        .map_err(|e| format!("HTTP 客户端创建失败: {}", e))?;

    let response = client.get(&url)
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

    let bytes = response.bytes()
        .await
        .map_err(|e| format!("读取响应失败: {}", e))?
        .to_vec();

    info!("[Cascaded] 外部 WMS 响应: {} bytes, Content-Type: {}", bytes.len(), content_type);
    Ok((bytes, content_type))
}
