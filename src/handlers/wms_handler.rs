use actix_web::{HttpRequest, HttpResponse, web};
use crate::services::wms::{self, WmsRequest, WmsCapabilities};
use crate::error::GeoServerError;
use crate::state::AppState;
use crate::utils::rendering::{MapRenderer, RenderOptions, RenderFormat, Style};
use crate::utils::sld_parser::{self, ParsedRule};
use crate::utils::projection::ProjectionTransformer;
use crate::utils::wkb;
use crate::models::{Bounds, CoordinateReferenceSystem, GeoJsonGeometry, Feature, DataSourceType, Layer};
use quick_xml::se::to_string;
use std::io::Cursor;
use std::collections::HashMap;
use image::ImageFormat;
use tracing::{info, debug, warn};

struct GetMapContext {
    layers: Vec<String>,
    bounds: Bounds,
    output_crs: String,
    width: u32,
    height: u32,
    format: String,
    transparent: bool,
    bg_color: Option<[u8; 4]>,
    scale_denominator: f64,
    options: RenderOptions,
    /// CQL 过滤器（以 ; 分隔，对应多个图层）
    cql_filter: Option<String>,
    /// FeatureId 过滤（逗号分隔）
    feature_id: Option<Vec<String>>,
    /// 地图旋转角度（度）
    angle: Option<f64>,
    /// 环境变量 (SLD 替换)
    env: Option<HashMap<String, String>>,
    /// 时间过滤 (ISO 8601)
    time: Option<String>,
    /// 高程过滤
    elevation: Option<String>,
}

struct LayerMetadata {
    layer_name: String,
    workspace: String,
    layer_obj: Layer,
    data_source_type: DataSourceType,
    connection: Option<crate::models::DataSourceConnection>,
    native_name: String,
    native_crs: String,
    rules: Vec<ParsedRule>,
    max_features: u32,
}

struct LayerRenderContext {
    metadata: LayerMetadata,
    features: Vec<Feature>,
    render_items: Vec<(GeoJsonGeometry, Style)>,
}

pub async fn handle_wms_request(
    _req: HttpRequest,
    query: web::Query<Vec<(String, String)>>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let params: Vec<(String, String)> = query.into_inner();
    let wms_request = match wms::parse_wms_request(&params) {
        Ok(r) => r,
        Err(e) => return format_wms_error_response(&e, &params),
    };

    let result = match wms_request.request {
        wms::WmsOperation::GetCapabilities => handle_get_capabilities(&state, &wms_request).await,
        wms::WmsOperation::GetMap => handle_get_map(&state, &wms_request).await,
        wms::WmsOperation::GetFeatureInfo => handle_get_feature_info(&state, &wms_request).await,
        wms::WmsOperation::GetLegendGraphic => handle_get_legend_graphic(&state, &wms_request).await,
        wms::WmsOperation::DescribeLayer => handle_describe_layer(&state, &wms_request).await,
        wms::WmsOperation::GetStyles => handle_get_styles(&state, &wms_request).await,
        _ => Err(GeoServerError::BadRequest("Operation not implemented".to_string())),
    };

    match result {
        Ok(resp) => resp,
        Err(e) => format_wms_error_response(&e, &params),
    }
}

fn format_wms_error_response(err: &GeoServerError, params: &[(String, String)]) -> HttpResponse {
    let exceptions = params.iter()
        .find(|(k, _)| k.to_uppercase() == "EXCEPTIONS")
        .map(|(_, v)| v.as_str());
    let width: u32 = params.iter()
        .find(|(k, _)| k.to_uppercase() == "WIDTH")
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(512);
    let height: u32 = params.iter()
        .find(|(k, _)| k.to_uppercase() == "HEIGHT")
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(512);

    let (body, content_type) = wms::format_wms_exception(err, exceptions, width, height);
    HttpResponse::Ok()
        .content_type(content_type)
        .body(body)
}

async fn handle_get_capabilities(state: &AppState, _request: &WmsRequest) -> Result<HttpResponse, GeoServerError> {
    let base_url = format!("http://{}:{}", state.config.server.host, state.config.server.port);
    let mut capabilities = WmsCapabilities::new(&base_url);

    let layers = state.layers.read().await;
    for layer in layers.iter() {
        capabilities.add_layer(layer);
    }

    let xml = to_string(&capabilities)
        .map_err(|e| GeoServerError::ServiceError(format!("Failed to serialize capabilities: {}", e)))?;

    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
{}"#,
        xml
    );

    Ok(HttpResponse::Ok()
        .content_type("application/xml")
        .body(xml))
}

async fn handle_get_map(state: &AppState, request: &WmsRequest) -> Result<HttpResponse, GeoServerError> {
    info!("[GetMap] 开始处理请求");
    debug!("[GetMap] 请求参数: layers={:?}, format={:?}, bbox={:?}, crs={:?}", 
           request.layers, request.format, request.bbox, request.crs);
    
    wms::validate_wms_get_map_request(request)?;

    let context = parse_get_map_params(request)?;
    info!("[GetMap] 参数解析完成, format={}, bbox={:?}, output_crs={}, size={}x{}", 
          context.format, context.bounds, context.output_crs, context.width, context.height);

    if context.format.to_lowercase().contains("openlayers") {
        info!("[GetMap] 返回 OpenLayers 预览页面");
        return render_openlayers_preview(&context, state);
    }

    info!("[GetMap] 开始查询图层元数据");
    let layer_metadata_list = resolve_layer_metadata(state, &context).await?;

    // 检查是否有级联 WMS 图层 → 直接代理请求
    for meta in &layer_metadata_list {
        if meta.data_source_type == DataSourceType::CascadedWms {
            if let Some(ref conn) = meta.connection {
                if let Some(ref _host) = conn.host {
                    info!("[GetMap] 图层 '{}' 使用级联 WMS，开始代理请求", meta.layer_name);
                    return handle_cascaded_wms_request(state, &context, meta).await;
                }
            }
        }
    }

    info!("[GetMap] 开始查询图层要素");
    let mut layer_contexts = query_all_layer_features(state, &context, &layer_metadata_list).await?;

    info!("[GetMap] 开始解析样式");
    resolve_feature_styles(&mut layer_contexts, context.scale_denominator, &context.env);

    info!("[GetMap] 开始渲染输出");
    render_map_image(&context, &layer_contexts)
}

fn parse_get_map_params(request: &WmsRequest) -> Result<GetMapContext, GeoServerError> {
    let layers_param = request.layers.as_ref()
        .ok_or_else(|| GeoServerError::BadRequest("LAYERS parameter is required".to_string()))?;

    let width = request.width.unwrap_or(512) as u32;
    let height = request.height.unwrap_or(512) as u32;

    let bbox = request.bbox.as_ref()
        .ok_or_else(|| GeoServerError::BadRequest("BBOX parameter is required".to_string()))?;
    let bounds = Bounds::new(bbox.minx, bbox.miny, bbox.maxx, bbox.maxy);

    let output_crs = request.crs.as_deref().unwrap_or("EPSG:4326").to_string();
    let scale_denominator = calculate_scale_denom(&bounds, width, height, &output_crs);

    let format = request.format.as_ref()
        .ok_or_else(|| GeoServerError::BadRequest("FORMAT parameter is required".to_string()))?
        .clone();

    let transparent = request.transparent.unwrap_or(false);
    let bg_color = request.bgcolor.as_ref().map(|c| parse_color(c));

    let options = RenderOptions {
        width,
        height,
        transparent,
        bg_color,
        format: RenderFormat::PNG,
    };

    // 解析 cql_filter：多个图层的过滤条件用 ; 分隔
    let cql_filter = request.cql_filter.clone();

    // 解析 feature_id
    let feature_id = request.feature_id.as_ref().map(|fid| {
        fid.split(',').map(|s| s.trim().to_string()).collect()
    });

    // 解析 env 参数: "key1:'val1';key2:'val2'"
    let env = request.env.as_ref().map(|env_str| {
        let mut map = HashMap::new();
        for pair in env_str.split(';') {
            let parts: Vec<&str> = pair.splitn(2, ':').collect();
            if parts.len() == 2 {
                let key = parts[0].trim().to_string();
                let val = parts[1].trim().trim_matches('\'').to_string();
                map.insert(key, val);
            }
        }
        map
    });

    Ok(GetMapContext {
        layers: layers_param.clone(),
        bounds,
        output_crs,
        width,
        height,
        format,
        transparent,
        bg_color,
        scale_denominator,
        options,
        cql_filter,
        feature_id,
        angle: request.angle,
        time: request.time.clone(),
        elevation: request.elevation.clone(),
        env,
    })
}

