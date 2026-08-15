use crate::error::GeoServerError;
use crate::models::{
    Bounds, CoordinateReferenceSystem, DataSourceType, Feature, GeoJsonGeometry, Layer,
};
use crate::services::wms::{self, WmsCapabilities, WmsRequest};
use crate::state::AppState;
use crate::utils::projection::ProjectionTransformer;
use crate::utils::rendering::{MapRenderer, RenderFormat, RenderOptions, Style};
use crate::utils::sld_parser::{self, ParsedRule};
use crate::utils::wkb;
use actix_web::{web, HttpRequest, HttpResponse};
use image::{ImageFormat, RgbaImage};
use quick_xml::se::to_string;
use std::collections::HashMap;
use std::io::Cursor;
use tracing::{debug, info, warn};

struct GetMapContext {
    layers: Vec<String>,
    /// 请求样式列表 (逗号分隔的 STYLES 参数, 逐图层对应; 空项 = 默认样式)
    styles: Vec<String>,
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
    /// 栅格图层渲染数据 (GeoTIFF / WorldImage / ArcGrid): 图像 + 地理边界。
    /// 矢量图层为 None。栅格图层无矢量要素。
    raster: Option<Option<RasterLayerData>>,
}

/// 栅格图层数据: 已解码的 RGBA 图像与其地理边界 (EPSG:4326)。
struct RasterLayerData {
    image: image::RgbaImage,
    bounds: Bounds,
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
        wms::WmsOperation::GetLegendGraphic => {
            handle_get_legend_graphic(&state, &wms_request).await
        },
        wms::WmsOperation::DescribeLayer => handle_describe_layer(&state, &wms_request).await,
        wms::WmsOperation::GetStyles => handle_get_styles(&state, &wms_request).await,
        _ => Err(GeoServerError::BadRequest(
            "Operation not implemented".to_string(),
        )),
    };

    match result {
        Ok(resp) => resp,
        Err(e) => format_wms_error_response(&e, &params),
    }
}

fn format_wms_error_response(err: &GeoServerError, params: &[(String, String)]) -> HttpResponse {
    let exceptions = params
        .iter()
        .find(|(k, _)| k.to_uppercase() == "EXCEPTIONS")
        .map(|(_, v)| v.as_str());
    let width: u32 = params
        .iter()
        .find(|(k, _)| k.to_uppercase() == "WIDTH")
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(512);
    let height: u32 = params
        .iter()
        .find(|(k, _)| k.to_uppercase() == "HEIGHT")
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(512);

    let (body, content_type) = wms::format_wms_exception(err, exceptions, width, height);
    HttpResponse::Ok().content_type(content_type).body(body)
}

/// Render a map for a single layer by delegating to the shared WMS GetMap
/// pipeline. Used by the OGC API - Maps `map` operation
/// (`GET /ogc/maps/collections/{id}/map`).
#[allow(clippy::too_many_arguments)] // signature mirrors the WMS GetMap query parameters
pub async fn render_ogc_map(
    state: &AppState,
    layer: &str,
    bbox: &Bounds,
    width: u32,
    height: u32,
    format: &str,
    crs: &str,
    transparent: bool,
    bgcolor: Option<String>,
    time: Option<String>,
    cql_filter: Option<String>,
) -> Result<HttpResponse, GeoServerError> {
    let request = wms::WmsRequest {
        service: "WMS".to_string(),
        version: Some("1.3.0".to_string()),
        request: wms::WmsOperation::GetMap,
        layers: Some(vec![layer.to_string()]),
        styles: None,
        crs: Some(crs.to_string()),
        bbox: Some(wms::Bbox {
            minx: bbox.minx,
            miny: bbox.miny,
            maxx: bbox.maxx,
            maxy: bbox.maxy,
        }),
        width: Some(width),
        height: Some(height),
        format: Some(format.to_string()),
        transparent: Some(transparent),
        bgcolor,
        exceptions: None,
        time,
        elevation: None,
        query_layers: None,
        info_format: None,
        feature_count: None,
        i: None,
        j: None,
        sld: None,
        sld_body: None,
        cql_filter,
        env: None,
        feature_id: None,
        angle: None,
        scale: None,
    };
    handle_get_map(state, &request).await
}

async fn handle_get_capabilities(
    state: &AppState,
    _request: &WmsRequest,
) -> Result<HttpResponse, GeoServerError> {
    let base_url = format!(
        "http://{}:{}",
        state.config.server.host, state.config.server.port
    );
    let mut capabilities = WmsCapabilities::new(&base_url);

    // 应用服务级设置 (标题/摘要/关键字, 经 /services/wms/settings 配置)
    {
        let map = state.service_settings.read().await;
        if let Some(settings) = map.get("wms") {
            if let Some(title) = &settings.title {
                capabilities.service.title = title.clone();
            }
            if let Some(ab) = &settings.abstract_text {
                capabilities.service.abstract_text = Some(ab.clone());
            }
            if !settings.keywords.is_empty() {
                capabilities.service.keywords = settings.keywords.clone();
            }
        }
    }

    let layers = state.layers.read().await;
    for layer in layers.iter() {
        capabilities.add_layer(layer);
    }

    let xml = to_string(&capabilities).map_err(|e| {
        GeoServerError::ServiceError(format!("Failed to serialize capabilities: {}", e))
    })?;

    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
{}"#,
        xml
    );

    Ok(HttpResponse::Ok().content_type("application/xml").body(xml))
}

async fn handle_get_map(
    state: &AppState,
    request: &WmsRequest,
) -> Result<HttpResponse, GeoServerError> {
    info!("[GetMap] 开始处理请求");
    debug!(
        "[GetMap] 请求参数: layers={:?}, format={:?}, bbox={:?}, crs={:?}",
        request.layers, request.format, request.bbox, request.crs
    );

    wms::validate_wms_get_map_request(request)?;

    let context = parse_get_map_params(request)?;
    info!(
        "[GetMap] 参数解析完成, format={}, bbox={:?}, output_crs={}, size={}x{}",
        context.format, context.bounds, context.output_crs, context.width, context.height
    );

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
                    info!(
                        "[GetMap] 图层 '{}' 使用级联 WMS，开始代理请求",
                        meta.layer_name
                    );
                    return handle_cascaded_wms_request(state, &context, meta).await;
                }
            }
        }
    }

    info!("[GetMap] 开始查询图层要素");
    let mut layer_contexts =
        query_all_layer_features(state, &context, &layer_metadata_list).await?;

    info!("[GetMap] 开始解析样式");
    resolve_feature_styles(&mut layer_contexts, context.scale_denominator, &context.env);

    info!("[GetMap] 开始渲染输出");
    render_map_image(&context, &layer_contexts)
}

