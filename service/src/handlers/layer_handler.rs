use super::rest_handler::ApiResponse;
use crate::error::GeoServerError;
use crate::models::{CoordinateReferenceSystem, FeatureCollection, Layer};
use crate::state::AppState;
use crate::store::FileStore;
use crate::utils::rendering;
use actix_web::{web, HttpRequest, HttpResponse};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CreateLayerRequest {
    pub name: String,
    pub title: String,
    pub workspace: String,
    pub store: String,
    pub native_name: Option<String>,
    pub srs: Option<String>,
    pub minx: Option<f64>,
    pub miny: Option<f64>,
    pub maxx: Option<f64>,
    pub maxy: Option<f64>,
    #[serde(rename = "abstract")]
    pub abstract_text: Option<String>,
    /// 瓦片缓存后端数据源名称 (type = "redis"); 缺省/空 = 默认内存/本地缓存
    #[serde(default)]
    pub cache_store: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateLayerRequest {
    pub title: Option<String>,
    #[serde(rename = "abstract")]
    pub abstract_text: Option<String>,
    pub native_name: Option<String>,
    pub enabled: Option<bool>,
    /// 瓦片缓存后端数据源: Some(ds) = 设置, null = 清除 (回到默认缓存), 缺省 = 不修改。
    /// 使用自定义反序列化以区分「缺省 (不修改)」与「显式 null (清除)」。
    #[serde(default, deserialize_with = "deserialize_optional_clear")]
    pub cache_store: Option<Option<String>>,
}

/// 反序列化 `Option<Option<String>>` 语义的缓存后端字段:
/// - 字段缺省 → `None` (不修改)
/// - 显式 `null` → `Some(None)` (清除, 回到默认缓存)
/// - 字符串 → `Some(Some(name))` (设置数据源)
///
/// 直接反序列化为 `Value` 以保留显式 `null` (若先包一层 `Option<Value>`,
/// `null` 会被吞成 `None`, 无法与「缺省」区分)。
fn deserialize_optional_clear<'de, D>(deserializer: D) -> Result<Option<Option<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(match value {
        serde_json::Value::Null => Some(None),
        serde_json::Value::String(s) => Some(Some(s)),
        other => {
            return Err(serde::de::Error::custom(format!(
                "cache_store 应为字符串或 null, 实际: {}",
                other
            )))
        },
    })
}