async fn resolve_layer_metadata(
    state: &AppState,
    context: &GetMapContext,
) -> Result<Vec<LayerMetadata>, GeoServerError> {
    info!("[resolve_layer_metadata] 开始处理 {} 个图层", context.layers.len());
    let layers_lock = state.layers.read().await;
    let styles_lock = state.styles.read().await;

    let mut metadata_list = Vec::with_capacity(context.layers.len());

    for layer_name in &context.layers {
        let (workspace, layer_short_name) = if layer_name.contains(':') {
            let parts: Vec<&str> = layer_name.splitn(2, ':').collect();
            (parts[0].to_string(), parts[1].to_string())
        } else {
            info!("[resolve_layer_metadata] 图层名 '{}' 无工作空间前缀，使用默认", layer_name);
            (String::new(), layer_name.clone())
        };

        info!("[resolve_layer_metadata] 解析图层: 输入='{}', workspace='{}', layer='{}'", 
              layer_name, workspace, layer_short_name);

        let layer = layers_lock.iter()
            .find(|l| l.name == *layer_name || (l.workspace == workspace && l.name == layer_short_name))
            .ok_or_else(|| GeoServerError::NotFound(format!("Layer '{}' not found", layer_name)))?;

        let data_source = if let Some(store) = &state.store {
            store.get_data_source(&layer.store).await
                .map_err(|e| GeoServerError::InternalError(format!("Failed to get data source: {}", e)))?
        } else {
            None
        };

        let (data_source_type, connection) = data_source
            .map(|ds| (ds.data_source_type, ds.connection))
            .unwrap_or((DataSourceType::Shapefile, None));

        let native_name = if let Some(ref nn) = layer.native_name {
            nn.clone()
        } else {
            layer_short_name.clone()
        };

        let native_crs = layer.srs.to_epsg();

        let sld_xml = request_sld_body(layer, &styles_lock);
        let rules = sld_parser::parse_sld(&sld_xml);

        info!("[resolve_layer_metadata] 图层 '{}' 详情: workspace='{}', layer='{}', native_name='{}', native_crs='{}', data_source_type={:?}, rules_count={}", 
              layer_name, layer.workspace, layer.name, native_name, native_crs, data_source_type, rules.len());

        metadata_list.push(LayerMetadata {
            layer_name: layer_name.clone(),
            workspace: layer.workspace.clone(),
            layer_obj: layer.clone(),
            data_source_type,
            connection,
            native_name,
            native_crs,
            rules,
            max_features: 5000,
        });
    }

    info!("[resolve_layer_metadata] 完成元数据查询，共 {} 个图层", metadata_list.len());
    Ok(metadata_list)
}

fn request_sld_body(layer: &Layer, styles: &HashMap<String, String>) -> String {
    let style_name = layer.styles.first()
        .map(|s| s.name.clone())
        .unwrap_or_default();

    styles.get(&style_name)
        .cloned()
        .unwrap_or_else(|| sld_parser::default_sld(&layer.name))
}

async fn query_all_layer_features(
    state: &AppState,
    context: &GetMapContext,
    metadata_list: &[LayerMetadata],
) -> Result<Vec<LayerRenderContext>, GeoServerError> {
    info!("[query_all_layer_features] 开始查询 {} 个图层的要素，查询范围: {:?}, 坐标系: {}", 
          metadata_list.len(), context.bounds, context.output_crs);
    let mut contexts = Vec::with_capacity(metadata_list.len());

    // 解析 cql_filter: 多个图层的过滤条件用 ; 分隔
    let cql_filters: Vec<&str> = context.cql_filter.as_deref()
        .map(|f| f.split(';').collect())
        .unwrap_or_default();

    for (idx, metadata) in metadata_list.iter().enumerate() {
        info!("[query_all_layer_features] 查询图层: {}", metadata.layer_name);
        let mut features = query_layer_features_optimized(
            state,
            metadata,
            &context.bounds,
            &context.output_crs,
            context.scale_denominator,
        ).await?;

        // 应用 CQL 过滤
        if let Some(cql_str) = cql_filters.get(idx).filter(|s| !s.is_empty()) {
            match crate::utils::cql_filter::parse_cql(cql_str) {
                Ok(expr) => {
                    debug!("[query_all_layer_features] 图层 '{}' 应用 CQL 过滤: '{}'", metadata.layer_name, cql_str);
                    features.retain(|f| crate::utils::cql_filter::evaluate_cql(f, &expr));
                }
                Err(e) => {
                    warn!("[query_all_layer_features] CQL 解析失败 (图层 '{}'): {}", metadata.layer_name, e);
                }
            }
        }

        // 应用 FeatureId 过滤
        if let Some(ref fid_list) = context.feature_id {
            features.retain(|f| fid_list.contains(&f.id));
        }

        // 应用时间过滤 (TIME)
        if let Some(ref time_str) = context.time {
            filter_by_time(&mut features, time_str);
        }

        // 应用高程过滤 (ELEVATION)
        if let Some(ref elev_str) = context.elevation {
            filter_by_elevation(&mut features, elev_str);
        }

        debug!("[query_all_layer_features] 图层 '{}' 过滤后剩余 {} 个要素", 
               metadata.layer_name, features.len());

        contexts.push(LayerRenderContext {
            metadata: LayerMetadata {
                layer_name: metadata.layer_name.clone(),
                workspace: metadata.workspace.clone(),
                layer_obj: metadata.layer_obj.clone(),
                data_source_type: metadata.data_source_type.clone(),
                connection: metadata.connection.clone(),
                native_name: metadata.native_name.clone(),
                native_crs: metadata.native_crs.clone(),
                rules: metadata.rules.clone(),
                max_features: metadata.max_features,
            },
            features,
            render_items: Vec::new(),
        });
    }

    info!("[query_all_layer_features] 要素查询完成");
    Ok(contexts)
}

async fn query_layer_features_optimized(
    state: &AppState,
    metadata: &LayerMetadata,
    bbox: &Bounds,
    output_crs: &str,
    scale_denominator: f64,
) -> Result<Vec<Feature>, GeoServerError> {
    info!("[query_layer_features_optimized] 查询图层 '{}', native_name='{}', native_crs='{}', data_source_type='{:?}'",
          metadata.layer_name, metadata.native_name, metadata.native_crs, metadata.data_source_type);

    let features = match metadata.data_source_type {
        DataSourceType::Postgis => {
            if let Some(ref conn) = metadata.connection {
                query_postgis_features_optimized(
                    state,
                    conn,
                    &metadata.native_name,
                    &metadata.native_crs,
                    output_crs,
                    bbox,
                    scale_denominator,
                    metadata.max_features,
                ).await
            } else {
                debug!("[query_layer_features_optimized] 无PostGIS连接配置，使用默认查询");
                Ok(Vec::new())
            }
        }
        _ => {
            debug!("[query_layer_features_optimized] 使用默认查询方式");
            let features = crate::handlers::features::query_layer_features(
                state,
                &metadata.layer_name,
                Some(bbox),
                None,
                None,
            ).await.unwrap_or_default();

            let needs_reproject = metadata.native_crs != output_crs;
            if needs_reproject {
                debug!("[query_layer_features_optimized] 需要坐标转换: {} -> {}", metadata.native_crs, output_crs);
                Ok(features.into_iter()
                    .map(|mut f| {
                        f.geometry = reproject_geometry(&f.geometry, &metadata.native_crs, output_crs);
                        f
                    })
                    .collect())
            } else {
                Ok(features)
            }
        }
    };

    info!("[query_layer_features_optimized] 图层 '{}' 查询结果: {} 个要素", metadata.layer_name, features.as_ref().map(|f| f.len()).unwrap_or(0));
    features
}