fn parse_get_map_params(request: &WmsRequest) -> Result<GetMapContext, GeoServerError> {
    let layers_param = request
        .layers
        .as_ref()
        .ok_or_else(|| GeoServerError::BadRequest("LAYERS parameter is required".to_string()))?;

    let width = request.width.unwrap_or(512);
    let height = request.height.unwrap_or(512);

    let bbox = request
        .bbox
        .as_ref()
        .ok_or_else(|| GeoServerError::BadRequest("BBOX parameter is required".to_string()))?;
    let bounds = Bounds::new(bbox.minx, bbox.miny, bbox.maxx, bbox.maxy);

    let output_crs = request.crs.as_deref().unwrap_or("EPSG:4326").to_string();
    let scale_denominator = calculate_scale_denom(&bounds, width, height, &output_crs);

    let format = request
        .format
        .as_ref()
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
    let feature_id = request
        .feature_id
        .as_ref()
        .map(|fid| fid.split(',').map(|s| s.trim().to_string()).collect());

    // STYLES 参数: 逐图层对应; 空项 = 默认样式。若未提供 (None) 或数量
    // 少于图层数, 缺失项按默认样式处理。
    let styles: Vec<String> = request
        .styles
        .as_ref()
        .map(|v| v.iter().map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();

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
        styles,
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
    info!(
        "[resolve_layer_metadata] 开始处理 {} 个图层",
        context.layers.len()
    );
    let layers_lock = state.layers.read().await;
    let styles_lock = state.styles.read().await;
    let styles_meta_lock = state.styles_meta.read().await;

    let mut metadata_list = Vec::with_capacity(context.layers.len());

    for (idx, layer_name) in context.layers.iter().enumerate() {
        let (workspace, layer_short_name) = if layer_name.contains(':') {
            let parts: Vec<&str> = layer_name.splitn(2, ':').collect();
            (parts[0].to_string(), parts[1].to_string())
        } else {
            info!(
                "[resolve_layer_metadata] 图层名 '{}' 无工作空间前缀，使用默认",
                layer_name
            );
            (String::new(), layer_name.clone())
        };

        info!(
            "[resolve_layer_metadata] 解析图层: 输入='{}', workspace='{}', layer='{}'",
            layer_name, workspace, layer_short_name
        );

        let layer = layers_lock
            .iter()
            .find(|l| {
                l.name == *layer_name || (l.workspace == workspace && l.name == layer_short_name)
            })
            .ok_or_else(|| GeoServerError::NotFound(format!("Layer '{}' not found", layer_name)))?;

        let data_source = if let Some(store) = &state.store {
            store.get_data_source(&layer.store).await.map_err(|e| {
                GeoServerError::InternalError(format!("Failed to get data source: {}", e))
            })?
        } else {
            None
        };

        // 内置 metadata 数据源被当作普通数据源看待: 复用内置合成数据源
        // (postgres 元数据 → PostGIS 优化查询路径, 含请求坐标系 bbox 转换;
        //  sqlite 元数据 → Metadata 类型, 走默认查询回落)
        let (data_source_type, connection) = if layer.store == crate::models::METADATA_DATA_SOURCE {
            crate::handlers::features::builtin_metadata_data_source(state)
                .map(|ds| (ds.data_source_type, ds.connection))
                .unwrap_or((DataSourceType::Metadata, None))
        } else {
            data_source
                .map(|ds| (ds.data_source_type, ds.connection))
                .unwrap_or((DataSourceType::Shapefile, None))
        };

        let native_name = if let Some(ref nn) = layer.native_name {
            nn.clone()
        } else {
            layer_short_name.clone()
        };

        let native_crs = layer.srs.to_epsg();

        // STYLES 参数: 第 idx 个图层使用请求中第 idx 个样式名; 空项或
        // 未提供 → 图层默认样式 (与 GeoServer 行为一致)。
        let requested_style = context.styles.get(idx).filter(|s| !s.is_empty());
        let effective_style = requested_style
            .cloned()
            .unwrap_or_else(|| style_name_of(layer));

        // 按样式格式分派解析 (SLD/CSS/YSLD/MBStyle), 保证 WMS 渲染与
        // 瓦片管线 (style_handler::parse_style_content) 行为一致。
        let style_content = request_sld_body(&effective_style, &styles_lock);
        let style_format = styles_meta_lock
            .get(&effective_style)
            .map(|m| m.format.clone())
            .unwrap_or_else(|| crate::models::style::detect_style_format(&style_content));
        let rules =
            crate::handlers::style_handler::parse_style_content(&style_content, &style_format);

        info!("[resolve_layer_metadata] 图层 '{}' 详情: workspace='{}', layer='{}', native_name='{}', native_crs='{}', data_source_type={:?}, style='{}', rules_count={}", 
              layer_name, layer.workspace, layer.name, native_name, native_crs, data_source_type, effective_style, rules.len());

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

    info!(
        "[resolve_layer_metadata] 完成元数据查询，共 {} 个图层",
        metadata_list.len()
    );
    Ok(metadata_list)
}

/// 图层绑定的第一个样式名 (WMS 渲染使用图层主样式)。
fn style_name_of(layer: &Layer) -> String {
    layer
        .styles
        .first()
        .map(|s| s.name.clone())
        .unwrap_or_default()
}

/// 取指定样式名的样式内容; 样式不存在时回退到图层默认 SLD。
fn request_sld_body(style_name: &str, styles: &HashMap<String, String>) -> String {
    styles
        .get(style_name)
        .cloned()
        .unwrap_or_else(|| sld_parser::default_sld(style_name))
}

async fn query_all_layer_features(
    state: &AppState,
    context: &GetMapContext,
    metadata_list: &[LayerMetadata],
) -> Result<Vec<LayerRenderContext>, GeoServerError> {
    info!(
        "[query_all_layer_features] 开始查询 {} 个图层的要素，查询范围: {:?}, 坐标系: {}",
        metadata_list.len(),
        context.bounds,
        context.output_crs
    );
    let mut contexts = Vec::with_capacity(metadata_list.len());

    // 解析 cql_filter: 多个图层的过滤条件用 ; 分隔
    let cql_filters: Vec<&str> = context
        .cql_filter
        .as_deref()
        .map(|f| f.split(';').collect())
        .unwrap_or_default();

    for (idx, metadata) in metadata_list.iter().enumerate() {
        info!(
            "[query_all_layer_features] 查询图层: {}",
            metadata.layer_name
        );

        // 栅格图层: 加载栅格图像 (GeoTIFF / WorldImage / ArcGrid / ImageMosaic /
        // ImagePyramid), 不查询矢量要素。
        let raster = match metadata.data_source_type {
            DataSourceType::Geotiff
            | DataSourceType::WorldImage
            | DataSourceType::ArcGrid
            | DataSourceType::ImageMosaic
            | DataSourceType::ImagePyramid => Some(load_raster_layer(state, metadata).await),
            _ => None,
        };

        let mut features = if raster.is_some() {
            Vec::new()
        } else {
            query_layer_features_optimized(
                state,
                metadata,
                &context.bounds,
                &context.output_crs,
                context.scale_denominator,
            )
            .await?
        };

        // 应用 CQL 过滤
        if let Some(cql_str) = cql_filters.get(idx).filter(|s| !s.is_empty()) {
            match crate::utils::cql_filter::parse_cql(cql_str) {
                Ok(expr) => {
                    debug!(
                        "[query_all_layer_features] 图层 '{}' 应用 CQL 过滤: '{}'",
                        metadata.layer_name, cql_str
                    );
                    features.retain(|f| crate::utils::cql_filter::evaluate_cql(f, &expr));
                },
                Err(e) => {
                    warn!(
                        "[query_all_layer_features] CQL 解析失败 (图层 '{}'): {}",
                        metadata.layer_name, e
                    );
                },
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

        // 应用地图旋转 (ANGLE): 绕请求 bbox 中心旋转所有要素几何。
        // GeoServer 语义 — 地图旋转而标注保持水平 (标注基于旋转后几何锚点)。
        if let Some(angle) = context.angle {
            if angle.abs() > 1e-6 {
                let cx = (context.bounds.minx + context.bounds.maxx) / 2.0;
                let cy = (context.bounds.miny + context.bounds.maxy) / 2.0;
                for feature in features.iter_mut() {
                    feature.geometry = rotate_geometry(&feature.geometry, cx, cy, angle);
                }
            }
        }

        debug!(
            "[query_all_layer_features] 图层 '{}' 过滤后剩余 {} 个要素",
            metadata.layer_name,
            features.len()
        );

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
            raster,
        });
    }

    info!("[query_all_layer_features] 要素查询完成");
    Ok(contexts)
}

/// 加载栅格图层数据 (GeoTIFF / WorldImage / ArcGrid / ImageMosaic /
/// ImagePyramid) 并归一化为 RGBA 图像 + 地理边界 (EPSG:4326)。
async fn load_raster_layer(_state: &AppState, metadata: &LayerMetadata) -> Option<RasterLayerData> {
    let conn = metadata.connection.as_ref()?;
    // WorldImage / ImageMosaic / ImagePyramid 是目录/伴生文件, 走目录物化;
    // 其余单文件。
    let materialized = match metadata.data_source_type {
        DataSourceType::WorldImage | DataSourceType::ImageMosaic | DataSourceType::ImagePyramid => {
            crate::store::materialize_dir(conn).await.ok()??
        },
        _ => crate::store::materialize_file(conn).await.ok()??,
    };
    let path = materialized.path;
    let (image, bounds) = match metadata.data_source_type {
        DataSourceType::Geotiff => {
            let cov = crate::utils::geotiff::read_geotiff(&path).ok()?;
            (cov.rgba_image, cov.bounds?)
        },
        DataSourceType::WorldImage => {
            let wim = crate::utils::worldimage::read_worldimage(&path).ok()?;
            (wim.rgba_image, wim.bounds)
        },
        DataSourceType::ArcGrid => {
            let ag = crate::utils::arcgrid::read_arcgrid(&path).ok()?;
            (ag.rgba_image, ag.bounds)
        },
        DataSourceType::ImageMosaic => {
            // 目录马赛克: 聚合所有 granule, 返回整幅合成图 (EPSG:4326)。
            let granules = crate::utils::mosaic::load_mosaic(&path);
            let b = crate::utils::mosaic::mosaic_bounds(&granules)?;
            let img = crate::utils::mosaic::render_mosaic(&granules, &b, 1024, 1024)?;
            (img, b)
        },
        DataSourceType::ImagePyramid => {
            // 金字塔: 选最精细层级 (level 0) 渲染整幅。
            let levels = crate::utils::pyramid::load_pyramid(&path);
            let b = crate::utils::pyramid::pyramid_bounds(&levels)?;
            let lvl = crate::utils::pyramid::select_level(&levels, f64::MIN_POSITIVE)?;
            let img = crate::utils::pyramid::render_level(lvl, &b, 1024, 1024)?;
            (img, b)
        },
        _ => return None,
    };
    Some(RasterLayerData { image, bounds })
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
                )
                .await
            } else {
                debug!("[query_layer_features_optimized] 无PostGIS连接配置，使用默认查询");
                Ok(Vec::new())
            }
        },
        _ => {
            debug!("[query_layer_features_optimized] 使用默认查询方式");
            let features = crate::handlers::features::query_layer_features(
                state,
                &metadata.layer_name,
                Some(bbox),
                None,
                None,
            )
            .await
            .unwrap_or_default();

            let needs_reproject = metadata.native_crs != output_crs;
            if needs_reproject {
                debug!(
                    "[query_layer_features_optimized] 需要坐标转换: {} -> {}",
                    metadata.native_crs, output_crs
                );
                Ok(features
                    .into_iter()
                    .map(|mut f| {
                        f.geometry =
                            reproject_geometry(&f.geometry, &metadata.native_crs, output_crs);
                        f
                    })
                    .collect())
            } else {
                Ok(features)
            }
        },
    };

    info!(
        "[query_layer_features_optimized] 图层 '{}' 查询结果: {} 个要素",
        metadata.layer_name,
        features.as_ref().map(|f| f.len()).unwrap_or(0)
    );
    features
}

