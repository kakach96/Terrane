use crate::error::GeoServerError;
use crate::models::{
    Bounds, DataSource, DataSourceConnection, DataSourceType, Feature, GeoJsonGeometry,
    METADATA_DATA_SOURCE,
};
use crate::state::AppState;
use crate::utils::wkb;
use tracing::info;

pub async fn query_layer_features(
    state: &AppState,
    layer_name: &str,
    bbox: Option<&Bounds>,
    limit: Option<u64>,
    offset: Option<u64>,
) -> Result<Vec<Feature>, GeoServerError> {
    let layer = {
        let layers = state.layers.read().await;
        layers.iter().find(|l| l.name == layer_name).cloned()
    };
    let layer = layer
        .ok_or_else(|| GeoServerError::NotFound(format!("Layer '{}' not found", layer_name)))?;

    // 解析图层数据源。内置 metadata 数据源被当作普通数据源看待: 它除了存储元数据外,
    // 也可发布其承载的业务数据 (postgres 元数据模式复用同一 PG, 走 PostGIS 查询)。
    let data_source = if let Some(store) = &state.store {
        store
            .get_data_source(&layer.store)
            .await
            .map_err(|e| GeoServerError::InternalError(format!("DB error: {}", e)))?
    } else {
        None
    };
    // 内置 metadata 数据源不持久化在元数据库中, 按需合成 (与 REST 层 builtin 呈现一致)
    let data_source = data_source.or_else(|| builtin_metadata_data_source(state));

    if let Some(ref ds) = data_source {
        match ds.data_source_type {
            DataSourceType::Postgis => {
                if let Some(ref conn) = ds.connection {
                    if let Some(ref native_name) = layer.native_name {
                        let pool = state.get_pg_pool(&ds.name, conn);
                        return query_postgis_features(
                            &pool,
                            conn,
                            native_name,
                            bbox,
                            limit,
                            offset,
                        )
                        .await;
                    }
                }
                info!(
                    "[Features] PostGIS 数据源 '{}' 缺少连接/表名, 返回空",
                    ds.name
                );
                return Ok(Vec::new());
            },
            DataSourceType::Shapefile => {
                return query_shapefile_features(ds, bbox, limit, offset).await;
            },
            DataSourceType::GeoJson => {
                return query_geojson_features(ds, bbox, limit, offset).await;
            },
            DataSourceType::Geopackage => {
                return query_geopackage_features(ds, bbox, limit, offset).await;
            },
            DataSourceType::Geotiff => {
                info!(
                    "[Features] GeoTIFF 数据源 '{}' 是栅格格式, 通过 WCS 访问, 不返回矢量要素",
                    ds.name
                );
                return Ok(Vec::new());
            },
            DataSourceType::WorldImage => {
                info!(
                    "[Features] WorldImage 数据源 '{}' 是栅格格式, 通过 WCS 访问",
                    ds.name
                );
                return Ok(Vec::new());
            },
            DataSourceType::CascadedWms => {
                info!(
                    "[Features] CascadedWms 数据源 '{}' 是级联服务, 通过 WMS 代理访问",
                    ds.name
                );
                return Ok(Vec::new());
            },
            DataSourceType::ArcGrid => {
                info!(
                    "[Features] ArcGrid 数据源 '{}' 是栅格格式, 通过 WCS 访问",
                    ds.name
                );
                return Ok(Vec::new());
            },
            DataSourceType::Metadata => {
                info!(
                    "[Features] metadata 数据源 '{}' (sqlite 元数据) 不承载业务表, 返回空",
                    ds.name
                );
                return Ok(Vec::new());
            },
        }
    }

    // 强制数据源: 无数据源的图层返回空 (避免 5xx), 由管理端配置数据源
    info!(
        "[Features] 图层 '{}' 无数据源 '{}', 返回空 (请先配置数据源)",
        layer_name, layer.store
    );
    Ok(Vec::new())
}