#[derive(Debug, Deserialize)]
pub struct PreviewRequest {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub format: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GeoJsonUploadRequest {
    pub name: String,
    pub title: String,
}

pub async fn list_layers(state: web::Data<AppState>) -> Result<HttpResponse, GeoServerError> {
    if let Some(store) = &state.store {
        match store.get_all_layers().await {
            Ok(layers) => {
                let result: Vec<_> = layers
                    .iter()
                    .map(|l| {
                        serde_json::json!({
                            "name": l.name,
                            "title": l.title,
                            "workspace": l.workspace,
                            "store": l.store,
                            "native_name": l.native_name,
                            "srs": l.srs,
                            "bounds": {
                                "minx": l.minx,
                                "miny": l.miny,
                                "maxx": l.maxx,
                                "maxy": l.maxy,
                            },
                            "enabled": l.enabled,
                            "cache_store": l.cache_store,
                        })
                    })
                    .collect();

                Ok(HttpResponse::Ok().json(ApiResponse::success(result)))
            },
            Err(e) => {
                eprintln!("Failed to list layers: {}", e);
                Err(GeoServerError::InternalError(
                    "Failed to list layers".to_string(),
                ))
            },
        }
    } else {
        let layers = state.list_layers().await;
        let result: Vec<_> = layers
            .iter()
            .map(|l| {
                serde_json::json!({
                    "name": l.name,
                    "title": l.title,
                    "workspace": l.workspace,
                    "store": l.store,
                    "native_name": l.native_name,
                    "srs": l.srs.to_epsg(),
                    "bounds": {
                        "minx": l.native_bounds.bounds.minx,
                        "miny": l.native_bounds.bounds.miny,
                        "maxx": l.native_bounds.bounds.maxx,
                        "maxy": l.native_bounds.bounds.maxy,
                    },
                    "styles": l.styles,
                    "enabled": l.enabled,
                    "cache_store": l.cache_store,
                })
            })
            .collect();

        Ok(HttpResponse::Ok().json(ApiResponse::success(result)))
    }
}

pub async fn get_layer(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let layer_name = req.match_info().get("layer").unwrap_or("");

    let response = if let Some(store) = &state.store {
        match store.get_layer(layer_name).await {
            Ok(Some(layer)) => {
                let mut map = serde_json::Map::new();
                map.insert("name".into(), serde_json::Value::String(layer.name.clone()));
                map.insert(
                    "title".into(),
                    serde_json::Value::String(layer.title.clone()),
                );
                map.insert(
                    "abstract".into(),
                    layer
                        .abstract_text
                        .clone()
                        .map(serde_json::Value::String)
                        .unwrap_or(serde_json::Value::Null),
                );
                map.insert(
                    "workspace".into(),
                    serde_json::Value::String(layer.workspace.clone()),
                );
                map.insert(
                    "store".into(),
                    serde_json::Value::String(layer.store.clone()),
                );
                map.insert(
                    "native_name".into(),
                    layer
                        .native_name
                        .clone()
                        .map(serde_json::Value::String)
                        .unwrap_or(serde_json::Value::Null),
                );
                map.insert("srs".into(), serde_json::Value::String(layer.srs.clone()));
                map.insert(
                    "bounds".into(),
                    serde_json::json!({
                        "minx": layer.minx, "miny": layer.miny,
                        "maxx": layer.maxx, "maxy": layer.maxy,
                    }),
                );
                map.insert(
                    "native_bounds".into(),
                    serde_json::json!({
                        "crs": layer.srs, "bounds": {
                            "minx": layer.minx, "miny": layer.miny,
                            "maxx": layer.maxx, "maxy": layer.maxy,
                        }
                    }),
                );
                map.insert(
                    "lat_lon_bounds".into(),
                    serde_json::json!({
                        "crs": "EPSG:4326", "bounds": {
                            "minx": layer.minx, "miny": layer.miny,
                            "maxx": layer.maxx, "maxy": layer.maxy,
                        }
                    }),
                );
                map.insert("enabled".into(), serde_json::Value::Bool(layer.enabled));
                map.insert("styles".into(), serde_json::Value::Array(vec![]));
                map.insert(
                    "cache_store".into(),
                    layer
                        .cache_store
                        .clone()
                        .map(serde_json::Value::String)
                        .unwrap_or(serde_json::Value::Null),
                );
                serde_json::Value::Object(map)
            },
            Ok(None) => {
                return Err(GeoServerError::NotFound(format!(
                    "Layer '{}' not found",
                    layer_name
                )))
            },
            Err(e) => {
                return Err(GeoServerError::InternalError(format!(
                    "Failed to get layer: {}",
                    e
                )))
            },
        }
    } else {
        if let Some(layer) = state.get_layer(layer_name).await {
            let mut map = serde_json::Map::new();
            map.insert("name".into(), serde_json::Value::String(layer.name.clone()));
            map.insert(
                "title".into(),
                serde_json::Value::String(layer.title.clone()),
            );
            map.insert(
                "abstract".into(),
                layer
                    .abstract_text
                    .clone()
                    .map(serde_json::Value::String)
                    .unwrap_or(serde_json::Value::Null),
            );
            map.insert(
                "workspace".into(),
                serde_json::Value::String(layer.workspace.clone()),
            );
            map.insert(
                "store".into(),
                serde_json::Value::String(layer.store.clone()),
            );
            map.insert(
                "native_name".into(),
                layer
                    .native_name
                    .clone()
                    .map(serde_json::Value::String)
                    .unwrap_or(serde_json::Value::Null),
            );
            map.insert("srs".into(), serde_json::Value::String(layer.srs.to_epsg()));
            map.insert("bounds".into(), serde_json::json!({
                "minx": layer.native_bounds.bounds.minx, "miny": layer.native_bounds.bounds.miny,
                "maxx": layer.native_bounds.bounds.maxx, "maxy": layer.native_bounds.bounds.maxy,
            }));
            map.insert("native_bounds".into(), serde_json::json!({
                "crs": layer.native_bounds.crs.to_epsg(), "bounds": {
                    "minx": layer.native_bounds.bounds.minx, "miny": layer.native_bounds.bounds.miny,
                    "maxx": layer.native_bounds.bounds.maxx, "maxy": layer.native_bounds.bounds.maxy,
                }
            }));
            map.insert("lat_lon_bounds".into(), serde_json::json!({
                "crs": layer.lat_lon_bounds.crs.to_epsg(), "bounds": {
                    "minx": layer.lat_lon_bounds.bounds.minx, "miny": layer.lat_lon_bounds.bounds.miny,
                    "maxx": layer.lat_lon_bounds.bounds.maxx, "maxy": layer.lat_lon_bounds.bounds.maxy,
                }
            }));
            map.insert("enabled".into(), serde_json::Value::Bool(layer.enabled));
            map.insert(
                "styles".into(),
                serde_json::to_value(&layer.styles).unwrap_or(serde_json::Value::Array(vec![])),
            );
            map.insert(
                "cache_store".into(),
                layer
                    .cache_store
                    .clone()
                    .map(serde_json::Value::String)
                    .unwrap_or(serde_json::Value::Null),
            );
            serde_json::Value::Object(map)
        } else {
            return Err(GeoServerError::NotFound(format!(
                "Layer '{}' not found",
                layer_name
            )));
        }
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
}

pub async fn create_layer(
    body: web::Json<CreateLayerRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let srs = body.srs.clone().unwrap_or_else(|| "EPSG:4326".to_string());

    // 1. 用户显式提供了边界 → 优先使用
    let (mut minx, mut miny, mut maxx, mut maxy) = if let (Some(x1), Some(y1), Some(x2), Some(y2)) =
        (body.minx, body.miny, body.maxx, body.maxy)
    {
        (x1, y1, x2, y2)
    } else {
        // 未提供 → 标记为未设置，后续尝试自动计算
        (f64::MAX, f64::MAX, f64::MIN, f64::MIN)
    };

    // 2. 如果边界是默认/未设置状态，尝试从数据源自动计算
    let user_provided = minx != f64::MAX;
    if !user_provided {
        if let Some(ref store) = state.store {
            if let Ok(Some(ds)) = store.get_data_source(&body.store).await {
                let crs = crate::models::CoordinateReferenceSystem::from_epsg(&srs);
                let native_name = body.native_name.as_deref();

                // 获取 PostGIS 连接池（如适用）
                let pg_pool = if ds.data_source_type == crate::models::DataSourceType::Postgis {
                    state.pg_pools.lock().unwrap().get(&body.store).cloned()
                } else {
                    None
                };

                match crate::utils::bounds::compute_layer_bounds(&ds, native_name, pg_pool.as_ref())
                    .await
                {
                    Ok(Some(computed)) => {
                        minx = computed.bounds.minx;
                        miny = computed.bounds.miny;
                        maxx = computed.bounds.maxx;
                        maxy = computed.bounds.maxy;
                        tracing::info!(
                            "[create_layer] 从数据源自动计算边界: ({}, {}, {}, {})",
                            minx,
                            miny,
                            maxx,
                            maxy
                        );
                    },
                    _ => {
                        // 自动计算失败 → 使用 CRS 世界范围
                        let world = crate::models::BoundingBox::world(crs.clone());
                        minx = world.bounds.minx;
                        miny = world.bounds.miny;
                        maxx = world.bounds.maxx;
                        maxy = world.bounds.maxy;
                        tracing::info!("[create_layer] 使用 CRS({}) 世界范围作为默认边界", srs);
                    },
                }
            }
        }
    }

    // 3. 如果仍为初始值，用 CRS 世界范围兜底
    if minx == f64::MAX {
        let crs = crate::models::CoordinateReferenceSystem::from_epsg(&srs);
        let world = crate::models::BoundingBox::world(crs);
        minx = world.bounds.minx;
        miny = world.bounds.miny;
        maxx = world.bounds.maxx;
        maxy = world.bounds.maxy;
    }

    if let Some(store) = &state.store {
        let layer = crate::store::sqlite_store::Layer {
            name: body.name.clone(),
            title: body.title.clone(),
            workspace: body.workspace.clone(),
            store: body.store.clone(),
            srs: srs.clone(),
            abstract_text: body.abstract_text.clone(),
            native_name: body.native_name.clone(),
            enabled: true,
            minx,
            miny,
            maxx,
            maxy,
            cache_store: body.cache_store.clone(),
            created: String::new(),
            modified: String::new(),
        };

        match store.create_layer(&layer).await {
            Ok(created_layer) => {
                // Apply the user-provided (or auto-computed) bounds to the
                // in-memory Layer, so the catalog reflects the real extent.
                let mut created = Layer::new(
                    created_layer.name.clone(),
                    created_layer.title.clone(),
                    created_layer.workspace.clone(),
                    created_layer.store.clone(),
                    CoordinateReferenceSystem::from_epsg(&created_layer.srs),
                );
                created.lat_lon_bounds = crate::models::BoundingBox::new(
                    CoordinateReferenceSystem::EPSG4326,
                    crate::models::Bounds::new(minx, miny, maxx, maxy),
                );
                created.native_bounds = created.lat_lon_bounds.clone();
                created.cache_store = created_layer.cache_store.clone();
                state.add_layer(created).await;

                let response = serde_json::json!({
                    "name": created_layer.name,
                    "title": created_layer.title,
                    "workspace": created_layer.workspace,
                    "store": created_layer.store,
                    "native_name": created_layer.native_name,
                    "srs": created_layer.srs,
                    "bounds": {
                        "minx": created_layer.minx,
                        "miny": created_layer.miny,
                        "maxx": created_layer.maxx,
                        "maxy": created_layer.maxy,
                    },
                    "enabled": created_layer.enabled,
                    "message": "Layer created successfully",
                });

                Ok(HttpResponse::Created().json(ApiResponse::success(response)))
            },
            Err(e) => {
                eprintln!("Failed to create layer: {}", e);
                Err(GeoServerError::InternalError(
                    "Failed to create layer".to_string(),
                ))
            },
        }
    } else {
        Err(GeoServerError::InternalError(
            "Database not available".to_string(),
        ))
    }
}

pub async fn update_layer(
    req: HttpRequest,
    body: web::Json<UpdateLayerRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let layer_name = req.match_info().get("layer").unwrap_or("");

    if let Some(store) = &state.store {
        match store
            .update_layer(
                layer_name,
                body.title.clone(),
                body.abstract_text.clone(),
                body.native_name.clone(),
                body.enabled,
                body.cache_store.clone(),
            )
            .await
        {
            Ok(_) => {
                // 事件驱动目录刷新: 立即重载内存目录, 消除"写后读旧"窗口。
                state.refresh_catalog().await;
                Ok(
                    HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
                        "message": format!("Layer '{}' updated successfully", layer_name),
                    }))),
                )
            },
            Err(e) => {
                eprintln!("Failed to update layer: {}", e);
                Err(GeoServerError::InternalError(
                    "Failed to update layer".to_string(),
                ))
            },
        }
    } else {
        let updates = crate::state::LayerUpdates {
            title: body.title.clone(),
            abstract_text: body.abstract_text.clone(),
            enabled: body.enabled,
            cache_store: body.cache_store.clone(),
        };
        if state.update_layer(layer_name, updates).await {
            Ok(
                HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
                    "message": format!("Layer '{}' updated", layer_name),
                }))),
            )
        } else {
            Err(GeoServerError::NotFound(format!(
                "Layer '{}' not found",
                layer_name
            )))
        }
    }
}