#[allow(clippy::too_many_arguments)] // signature mirrors the WMS GetMap query parameters
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
    let schema_name = conn
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

    info!(
        "[PostGIS] 开始查询 PostGIS, table='{}', schema='{}', storage_crs='{}', output_crs='{}'",
        table_name, schema_name, storage_crs, output_crs
    );

    let pool = state.get_pg_pool(table_name, conn);
    let client = pool
        .get()
        .await
        .map_err(|e| GeoServerError::InternalError(format!("Pool error: {}", e)))?;

    let schema = schema_name;

    let geom_col = get_geometry_column(&client, &schema, table_name)
        .await
        .unwrap_or_else(|| "geom".to_string());

    let needs_transform = storage_crs != output_crs;
    let storage_srid = parse_srid(storage_crs);
    let output_srid = parse_srid(output_crs);

    let simplify_tolerance = calculate_simplify_tolerance(scale_denominator, bbox);

    debug!("[PostGIS] 查询参数: needs_transform={}, storage_srid={}, output_srid={}, simplify_tolerance={:?}",
           needs_transform, storage_srid, output_srid, simplify_tolerance);

    let geom_expr = if needs_transform {
        if let Some(tol) = simplify_tolerance {
            format!(
                "ST_AsBinary(ST_SimplifyPreserveTopology(ST_Transform({}, {}), {}))",
                geom_col, output_srid, tol
            )
        } else {
            format!("ST_AsBinary(ST_Transform({}, {}))", geom_col, output_srid)
        }
    } else {
        if let Some(tol) = simplify_tolerance {
            format!(
                "ST_AsBinary(ST_SimplifyPreserveTopology({}, {}))",
                geom_col, tol
            )
        } else {
            format!("ST_AsBinary({})", geom_col)
        }
    };

    // 请求 bbox 处于请求坐标系 (output_srid) 下; 空间过滤需转换到存储坐标系 (storage_srid)
    let bbox_env = if needs_transform {
        format!(
            "ST_Transform(ST_MakeEnvelope({}, {}, {}, {}, {}), {})",
            bbox.minx, bbox.miny, bbox.maxx, bbox.maxy, output_srid, storage_srid
        )
    } else {
        format!(
            "ST_MakeEnvelope({}, {}, {}, {}, {})",
            bbox.minx, bbox.miny, bbox.maxx, bbox.maxy, storage_srid
        )
    };

    let sql = format!(
        "SELECT {} as geom_wkb, {} && {} as _in_bbox \
         FROM \"{}\".\"{}\" \
         WHERE {} && {} \
         LIMIT {}",
        geom_expr, geom_col, bbox_env, schema, table_name, geom_col, bbox_env, max_features
    );

    debug!("[PostGIS] 执行SQL: {}", sql);

    let rows = client
        .query(&sql, &[])
        .await
        .map_err(|e| GeoServerError::InternalError(format!("PostGIS query error: {}", e)))?;

    info!("[PostGIS] SQL执行完成, 返回 {} 行", rows.len());

    let mut features = Vec::with_capacity(rows.len());
    for (idx, row) in rows.iter().enumerate() {
        let wkb_data: Vec<u8> = row.try_get("geom_wkb").unwrap_or_default();
        let geometry = wkb::parse_wkb_geometry(&wkb_data);
        let mut properties = HashMap::new();
        properties.insert(
            "id".to_string(),
            crate::models::PropertyValue::String(format!("feat_{}", idx)),
        );
        features.push(Feature::with_id(
            format!("feat_{}", idx),
            geometry,
            properties,
        ));
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
    info!(
        "[resolve_feature_styles] 开始解析样式，比例尺分母: {}",
        scale_denominator
    );
    let mut total_items = 0;

    // 空环境变量映射: 无 env 时标签/颜色保持原样
    let empty_env: HashMap<String, String> = HashMap::new();

    for layer_ctx in layer_contexts.iter_mut() {
        debug!(
            "[resolve_feature_styles] 处理图层: {}, 要素数: {}",
            layer_ctx.metadata.layer_name,
            layer_ctx.features.len()
        );

        for feature in &layer_ctx.features {
            let env_map = env.as_ref().filter(|m| !m.is_empty());
            let style = sld_parser::resolve_style_with_env(
                &layer_ctx.metadata.rules,
                feature,
                Some(scale_denominator),
                env_map.unwrap_or(&empty_env),
            );

            layer_ctx
                .render_items
                .push((feature.geometry.clone(), style));
            total_items += 1;
        }
    }

    info!(
        "[resolve_feature_styles] 样式解析完成，共 {} 个渲染项",
        total_items
    );
}

