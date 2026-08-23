//! Built-in sample data seeding.
//!
//! On first startup (when the catalog contains no layers yet) the backend
//! copies the curated sample files from `config.samples.source_dir` into
//! `<data_dir>/samples/` and auto-registers a `demo` workspace with one GeoJSON
//! data source + layer per dataset, so the product demos out-of-the-box.
//!
//! Seeding is opt-in via `[samples] enabled` (default: true) and only runs on a
//! fresh catalog, so existing installs are never modified.

use crate::config::GeoServerConfig;
use crate::models::{
    BoundingBox, CoordinateReferenceSystem, DataSource, DataSourceConnection, DataSourceType, Layer,
};
use crate::store::Store;
use crate::utils::bounds::compute_layer_bounds;
use std::sync::Arc;

/// A single built-in sample dataset descriptor.
pub struct SampleDataset {
    /// Layer / data-source name (also the GeoJSON file stem).
    pub name: &'static str,
    /// Human-readable layer title.
    pub title: &'static str,
    /// GeoJSON file name inside the samples directory.
    pub file: &'static str,
    /// Workspace the layer is published into.
    pub workspace: &'static str,
    /// Layer abstract / description.
    pub abstract_text: &'static str,
}

/// The curated built-in sample datasets (mirrors GeoServer's sample data).
pub const SAMPLE_DATASETS: &[SampleDataset] = &[
    SampleDataset {
        name: "major_cities",
        title: "Major World Cities",
        file: "major_cities.geojson",
        workspace: "demo",
        abstract_text: "25 major world cities (points) with population and country.",
    },
    SampleDataset {
        name: "sample_routes",
        title: "Sample Flight Routes",
        file: "sample_routes.geojson",
        workspace: "demo",
        abstract_text: "14 sample flight routes (lines) between major cities.",
    },
    SampleDataset {
        name: "world_countries",
        title: "World Countries (Simplified)",
        file: "world_countries.geojson",
        workspace: "demo",
        abstract_text: "6 simplified world country polygons (not cartographically accurate).",
    },
];

/// Seed the built-in sample data into the metadata store.
///
/// 1. Copies each sample file from `config.samples.source_dir` to
///    `<data_dir>/samples/` (only when the target file is missing).
/// 2. Ensures the target workspace exists (with its namespace).
/// 3. Registers a GeoJSON data source + layer per dataset (skipping any that
///    already exist).
///
/// Returns the newly registered layers (empty when seeding is skipped or fails).
pub async fn seed_samples(config: &GeoServerConfig, store: &Arc<dyn Store>) -> Vec<Layer> {
    let source_dir = config.samples.source_dir.clone();
    let target_dir = config.data_dir.join("samples");

    if let Err(e) = std::fs::create_dir_all(&target_dir) {
        tracing::warn!(
            "[Samples] 无法创建示例数据目录 {}: {}",
            target_dir.display(),
            e
        );
        return Vec::new();
    }

    let mut seeded = Vec::new();
    for ds in SAMPLE_DATASETS {
        let src = source_dir.join(ds.file);
        let dst = target_dir.join(ds.file);

        // 1. Copy the sample file into the runtime data dir (if missing).
        if !dst.exists() {
            if !src.exists() {
                tracing::warn!("[Samples] 示例数据源文件不存在: {}", src.display());
                continue;
            }
            if let Err(e) = std::fs::copy(&src, &dst) {
                tracing::warn!("[Samples] 复制示例数据失败 {}: {}", src.display(), e);
                continue;
            }
            tracing::info!(
                "[Samples] 复制示例数据 {} -> {}",
                src.display(),
                dst.display()
            );
        }

        // 2. Ensure the workspace (and its namespace) exists.
        ensure_workspace(store, ds.workspace).await;

        // 3. Register the GeoJSON data source (skip if already present).
        let ds_name = ds.name.to_string();
        let conn = DataSourceConnection::file(dst.to_string_lossy().to_string());
        if store
            .get_data_source(&ds_name)
            .await
            .ok()
            .flatten()
            .is_none()
        {
            if let Err(e) = store
                .create_data_source(
                    &ds_name,
                    &DataSourceType::GeoJson,
                    Some(ds.workspace.to_string()),
                    true,
                    &conn,
                )
                .await
            {
                tracing::warn!("[Samples] 创建数据源失败 {}: {}", ds_name, e);
                continue;
            }
        }

        // 4. Compute the native bounds from the GeoJSON file.
        let data_source = DataSource {
            name: ds_name.clone(),
            data_source_type: DataSourceType::GeoJson,
            workspace: Some(ds.workspace.to_string()),
            enabled: true,
            connection: Some(conn),
            created: None,
            modified: None,
        };
        let bounds = compute_layer_bounds(&data_source, None, None)
            .await
            .ok()
            .flatten();

        // 5. Build + persist the layer.
        let mut layer = Layer::new(
            ds.name.to_string(),
            ds.title.to_string(),
            ds.workspace.to_string(),
            ds_name.clone(),
            CoordinateReferenceSystem::EPSG4326,
        );
        layer.native_name = Some(ds.file.to_string());
        layer.abstract_text = Some(ds.abstract_text.to_string());
        if let Some(b) = bounds {
            layer = layer.with_bounds(BoundingBox::new(
                CoordinateReferenceSystem::EPSG4326,
                b.bounds,
            ));
        }

        let store_layer = crate::store::types::Layer {
            name: layer.name.clone(),
            title: layer.title.clone(),
            workspace: layer.workspace.clone(),
            store: layer.store.clone(),
            srs: "EPSG:4326".to_string(),
            abstract_text: layer.abstract_text.clone(),
            native_name: layer.native_name.clone(),
            enabled: true,
            minx: layer.native_bounds.bounds.minx,
            miny: layer.native_bounds.bounds.miny,
            maxx: layer.native_bounds.bounds.maxx,
            maxy: layer.native_bounds.bounds.maxy,
            created: String::new(),
            modified: String::new(),
            cache_store: None,
        };
        if let Err(e) = store.create_layer(&store_layer).await {
            tracing::warn!("[Samples] 创建图层失败 {}: {}", layer.name, e);
            continue;
        }

        tracing::info!(
            "[Samples] 已注册示例图层 {} (workspace={}, store={})",
            layer.name,
            layer.workspace,
            layer.store
        );
        seeded.push(layer);
    }
    seeded
}

/// Ensure a workspace (and its namespace) exists in the metadata store.
async fn ensure_workspace(store: &Arc<dyn Store>, name: &str) {
    if store.get_workspace(name).await.ok().flatten().is_some() {
        return;
    }
    let request = crate::handlers::CreateWorkspaceRequest {
        name: name.to_string(),
        title: Some(name.to_string()),
        description: Some("Built-in sample data workspace".to_string()),
    };
    match store.create_workspace(&request).await {
        Ok(ws) => {
            let ns_uri = format!("http://geoserver.org/{}", ws.name);
            let _ = store
                .create_namespace(&ws.name, &ns_uri, Some(&ws.name), false)
                .await;
            tracing::info!("[Samples] 已创建工作空间 {}", ws.name);
        },
        Err(e) => {
            tracing::warn!("[Samples] 创建工作空间失败 {}: {}", name, e);
        },
    }
}
