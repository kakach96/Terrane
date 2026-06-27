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

    info!("[GetMap] 开始查询图层要素");
    let mut layer_contexts = query_all_layer_features(state, &context, &layer_metadata_list).await?;

    info!("[GetMap] 开始解析样式");
    resolve_feature_styles(&mut layer_contexts, context.scale_denominator);

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
) {
    info!("[resolve_feature_styles] 开始解析样式，比例尺分母: {}", scale_denominator);
    let mut total_items = 0;

    for layer_ctx in layer_contexts.iter_mut() {
        debug!("[resolve_feature_styles] 处理图层: {}, 要素数: {}", 
               layer_ctx.metadata.layer_name, layer_ctx.features.len());
        
        for feature in &layer_ctx.features {
            let style = layer_ctx.metadata.rules.iter()
                .find(|rule| sld_parser::match_rule(rule, &feature.properties, Some(scale_denominator)))
                .map(|rule| rule.style.clone())
                .unwrap_or_default();

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
    
    let renderer = MapRenderer::new(context.options.clone(), context.bounds.clone());

    let all_render_items: Vec<(GeoJsonGeometry, Style)> = layer_contexts.iter()
        .flat_map(|ctx| ctx.render_items.clone())
        .collect();

    info!("[render_map_image] 共 {} 个渲染项待渲染", all_render_items.len());

    let img = renderer.render(all_render_items);

    let image_format = match context.format.to_lowercase().as_str() {
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
  </style>
</head>
<body>
  <div id="map"></div>
  <div class="layer-info">图层: {layer_name}</div>
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

    // 视图变化时更新 WMS 请求
    map.getView().on('change:resolution', function() {{
      wmsSource.updateParams({{
        'TIME': new Date().getTime().toString()
      }});
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

    let layers_lock = state.layers.read().await;
    let mut found_features: Vec<(String, String, HashMap<String, String>)> = Vec::new();

    if let Some(query_layers) = &request.query_layers {
        for layer_name in query_layers {
            if let Some(layer) = layers_lock.iter().find(|l| l.name == *layer_name) {
                if let Some(features) = state.get_layer_features(&layer.name).await {
                    for feature in &features {
                        let hit = if let Some((cx, cy)) = click_point {
                            feature_hit_test(&feature.geometry, cx, cy, &bbox_to_bounds(request))
                        } else {
                            true
                        };
                        if hit {
                            let mut props = HashMap::new();
                            for (k, v) in &feature.properties {
                                props.insert(k.clone(), v.to_string());
                            }
                            found_features.push((layer.name.clone(), feature.id.clone(), props));
                            if found_features.len() >= feature_count {
                                break;
                            }
                        }
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
            if coordinates.len() >= 2 {
                let dx = coordinates[0] - cx;
                let dy = coordinates[1] - cy;
                (dx * dx + dy * dy).sqrt() <= tolerance
            } else {
                false
            }
        }
        GeoJsonGeometry::LineString { coordinates } => {
            coordinates.windows(2).any(|seg| {
                if seg.len() < 2 || seg[0].len() < 2 || seg[1].len() < 2 {
                    return false;
                }
                point_to_segment_distance(cx, cy, seg[0][0], seg[0][1], seg[1][0], seg[1][1]) <= tolerance
            })
        }
        GeoJsonGeometry::Polygon { coordinates } => {
            coordinates.first().map(|ring| point_in_ring(cx, cy, ring)).unwrap_or(false)
        }
        _ => false,
    }
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