fn render_map_image(
    context: &GetMapContext,
    layer_contexts: &[LayerRenderContext],
) -> Result<HttpResponse, GeoServerError> {
    info!(
        "[render_map_image] 开始渲染地图，尺寸: {}x{}, 透明背景: {}",
        context.width, context.height, context.transparent
    );

    let format_lower = context.format.to_lowercase();

    let all_render_items: Vec<(GeoJsonGeometry, Style)> = layer_contexts
        .iter()
        .flat_map(|ctx| ctx.render_items.clone())
        .collect();

    info!(
        "[render_map_image] 共 {} 个渲染项待渲染",
        all_render_items.len()
    );

    // 非图片格式: SVG
    if format_lower.contains("svg") {
        let svg = crate::utils::rendering::render_to_svg(
            &all_render_items,
            &context.bounds,
            context.width,
            context.height,
        );
        return Ok(HttpResponse::Ok().content_type("image/svg+xml").body(svg));
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

    // 非图片格式: GeoRSS (RSS 2.0 + GeoRSS 命名空间, 要素点/线/面)
    if format_lower.contains("rss") || format_lower.contains("georss") {
        let layer_name = context.layers.first().map(|s| s.as_str()).unwrap_or("map");
        let georss = crate::utils::rendering::render_to_georss(&all_render_items, layer_name);
        return Ok(HttpResponse::Ok()
            .content_type("application/rss+xml")
            .body(georss));
    }

    // 非图片格式: PDF (渲染地图图像并封装为单页 PDF)
    if format_lower.contains("pdf") {
        let pdf = crate::utils::rendering::render_to_pdf(
            &all_render_items,
            &context.bounds,
            context.width,
            context.height,
        );
        return Ok(HttpResponse::Ok().content_type("application/pdf").body(pdf));
    }

    // 图片格式: 使用 MapRenderer 渲染。栅格图层先绘制 (按图层顺序),
    // 矢量要素叠加在上。
    let renderer = MapRenderer::new(context.options, context.bounds.clone());
    let has_raster = layer_contexts
        .iter()
        .any(|ctx| ctx.raster.as_ref().is_some_and(|r| r.is_some()));
    let mut img = RgbaImage::new(context.width, context.height);

    // 背景: 非透明时填充背景色。
    if !context.transparent {
        let bg = context.bg_color.unwrap_or([255, 255, 255, 255]);
        for pixel in img.pixels_mut() {
            *pixel = image::Rgba(bg);
        }
    }

    // 栅格图层: 裁剪 + 缩放到请求 bbox。
    for ctx in layer_contexts {
        if let Some(Some(raster)) = &ctx.raster {
            if let Some(tile) = render_raster_to_map(
                &raster.image,
                &raster.bounds,
                &context.bounds,
                context.width,
                context.height,
            ) {
                composite_image(&mut img, &tile, 0, 0);
            }
        }
    }

    // 矢量要素叠加 (含标签与 z-order)。空渲染项时渲染器会画"空地图"
    // 占位框, 仅在无栅格图层时保留该行为。
    if !all_render_items.is_empty() || !has_raster {
        let vector_img = renderer.render(all_render_items);
        composite_image(&mut img, &vector_img, 0, 0);
    }

    let image_format = match format_lower.as_str() {
        s if s.contains("png") => ImageFormat::Png,
        s if s.contains("jpeg") || s.contains("jpg") => ImageFormat::Jpeg,
        s if s.contains("gif") => ImageFormat::Gif,
        s if s.contains("webp") => ImageFormat::WebP,
        _ => ImageFormat::Png,
    };

    debug!("[render_map_image] 渲染格式: {:?}", image_format);

    // JPEG / GIF 无 alpha 通道: RGBA 直接编码会报 UnsupportedColor,
    // 先合成到不透明白底再转 RGB。
    let mut buffer = Cursor::new(Vec::new());
    match image_format {
        ImageFormat::Jpeg | ImageFormat::Gif => {
            let mut opaque = RgbaImage::new(context.width, context.height);
            for (y, row) in img.rows().enumerate() {
                for (x, px) in row.enumerate() {
                    let a = px.0[3] as f32 / 255.0;
                    let bg = [255u8, 255, 255];
                    let out = [
                        (px.0[0] as f32 * a + bg[0] as f32 * (1.0 - a)).round() as u8,
                        (px.0[1] as f32 * a + bg[1] as f32 * (1.0 - a)).round() as u8,
                        (px.0[2] as f32 * a + bg[2] as f32 * (1.0 - a)).round() as u8,
                    ];
                    opaque.put_pixel(
                        x as u32,
                        y as u32,
                        image::Rgba([out[0], out[1], out[2], 255]),
                    );
                }
            }
            image::DynamicImage::ImageRgba8(opaque)
                .to_rgb8()
                .write_to(&mut buffer, image_format)
                .map_err(|e| {
                    GeoServerError::RenderingError(format!("Failed to render image: {}", e))
                })?;
        },
        _ => img.write_to(&mut buffer, image_format).map_err(|e| {
            GeoServerError::RenderingError(format!("Failed to render image: {}", e))
        })?,
    }

    let image_size = buffer.get_ref().len();
    let content_type = match image_format {
        ImageFormat::Png => "image/png",
        ImageFormat::Jpeg => "image/jpeg",
        ImageFormat::Gif => "image/gif",
        ImageFormat::WebP => "image/webp",
        _ => "image/png",
    };

    info!(
        "[render_map_image] 渲染完成，返回图片，大小: {} 字节, Content-Type: {}",
        image_size, content_type
    );

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

    // OpenLayers 原生支持的投影 (无需 proj4js)。请求 bbox 处于请求 SRS 下,
    // 对 ol 原生投影直接使用; 其他投影将 bbox 转换到 EPSG:4326 渲染。
    let ol_native = {
        let c = context.output_crs.to_uppercase();
        c.contains("3857") || c.contains("900913") || c.contains("4326")
    };

    let (view_crs, view_bounds) = if ol_native {
        (context.output_crs.clone(), context.bounds.clone())
    } else {
        // 非 ol 原生投影: 将请求 bbox (请求 SRS 下) 转换到 EPSG:4326
        let from = &context.bounds;
        let b1 = crate::utils::geometry::transform_coordinates(
            &[from.minx, from.miny],
            &context.output_crs,
            "EPSG:4326",
        );
        let b2 = crate::utils::geometry::transform_coordinates(
            &[from.maxx, from.maxy],
            &context.output_crs,
            "EPSG:4326",
        );
        match (b1, b2) {
            (Ok(p1), Ok(p2)) => (
                "EPSG:4326".to_string(),
                crate::models::Bounds::new(p1[0], p1[1], p2[0], p2[1]),
            ),
            _ => (context.output_crs.clone(), context.bounds.clone()),
        }
    };

    let center_x = (view_bounds.minx + view_bounds.maxx) / 2.0;
    let center_y = (view_bounds.miny + view_bounds.maxy) / 2.0;
    let zoom = calculate_openlayers_zoom(&view_bounds, &view_crs);
    let layers_json = serde_json::to_string(&context.layers).unwrap_or_default();
    let extent = format!(
        "{}, {}, {}, {}",
        view_bounds.minx, view_bounds.miny, view_bounds.maxx, view_bounds.maxy
    );

    // ANGLE 地图旋转: OpenLayers 使用弧度且逆时针为正; WMS ANGLE 为角度且
    // 逆时针为正, 因此 rotation = -angle * pi / 180 才能让预览与 GetMap 一致。
    let angle_deg = context.angle.unwrap_or(0.0);
    let rotation_rad = -angle_deg.to_radians();
    let has_rotation = angle_deg.abs() > 1e-6;

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
    var extent = [{extent}];

    var wmsSource = new ol.source.ImageWMS({{
      url: '{wms_url}',
      params: {{
        'LAYERS': layers.join(','),
        'VERSION': '1.1.1',
        'FORMAT': 'image/png',
        'TRANSPARENT': true{angle_param}
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
        center: [{center_x}, {center_y}],
        zoom: {zoom},
        extent: extent{rotation_attr}
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
        angle_param = if has_rotation {
            format!(", 'ANGLE': '{}'", angle_deg)
        } else {
            String::new()
        },
        rotation_attr = if has_rotation {
            format!(", rotation: {}", rotation_rad)
        } else {
            String::new()
        },
    );

    Ok(HttpResponse::Ok().content_type("text/html").body(html))
}

async fn handle_get_feature_info(
    state: &AppState,
    request: &WmsRequest,
) -> Result<HttpResponse, GeoServerError> {
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

    let click_point =
        click_point.map(|(cx, cy)| transformer.transform_point(cx, cy).unwrap_or((cx, cy)));

    let view_bounds = bbox_to_bounds(request);
    let view_bounds = if transformer.needs_reprojection() {
        transformer
            .transform_bounds(
                view_bounds.minx,
                view_bounds.miny,
                view_bounds.maxx,
                view_bounds.maxy,
            )
            .map(|(a, b, c, d)| Bounds::new(a, b, c, d))
            .unwrap_or(view_bounds)
    } else {
        view_bounds
    };

    let range = (view_bounds.maxx - view_bounds.minx).max(view_bounds.maxy - view_bounds.miny);
    let tolerance = (range / 200.0).max(0.0001);

    let mut found_features: Vec<(
        String,
        String,
        crate::models::GeoJsonGeometry,
        HashMap<String, String>,
    )> = Vec::new();

    if let Some(query_layers) = &request.query_layers {
        for layer_name in query_layers {
            // 以点击点为中心的小范围 bbox 查询，再精确定位命中的要素
            let query_bbox = click_point.map(|(cx, cy)| {
                Bounds::new(
                    cx - tolerance,
                    cy - tolerance,
                    cx + tolerance,
                    cy + tolerance,
                )
            });

            let features: Vec<crate::models::feature::Feature> =
                crate::handlers::features::query_layer_features(
                    state,
                    layer_name,
                    query_bbox.as_ref(),
                    Some(feature_count as u64 * 2),
                    None,
                )
                .await
                .unwrap_or_default();

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
                    found_features.push((
                        layer_name.clone(),
                        feature.id.clone(),
                        feature.geometry.clone(),
                        props,
                    ));
                    if found_features.len() >= feature_count {
                        break;
                    }
                }
            }
        }
    }

    let response = match info_format {
        "application/json" => {
            let json_features: Vec<serde_json::Value> = found_features
                .iter()
                .map(|(layer, fid, _geometry, props)| {
                    serde_json::json!({
                        "layer": layer,
                        "feature_id": fid,
                        "properties": props,
                    })
                })
                .collect();
            serde_json::to_string_pretty(&json_features)
                .map_err(|e| GeoServerError::ServiceError(e.to_string()))?
        },
        "text/html" => {
            let rows: String = found_features
                .iter()
                .map(|(layer, fid, _geometry, props)| {
                    let prop_rows: String = props
                        .iter()
                        .map(|(k, v)| {
                            format!(
                                "<tr><td>{}</td><td>{}</td></tr>",
                                escape_html(k),
                                escape_html(v)
                            )
                        })
                        .collect();
                    format!(
                        "<h3>Layer: {} (ID: {})</h3><table border='1'>{}</table>",
                        escape_html(layer),
                        escape_html(fid),
                        prop_rows
                    )
                })
                .collect();
            format!(
                "<html><body><h1>Feature Information</h1>{}</body></html>",
                rows
            )
        },
        // GML 3.1.1 要素集合 (GeoServer `application/vnd.ogc.gml` 语义):
        // 与 WFS GetFeature 共用同一 GML 序列化器。
        "application/vnd.ogc.gml" => {
            let features: Vec<crate::models::Feature> = found_features
                .iter()
                .map(|(layer, fid, geometry, props)| {
                    let properties: HashMap<String, crate::models::PropertyValue> = props
                        .iter()
                        .map(|(k, v)| (k.clone(), crate::models::PropertyValue::String(v.clone())))
                        .collect();
                    let mut feature =
                        crate::models::Feature::with_id(fid.clone(), geometry.clone(), properties);
                    // 属性中补一个图层字段, 便于客户端区分来源 (与 GeoServer 类似)
                    feature.properties.insert(
                        "layer".to_string(),
                        crate::models::PropertyValue::String(layer.clone()),
                    );
                    feature
                })
                .collect();
            let collection = crate::models::FeatureCollection::new(features);
            crate::handlers::wfs_handler::generate_gml_response(
                &collection,
                "text/xml; subtype=gml/3.1.1",
                None,
            )
        },
        _ => found_features
            .iter()
            .map(|(layer, fid, _geometry, props)| {
                let prop_str: String = props
                    .iter()
                    .map(|(k, v)| format!("  {} = {}", k, v))
                    .collect::<Vec<_>>()
                    .join("\n");
                format!("Layer: {}\nFeature ID: {}\n{}\n", layer, fid, prop_str)
            })
            .collect::<Vec<_>>()
            .join("---\n"),
    };

    let content_type = match info_format {
        "application/json" => "application/json",
        "text/html" => "text/html",
        "application/vnd.ogc.gml" => "application/vnd.ogc.gml",
        _ => "text/plain",
    };

    Ok(HttpResponse::Ok().content_type(content_type).body(response))
}

fn bbox_to_bounds(request: &WmsRequest) -> Bounds {
    request
        .bbox
        .as_ref()
        .map(|b| Bounds::new(b.minx, b.miny, b.maxx, b.maxy))
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
        },
        GeoJsonGeometry::MultiPoint { coordinates } => coordinates
            .iter()
            .any(|c| c.len() >= 2 && point_distance(c[0], c[1], cx, cy) <= tolerance),
        GeoJsonGeometry::LineString { coordinates } => coordinates.windows(2).any(|seg| {
            if seg.len() < 2 || seg[0].len() < 2 || seg[1].len() < 2 {
                return false;
            }
            point_to_segment_distance(cx, cy, seg[0][0], seg[0][1], seg[1][0], seg[1][1])
                <= tolerance
        }),
        GeoJsonGeometry::MultiLineString { coordinates } => coordinates.iter().any(|line| {
            line.windows(2).any(|seg| {
                if seg.len() < 2 || seg[0].len() < 2 || seg[1].len() < 2 {
                    return false;
                }
                point_to_segment_distance(cx, cy, seg[0][0], seg[0][1], seg[1][0], seg[1][1])
                    <= tolerance
            })
        }),
        GeoJsonGeometry::Polygon { coordinates } => coordinates
            .first()
            .map(|ring| point_in_ring(cx, cy, ring))
            .unwrap_or(false),
        GeoJsonGeometry::MultiPolygon { coordinates } => coordinates.iter().any(|poly| {
            poly.first()
                .map(|ring| point_in_ring(cx, cy, ring))
                .unwrap_or(false)
        }),
        GeoJsonGeometry::GeometryCollection { geometries } => geometries
            .iter()
            .any(|g| feature_hit_test(g, cx, cy, bounds)),
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

async fn handle_describe_layer(
    state: &AppState,
    request: &WmsRequest,
) -> Result<HttpResponse, GeoServerError> {
    let layers_param = request
        .layers
        .as_ref()
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
        layer_descriptions
            .iter()
            .map(|desc| {
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
            })
            .collect::<Vec<_>>()
            .join("\n")
    );

    Ok(HttpResponse::Ok().content_type("text/xml").body(xml))
}

async fn handle_get_styles(
    state: &AppState,
    request: &WmsRequest,
) -> Result<HttpResponse, GeoServerError> {
    let layers_param = request.layers.as_ref().ok_or_else(|| {
        GeoServerError::BadRequest("LAYERS parameter required for GetStyles".to_string())
    })?;

    let styles_lock = state.styles.read().await;
    let layers_lock = state.layers.read().await;

    let mut sld_doc = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<StyledLayerDescriptor version="1.0.0"
  xmlns="http://www.opengis.net/sld"
  xmlns:ogc="http://www.opengis.net/ogc"
  xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
"#,
    );

    for layer_name in layers_param {
        let style_name = layers_lock
            .iter()
            .find(|l| l.name == *layer_name)
            .and_then(|l| l.styles.first().map(|s| s.name.clone()))
            .unwrap_or_else(|| "default".to_string());

        let style_content = styles_lock
            .get(&style_name)
            .cloned()
            .unwrap_or_else(String::new);

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

async fn handle_get_legend_graphic(
    state: &AppState,
    request: &WmsRequest,
) -> Result<HttpResponse, GeoServerError> {
    let layer_name = request
        .layers
        .as_ref()
        .and_then(|l| l.first())
        .ok_or_else(|| {
            GeoServerError::BadRequest("LAYER parameter required for GetLegendGraphic".to_string())
        })?;

    let layers_lock = state.layers.read().await;
    let styles_lock = state.styles.read().await;

    let layer = layers_lock
        .iter()
        .find(|l| l.name == *layer_name)
        .ok_or_else(|| GeoServerError::NotFound(format!("Layer '{}' not found", layer_name)))?;

    // 规则列表; 若请求带 SCALE (比例尺分母), 仅保留在该比例尺下激活的规则。
    let all_rules = get_layer_rules(request, &styles_lock, layer);
    let rules: Vec<&ParsedRule> = match request.scale {
        Some(scale) => all_rules
            .iter()
            .filter(|r| {
                if let Some(min) = r.min_scale {
                    if scale < min {
                        return false;
                    }
                }
                if let Some(max) = r.max_scale {
                    if scale > max {
                        return false;
                    }
                }
                true
            })
            .collect(),
        None => all_rules.iter().collect(),
    };

    let padding = 5u32;
    let icon_size = 20u32;
    let row_height = icon_size + 4;
    let total_height = if rules.is_empty() {
        row_height
    } else {
        (rules.len() as u32) * row_height + padding * 2
    };
    // 请求 WIDTH 限制色块宽度; 宽度不足时缩小图标。
    let req_width = request.width.unwrap_or(40).max(icon_size);
    let icon_size = icon_size.min(req_width - 4);
    let total_width = req_width;

    let mut img = image::RgbaImage::new(total_width, total_height);
    for pixel in img.pixels_mut() {
        *pixel = image::Rgba([255, 255, 255, 255]);
    }

    for (idx, rule) in rules.iter().enumerate() {
        let y = padding + idx as u32 * row_height;
        let style = &rule.style;

        let swatch_x = (total_width - icon_size) / 2;
        let swatch_y = y + 2;

        // 点标记: 居中绘制标记 (circle/square/cross/...)。
        if let Some(mark) = &style.mark {
            let r = (style.point_size.unwrap_or(6.0) / 2.0) as i32;
            let (cx, cy) = (
                swatch_x as i32 + icon_size as i32 / 2,
                swatch_y as i32 + icon_size as i32 / 2,
            );
            let fill = style.parse_fill_color().unwrap_or([255, 0, 0, 255]);
            let stroke = style.parse_stroke_color().unwrap_or([0, 0, 0, 255]);
            match mark.as_str() {
                "square" => draw_legend_square(&mut img, cx, cy, r, fill, stroke),
                "cross" => draw_legend_cross(&mut img, cx, cy, r, stroke),
                "x" | "X" => draw_legend_x(&mut img, cx, cy, r, stroke),
                "triangle" => draw_legend_triangle(&mut img, cx, cy, r, fill, stroke),
                _ => draw_legend_circle(&mut img, cx, cy, r, fill, stroke),
            }
        } else if let Some(fill) = &style.fill {
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

        // 规则名标签 (使用内置位图字体)。
        if let Some(name) = rule.name.as_deref().filter(|n| !n.trim().is_empty()) {
            let label = rule.style.label.as_ref();
            let color = label
                .and_then(|l| l.parse_color())
                .unwrap_or([40, 40, 40, 255]);
            // 图例宽度大于色块时, 在色块右侧绘制规则名。
            if total_width > icon_size + 6 {
                let label_x = swatch_x + icon_size + 3;
                let label_y = swatch_y as i32 + (icon_size / 2) as i32 - 3;
                crate::utils::bitmap_font::draw_text(
                    label_x as i32,
                    label_y,
                    name,
                    1.0,
                    |px, py| {
                        if px < total_width && py < total_height {
                            img.put_pixel(px, py, image::Rgba(color));
                        }
                    },
                );
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

// ---- GetLegendGraphic 点标记绘制原语 (复用渲染器的几何语义) ----

fn draw_legend_circle(
    img: &mut image::RgbaImage,
    cx: i32,
    cy: i32,
    r: i32,
    fill: [u8; 4],
    stroke: [u8; 4],
) {
    for dy in -r..=r {
        for dx in -r..=r {
            let dist = dx * dx + dy * dy;
            if dist > r * r {
                continue;
            }
            let px = (cx + dx) as u32;
            let py = (cy + dy) as u32;
            if px < img.width() && py < img.height() {
                let c = if dist >= (r - 1) * (r - 1) {
                    stroke
                } else {
                    fill
                };
                img.put_pixel(px, py, image::Rgba(c));
            }
        }
    }
}

fn draw_legend_square(
    img: &mut image::RgbaImage,
    cx: i32,
    cy: i32,
    r: i32,
    fill: [u8; 4],
    stroke: [u8; 4],
) {
    for dy in -r..=r {
        for dx in -r..=r {
            let px = (cx + dx) as u32;
            let py = (cy + dy) as u32;
            if px < img.width() && py < img.height() {
                let c = if dx.abs() == r || dy.abs() == r {
                    stroke
                } else {
                    fill
                };
                img.put_pixel(px, py, image::Rgba(c));
            }
        }
    }
}

fn draw_legend_cross(img: &mut image::RgbaImage, cx: i32, cy: i32, r: i32, color: [u8; 4]) {
    for i in -r..=r {
        for w in -1..=1 {
            for (px, py) in [(cx + i, cy + w), (cx + w, cy + i)] {
                if px >= 0 && py >= 0 && (px as u32) < img.width() && (py as u32) < img.height() {
                    img.put_pixel(px as u32, py as u32, image::Rgba(color));
                }
            }
        }
    }
}

fn draw_legend_x(img: &mut image::RgbaImage, cx: i32, cy: i32, r: i32, color: [u8; 4]) {
    for i in -r..=r {
        for w in -1..=1 {
            for (px, py) in [(cx + i, cy + i + w), (cx + i, cy - i + w)] {
                if px >= 0 && py >= 0 && (px as u32) < img.width() && (py as u32) < img.height() {
                    img.put_pixel(px as u32, py as u32, image::Rgba(color));
                }
            }
        }
    }
}

fn draw_legend_triangle(
    img: &mut image::RgbaImage,
    cx: i32,
    cy: i32,
    r: i32,
    fill: [u8; 4],
    stroke: [u8; 4],
) {
    let pts = [(cx, cy - r), (cx - r, cy + r), (cx + r, cy + r)];
    // 边框。
    for i in 0..3 {
        let j = (i + 1) % 3;
        draw_legend_line(img, pts[i], pts[j], stroke);
    }
    // 扫描线填充。
    let min_y = pts.iter().map(|p| p.1).min().unwrap_or(0);
    let max_y = pts.iter().map(|p| p.1).max().unwrap_or(0);
    for y in min_y..=max_y {
        let mut xs: Vec<i32> = Vec::new();
        for i in 0..3 {
            let j = (i + 1) % 3;
            let (x1, y1) = pts[i];
            let (x2, y2) = pts[j];
            if ((y1 <= y && y2 > y) || (y2 <= y && y1 > y)) && y1 != y2 {
                let x = x1 as f64 + (y - y1) as f64 / (y2 - y1) as f64 * (x2 - x1) as f64;
                xs.push(x as i32);
            }
        }
        xs.sort();
        for pair in xs.chunks(2) {
            if pair.len() == 2 {
                for x in pair[0]..=pair[1] {
                    if x >= 0 && y >= 0 && (x as u32) < img.width() && (y as u32) < img.height() {
                        img.put_pixel(x as u32, y as u32, image::Rgba(fill));
                    }
                }
            }
        }
    }
}

fn draw_legend_line(img: &mut image::RgbaImage, a: (i32, i32), b: (i32, i32), color: [u8; 4]) {
    let dx = (b.0 - a.0).abs();
    let dy = -(b.1 - a.1).abs();
    let sx = if a.0 < b.0 { 1 } else { -1 };
    let sy = if a.1 < b.1 { 1 } else { -1 };
    let mut err = dx + dy;
    let (mut x, mut y) = a;
    loop {
        if x >= 0 && y >= 0 && (x as u32) < img.width() && (y as u32) < img.height() {
            img.put_pixel(x as u32, y as u32, image::Rgba(color));
        }
        if x == b.0 && y == b.1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

fn get_layer_rules(
    request: &WmsRequest,
    styles: &std::collections::HashMap<String, String>,
    layer: &crate::models::Layer,
) -> Vec<ParsedRule> {
    let sld_xml = request.sld_body.clone().or_else(|| {
        let style_name = layer
            .styles
            .first()
            .map(|s| &s.name)
            .cloned()
            .unwrap_or_default();
        styles.get(&style_name).cloned()
    });
    match sld_xml {
        Some(xml) => sld_parser::parse_sld(&xml),
        None => sld_parser::parse_sld(&sld_parser::default_sld(&layer.name)),
    }
}

fn parse_color_opt(color: &str) -> Option<[u8; 4]> {
    if let Some(hex) = color.strip_prefix('#') {
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
                    return GeoJsonGeometry::Point {
                        coordinates: vec![x, y],
                    };
                }
            }
            geom.clone()
        },
        GeoJsonGeometry::LineString { coordinates } => {
            let projected: Vec<Vec<f64>> = coordinates
                .iter()
                .filter_map(|c| {
                    if c.len() >= 2 {
                        transformer
                            .transform_point(c[0], c[1])
                            .ok()
                            .map(|(x, y)| vec![x, y])
                    } else {
                        None
                    }
                })
                .collect();
            if projected.len() == coordinates.len() {
                GeoJsonGeometry::LineString {
                    coordinates: projected,
                }
            } else {
                geom.clone()
            }
        },
        GeoJsonGeometry::Polygon { coordinates } => {
            let projected: Vec<Vec<Vec<f64>>> = coordinates
                .iter()
                .map(|ring| {
                    ring.iter()
                        .filter_map(|c| {
                            if c.len() >= 2 {
                                transformer
                                    .transform_point(c[0], c[1])
                                    .ok()
                                    .map(|(x, y)| vec![x, y])
                            } else {
                                None
                            }
                        })
                        .collect()
                })
                .collect();
            if projected.len() == coordinates.len()
                && projected
                    .iter()
                    .zip(coordinates.iter())
                    .all(|(p, o)| p.len() == o.len())
            {
                GeoJsonGeometry::Polygon {
                    coordinates: projected,
                }
            } else {
                geom.clone()
            }
        },
        _ => geom.clone(),
    }
}

fn calculate_openlayers_zoom(bounds: &Bounds, crs: &str) -> f64 {
    let world_width = match crs {
        "EPSG:3857" | "3857" | "EPSG:900913" | "900913" => 20037508.34 * 2.0,
        _ => 360.0,
    };
    let range = (bounds.maxx - bounds.minx).max(bounds.maxy - bounds.miny);
    if range <= 0.0 {
        return 1.0;
    }
    // ImageWMS 不需要匹配瓦片网格，直接计算合适的缩放级别
    let zoom = (world_width / range).log2().clamp(0.0, 20.0);
    // 减少 0.5 让视图稍微缩小，确保数据完整显示
    (zoom - 0.5).max(0.0)
}

fn calculate_scale_denom(bounds: &Bounds, width: u32, height: u32, crs: &str) -> f64 {
    let res_x = (bounds.maxx - bounds.minx) / width as f64;
    let res_y = (bounds.maxy - bounds.miny) / height as f64;
    let ground_res = res_x.max(res_y);
    const PIXEL_SIZE: f64 = 0.00028;
    match crs {
        "EPSG:3857" | "3857" | "EPSG:900913" | "900913" => ground_res / PIXEL_SIZE,
        _ => {
            let center_lat = (bounds.miny + bounds.maxy) / 2.0;
            let meters_per_degree = 111319.5 * center_lat.to_radians().cos().abs().max(0.01);
            ground_res * meters_per_degree / PIXEL_SIZE
        },
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
    if time_str.is_empty() {
        return;
    }

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
    let ranges: Vec<(Option<chrono::NaiveDateTime>, Option<chrono::NaiveDateTime>)> =
        if time_str.contains('/') {
            // 范围格式
            time_str
                .split('/')
                .map(|part| {
                    let t = parse_time(part.trim());
                    (t, t)
                })
                .collect::<Vec<_>>()
                .chunks(2)
                .filter_map(|chunk| {
                    if chunk.len() == 2 {
                        Some((chunk[0].0, chunk[1].0))
                    } else if chunk.len() == 1 {
                        Some((chunk[0].0, chunk[0].0))
                    } else {
                        None
                    }
                })
                .collect()
        } else {
            // 逗号分隔或单个值
            time_str
                .split(',')
                .filter_map(|s| {
                    let t = parse_time(s.trim());
                    t.map(|t| (Some(t), Some(t)))
                })
                .collect()
        };

    if ranges.is_empty() {
        return;
    }

    // 查找要素中的时间属性
    let time_keys = ["time", "datetime", "date", "timestamp", "t"];
    features.retain(|f| {
        for key in &time_keys {
            if let Some(val) = f.properties.get(*key) {
                let val_str = val.to_string();
                if let Some(ft) = parse_time(&val_str) {
                    // 检查是否在任意一个时间范围内
                    return ranges.iter().any(|(start, end)| match (start, end) {
                        (Some(s), Some(e)) => ft >= *s && ft <= *e,
                        (Some(s), None) => ft >= *s,
                        (None, Some(e)) => ft <= *e,
                        (None, None) => true,
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
    if elev_str.is_empty() {
        return;
    }

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
        elev_str
            .split(',')
            .filter_map(|s| {
                let v = s.trim().parse::<f64>().ok();
                v.map(|v| (Some(v), Some(v)))
            })
            .collect()
    };

    if ranges.is_empty() {
        return;
    }

    features.retain(|f| {
        for key in &elev_keys {
            if let Some(val) = f.properties.get(*key) {
                let val_str = val.to_string();
                if let Ok(fe) = val_str.parse::<f64>() {
                    return ranges.iter().any(|(start, end)| match (start, end) {
                        (Some(s), Some(e)) => fe >= *s && fe <= *e,
                        (Some(s), None) => fe >= *s,
                        (None, Some(e)) => fe <= *e,
                        (None, None) => true,
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
    use crate::utils::cascaded::{extract_cascaded_config, fetch_cascaded_map, CascadedResilience};

    let conn = meta
        .connection
        .as_ref()
        .ok_or_else(|| GeoServerError::BadRequest("级联 WMS 缺少连接配置".to_string()))?;

    let config = extract_cascaded_config(conn)
        .ok_or_else(|| GeoServerError::BadRequest("无法解析级联 WMS 配置".to_string()))?;

    // 级联韧性参数来自 [server] 配置 (重试 + 指数退避); 熔断器由
    // state.cascaded_circuits 持有 (按上游 URL 隔离, 配置于 AppState 初始化)
    let resilience = CascadedResilience {
        max_retries: state.config.server.cascaded_max_retries,
        retry_base_ms: state.config.server.cascaded_retry_base_ms,
    };

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

    let bbox_str = format!(
        "{},{},{},{}",
        context.bounds.minx, context.bounds.miny, context.bounds.maxx, context.bounds.maxy
    );

    let srs = &context.output_crs;
    let style: Option<&str> = None; // 暂不使用样式

    // 收集请求级厂商参数, 透传到上游 WMS (CQL_FILTER / TIME / ELEVATION / ENV / ANGLE / FEATUREID)
    let mut passthrough: HashMap<String, String> = HashMap::new();
    if let Some(cql) = context.cql_filter.as_ref().filter(|c| !c.trim().is_empty()) {
        passthrough.insert("CQL_FILTER".to_string(), cql.clone());
    }
    if let Some(time) = context.time.as_ref().filter(|t| !t.trim().is_empty()) {
        passthrough.insert("TIME".to_string(), time.clone());
    }
    if let Some(elev) = context.elevation.as_ref().filter(|e| !e.trim().is_empty()) {
        passthrough.insert("ELEVATION".to_string(), elev.clone());
    }
    if let Some(env) = &context.env {
        if !env.is_empty() {
            let env_str = env
                .iter()
                .map(|(k, v)| format!("{}:'{}'", k, v))
                .collect::<Vec<_>>()
                .join(";");
            passthrough.insert("ENV".to_string(), env_str);
        }
    }
    if let Some(angle) = context.angle {
        passthrough.insert("ANGLE".to_string(), angle.to_string());
    }
    if let Some(fids) = context.feature_id.as_ref().filter(|f| !f.is_empty()) {
        passthrough.insert("FEATUREID".to_string(), fids.join(","));
    }

    match fetch_cascaded_map(
        &config,
        &resilience,
        Some(state.cascaded_circuits.as_ref()),
        &bbox_str,
        context.width,
        context.height,
        remote_format,
        srs,
        style,
        context.transparent,
        &passthrough,
    )
    .await
    {
        Ok((bytes, content_type)) => {
            state
                .request_count
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Ok(HttpResponse::Ok()
                .content_type(content_type.as_str())
                .body(bytes))
        },
        Err(e) => {
            warn!("[Cascaded] 代理请求失败: {}", e);
            Err(GeoServerError::ServiceError(format!(
                "级联 WMS 请求失败: {}",
                e
            )))
        },
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

/// HTML 转义 (GetFeatureInfo 的 text/html 输出, 防 XSS)。
fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// 绕点 (cx, cy) 旋转坐标 (角度制, 逆时针为正, 与 GeoServer ANGLE 语义一致)。
fn rotate_coord(x: f64, y: f64, cx: f64, cy: f64, angle_deg: f64) -> (f64, f64) {
    let rad = angle_deg.to_radians();
    let cos = rad.cos();
    let sin = rad.sin();
    let dx = x - cx;
    let dy = y - cy;
    (cx + dx * cos - dy * sin, cy + dx * sin + dy * cos)
}

/// 递归旋转 GeoJSON 几何的所有坐标。
fn rotate_geometry(geom: &GeoJsonGeometry, cx: f64, cy: f64, angle: f64) -> GeoJsonGeometry {
    match geom {
        GeoJsonGeometry::Point { coordinates } => {
            if coordinates.len() >= 2 {
                let (x, y) = rotate_coord(coordinates[0], coordinates[1], cx, cy, angle);
                GeoJsonGeometry::Point {
                    coordinates: vec![x, y],
                }
            } else {
                geom.clone()
            }
        },
        GeoJsonGeometry::MultiPoint { coordinates } => GeoJsonGeometry::MultiPoint {
            coordinates: rotate_coords(coordinates, cx, cy, angle),
        },
        GeoJsonGeometry::LineString { coordinates } => GeoJsonGeometry::LineString {
            coordinates: rotate_coords(coordinates, cx, cy, angle),
        },
        GeoJsonGeometry::Polygon { coordinates } => GeoJsonGeometry::Polygon {
            coordinates: coordinates
                .iter()
                .map(|ring| rotate_coords(ring, cx, cy, angle))
                .collect(),
        },
        GeoJsonGeometry::MultiLineString { coordinates } => GeoJsonGeometry::MultiLineString {
            coordinates: coordinates
                .iter()
                .map(|ring| rotate_coords(ring, cx, cy, angle))
                .collect(),
        },
        GeoJsonGeometry::MultiPolygon { coordinates } => GeoJsonGeometry::MultiPolygon {
            coordinates: coordinates
                .iter()
                .map(|poly| {
                    poly.iter()
                        .map(|ring| rotate_coords(ring, cx, cy, angle))
                        .collect()
                })
                .collect(),
        },
        GeoJsonGeometry::GeometryCollection { geometries } => GeoJsonGeometry::GeometryCollection {
            geometries: geometries
                .iter()
                .map(|g| rotate_geometry(g, cx, cy, angle))
                .collect(),
        },
    }
}

/// 旋转一组坐标点。
fn rotate_coords(coords: &[Vec<f64>], cx: f64, cy: f64, angle: f64) -> Vec<Vec<f64>> {
    coords
        .iter()
        .map(|c| {
            if c.len() >= 2 {
                let (x, y) = rotate_coord(c[0], c[1], cx, cy, angle);
                vec![x, y]
            } else {
                c.clone()
            }
        })
        .collect()
}

/// 将栅格图像裁剪 + 缩放为与请求 bbox 对齐的瓦片图像。
///
/// 栅格覆盖的边界 (EPSG:4326) 与请求 bbox 求交, 相交区域按比例映射到
/// 输出图像的对应像素区域。返回与输出尺寸一致的 RGBA 图像 (未相交区域
/// 保持透明), 便于直接合成到地图底图上。
pub(crate) fn render_raster_to_map(
    raster: &image::RgbaImage,
    raster_bounds: &Bounds,
    map_bounds: &Bounds,
    map_width: u32,
    map_height: u32,
) -> Option<image::RgbaImage> {
    // 无相交 → 不绘制。
    if map_bounds.minx >= raster_bounds.maxx
        || map_bounds.maxx <= raster_bounds.minx
        || map_bounds.miny >= raster_bounds.maxy
        || map_bounds.maxy <= raster_bounds.miny
    {
        return None;
    }

    // 相交区域 (地理坐标)。
    let inter_minx = map_bounds.minx.max(raster_bounds.minx);
    let inter_maxx = map_bounds.maxx.min(raster_bounds.maxx);
    let inter_miny = map_bounds.miny.max(raster_bounds.miny);
    let inter_maxy = map_bounds.maxy.min(raster_bounds.maxy);

    // 相交区域在输出图像中的像素范围。
    let map_range_x = map_bounds.maxx - map_bounds.minx;
    let map_range_y = map_bounds.maxy - map_bounds.miny;
    if map_range_x <= 0.0 || map_range_y <= 0.0 {
        return None;
    }
    let dst_x0 = ((inter_minx - map_bounds.minx) / map_range_x * map_width as f64) as i64;
    let dst_x1 = ((inter_maxx - map_bounds.minx) / map_range_x * map_width as f64) as i64;
    // 屏幕 y 轴向下, 地理 y 轴向上。
    let dst_y0 = ((map_bounds.maxy - inter_maxy) / map_range_y * map_height as f64) as i64;
    let dst_y1 = ((map_bounds.maxy - inter_miny) / map_range_y * map_height as f64) as i64;
    let dst_w = (dst_x1 - dst_x0).max(1) as u32;
    let dst_h = (dst_y1 - dst_y0).max(1) as u32;

    // 源栅格中的相交区域。
    let raster_range_x = raster_bounds.maxx - raster_bounds.minx;
    let raster_range_y = raster_bounds.maxy - raster_bounds.miny;
    if raster_range_x <= 0.0 || raster_range_y <= 0.0 {
        return None;
    }
    let src_x0 =
        ((inter_minx - raster_bounds.minx) / raster_range_x * raster.width() as f64).floor() as u32;
    let src_x1 =
        ((inter_maxx - raster_bounds.minx) / raster_range_x * raster.width() as f64).ceil() as u32;
    let src_y0 = ((raster_bounds.maxy - inter_maxy) / raster_range_y * raster.height() as f64)
        .floor() as u32;
    let src_y1 =
        ((raster_bounds.maxy - inter_miny) / raster_range_y * raster.height() as f64).ceil() as u32;
    let src_w = (src_x1 - src_x0)
        .max(1)
        .min(raster.width() - src_x0.min(raster.width() - 1));
    let src_h = (src_y1 - src_y0)
        .max(1)
        .min(raster.height() - src_y0.min(raster.height() - 1));

    let cropped = image::imageops::crop_imm(raster, src_x0, src_y0, src_w, src_h).to_image();
    let resized = if dst_w != cropped.width() || dst_h != cropped.height() {
        image::imageops::resize(
            &cropped,
            dst_w,
            dst_h,
            image::imageops::FilterType::Triangle,
        )
    } else {
        cropped
    };

    // 粘贴到输出图像的正确位置。
    let mut out = image::RgbaImage::new(map_width, map_height);
    let ox = dst_x0.max(0) as u32;
    let oy = dst_y0.max(0) as u32;
    let rw = resized.width().min(map_width - ox);
    let rh = resized.height().min(map_height - oy);
    if rw == 0 || rh == 0 {
        return None;
    }
    image::imageops::overlay(&mut out, &resized, ox as i64, oy as i64);
    let _ = (rw, rh);
    Some(out)
}

/// 将 `src` 以 alpha 合成方式覆盖到 `dst` 的 (x, y) 处 (source-over)。
fn composite_image(dst: &mut image::RgbaImage, src: &image::RgbaImage, x: i64, y: i64) {
    for (sy, _dy) in (0..src.height()).enumerate() {
        let dy = y + sy as i64;
        if dy < 0 || dy >= dst.height() as i64 {
            continue;
        }
        for (sx, _dx) in (0..src.width()).enumerate() {
            let dx = x + sx as i64;
            if dx < 0 || dx >= dst.width() as i64 {
                continue;
            }
            let fg = src.get_pixel(sx as u32, sy as u32).0;
            let a = fg[3] as f32 / 255.0;
            if a <= 0.0 {
                continue;
            }
            if a >= 1.0 {
                dst.put_pixel(dx as u32, dy as u32, image::Rgba(fg));
                continue;
            }
            let bg = dst.get_pixel(dx as u32, dy as u32).0;
            let b = bg[3] as f32 / 255.0;
            let out_a = a + b * (1.0 - a);
            if out_a <= 0.0 {
                continue;
            }
            let blend = |f: u8, g: u8| -> u8 {
                ((f as f32 * a + g as f32 * b * (1.0 - a)) / out_a).round() as u8
            };
            dst.put_pixel(
                dx as u32,
                dy as u32,
                image::Rgba([
                    blend(fg[0], bg[0]),
                    blend(fg[1], bg[1]),
                    blend(fg[2], bg[2]),
                    (out_a * 255.0).round() as u8,
                ]),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_raster_to_map_full_overlap() {
        // 4x4 red raster covering the whole map area.
        let mut raster = RgbaImage::new(4, 4);
        for p in raster.pixels_mut() {
            *p = image::Rgba([255, 0, 0, 255]);
        }
        let raster_bounds = Bounds::new(0.0, 0.0, 10.0, 10.0);
        let map_bounds = Bounds::new(0.0, 0.0, 10.0, 10.0);
        let out = render_raster_to_map(&raster, &raster_bounds, &map_bounds, 100, 100)
            .expect("raster tile");
        assert_eq!(out.dimensions(), (100, 100));
        // Center pixel must be red.
        let px = out.get_pixel(50, 50).0;
        assert_eq!((px[0], px[3]), (255, 255), "raster must be composited");
    }

    #[test]
    fn test_render_raster_to_map_partial_overlap() {
        let mut raster = RgbaImage::new(4, 4);
        for p in raster.pixels_mut() {
            *p = image::Rgba([0, 0, 255, 255]);
        }
        // Raster covers [0,10]x[0,10]; map asks for [5,15]x[5,15] →
        // only the left/bottom quarter of the output is covered.
        let raster_bounds = Bounds::new(0.0, 0.0, 10.0, 10.0);
        let map_bounds = Bounds::new(5.0, 5.0, 15.0, 15.0);
        let out = render_raster_to_map(&raster, &raster_bounds, &map_bounds, 100, 100)
            .expect("raster tile");
        // Top-right quadrant (map coords 12.5,12.5 → pixel 75,25): outside raster → transparent.
        let outside = out.get_pixel(75, 25).0;
        assert_eq!(outside[3], 0, "outside raster must stay transparent");
        // Bottom-left quadrant (map coords 7.5,7.5 → pixel 25,75): inside raster → blue.
        let inside = out.get_pixel(25, 75).0;
        assert_eq!((inside[0], inside[2], inside[3]), (0, 255, 255));
    }

    #[test]
    fn test_render_raster_to_map_no_overlap() {
        let raster = RgbaImage::new(4, 4);
        let raster_bounds = Bounds::new(0.0, 0.0, 10.0, 10.0);
        let map_bounds = Bounds::new(100.0, 100.0, 110.0, 110.0);
        assert!(render_raster_to_map(&raster, &raster_bounds, &map_bounds, 100, 100).is_none());
    }

    #[test]
    fn test_composite_image_source_over() {
        let mut dst = RgbaImage::new(2, 1);
        dst.put_pixel(0, 0, image::Rgba([255, 255, 255, 255]));
        let mut src = RgbaImage::new(1, 1);
        src.put_pixel(0, 0, image::Rgba([255, 0, 0, 128]));
        composite_image(&mut dst, &src, 0, 0);
        let px = dst.get_pixel(0, 0).0;
        assert_eq!(px[0], 255);
        assert_eq!(px[1], 127);
        assert_eq!(px[3], 255);
    }

    #[test]
    fn test_rotate_coord_90_degrees() {
        // 绕原点旋转 (1,0) 逆时针 90° → (0,1)。
        let (x, y) = rotate_coord(1.0, 0.0, 0.0, 0.0, 90.0);
        assert!((x - 0.0).abs() < 1e-9, "x 应 ≈ 0, 实际 {}", x);
        assert!((y - 1.0).abs() < 1e-9, "y 应 ≈ 1, 实际 {}", y);
        // 旋转 360° 回到原位。
        let (x2, y2) = rotate_coord(3.0, 4.0, 0.0, 0.0, 360.0);
        assert!((x2 - 3.0).abs() < 1e-9 && (y2 - 4.0).abs() < 1e-9);
    }

    #[test]
    fn test_rotate_geometry_all_types() {
        let pt = GeoJsonGeometry::Point {
            coordinates: vec![1.0, 0.0],
        };
        // 绕 (0,0) 旋转 90°: 点 (1,0) → (0,1)。
        let rp = rotate_geometry(&pt, 0.0, 0.0, 90.0);
        if let GeoJsonGeometry::Point { coordinates } = rp {
            assert!((coordinates[0] - 0.0).abs() < 1e-9);
            assert!((coordinates[1] - 1.0).abs() < 1e-9);
        } else {
            panic!("应为 Point");
        }

        // MultiPoint / LineString / Polygon 递归旋转。
        let mp = GeoJsonGeometry::MultiPoint {
            coordinates: vec![vec![1.0, 0.0], vec![0.0, 1.0]],
        };
        let rm = rotate_geometry(&mp, 0.0, 0.0, 90.0);
        match rm {
            GeoJsonGeometry::MultiPoint { coordinates } => {
                assert_eq!(coordinates.len(), 2);
                assert!((coordinates[0][0] - 0.0).abs() < 1e-9);
            },
            _ => panic!("应为 MultiPoint"),
        }

        let poly = GeoJsonGeometry::Polygon {
            coordinates: vec![vec![
                vec![1.0, 0.0],
                vec![1.0, 1.0],
                vec![0.0, 1.0],
                vec![1.0, 0.0],
            ]],
        };
        let rpoly = rotate_geometry(&poly, 0.0, 0.0, 90.0);
        match rpoly {
            GeoJsonGeometry::Polygon { coordinates } => {
                assert_eq!(coordinates.len(), 1);
                assert_eq!(coordinates[0].len(), 4);
                assert!((coordinates[0][0][0] - 0.0).abs() < 1e-9);
                assert!((coordinates[0][0][1] - 1.0).abs() < 1e-9);
            },
            _ => panic!("应为 Polygon"),
        }
    }

    #[test]
    fn test_escape_html_prevents_xss() {
        // GetFeatureInfo text/html 输出必须转义属性值, 防止脚本注入。
        let malicious = r#"<script>alert('x')</script>&"'<>"#;
        let escaped = escape_html(malicious);
        assert!(!escaped.contains('<'), "不应保留 '<': {}", escaped);
        assert!(!escaped.contains('>'), "不应保留 '>'");
        assert!(!escaped.contains("\"<script"), "不应保留原始 <script>");
        assert!(escaped.contains("&lt;script&gt;"), "应转义 <script>");
        assert!(escaped.contains("&amp;"), "应转义 &");
        assert!(escaped.contains("&#39;"), "应转义单引号");
        assert!(escaped.contains("&quot;"), "应转义双引号");
        // 纯文本保持不变。
        assert_eq!(escape_html("plain text 123"), "plain text 123");
    }
}
