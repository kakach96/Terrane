use super::bounds::{BoundingBox, CoordinateReferenceSystem};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layer {
    pub name: String,
    pub title: String,
    pub abstract_text: Option<String>,
    pub workspace: String,
    pub store: String,
    pub native_name: Option<String>,
    pub native_bounds: BoundingBox,
    pub lat_lon_bounds: BoundingBox,
    pub srs: CoordinateReferenceSystem,
    pub styles: Vec<StyleRef>,
    pub resource: LayerResource,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerResource {
    pub resource_type: ResourceType,
    pub path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResourceType {
    FeatureType,
    Coverage,
}

impl Layer {
    pub fn new(
        name: String,
        title: String,
        workspace: String,
        store: String,
        srs: CoordinateReferenceSystem,
    ) -> Self {
        let native_bounds = BoundingBox::world(srs.clone());
        Layer {
            name,
            title,
            abstract_text: None,
            workspace,
            store,
            native_name: None,
            native_bounds: native_bounds.clone(),
            lat_lon_bounds: native_bounds,
            srs,
            styles: vec![],
            resource: LayerResource {
                resource_type: ResourceType::FeatureType,
                path: None,
            },
            enabled: true,
        }
    }

    pub fn with_bounds(mut self, bounds: BoundingBox) -> Self {
        self.native_bounds = bounds.clone();
        self.lat_lon_bounds = bounds;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleRef {
    pub name: String,
    pub href: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerGroup {
    pub name: String,
    pub title: String,
    pub layers: Vec<String>,
    pub styles: Vec<Option<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub name: String,
    pub uri: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Store {
    pub name: String,
    pub workspace: String,
    pub store_type: StoreType,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StoreType {
    DataStore,
    CoverageStore,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegendInfo {
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub legend: Vec<LegendItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegendItem {
    pub label: String,
    pub style: String,
}