async fn query_postgis_features_optimized(
    state: &AppState,
    conn: &crate::models::DataSourceConnection,
    table_name: &str,
    storage_crs: &str,
    output_crs: &str,
    bbox: &Bounds,
    scale_denominator: f64,
    max_features: u32,
) -> Result<Vec<Feature>, GeoServerError> {
    let schema_name = conn.schema.as_deref()
        .map(|s| if s.is_empty() || s == "public" { "public" } else { s })
        .unwrap_or("public")
        .to_string();

    info!("[PostGIS] 开始查询 PostGIS, table='{}', schema='{}', storage_crs='{}', output_crs='{}'",
          table_name, schema_name, storage_crs, output_crs);

    let pool = state.get_pg_pool(table_name, conn);
    let client = pool.get().await
        .map_err(|e| GeoServerError::InternalError(format!("Pool error: {}", e)))?;

    let schema = schema_name;

    let geom_col = get_geometry_column(&client, &schema, table_name).await
        .unwrap_or_else(|| "geom".to_string());

    let needs_transform = storage_crs != output_crs;
    let storage_srid = parse_srid(storage_crs);
    let output_srid = parse_srid(output_crs);

    let simplify_tolerance = calculate_simplify_tolerance(scale_denominator, bbox);

    debug!("[PostGIS] 查询参数: needs_transform={}, storage_srid={}, output_srid={}, simplify_tolerance={:?}",
           needs_transform, storage_srid, output_srid, simplify_tolerance);

    let geom_expr = if needs_transform {
        if let Some(tol) = simplify_tolerance {
            format!("ST_AsBinary(ST_SimplifyPreserveTopology(ST_Transform({}, {}), {}))",
                geom_col, output_srid, tol)
        } else {
            format!("ST_AsBinary(ST_Transform({}, {}))", geom_col, output_srid)
        }
    } else {
        if let Some(tol) = simplify_tolerance {
            format!("ST_AsBinary(ST_SimplifyPreserveTopology({}, {}))", geom_col, tol)
        } else {
            format!("ST_AsBinary({})", geom_col)
        }
    };

    let sql = format!(
        "SELECT {} as geom_wkb, {} && ST_MakeEnvelope({}, {}, {}, {}, {}) as _in_bbox \
         FROM \"{}\".\"{}\" \
         WHERE {} && ST_MakeEnvelope({}, {}, {}, {}, {}) \
         LIMIT {}",
        geom_expr,
        geom_col, bbox.minx, bbox.miny, bbox.maxx, bbox.maxy, storage_srid,
        schema, table_name,
        geom_col, bbox.minx, bbox.miny, bbox.maxx, bbox.maxy, storage_srid,
        max_features
    );

    debug!("[PostGIS] 执行SQL: {}", sql);

    let rows = client.query(&sql, &[]).await
        .map_err(|e| GeoServerError::InternalError(format!("PostGIS query error: {}", e)))?;

    info!("[PostGIS] SQL执行完成, 返回 {} 行", rows.len());

    let mut features = Vec::with_capacity(rows.len());
    for (idx, row) in rows.iter().enumerate() {
        let wkb_data: Vec<u8> = row.try_get("geom_wkb").unwrap_or_default();
        let geometry = wkb::parse_wkb_geometry(&wkb_data);
        let mut properties = HashMap::new();
        properties.insert("id".to_string(), crate::models::PropertyValue::String(format!("feat_{}", idx)));
        features.push(Feature::with_id(format!("feat_{}", idx), geometry, properties));
    }

    info!("[PostGIS] 解析完成, 共 {} 个要素", features.len());
    Ok(features)
}

fn parse_srid(crs: &str) -> i32 {
    crs.trim_start_matches("EPSG:")
        .trim_start_matches("epsg:")
        .parse()
        .unwrap_or(4326)
}