pub async fn delete_layer(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let layer_name = req.match_info().get("layer").unwrap_or("");

    if let Some(store) = &state.store {
        match store.delete_layer(layer_name).await {
            Ok(_) => {
                // 事件驱动目录刷新: 立即从内存目录移除已删除图层
                // (refresh_catalog 按名称更新/新增不删除, 故此处显式移除)。
                state.delete_layer(layer_name).await;
                Ok(
                    HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
                        "message": format!("Layer '{}' deleted", layer_name),
                    }))),
                )
            },
            Err(e) => {
                eprintln!("Failed to delete layer: {}", e);
                Err(GeoServerError::InternalError(
                    "Failed to delete layer".to_string(),
                ))
            },
        }
    } else {
        if state.delete_layer(layer_name).await {
            Ok(
                HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
                    "deleted": true,
                    "layer": layer_name,
                }))),
            )
        } else {
            Err(GeoServerError::NotFound(format!(
                "Layer '{}' not found",
                layer_name
            )))
        }
    }
}

pub async fn preview_layer(
    req: HttpRequest,
    query: web::Query<PreviewRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let layer_name = req.match_info().get("layer").unwrap_or("");

    if state.get_layer(layer_name).await.is_none() {
        return Err(GeoServerError::NotFound(format!(
            "Layer '{}' not found",
            layer_name
        )));
    }

    let width = query.width.unwrap_or(512);
    let height = query.height.unwrap_or(512);
    let format = query.format.clone().unwrap_or_else(|| "png".to_string());

    let features =
        crate::handlers::features::query_layer_features(&state, layer_name, None, None, None)
            .await
            .unwrap_or_default();

    let image_data = rendering::render_map(&features, width, height);

    let content_type = match format.as_str() {
        "jpeg" | "jpg" => "image/jpeg",
        "gif" => "image/gif",
        _ => "image/png",
    };

    Ok(HttpResponse::Ok()
        .content_type(content_type)
        .body(image_data))
}