/// 构造内置 metadata 数据源 (不持久化在元数据库中, 按需合成)。
///
/// 内置 metadata 数据源被当作普通数据源看待, 与其它数据源无异:
/// - postgres 元数据模式: 复用同一 PG, 类型为 PostGIS (postgis 语义, 可发布业务表)
/// - sqlite 元数据模式: 不承载业务表, 类型为 Metadata (要素查询返回空)
pub(crate) fn builtin_metadata_data_source(state: &AppState) -> Option<DataSource> {
    let mc = &state.config.metadata;
    if mc.kind == "postgres" {
        let pg = &mc.postgres;
        Some(DataSource {
            name: METADATA_DATA_SOURCE.to_string(),
            data_source_type: DataSourceType::Postgis,
            workspace: None,
            enabled: true,
            connection: Some(DataSourceConnection {
                host: Some(pg.host.clone()),
                port: Some(pg.port),
                database: Some(pg.instance.clone()),
                schema: Some(pg.schema.clone()),
                username: Some(pg.user.clone()),
                password: Some(pg.password.clone()),
                ..Default::default()
            }),
            created: None,
            modified: None,
        })
    } else {
        Some(DataSource {
            name: METADATA_DATA_SOURCE.to_string(),
            data_source_type: DataSourceType::Metadata,
            workspace: None,
            enabled: true,
            connection: None,
            created: None,
            modified: None,
        })
    }
}

/// 从 GeoJSON 数据源查询要素。
///
/// 从 GeoJSON 数据源查询要素 (支持 local / s3)。
async fn query_geojson_features(
    ds: &crate::models::DataSource,
    bbox: Option<&Bounds>,
    limit: Option<u64>,
    offset: Option<u64>,
) -> Result<Vec<Feature>, GeoServerError> {
    let conn = ds
        .connection
        .as_ref()
        .ok_or_else(|| GeoServerError::BadRequest("GeoJSON 数据源缺少连接信息".to_string()))?;
    let file_path = conn
        .file_path
        .as_ref()
        .ok_or_else(|| GeoServerError::BadRequest("GeoJSON 数据源缺少文件路径".to_string()))?;

    info!("[Features] 从 GeoJSON 读取要素: {}", file_path);

    let bytes = crate::store::read_bytes(conn).await?.ok_or_else(|| {
        GeoServerError::NotFound(format!("GeoJSON 文件不存在: {}", file_path))
    })?;
    let raw = String::from_utf8(bytes)
        .map_err(|e| GeoServerError::InternalError(format!("GeoJSON 编码错误: {}", e)))?;
    let root: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| GeoServerError::InternalError(format!("解析 GeoJSON 失败: {}", e)))?;

    let features: Vec<Feature> = root
        .get("features")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|f| serde_json::from_value::<Feature>(f.clone()).ok())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let mut filtered = filter_features(features, bbox);
    let total = filtered.len();
    if let Some(o) = offset {
        let o = o as usize;
        if o < total {
            filtered = filtered.into_iter().skip(o).collect();
        } else {
            return Ok(Vec::new());
        }
    }
    if let Some(l) = limit {
        filtered.truncate(l as usize);
    }
    Ok(filtered)
}

/// 从 Shapefile 数据源查询要素 (支持 local / s3)。
async fn query_shapefile_features(
    ds: &crate::models::DataSource,
    bbox: Option<&Bounds>,
    limit: Option<u64>,
    offset: Option<u64>,
) -> Result<Vec<Feature>, GeoServerError> {
    let conn = ds
        .connection
        .as_ref()
        .ok_or_else(|| GeoServerError::BadRequest("Shapefile 数据源缺少连接信息".to_string()))?;
    let file_path = conn
        .file_path
        .as_ref()
        .ok_or_else(|| GeoServerError::BadRequest("Shapefile 数据源缺少文件路径".to_string()))?;

    info!("[Features] 从 Shapefile 读取要素: {}", file_path);

    let materialized = crate::store::materialize_dir(conn).await?.ok_or_else(|| {
        GeoServerError::NotFound(format!("Shapefile 文件不存在: {}", file_path))
    })?;
    let result = crate::utils::shapefile::read_shapefile(&materialized.path)
        .map_err(|e| GeoServerError::InternalError(format!("读取 Shapefile 失败: {}", e)))?;

    let mut features = result.features;

    // 应用 bbox 过滤
    if let Some(b) = bbox {
        features.retain(|f| feature_in_bbox(f, b));
    }

    // 应用 offset / limit
    let total = features.len();
    if let Some(o) = offset {
        let o = o as usize;
        if o < total {
            features = features.into_iter().skip(o).collect();
        } else {
            return Ok(Vec::new());
        }
    }
    if let Some(l) = limit {
        features.truncate(l as usize);
    }

    Ok(features)
}