fn calculate_simplify_tolerance(scale_denom: f64, bbox: &Bounds) -> Option<f64> {
    if scale_denom > 100000.0 {
        let range = (bbox.maxx - bbox.minx).max(bbox.maxy - bbox.miny);
        Some(range / 1000.0)
    } else {
        None
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

fn resolve_feature_styles(
    layer_contexts: &mut [LayerRenderContext],
    scale_denominator: f64,
    env: &Option<HashMap<String, String>>,
) {
    info!("[resolve_feature_styles] 开始解析样式，比例尺分母: {}", scale_denominator);
    let mut total_items = 0;

    for layer_ctx in layer_contexts.iter_mut() {
        debug!("[resolve_feature_styles] 处理图层: {}, 要素数: {}", 
               layer_ctx.metadata.layer_name, layer_ctx.features.len());
        
        for feature in &layer_ctx.features {
            let style = if let Some(env_map) = env {
                if !env_map.is_empty() {
                    sld_parser::resolve_style_with_env(
                        &layer_ctx.metadata.rules, feature, Some(scale_denominator), env_map
                    )
                } else {
                    layer_ctx.metadata.rules.iter()
                        .find(|rule| sld_parser::match_rule(rule, &feature.properties, Some(scale_denominator)))
                        .map(|rule| rule.style.clone())
                        .unwrap_or_default()
                }
            } else {
                layer_ctx.metadata.rules.iter()
                    .find(|rule| sld_parser::match_rule(rule, &feature.properties, Some(scale_denominator)))
                    .map(|rule| rule.style.clone())
                    .unwrap_or_default()
            };

            layer_ctx.render_items.push((feature.geometry.clone(), style));
            total_items += 1;
        }
    }

    info!("[resolve_feature_styles] 样式解析完成，共 {} 个渲染项", total_items);
}

fn render_map_image(
    context: &GetMapContext,
    layer_contexts: &[LayerRenderContext],
) -> Result<HttpResponse, GeoServerError> {
    info!("[render_map_image] 开始渲染地图，尺寸: {}x{}, 透明背景: {}", 
          context.width, context.height, context.transparent);
    
    let format_lower = context.format.to_lowercase();

    let all_render_items: Vec<(GeoJsonGeometry, Style)> = layer_contexts.iter()
        .flat_map(|ctx| ctx.render_items.clone())
        .collect();

    info!("[render_map_image] 共 {} 个渲染项待渲染", all_render_items.len());

    // 非图片格式: SVG
    if format_lower.contains("svg") {
        let svg = crate::utils::rendering::render_to_svg(
            &all_render_items, &context.bounds, context.width, context.height
        );
        return Ok(HttpResponse::Ok()
            .content_type("image/svg+xml")
            .body(svg));
    }

    // 非图片格式: KML
    if format_lower.contains("kml") {
        let layer_name = context.layers.first().map(|s| s.as_str()).unwrap_or("map");
        let kml = crate::utils::rendering::render_to_kml(&all_render_items, layer_name);
        return Ok(HttpResponse::Ok()
            .content_type("application/vnd.google-earth.kml+xml")
            .body(kml));
    }

    // 非图片格式: GeoJSON (输出要素 GeoJSON)
    if format_lower.contains("json") || format_lower.contains("geojson") {
        let mut features: Vec<serde_json::Value> = Vec::new();
        for ctx in layer_contexts {
            for (geom, _style) in &ctx.render_items {
                features.push(serde_json::json!({
                    "type": "Feature",
                    "geometry": geom,
                    "properties": {},
                }));
            }
        }
        let geojson = serde_json::json!({
            "type": "FeatureCollection",
            "features": features,
        });
        return Ok(HttpResponse::Ok()
            .content_type("application/geo+json")
            .json(geojson));
    }

    // 图片格式: 使用 MapRenderer 渲染
    let renderer = MapRenderer::new(context.options.clone(), context.bounds.clone());
    let img = renderer.render(all_render_items);

    let image_format = match format_lower.as_str() {
        s if s.contains("png") => ImageFormat::Png,
        s if s.contains("jpeg") || s.contains("jpg") => ImageFormat::Jpeg,
        s if s.contains("gif") => ImageFormat::Gif,
        s if s.contains("webp") => ImageFormat::WebP,
        _ => ImageFormat::Png,
    };

    debug!("[render_map_image] 渲染格式: {:?}", image_format);

    let mut buffer = Cursor::new(Vec::new());
    img.write_to(&mut buffer, image_format)
        .map_err(|e| GeoServerError::RenderingError(format!("Failed to render image: {}", e)))?;

    let image_size = buffer.get_ref().len();
    let content_type = match image_format {
        ImageFormat::Png => "image/png",
        ImageFormat::Jpeg => "image/jpeg",
        ImageFormat::Gif => "image/gif",
        ImageFormat::WebP => "image/webp",
        _ => "image/png",
    };

    info!("[render_map_image] 渲染完成，返回图片，大小: {} 字节, Content-Type: {}", 
          image_size, content_type);

    Ok(HttpResponse::Ok()
        .content_type(content_type)
        .body(buffer.into_inner()))
}

fn render_openlayers_preview(
    context: &GetMapContext,
    _state: &AppState,
) -> Result<HttpResponse, GeoServerError> {
    // 使用相对 URL，通过前端代理或同源访问 WMS
    let wms_url = "/wms?";

    // 如果输出 CRS 不是 Web Mercator，用 EPSG:4326 作为备用
    let view_crs = if context.output_crs.to_uppercase().contains("3857")
        || context.output_crs.to_uppercase().contains("900913")
    {
        context.output_crs.clone()
    } else {
        // 默认使用 EPSG:4326，但 TileWMS 在 4326 下格子显示不正常
        // 改用 ImageWMS 方式，通过单次请求渲染
        "EPSG:4326".to_string()
    };

    let center_x = (context.bounds.minx + context.bounds.maxx) / 2.0;
    let center_y = (context.bounds.miny + context.bounds.maxy) / 2.0;
    let zoom = calculate_openlayers_zoom(&context.bounds, &view_crs);
    let layers_json = serde_json::to_string(&context.layers).unwrap_or_default();
    let extent = format!("{}, {}, {}, {}",
        context.bounds.minx, context.bounds.miny,
        context.bounds.maxx, context.bounds.maxy);

    let html = format!(
        r#"<!DOCTYPE html>
<html>
<head>
  <title>Layer Preview - {layer_name}</title>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/ol@v10.4.0/ol.css" type="text/css">
  <script src="https://cdn.jsdelivr.net/npm/ol@v10.4.0/dist/ol.min.js"></script>
  <style>
    html, body, #map {{ height: 100%; width: 100%; margin: 0; padding: 0; }}
    .ol-zoom {{ top: 1em; left: 1em; }}
    .layer-info {{ position: absolute; bottom: 1em; left: 1em; background: rgba(255,255,255,0.85); padding: 6px 12px; border-radius: 4px; font: 13px sans-serif; }}
    .map-hint {{
      position: absolute; top: 14px; left: 50%; transform: translateX(-50%);
      background: rgba(255,255,255,0.92); border: 1px solid #e0e0e8;
      padding: 6px 14px; border-radius: 20px; font: 12px sans-serif; color: #666680;
      box-shadow: 0 2px 8px rgba(0,0,0,0.08); pointer-events: none; z-index: 5;
      transition: opacity 0.25s ease;
    }}
    .map-hint.hidden {{ opacity: 0; }}
    #feature-info {{
      position: absolute; top: 0; right: 0; bottom: 0; width: 340px; max-width: 60%;
      background: #fff; border-left: 1px solid #ddd; box-shadow: -2px 0 12px rgba(0,0,0,0.15);
      display: none; flex-direction: column; z-index: 10;
      font: 13px/1.5 sans-serif;
    }}
    #feature-info.open {{ display: flex; }}
    #feature-info .popup-header {{
      display: flex; align-items: center; justify-content: space-between;
      padding: 10px 14px; border-bottom: 1px solid #eee; font-weight: 600;
    }}
    #feature-info .popup-close {{
      cursor: pointer; border: none; background: none; font-size: 18px; color: #666;
      line-height: 1; padding: 2px 6px; border-radius: 4px;
    }}
    #feature-info .popup-close:hover {{ background: #f0f0f0; }}
    #feature-info-content {{ overflow-y: auto; padding: 12px 14px; }}
    .feature-group {{ margin-bottom: 14px; }}
    .feature-header {{
      font-weight: 600; color: #1565c0; margin-bottom: 4px;
    }}
    .feature-header .fid {{ color: #888; font-weight: 400; font-size: 12px; }}
    .feature-table {{ width: 100%; border-collapse: collapse; }}
    .feature-table td {{
      padding: 4px 8px; border: 1px solid #e6e6e6; font-size: 12px; vertical-align: top;
      word-break: break-all;
    }}
    .feature-table td.pkey {{ background: #f8f9fb; color: #555; width: 38%; font-weight: 500; }}
    .feature-table td.pval {{ color: #222; }}
    .popup-empty {{ color: #999; padding: 8px 0; }}
    #feature-info .loading {{ color: #999; }}
  </style>
</head>
<body>
  <div id="map"></div>
  <div class="map-hint" id="map-hint">点击地图要素可查看属性</div>
  <div class="layer-info">图层: {layer_name}</div>
  <div id="feature-info">
    <div class="popup-header">
      <span>要素属性</span>
      <button class="popup-close" onclick="closeFeatureInfo()">&times;</button>
    </div>
    <div id="feature-info-content"></div>
  </div>
  <script>
    var layers = {layers_json};
    var extent = ol.proj.transformExtent([{extent}], 'EPSG:4326', '{view_crs}');

    var wmsSource = new ol.source.ImageWMS({{
      url: '{wms_url}',
      params: {{
        'LAYERS': layers.join(','),
        'VERSION': '1.1.1',
        'FORMAT': 'image/png',
        'TRANSPARENT': true
      }},
      ratio: 1,
      serverType: 'geoserver'
    }});

    var map = new ol.Map({{
      target: 'map',
      layers: [
        new ol.layer.Image({{
          source: wmsSource,
          extent: extent
        }})
      ],
      view: new ol.View({{
        projection: '{view_crs}',
        center: ol.proj.fromLonLat([{center_x}, {center_y}], '{view_crs}'),
        zoom: {zoom},
        extent: extent
      }})
    }});

    // ============ 矢量要素层（用于点击查询与要素显示） ============
    var vectorSource = new ol.source.Vector();
    var vectorLayer = new ol.layer.Vector({{
      source: vectorSource,
      style: new ol.style.Style({{
        image: new ol.style.Circle({{
          radius: 5,
          fill: new ol.style.Fill({{ color: 'rgba(21,101,192,0.75)' }}),
          stroke: new ol.style.Stroke({{ color: '#ffffff', width: 1.5 }})
        }}),
        fill: new ol.style.Fill({{ color: 'rgba(21,101,192,0.15)' }}),
        stroke: new ol.style.Stroke({{ color: 'rgba(21,101,192,0.85)', width: 1.5 }})
      }})
    }});
    map.addLayer(vectorLayer);

    var featureFormat = new ol.format.GeoJSON();
    var featureProjection = map.getView().getProjection().getCode();

    function loadLayerFeatures(layerName) {{
      var xhr = new XMLHttpRequest();
      xhr.open('GET', '/geoserver/layers/' + encodeURIComponent(layerName) + '/features?limit=5000', true);
      xhr.onreadystatechange = function() {{
        if (xhr.readyState === 4 && xhr.status === 200) {{
          try {{
            var fc = JSON.parse(xhr.responseText);
            var feats = fc.features || [];
            for (var i = 0; i < feats.length; i++) {{
              try {{
                var olFeature = featureFormat.readFeature({{
                  type: 'Feature',
                  geometry: feats[i].geometry,
                  properties: {{}}
                }}, {{ dataProjection: 'EPSG:4326', featureProjection: featureProjection }});
                olFeature.set('_layer', layerName);
                olFeature.set('_fid', feats[i].id);
                olFeature.set('_props', feats[i].properties || {{}});
                vectorSource.addFeature(olFeature);
              }} catch (e2) {{}}
            }}
          }} catch (e) {{}}
        }}
      }};
      xhr.send();
    }}

    layers.forEach(function(layerName) {{
      loadLayerFeatures(layerName);
    }});

    // 视图变化时更新 WMS 请求
    map.getView().on('change:resolution', function() {{
      wmsSource.updateParams({{
        'TIME': new Date().getTime().toString()
      }});
    }});

    // ============ 点击要素查询属性 ============
    var popup = document.getElementById('feature-info');
    var popupContent = document.getElementById('feature-info-content');
    var loadingFeature = false;

    function escapeHtml(str) {{
      if (str === null || str === undefined) return '';
      var div = document.createElement('div');
      div.textContent = String(str);
      return div.innerHTML;
    }}

    function openFeatureInfo(html) {{
      popupContent.innerHTML = html;
      popup.classList.add('open');
    }}

    function closeFeatureInfo() {{
      popup.classList.remove('open');
      popupContent.innerHTML = '';
    }}

    function renderFeatureInfo(data) {{
      if (!data || data.length === 0) {{
        closeFeatureInfo();
        return;
      }}
      var html = '';
      for (var i = 0; i < data.length; i++) {{
        var item = data[i];
        html += '<div class="feature-group">';
        html += '<div class="feature-header">' + escapeHtml(item.layer)
              + (item.feature_id ? ' <span class="fid">#' + escapeHtml(item.feature_id) + '</span>' : '')
              + '</div>';
        html += '<table class="feature-table"><tbody>';
        var props = item.properties || {{}};
        var keys = Object.keys(props);
        if (keys.length === 0) {{
          html += '<tr><td class="popup-empty" colspan="2">无属性</td></tr>';
        }}
        for (var k = 0; k < keys.length; k++) {{
          html += '<tr><td class="pkey">' + escapeHtml(keys[k]) + '</td>'
                + '<td class="pval">' + escapeHtml(props[keys[k]]) + '</td></tr>';
        }}
        html += '</tbody></table></div>';
      }}
      openFeatureInfo(html);
    }}

    map.on('singleclick', function(evt) {{
      if (loadingFeature) return;
      // 点击后隐藏"点击查看属性"提示
      var hintEl = document.getElementById('map-hint');
      if (hintEl) {{ hintEl.classList.add('hidden'); }}
      // 1. 优先做矢量要素像素命中检测（容差 8px，缩放较小时也能轻松点中）
      var hitItems = [];
      map.forEachFeatureAtPixel(evt.pixel, function(feature) {{
        hitItems.push({{
          layer: feature.get('_layer'),
          feature_id: feature.get('_fid'),
          properties: feature.get('_props') || {{}}
        }});
        return hitItems.length >= 10;
      }}, {{ hitTolerance: 8 }});

      if (hitItems.length > 0) {{
        renderFeatureInfo(hitItems);
        return;
      }}

      // 2. 矢量要素未能加载时，回退到 WMS GetFeatureInfo
      if (vectorSource.getFeatures().length === 0) {{
        var view = map.getView();
        var resolution = view.getResolution();
        if (!resolution) return;
        var projection = view.getProjection();
        var url = wmsSource.getFeatureInfoUrl(
          evt.coordinate, resolution, projection,
          {{
            'INFO_FORMAT': 'application/json',
            'QUERY_LAYERS': layers.join(','),
            'FEATURE_COUNT': '10'
          }}
        );
        if (!url) return;
        loadingFeature = true;
        popupContent.innerHTML = '<p class="loading">正在查询要素...</p>';
        popup.classList.add('open');
        var xhr = new XMLHttpRequest();
        xhr.open('GET', url, true);
        xhr.onreadystatechange = function() {{
          if (xhr.readyState === 4) {{
            loadingFeature = false;
            if (xhr.status === 200) {{
              try {{
                var data = JSON.parse(xhr.responseText);
                renderFeatureInfo(data);
              }} catch (e) {{
                openFeatureInfo('<p class="popup-empty">解析属性数据失败</p>');
              }}
            }} else {{
              openFeatureInfo('<p class="popup-empty">查询要素属性失败 (HTTP ' + xhr.status + ')</p>');
            }}
          }}
        }};
        xhr.send();
        return;
      }}

      closeFeatureInfo();
    }});
  </script>
</body>
</html>"#,
        layer_name = context.layers.join(","),
        layers_json = layers_json,
        wms_url = wms_url,
        view_crs = view_crs,
        center_x = center_x,
        center_y = center_y,
        zoom = zoom,
        extent = extent,
    );

    Ok(HttpResponse::Ok()
        .content_type("text/html")
        .body(html))
}

async fn handle_get_feature_info(state: &AppState, request: &WmsRequest) -> Result<HttpResponse, GeoServerError> {
    let i = request.i.unwrap_or(0.0);
    let j = request.j.unwrap_or(0.0);
    let width = request.width.unwrap_or(512) as f64;
    let height = request.height.unwrap_or(512) as f64;
    let feature_count = request.feature_count.unwrap_or(10) as usize;

    let info_format = request.info_format.as_deref().unwrap_or("text/plain");

    let click_point = request.bbox.as_ref().map(|bbox| {
        let wx = bbox.minx + (i / width) * (bbox.maxx - bbox.minx);
        let wy = bbox.maxy - (j / height) * (bbox.maxy - bbox.miny);
        (wx, wy)
    });

    // 将点击点与视图范围统一转换到 EPSG:4326，与要素坐标系保持一致
    let request_crs = request.crs.as_deref().unwrap_or("EPSG:4326");
    let transformer = ProjectionTransformer::new(
        CoordinateReferenceSystem::from_epsg(request_crs),
        CoordinateReferenceSystem::EPSG4326,
    );

    let click_point = click_point.map(|(cx, cy)| {
        transformer.transform_point(cx, cy).unwrap_or((cx, cy))
    });

    let view_bounds = bbox_to_bounds(request);
    let view_bounds = if transformer.needs_reprojection() {
        transformer.transform_bounds(
            view_bounds.minx, view_bounds.miny, view_bounds.maxx, view_bounds.maxy,
        )
        .map(|(a, b, c, d)| Bounds::new(a, b, c, d))
        .unwrap_or(view_bounds)
    } else {
        view_bounds
    };

    let range = (view_bounds.maxx - view_bounds.minx).max(view_bounds.maxy - view_bounds.miny);
    let tolerance = (range / 200.0).max(0.0001);

    let mut found_features: Vec<(String, String, HashMap<String, String>)> = Vec::new();

    if let Some(query_layers) = &request.query_layers {
        for layer_name in query_layers {
            // 以点击点为中心的小范围 bbox 查询，再精确定位命中的要素
            let query_bbox = click_point.map(|(cx, cy)| {
                Bounds::new(cx - tolerance, cy - tolerance, cx + tolerance, cy + tolerance)
            });

            let features = match crate::handlers::features::query_layer_features(
                state,
                layer_name,
                query_bbox.as_ref(),
                Some(feature_count as u64 * 2),
                None,
            ).await {
                Ok(f) => f,
                Err(_) => Vec::new(),
            };

            for feature in &features {
                let hit = if let Some((cx, cy)) = click_point {
                    feature_hit_test(&feature.geometry, cx, cy, &view_bounds)
                } else {
                    true
                };
                if hit {
                    let mut props = HashMap::new();
                    for (k, v) in &feature.properties {
                        props.insert(k.clone(), v.to_string());
                    }
                    found_features.push((layer_name.clone(), feature.id.clone(), props));
                    if found_features.len() >= feature_count {
                        break;
                    }
                }
            }
        }
    }

    let response = match info_format {
        "application/json" => {
            let json_features: Vec<serde_json::Value> = found_features.iter().map(|(layer, fid, props)| {
                serde_json::json!({
                    "layer": layer,
                    "feature_id": fid,
                    "properties": props,
                })
            }).collect();
            serde_json::to_string_pretty(&json_features)
                .map_err(|e| GeoServerError::ServiceError(e.to_string()))?
        }
        "text/html" => {
            let rows: String = found_features.iter().map(|(layer, fid, props)| {
                let prop_rows: String = props.iter()
                    .map(|(k, v)| format!("<tr><td>{}</td><td>{}</td></tr>", k, v))
                    .collect();
                format!("<h3>Layer: {} (ID: {})</h3><table border='1'>{}</table>", layer, fid, prop_rows)
            }).collect();
            format!("<html><body><h1>Feature Information</h1>{}</body></html>", rows)
        }
        _ => {
            found_features.iter().map(|(layer, fid, props)| {
                let prop_str: String = props.iter()
                    .map(|(k, v)| format!("  {} = {}", k, v))
                    .collect::<Vec<_>>()
                    .join("\n");
                format!("Layer: {}\nFeature ID: {}\n{}\n", layer, fid, prop_str)
            }).collect::<Vec<_>>().join("---\n")
        }
    };

    let content_type = match info_format {
        "application/json" => "application/json",
        "text/html" => "text/html",
        _ => "text/plain",
    };

    Ok(HttpResponse::Ok()
        .content_type(content_type)
        .body(response))
}

fn bbox_to_bounds(request: &WmsRequest) -> Bounds {
    request.bbox.as_ref().map(|b| Bounds::new(b.minx, b.miny, b.maxx, b.maxy))
        .unwrap_or_default()
}

fn feature_hit_test(geom: &GeoJsonGeometry, cx: f64, cy: f64, bounds: &Bounds) -> bool {
    const TOLERANCE: f64 = 0.05;
    let range = (bounds.maxx - bounds.minx).max(bounds.maxy - bounds.miny);
    let tolerance = (range / 200.0).max(TOLERANCE);

    match geom {
        GeoJsonGeometry::Point { coordinates } => {
            coordinates.len() >= 2
                && point_distance(coordinates[0], coordinates[1], cx, cy) <= tolerance
        }
        GeoJsonGeometry::MultiPoint { coordinates } => {
            coordinates.iter().any(|c| {
                c.len() >= 2 && point_distance(c[0], c[1], cx, cy) <= tolerance
            })
        }
        GeoJsonGeometry::LineString { coordinates } => {
            coordinates.windows(2).any(|seg| {
                if seg.len() < 2 || seg[0].len() < 2 || seg[1].len() < 2 {
                    return false;
                }
                point_to_segment_distance(cx, cy, seg[0][0], seg[0][1], seg[1][0], seg[1][1]) <= tolerance
            })
        }
        GeoJsonGeometry::MultiLineString { coordinates } => {
            coordinates.iter().any(|line| {
                line.windows(2).any(|seg| {
                    if seg.len() < 2 || seg[0].len() < 2 || seg[1].len() < 2 {
                        return false;
                    }
                    point_to_segment_distance(cx, cy, seg[0][0], seg[0][1], seg[1][0], seg[1][1]) <= tolerance
                })
            })
        }
        GeoJsonGeometry::Polygon { coordinates } => {
            coordinates.first().map(|ring| point_in_ring(cx, cy, ring)).unwrap_or(false)
        }
        GeoJsonGeometry::MultiPolygon { coordinates } => {
            coordinates.iter().any(|poly| {
                poly.first().map(|ring| point_in_ring(cx, cy, ring)).unwrap_or(false)
            })
        }
        GeoJsonGeometry::GeometryCollection { geometries } => {
            geometries.iter().any(|g| feature_hit_test(g, cx, cy, bounds))
        }
    }
}

fn point_distance(x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    let dx = x1 - x2;
    let dy = y1 - y2;
    (dx * dx + dy * dy).sqrt()
}

fn point_to_segment_distance(px: f64, py: f64, ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    let abx = bx - ax;
    let aby = by - ay;
    let apx = px - ax;
    let apy = py - ay;
    let ab2 = abx * abx + aby * aby;
    if ab2 == 0.0 {
        return ((px - ax).powi(2) + (py - ay).powi(2)).sqrt();
    }
    let t = ((apx * abx + apy * aby) / ab2).clamp(0.0, 1.0);
    let projx = ax + t * abx;
    let projy = ay + t * aby;
    ((px - projx).powi(2) + (py - projy).powi(2)).sqrt()
}

fn point_in_ring(px: f64, py: f64, ring: &[Vec<f64>]) -> bool {
    let mut inside = false;
    let mut j = ring.len() - 1;
    for i in 0..ring.len() {
        if ring[i].len() < 2 || ring[j].len() < 2 {
            j = i;
            continue;
        }
        let (xi, yi) = (ring[i][0], ring[i][1]);
        let (xj, yj) = (ring[j][0], ring[j][1]);
        if ((yi > py) != (yj > py)) && (px < (xj - xi) * (py - yi) / (yj - yi) + xi) {
            inside = !inside;
        }
        j = i;
    }
    inside
}

async fn handle_describe_layer(state: &AppState, request: &WmsRequest) -> Result<HttpResponse, GeoServerError> {
    let layers_param = request.layers.as_ref()
        .ok_or_else(|| GeoServerError::BadRequest("LAYERS parameter required".to_string()))?;

    let layers_lock = state.layers.read().await;
    let mut layer_descriptions = Vec::new();

    for layer_name in layers_param {
        if let Some(layer) = layers_lock.iter().find(|l| l.name == *layer_name) {
            layer_descriptions.push(serde_json::json!({
                "name": layer.name,
                "title": layer.title,
                "crs": layer.srs.to_epsg(),
                "native_bounds": {
                    "crs": layer.native_bounds.crs.to_epsg(),
                    "minx": layer.native_bounds.bounds.minx,
                    "miny": layer.native_bounds.bounds.miny,
                    "maxx": layer.native_bounds.bounds.maxx,
                    "maxy": layer.native_bounds.bounds.maxy,
                },
                "lat_lon_bounds": {
                    "crs": layer.lat_lon_bounds.crs.to_epsg(),
                    "minx": layer.lat_lon_bounds.bounds.minx,
                    "miny": layer.lat_lon_bounds.bounds.miny,
                    "maxx": layer.lat_lon_bounds.bounds.maxx,
                    "maxy": layer.lat_lon_bounds.bounds.maxy,
                },
                "styles": layer.styles.iter().map(|s| {
                    serde_json::json!({ "name": s.name })
                }).collect::<Vec<_>>(),
            }));
        }
    }

    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<DescribeLayerResponse version="1.1.1" xmlns="http://www.opengis.net/wms">
{}
</DescribeLayerResponse>"#,
        layer_descriptions.iter().map(|desc| {
            format!(
                r#"  <LayerDescription name="{name}" crs="{crs}">
    <Bounds CRS="{crs}" minx="{minx}" miny="{miny}" maxx="{maxx}" maxy="{maxy}"/>
  </LayerDescription>"#,
                name = desc["name"].as_str().unwrap_or(""),
                crs = desc["crs"].as_str().unwrap_or(""),
                minx = desc["native_bounds"]["minx"].as_f64().unwrap_or(0.0),
                miny = desc["native_bounds"]["miny"].as_f64().unwrap_or(0.0),
                maxx = desc["native_bounds"]["maxx"].as_f64().unwrap_or(0.0),
                maxy = desc["native_bounds"]["maxy"].as_f64().unwrap_or(0.0),
            )
        }).collect::<Vec<_>>().join("\n")
    );

    Ok(HttpResponse::Ok()
        .content_type("text/xml")
        .body(xml))
}

async fn handle_get_styles(state: &AppState, request: &WmsRequest) -> Result<HttpResponse, GeoServerError> {
    let layers_param = request.layers.as_ref()
        .ok_or_else(|| GeoServerError::BadRequest("LAYERS parameter required for GetStyles".to_string()))?;

    let styles_lock = state.styles.read().await;
    let layers_lock = state.layers.read().await;

    let mut sld_doc = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<StyledLayerDescriptor version="1.0.0"
  xmlns="http://www.opengis.net/sld"
  xmlns:ogc="http://www.opengis.net/ogc"
  xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
"#
    );

    for layer_name in layers_param {
        let style_name = layers_lock.iter()
            .find(|l| l.name == *layer_name)
            .and_then(|l| l.styles.first().map(|s| s.name.clone()))
            .unwrap_or_else(|| "default".to_string());

        let style_content = styles_lock.get(&style_name)
            .cloned()
            .unwrap_or_else(|| String::new());

        sld_doc.push_str(&format!(
            r#"  <NamedLayer>
    <Name>{}</Name>
    <UserStyle>
      <Name>{}</Name>
      {}
    </UserStyle>
  </NamedLayer>
"#,
            layer_name,
            style_name,
            if style_content.is_empty() {
                "<FeatureTypeStyle><Rule><PolygonSymbolizer><Fill><CssParameter name=\"fill\">#808080</CssParameter></Fill></PolygonSymbolizer></Rule></FeatureTypeStyle>".to_string()
            } else {
                style_content
            }
        ));
    }

    sld_doc.push_str("</StyledLayerDescriptor>");

    Ok(HttpResponse::Ok()
        .content_type("application/vnd.ogc.sld+xml")
        .body(sld_doc))
}

async fn handle_get_legend_graphic(state: &AppState, request: &WmsRequest) -> Result<HttpResponse, GeoServerError> {
    let layer_name = request.layers.as_ref()
        .and_then(|l| l.first())
        .ok_or_else(|| GeoServerError::BadRequest("LAYER parameter required for GetLegendGraphic".to_string()))?;

    let layers_lock = state.layers.read().await;
    let styles_lock = state.styles.read().await;

    let layer = layers_lock.iter().find(|l| l.name == *layer_name)
        .ok_or_else(|| GeoServerError::NotFound(format!("Layer '{}' not found", layer_name)))?;

    let rules = get_layer_rules(request, &styles_lock, layer);
    let padding = 5u32;
    let icon_size = 20u32;
    let row_height = icon_size + 4;
    let total_height = if rules.is_empty() { row_height } else { (rules.len() as u32) * row_height + padding * 2 };
    let total_width = 40u32;

    let mut img = image::RgbaImage::new(total_width, total_height);
    for pixel in img.pixels_mut() {
        *pixel = image::Rgba([255, 255, 255, 255]);
    }

    for (idx, rule) in rules.iter().enumerate() {
        let y = padding + idx as u32 * row_height;
        let style = &rule.style;

        let swatch_x = (total_width - icon_size) / 2;
        let swatch_y = y + 2;

        if let Some(fill) = &style.fill {
            if let Some(color) = parse_color_opt(&fill.color) {
                for dy in 0..icon_size {
                    for dx in 0..icon_size {
                        let px = swatch_x + dx;
                        let py = swatch_y + dy;
                        if px < total_width && py < total_height {
                            img.put_pixel(px, py, image::Rgba(color));
                        }
                    }
                }
            }
        }
        if let Some(stroke) = &style.stroke {
            if let Some(color) = parse_color_opt(&stroke.color) {
                for dx in 0..icon_size {
                    let px = swatch_x + dx;
                    for py in [swatch_y, swatch_y + icon_size - 1] {
                        if px < total_width && py < total_height {
                            img.put_pixel(px, py, image::Rgba(color));
                        }
                    }
                }
                for dy in 0..icon_size {
                    let py = swatch_y + dy;
                    for px in [swatch_x, swatch_x + icon_size - 1] {
                        if px < total_width && py < total_height {
                            img.put_pixel(px, py, image::Rgba(color));
                        }
                    }
                }
            }
        }
    }

    let mut buffer = Cursor::new(Vec::new());
    img.write_to(&mut buffer, ImageFormat::Png)
        .map_err(|e| GeoServerError::RenderingError(e.to_string()))?;

    Ok(HttpResponse::Ok()
        .content_type("image/png")
        .body(buffer.into_inner()))
}

fn get_layer_rules(
    request: &WmsRequest,
    styles: &std::collections::HashMap<String, String>,
    layer: &crate::models::Layer,
) -> Vec<ParsedRule> {
    let sld_xml = request.sld_body.clone().or_else(|| {
        let style_name = layer.styles.first().map(|s| &s.name).cloned().unwrap_or_default();
        styles.get(&style_name).cloned()
    });
    match sld_xml {
        Some(xml) => sld_parser::parse_sld(&xml),
        None => sld_parser::parse_sld(&sld_parser::default_sld(&layer.name)),
    }
}

fn parse_color_opt(color: &str) -> Option<[u8; 4]> {
    if color.starts_with('#') {
        let hex = &color[1..];
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            return Some([r, g, b, 255]);
        }
    }
    None
}

fn reproject_geometry(geom: &GeoJsonGeometry, from_crs: &str, to_crs: &str) -> GeoJsonGeometry {
    let transformer = ProjectionTransformer::new(
        CoordinateReferenceSystem::from_epsg(from_crs),
        CoordinateReferenceSystem::from_epsg(to_crs),
    );
    match geom {
        GeoJsonGeometry::Point { coordinates } => {
            if coordinates.len() >= 2 {
                if let Ok((x, y)) = transformer.transform_point(coordinates[0], coordinates[1]) {
                    return GeoJsonGeometry::Point { coordinates: vec![x, y] };
                }
            }
            geom.clone()
        }
        GeoJsonGeometry::LineString { coordinates } => {
            let projected: Vec<Vec<f64>> = coordinates.iter()
                .filter_map(|c| {
                    if c.len() >= 2 {
                        transformer.transform_point(c[0], c[1]).ok()
                            .map(|(x, y)| vec![x, y])
                    } else {
                        None
                    }
                })
                .collect();
            if projected.len() == coordinates.len() {
                GeoJsonGeometry::LineString { coordinates: projected }
            } else {
                geom.clone()
            }
        }
        GeoJsonGeometry::Polygon { coordinates } => {
            let projected: Vec<Vec<Vec<f64>>> = coordinates.iter()
                .map(|ring| {
                    ring.iter()
                        .filter_map(|c| {
                            if c.len() >= 2 {
                                transformer.transform_point(c[0], c[1]).ok()
                                    .map(|(x, y)| vec![x, y])
                            } else {
                                None
                            }
                        })
                        .collect()
                })
                .collect();
            if projected.len() == coordinates.len()
                && projected.iter().zip(coordinates.iter()).all(|(p, o)| p.len() == o.len())
            {
                GeoJsonGeometry::Polygon { coordinates: projected }
            } else {
                geom.clone()
            }
        }
        _ => geom.clone(),
    }
}

fn calculate_openlayers_zoom(bounds: &Bounds, crs: &str) -> f64 {
    let world_width = match crs {
        "EPSG:3857" | "3857" | "EPSG:900913" | "900913" => 20037508.34 * 2.0,
        _ => 360.0,
    };
    let range = (bounds.maxx - bounds.minx).max(bounds.maxy - bounds.miny);
    if range <= 0.0 { return 1.0; }
    // ImageWMS 不需要匹配瓦片网格，直接计算合适的缩放级别
    let zoom = (world_width / range).log2().max(0.0).min(20.0);
    // 减少 0.5 让视图稍微缩小，确保数据完整显示
    (zoom - 0.5).max(0.0)
}

fn calculate_scale_denom(bounds: &Bounds, width: u32, height: u32, crs: &str) -> f64 {
    let res_x = (bounds.maxx - bounds.minx) / width as f64;
    let res_y = (bounds.maxy - bounds.miny) / height as f64;
    let ground_res = res_x.max(res_y);
    const PIXEL_SIZE: f64 = 0.00028;
    match crs {
        "EPSG:3857" | "3857" | "EPSG:900913" | "900913" => {
            ground_res / PIXEL_SIZE
        }
        _ => {
            let center_lat = (bounds.miny + bounds.maxy) / 2.0;
            let meters_per_degree = 111319.5 * center_lat.to_radians().cos().abs().max(0.01);
            ground_res * meters_per_degree / PIXEL_SIZE
        }
    }
}

/// 按时间过滤要素
///
/// TIME 参数格式: ISO 8601
/// - 单个时间点: `2024-01-15`
/// - 时间范围: `2024-01-01/2024-02-01`
/// - 逗号分隔多个值: `2024-01-01,2024-02-01`
///
/// 要素属性中匹配字段: `time`, `datetime`, `date`, `timestamp`, `t`
fn filter_by_time(features: &mut Vec<crate::models::Feature>, time_str: &str) {
    let time_str = time_str.trim();
    if time_str.is_empty() { return; }

    // 尝试解析时间
    let parse_time = |s: &str| -> Option<chrono::NaiveDateTime> {
        // 尝试多种格式
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
            return Some(dt);
        }
        if let Ok(d) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
            return Some(d.and_hms_opt(0, 0, 0).unwrap());
        }
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
            return Some(dt);
        }
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
            return Some(dt.naive_utc());
        }
        None
    };

    // 解析时间范围: start/end 或 comma,separated,values
    let ranges: Vec<(Option<chrono::NaiveDateTime>, Option<chrono::NaiveDateTime>)> = if time_str.contains('/') {
        // 范围格式
        time_str.split('/').map(|part| {
            let t = parse_time(part.trim());
            (t, t)
        }).collect::<Vec<_>>()
        .chunks(2)
        .filter_map(|chunk| {
            if chunk.len() == 2 {
                Some((chunk[0].0, chunk[1].0))
            } else if chunk.len() == 1 {
                Some((chunk[0].0, chunk[0].0))
            } else {
                None
            }
        }).collect()
    } else {
        // 逗号分隔或单个值
        time_str.split(',').filter_map(|s| {
            let t = parse_time(s.trim());
            t.map(|t| (Some(t), Some(t)))
        }).collect()
    };

    if ranges.is_empty() { return; }

    // 查找要素中的时间属性
    let time_keys = ["time", "datetime", "date", "timestamp", "t"];
    features.retain(|f| {
        for key in &time_keys {
            if let Some(val) = f.properties.get(*key) {
                let val_str = val.to_string();
                if let Some(ft) = parse_time(&val_str) {
                    // 检查是否在任意一个时间范围内
                    return ranges.iter().any(|(start, end)| {
                        match (start, end) {
                            (Some(s), Some(e)) => ft >= *s && ft <= *e,
                            (Some(s), None) => ft >= *s,
                            (None, Some(e)) => ft <= *e,
                            (None, None) => true,
                        }
                    });
                }
            }
        }
        // 没有时间属性则不过滤（保留）
        true
    });
}

