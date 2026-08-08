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

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::VectorStore;
    use std::path::PathBuf;
    use crate::models::{Feature, GeoJsonGeometry, PropertyValue};
    use std::collections::HashMap;

    fn sample_feature(name: &str, x: f64, y: f64) -> Feature {
        let mut props = HashMap::new();
        props.insert("name".to_string(), PropertyValue::String(name.to_string()));
        Feature::new(GeoJsonGeometry::Point { coordinates: vec![x, y] }, props)
    }

    fn temp_dir(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("terrane-vstore-{}-{}", tag, std::process::id()))
    }

    #[actix_rt::test]
    async fn test_save_load_roundtrip() {
        let dir = temp_dir("rt");
        let store = LocalVectorStore::new(dir.clone());

        store.save_features("layer1", &[sample_feature("a", 1.0, 2.0)]).await.unwrap();
        let loaded = store.load_features("layer1").await.unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].properties.get("name").unwrap().to_string(), "a");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[actix_rt::test]
    async fn test_load_missing_returns_empty() {
        let dir = temp_dir("miss");
        let store = LocalVectorStore::new(dir.clone());
        assert!(store.load_features("nope").await.unwrap().is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[actix_rt::test]
    async fn test_delete_and_list_tables() {
        let dir = temp_dir("dl");
        let store = LocalVectorStore::new(dir.clone());
        store.save_features("alpha", &[sample_feature("1", 0.0, 0.0)]).await.unwrap();
        store.save_features("beta", &[sample_feature("2", 1.0, 1.0)]).await.unwrap();

        let tables = store.list_tables().await.unwrap();
        assert_eq!(tables, vec!["alpha".to_string(), "beta".to_string()]);

        store.delete_features("alpha").await.unwrap();
        let tables = store.list_tables().await.unwrap();
        assert_eq!(tables, vec!["beta".to_string()]);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[actix_rt::test]
    async fn test_file_path_sanitizes_illegal_chars() {
        let dir = temp_dir("san");
        let store = LocalVectorStore::new(dir.clone());
        let p = store.file_path("ws:layer/name?x");
        let s = p.to_string_lossy().to_string();
        assert!(s.contains("ws_layer_name_x"), "非法字符应消毒, 实际: {}", s);
        assert!(s.ends_with(".geojson"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