fn filter_features(features: Vec<Feature>, bbox: Option<&Bounds>) -> Vec<Feature> {
    match bbox {
        Some(b) => features
            .into_iter()
            .filter(|f| feature_in_bbox(f, b))
            .collect(),
        None => features,
    }
}

fn feature_in_bbox(feature: &Feature, bounds: &Bounds) -> bool {
    match &feature.geometry {
        GeoJsonGeometry::Point { coordinates } => {
            if coordinates.len() >= 2 {
                let (x, y) = (coordinates[0], coordinates[1]);
                x >= bounds.minx && x <= bounds.maxx && y >= bounds.miny && y <= bounds.maxy
            } else {
                true
            }
        },
        _ => true,
    }
}

async fn query_postgis_features(
    pool: &deadpool_postgres::Pool,
    conn: &crate::models::DataSourceConnection,
    native_name: &str,
    bbox: Option<&Bounds>,
    limit: Option<u64>,
    _offset: Option<u64>,
) -> Result<Vec<Feature>, GeoServerError> {
    let client = pool
        .get()
        .await
        .map_err(|e| GeoServerError::InternalError(format!("Pool error: {}", e)))?;

    let schema = conn
        .schema
        .as_deref()
        .map(|s| {
            if s.is_empty() || s == "public" {
                "public"
            } else {
                s
            }
        })
        .unwrap_or("public")
        .to_string();

    let cols = get_table_columns(&client, &schema, native_name).await;
    let geom_col = get_geometry_column(&client, &schema, native_name)
        .await
        .unwrap_or_else(|| "geom".to_string());

    let non_geom_cols: Vec<String> = cols.iter().filter(|c| *c != &geom_col).cloned().collect();

    let raw_names: Vec<&str> = non_geom_cols.iter().map(|c| c.as_str()).collect();
    let col_list = raw_names.join(", ");

    let mut sql = format!(
        "SELECT {} as _id, ST_AsGeoJSON({}) as _geometry, {} FROM \"{}\".\"{}\"",
        get_id_expr(&client, &schema, native_name).await,
        geom_col,
        if col_list.is_empty() {
            "1 as _dummy".to_string()
        } else {
            col_list
        },
        schema,
        native_name,
    );

    let limit_val = limit.unwrap_or(10000);

    if let Some(b) = bbox {
        sql.push_str(&format!(
            " WHERE {} && ST_MakeEnvelope({}, {}, {}, {}, 4326) LIMIT {}",
            geom_col, b.minx, b.miny, b.maxx, b.maxy, limit_val
        ));
    } else {
        sql.push_str(&format!(" LIMIT {}", limit_val));
    }

    let rows = client
        .query(&sql, &[])
        .await
        .map_err(|e| GeoServerError::InternalError(format!("PostGIS query error: {}", e)))?;

    let mut features = Vec::with_capacity(rows.len());
    for row in &rows {
        let geojson_str: String = row.try_get("_geometry").unwrap_or_default();
        let geometry = wkb::parse_geojson_geometry(&geojson_str);

        let (id, properties) = match wkb::parse_postgres_row(row, &non_geom_cols, &geom_col) {
            Ok((id, _, props)) => (id, props),
            Err(_) => {
                let id: String = row
                    .try_get("_id")
                    .unwrap_or_else(|_| uuid::Uuid::new_v4().to_string());
                let props = std::collections::HashMap::new();
                (id, props)
            },
        };
        features.push(Feature::with_id(id, geometry, properties));
    }

    Ok(features)
}