/// 按高程过滤要素
///
/// ELEVATION 参数格式:
/// - 单个值: `1000`
/// - 范围: `500/2000`
/// - 逗号分隔: `100,200,300`
fn filter_by_elevation(features: &mut Vec<crate::models::Feature>, elev_str: &str) {
    let elev_str = elev_str.trim();
    if elev_str.is_empty() { return; }

    let elev_keys = ["elevation", "elev", "height", "z", "altitude", "depth"];

    // 解析高程范围
    let ranges: Vec<(Option<f64>, Option<f64>)> = if elev_str.contains('/') {
        let parts: Vec<&str> = elev_str.split('/').collect();
        if parts.len() == 2 {
            let low = parts[0].trim().parse::<f64>().ok();
            let high = parts[1].trim().parse::<f64>().ok();
            vec![(low, high)]
        } else {
            vec![]
        }
    } else {
        // 逗号分隔或单个值
        elev_str.split(',').filter_map(|s| {
            let v = s.trim().parse::<f64>().ok();
            v.map(|v| (Some(v), Some(v)))
        }).collect()
    };

    if ranges.is_empty() { return; }

    features.retain(|f| {
        for key in &elev_keys {
            if let Some(val) = f.properties.get(*key) {
                let val_str = val.to_string();
                if let Ok(fe) = val_str.parse::<f64>() {
                    return ranges.iter().any(|(start, end)| {
                        match (start, end) {
                            (Some(s), Some(e)) => fe >= *s && fe <= *e,
                            (Some(s), None) => fe >= *s,
                            (None, Some(e)) => fe <= *e,
                            (None, None) => true,
                        }
                    });
                }
            }
        }
        true
    });
}

