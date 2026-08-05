//! 本地目录矢量数据存储 — 每图层一个 GeoJSON 文件。
//!
//! 目录可挂载 NFS / 对象存储 (如通过 FUSE 或 CSI 驱动), 便于扩展。
//! 文件格式: `<dir>/<layer>.geojson`, 内容为标准 GeoJSON FeatureCollection。

use std::path::PathBuf;

use crate::models::Feature;
use crate::store::StoreError;

/// 本地目录矢量数据存储
pub struct LocalVectorStore {
    dir: PathBuf,
}

impl LocalVectorStore {
    /// 创建本地目录业务存储 (目录按需懒创建)
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// 图层名 → 安全文件名 (去除路径分隔符与非法字符)
    fn file_path(&self, layer_name: &str) -> PathBuf {
        let safe: String = layer_name
            .chars()
            .map(|c| match c {
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
                _ => c,
            })
            .collect();
        self.dir.join(format!("{}.geojson", safe))
    }
}

#[async_trait::async_trait]
impl super::VectorStore for LocalVectorStore {
    async fn save_features(&self, layer_name: &str, features: &[Feature]) -> Result<usize, StoreError> {
        std::fs::create_dir_all(&self.dir)?;

        // 序列化为标准 GeoJSON FeatureCollection
        let features_json: Vec<serde_json::Value> = features
            .iter()
            .map(|f| {
                let mut v = serde_json::to_value(f).unwrap_or(serde_json::Value::Null);
                if let serde_json::Value::Object(ref mut obj) = v {
                    obj.insert("type".to_string(), serde_json::Value::String("Feature".to_string()));
                }
                v
            })
            .collect();
        let fc = serde_json::json!({
            "type": "FeatureCollection",
            "features": features_json,
        });

        std::fs::write(self.file_path(layer_name), serde_json::to_string_pretty(&fc)?)?;
        Ok(features.len())
    }

    async fn load_features(&self, layer_name: &str) -> Result<Vec<Feature>, StoreError> {
        let path = self.file_path(layer_name);
        if !path.exists() {
            return Ok(Vec::new());
        }

        let raw = std::fs::read_to_string(path)?;
        let root: serde_json::Value = serde_json::from_str(&raw)?;
        let features = root
            .get("features")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|f| serde_json::from_value::<Feature>(f.clone()).ok())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        Ok(features)
    }

    async fn delete_features(&self, layer_name: &str) -> Result<usize, StoreError> {
        let path = self.file_path(layer_name);
        if path.exists() {
            std::fs::remove_file(path)?;
            Ok(1)
        } else {
            Ok(0)
        }
    }

    async fn list_tables(&self) -> Result<Vec<String>, StoreError> {
        if !self.dir.exists() {
            return Ok(Vec::new());
        }
        let mut tables = Vec::new();
        for entry in std::fs::read_dir(&self.dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(base) = name.strip_suffix(".geojson") {
                tables.push(base.to_string());
            }
        }
        tables.sort();
        Ok(tables)
    }
}
