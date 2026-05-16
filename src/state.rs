use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::RwLock;
use std::time::Instant;
use crate::config::GeoServerConfig;
use crate::models::{Layer, Feature};

pub struct AppState {
    pub config: GeoServerConfig,
    pub layers: Arc<RwLock<Vec<Layer>>>,
    pub features: Arc<RwLock<HashMap<String, Vec<Feature>>>>,
    pub styles: Arc<RwLock<HashMap<String, String>>>,
    pub start_time: Instant,
    pub request_count: AtomicU64,
    pub error_count: AtomicU64,
}

impl AppState {
    pub fn new(config: GeoServerConfig) -> Self {
        let layers: Vec<Layer> = config.workspaces.iter()
            .flat_map(|workspace| {
                workspace.stores.iter().flat_map(|store| {
                    store.layers.iter().map(|layer_config| {
                        Layer::new(
                            layer_config.name.clone(),
                            layer_config.title.clone(),
                            workspace.name.clone(),
                            store.name.clone(),
                            crate::models::CoordinateReferenceSystem::from_epsg(&layer_config.srs),
                        ).with_bounds(crate::models::BoundingBox::new(
                            crate::models::CoordinateReferenceSystem::from_epsg(&layer_config.srs),
                            crate::models::Bounds::new(
                                layer_config.bounds.minx,
                                layer_config.bounds.miny,
                                layer_config.bounds.maxx,
                                layer_config.bounds.maxy,
                            ),
                        ))
                    }).collect::<Vec<_>>()
                }).collect::<Vec<_>>()
            }).collect();
        
        let features_layers = if layers.is_empty() {
            vec![Layer::new(
                "world".to_string(),
                "World".to_string(),
                "default".to_string(),
                "shapes".to_string(),
                crate::models::CoordinateReferenceSystem::EPSG4326,
            )]
        } else {
            layers.clone()
        };
        
        AppState {
            config,
            layers: Arc::new(RwLock::new(features_layers)),
            features: Arc::new(RwLock::new(HashMap::new())),
            styles: Arc::new(RwLock::new(HashMap::new())),
            start_time: Instant::now(),
            request_count: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
        }
    }
    
    pub fn increment_request_count(&self) {
        self.request_count.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn increment_error_count(&self) {
        self.error_count.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn get_uptime(&self) -> String {
        let duration = self.start_time.elapsed();
        let total_secs = duration.as_secs();
        let days = total_secs / 86400;
        let hours = (total_secs % 86400) / 3600;
        let minutes = (total_secs % 3600) / 60;
        format!("{}天 {}小时 {}分钟", days, hours, minutes)
    }
    
    pub async fn get_layer_features(&self, layer_name: &str) -> Option<Vec<Feature>> {
        let features = self.features.read().await;
        features.get(layer_name).cloned()
    }
    
    pub async fn add_layer_features(&self, layer_name: &str, features: Vec<Feature>) {
        let mut features_map = self.features.write().await;
        features_map.insert(layer_name.to_string(), features);
    }
    
    pub async fn add_feature(&self, layer_name: &str, feature: Feature) {
        let mut features_map = self.features.write().await;
        features_map
            .entry(layer_name.to_string())
            .or_insert_with(Vec::new)
            .push(feature);
    }
    
    pub async fn get_layer(&self, layer_name: &str) -> Option<Layer> {
        let layers = self.layers.read().await;
        layers.iter().find(|l| l.name == layer_name).cloned()
    }
    
    pub async fn list_layers(&self) -> Vec<Layer> {
        let layers = self.layers.read().await;
        layers.clone()
    }
    
    pub async fn get_style(&self, style_name: &str) -> Option<String> {
        let styles = self.styles.read().await;
        styles.get(style_name).cloned()
    }
    
    pub async fn add_style(&self, style_name: &str, content: String) {
        let mut styles = self.styles.write().await;
        styles.insert(style_name.to_string(), content);
    }
    
    pub async fn add_layer(&self, layer: Layer) {
        let mut layers = self.layers.write().await;
        layers.push(layer);
    }
    
    pub async fn update_layer(&self, layer_name: &str, updates: LayerUpdates) -> bool {
        let mut layers = self.layers.write().await;
        if let Some(layer) = layers.iter_mut().find(|l| l.name == layer_name) {
            if let Some(title) = updates.title {
                layer.title = title;
            }
            if let Some(abstract_text) = updates.abstract_text {
                layer.abstract_text = Some(abstract_text);
            }
            if let Some(enabled) = updates.enabled {
                layer.enabled = enabled;
            }
            true
        } else {
            false
        }
    }
    
    pub async fn delete_layer(&self, layer_name: &str) -> bool {
        let mut layers = self.layers.write().await;
        if let Some(pos) = layers.iter().position(|l| l.name == layer_name) {
            layers.remove(pos);
            let mut features_map = self.features.write().await;
            features_map.remove(layer_name);
            true
        } else {
            false
        }
    }
}

#[derive(Debug, Clone)]
pub struct LayerUpdates {
    pub title: Option<String>,
    pub abstract_text: Option<String>,
    pub enabled: Option<bool>,
}