/// 处理级联 WMS 请求 — 代理到上游 WMS 服务器
async fn handle_cascaded_wms_request(
    state: &AppState,
    context: &GetMapContext,
    meta: &LayerMetadata,
) -> Result<HttpResponse, GeoServerError> {
    use crate::utils::cascaded::{extract_cascaded_config, fetch_cascaded_map};

    let conn = meta.connection.as_ref()
        .ok_or_else(|| GeoServerError::BadRequest("级联 WMS 缺少连接配置".to_string()))?;

    let config = extract_cascaded_config(conn)
        .ok_or_else(|| GeoServerError::BadRequest("无法解析级联 WMS 配置".to_string()))?;

    // 如果请求的是 OpenLayers 预览，回退到本地渲染
    if context.format.to_lowercase().contains("openlayers") {
        return render_openlayers_preview(context, state);
    }

    // 映射输出格式
    let remote_format = match context.format.to_lowercase().as_str() {
        s if s.contains("png") => "image/png",
        s if s.contains("jpeg") || s.contains("jpg") => "image/jpeg",
        s if s.contains("gif") => "image/gif",
        s if s.contains("geojson") || s.contains("json") => "application/json",
        s if s.contains("svg") => "image/svg+xml",
        _ => "image/png",
    };

    let bbox_str = format!("{},{},{},{}",
        context.bounds.minx, context.bounds.miny,
        context.bounds.maxx, context.bounds.maxy);

    let srs = &context.output_crs;
    let style = context.layers.first()
        .and_then(|_| None); // 暂不使用样式

    match fetch_cascaded_map(
        &config, &bbox_str, context.width, context.height,
        remote_format, srs, style, context.transparent,
    ).await {
        Ok((bytes, content_type)) => {
            state.request_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Ok(HttpResponse::Ok()
                .content_type(content_type.as_str())
                .body(bytes))
        }
        Err(e) => {
            warn!("[Cascaded] 代理请求失败: {}", e);
            Err(GeoServerError::ServiceError(format!("级联 WMS 请求失败: {}", e)))
        }
    }
}

fn parse_color(color: &str) -> [u8; 4] {
    if color.starts_with('#') && color.len() >= 7 {
        let r = u8::from_str_radix(&color[1..3], 16).unwrap_or(255);
        let g = u8::from_str_radix(&color[3..5], 16).unwrap_or(255);
        let b = u8::from_str_radix(&color[5..7], 16).unwrap_or(255);
        [r, g, b, 255]
    } else {
        [255, 255, 255, 255]
    }
}
