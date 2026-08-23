//! Tile seeding engine (GeoWebCache-style seed / truncate).
//!
//! A seed job pre-generates tiles for a `(layer, gridset, zoom range)` into the
//! tile cache by driving the shared [`render_tile_bytes`] pipeline, running as a
//! background tokio task with a progress/cancel surface (see `/tiles/seed`).
//!
//! Cancellation is cooperative: the worker checks the job status in the shared
//! table between tiles and stops as soon as it sees `Cancelled`. Tiles already
//! written before cancellation are kept (same as GWC).

use crate::handlers::tile_common::{render_tile_bytes, TileFormat};
use crate::state::AppState;
use crate::utils::tile_grid;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Seed job lifecycle status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SeedStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// A single seed (or truncate) job record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeedJob {
    pub id: String,
    pub layer: String,
    pub gridset: String,
    pub z_min: u32,
    pub z_max: u32,
    pub format: String,
    pub status: SeedStatus,
    pub total: u64,
    pub done: u64,
    pub error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Shared job table (`AppState.seed_jobs`): `job_id -> SeedJob`.
pub type SeedJobTable = Arc<Mutex<HashMap<String, SeedJob>>>;

/// Request body for creating a seed job (`POST /tiles/seed`).
#[derive(Debug, Clone, Deserialize)]
pub struct SeedRequest {
    pub layer: String,
    /// Gridset name (`EPSG:4326` / `EPSG:3857`); defaults to `EPSG:4326`.
    pub gridset: Option<String>,
    pub z_min: u32,
    pub z_max: u32,
    /// Output format: `png` (default) or `jpeg`.
    pub format: Option<String>,
}

/// Create and launch a seed job in the background. Returns the initial job.
pub fn start_seed_job(jobs: SeedJobTable, state: Arc<AppState>, job: SeedJob) -> SeedJob {
    let id = job.id.clone();
    {
        let mut map = jobs.lock().unwrap();
        map.insert(id.clone(), job.clone());
    }

    tokio::spawn(async move {
        let outcome = run_seed_job(&state, &jobs, &id).await;
        let mut map = jobs.lock().unwrap();
        if let Some(j) = map.get_mut(&id) {
            match outcome {
                Ok(SeedOutcome::Completed) => {
                    j.status = SeedStatus::Completed;
                    j.error = None;
                },
                Ok(SeedOutcome::Cancelled) => {
                    j.status = SeedStatus::Cancelled;
                },
                Err(e) => {
                    j.status = SeedStatus::Failed;
                    j.error = Some(e);
                },
            }
            j.updated_at = now_str();
        }
    });

    job
}

enum SeedOutcome {
    Completed,
    Cancelled,
}

/// Worker: enumerate every tile in `[z_min, z_max]` and render it through the
/// shared pipeline (which writes the tile cache). Progress is pushed into the
/// job table; cancellation is detected between tiles.
async fn run_seed_job(
    state: &AppState,
    jobs: &SeedJobTable,
    id: &str,
) -> Result<SeedOutcome, String> {
    let (layer, gridset, z_min, z_max, format) = {
        let map = jobs.lock().unwrap();
        let j = map
            .get(id)
            .ok_or_else(|| "seed job not found".to_string())?;
        (
            j.layer.clone(),
            j.gridset.clone(),
            j.z_min,
            j.z_max,
            j.format.clone(),
        )
    };

    let tile_format = if format.contains("jpeg") || format.contains("jpg") {
        TileFormat::Jpeg
    } else {
        TileFormat::Png
    };

    // Total tile count across the zoom range.
    let total: u64 = (z_min..=z_max)
        .map(|z| {
            tile_grid::matrix_width(&gridset, z) as u64
                * tile_grid::matrix_height(&gridset, z) as u64
        })
        .sum();

    {
        let mut map = jobs.lock().unwrap();
        if let Some(j) = map.get_mut(id) {
            j.total = total;
            j.status = SeedStatus::Running;
            j.updated_at = now_str();
        }
    }

    let started = Instant::now();
    let mut done: u64 = 0;
    'outer: for z in z_min..=z_max {
        let width = tile_grid::matrix_width(&gridset, z);
        let height = tile_grid::matrix_height(&gridset, z);
        for col in 0..width {
            for row in 0..height {
                // Cooperative cancellation check.
                {
                    let map = jobs.lock().unwrap();
                    let cancelled = map
                        .get(id)
                        .map(|j| j.status == SeedStatus::Cancelled)
                        .unwrap_or(true);
                    if cancelled {
                        break 'outer;
                    }
                }
                let _ = render_tile_bytes(state, &layer, &gridset, z, col, row, tile_format).await;
                done += 1;
                if done.is_multiple_of(32) || done == total {
                    let mut map = jobs.lock().unwrap();
                    if let Some(j) = map.get_mut(id) {
                        j.done = done;
                        j.updated_at = now_str();
                    }
                }
            }
        }
    }

    let cancelled = {
        let map = jobs.lock().unwrap();
        map.get(id)
            .map(|j| j.status == SeedStatus::Cancelled)
            .unwrap_or(true)
    };
    {
        let mut map = jobs.lock().unwrap();
        if let Some(j) = map.get_mut(id) {
            j.done = done;
            j.updated_at = now_str();
            if !cancelled {
                tracing::info!(
                    "[Seed] job {} done: {}/{} tiles in {:?}",
                    id,
                    done,
                    total,
                    started.elapsed()
                );
            }
        }
    }

    if cancelled {
        Ok(SeedOutcome::Cancelled)
    } else {
        Ok(SeedOutcome::Completed)
    }
}

fn now_str() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_seed_job_serde() {
        let job = SeedJob {
            id: "j1".to_string(),
            layer: "world".to_string(),
            gridset: "EPSG:4326".to_string(),
            z_min: 0,
            z_max: 2,
            format: "png".to_string(),
            status: SeedStatus::Pending,
            total: 0,
            done: 0,
            error: None,
            created_at: "t".to_string(),
            updated_at: "t".to_string(),
        };
        let json = serde_json::to_string(&job).unwrap();
        let back: SeedJob = serde_json::from_str(&json).unwrap();
        assert_eq!(back.layer, "world");
        assert_eq!(back.z_max, 2);
    }

    #[test]
    fn test_seed_status_variants() {
        assert_eq!(SeedStatus::Running as u8, 1);
        assert_ne!(SeedStatus::Completed, SeedStatus::Failed);
    }
}