pub async fn upload_geojson(
    body: web::Json<serde_json::Value>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    // 校验 GeoJSON FeatureCollection
    let fc = if let Some(fc) = body.get("FeatureCollection") {
        serde_json::from_value::<FeatureCollection>(fc.clone())
            .map_err(|e| GeoServerError::BadRequest(format!("Invalid GeoJSON: {}", e)))?
    } else {
        serde_json::from_value::<FeatureCollection>(body.clone())
            .map_err(|e| GeoServerError::BadRequest(format!("Invalid GeoJSON: {}", e)))?
    };

    // 数据源名: 优先从要素属性 layer_name 读取, 否则用默认
    let ds_name = fc
        .features
        .first()
        .and_then(|f| f.properties.get("layer_name"))
        .and_then(|v| {
            if let crate::models::PropertyValue::String(s) = v {
                Some(s.as_str())
            } else {
                None
            }
        })
        .unwrap_or("uploaded")
        .to_string();

    if let Some(store) = &state.store {
        if let Ok(Some(_)) = store.get_data_source(&ds_name).await {
            return Err(GeoServerError::Conflict(format!(
                "Data source '{}' already exists",
                ds_name
            )));
        }
    }

    // 保存为本地文件 (数据目录 <data_dir>/geojson/<name>.geojson), 本地存储后端
    let data_dir = state.config.data_dir.clone();
    let geojson_dir = data_dir.join("geojson");
    let file_store = crate::store::LocalFileStore::new(geojson_dir.clone());
    let file_name = format!("{}.geojson", ds_name);
    let bytes = serde_json::to_vec_pretty(&fc)
        .map_err(|e| GeoServerError::InternalError(format!("序列化 GeoJSON 失败: {}", e)))?;
    file_store
        .put(&file_name, &bytes)
        .await
        .map_err(|e| GeoServerError::InternalError(format!("保存 GeoJSON 文件失败: {}", e)))?;
    let file_path = file_store
        .local_path(&file_name)
        .unwrap_or_else(|| geojson_dir.join(&file_name));

    // 创建 GeoJSON 数据源 (本地存储, file_storage_type = "local")
    let connection =
        crate::models::DataSourceConnection::file(file_path.to_string_lossy().to_string());

    if let Some(store) = &state.store {
        match store
            .create_data_source(
                &ds_name,
                &crate::models::DataSourceType::GeoJson,
                Some("default".to_string()),
                true,
                &connection,
            )
            .await
        {
            Ok(ds) => {
                tracing::info!("[Upload] GeoJSON 数据源已创建: {}", ds.name);
                Ok(HttpResponse::Created().json(ApiResponse::success(serde_json::json!({
                    "name": ds.name,
                    "type": "geojson",
                    "file_path": ds.connection.as_ref().and_then(|c| c.file_path.as_ref()),
                    "file_storage_type": "local",
                    "message": format!("GeoJSON '{}' uploaded and data source created", ds.name),
                }))))
            },
            Err(e) => {
                tracing::warn!("[Upload] 创建数据源失败: {}", e);
                Err(GeoServerError::InternalError("创建数据源失败".to_string()))
            },
        }
    } else {
        Err(GeoServerError::InternalError("数据库不可用".to_string()))
    }
}