async fn get_table_columns(
    client: &tokio_postgres::Client,
    schema: &str,
    table: &str,
) -> Vec<String> {
    let sql = format!(
        "SELECT column_name FROM information_schema.columns
         WHERE table_schema = $1 AND table_name = $2
         ORDER BY ordinal_position"
    );
    match client.query(&sql, &[&schema, &table]).await {
        Ok(rows) => rows.iter().map(|r| r.get::<_, String>(0)).collect(),
        Err(_) => vec![],
    }
}

async fn get_geometry_column(
    client: &tokio_postgres::Client,
    schema: &str,
    table: &str,
) -> Option<String> {
    let sql = "SELECT f_geometry_column FROM geometry_columns WHERE f_table_schema = $1 AND f_table_name = $2";
    if let Ok(rows) = client.query(sql, &[&schema, &table]).await {
        if let Some(row) = rows.first() {
            return row.get::<_, String>(0).into();
        }
    }
    None
}

/// 从 GeoPackage 查询要素 (支持 local / s3)。
async fn query_geopackage_features(
    ds: &crate::models::DataSource,
    bbox: Option<&Bounds>,
    limit: Option<u64>,
    offset: Option<u64>,
) -> Result<Vec<Feature>, GeoServerError> {
    let conn = ds
        .connection
        .as_ref()
        .ok_or_else(|| GeoServerError::BadRequest("GeoPackage 数据源缺少连接信息".to_string()))?;
    let file_path = conn
        .file_path
        .as_ref()
        .ok_or_else(|| GeoServerError::BadRequest("GeoPackage 数据源缺少文件路径".to_string()))?;

    info!("[Features] 从 GeoPackage 读取要素: {}", file_path);

    let materialized = crate::store::materialize_file(conn).await?.ok_or_else(|| {
        GeoServerError::NotFound(format!("GeoPackage 文件不存在: {}", file_path))
    })?;
    let local_path = materialized.path.as_path();

    // 读取所有图层（取第一个有数据的图层）
    let layers = crate::utils::geopackage::read_geopackage_layers(local_path)
        .map_err(|e| GeoServerError::InternalError(format!("读取 GeoPackage 失败: {}", e)))?;

    if layers.is_empty() {
        return Err(GeoServerError::NotFound(
            "GeoPackage 中没有找到图层".to_string(),
        ));
    }

    // 使用第一个图层
    let first_layer = &layers[0];
    let result = crate::utils::geopackage::read_geopackage_layer_features(
        local_path,
        &first_layer.table_name,
        limit,
    )
    .map_err(|e| GeoServerError::InternalError(format!("读取要素失败: {}", e)))?;

    let mut features = result.features;

    // 应用 bbox 过滤
    if let Some(b) = bbox {
        features.retain(|f| feature_in_bbox(f, b));
    }

    // 应用 offset
    if let Some(o) = offset {
        let o = o as usize;
        if o < features.len() {
            features = features.into_iter().skip(o).collect();
        } else {
            return Ok(Vec::new());
        }
    }
    // limit 已经在 read_geopackage_layer_features 中应用了

    Ok(features)
}

async fn get_id_expr(client: &tokio_postgres::Client, schema: &str, table: &str) -> String {
    let sql = format!(
        "SELECT column_name FROM information_schema.columns
         WHERE table_schema = $1 AND table_name = $2
         AND (column_name = 'id' OR column_name LIKE '%_id' OR column_name = 'gid')
         ORDER BY ordinal_position LIMIT 1"
    );
    match client.query(&sql, &[&schema, &table]).await {
        Ok(rows) if !rows.is_empty() => rows[0].get::<_, String>(0),
        Err(_) | Ok(_) => format!("'{}'", uuid::Uuid::new_v4()),
    }
}
