use crate::error::GeoServerError;
use crate::models::DataSourceType;
use crate::services::wfs::{self, DescribeFeatureTypeResponse, WfsCapabilities, WfsRequest};
use crate::state::AppState;
use crate::utils::gml::{escape_xml, geometry_to_gml, properties_to_gml};
use actix_web::{web, HttpRequest, HttpResponse};
use quick_xml::se::to_string;

// ---- GET handler ----

pub async fn handle_wfs_request(
    req: HttpRequest,
    query: web::Query<Vec<(String, String)>>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let params = query.as_ref();
    let wfs_request = wfs::parse_wfs_request(params)?;

    match wfs_request.request {
        wfs::WfsOperation::GetCapabilities => handle_get_capabilities(&state, &wfs_request).await,
        wfs::WfsOperation::DescribeFeatureType => {
            handle_describe_feature_type(&state, &wfs_request).await
        },
        wfs::WfsOperation::GetFeature => handle_get_feature(&state, &wfs_request, &req).await,
        wfs::WfsOperation::GetFeatureWithLock => {
            handle_get_feature_with_lock(&state, &wfs_request, &req).await
        },
        wfs::WfsOperation::LockFeature => handle_lock_feature(&state, &wfs_request, &req).await,
        wfs::WfsOperation::GetPropertyValue => {
            handle_get_property_value(&state, &wfs_request, &req).await
        },
        wfs::WfsOperation::GetGmlObject => handle_get_gml_object(&state, &wfs_request, &req).await,
        // WFS-T 尚未实现 (计划后续支持)
        wfs::WfsOperation::Transaction => Err(GeoServerError::NotImplemented(
            "WFS Transaction is not implemented yet (planned for a later milestone)".to_string(),
        )),
    }
}

// ---- POST handler (解析 KVP body) ----

pub async fn handle_wfs_post_request(
    req: HttpRequest,
    body: String,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    // WFS-T (Transaction XML) 尚未实现 (计划后续支持)
    if body.contains("Transaction") || body.contains("<wfs:") {
        return Err(GeoServerError::NotImplemented(
            "WFS Transaction is not implemented yet (planned for a later milestone)".to_string(),
        ));
    }

    // 按 KVP 解析 POST body
    let params: Vec<(String, String)> = body
        .split('&')
        .filter_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            let k = parts.next()?.to_string();
            let v = parts.next().unwrap_or("").to_string();
            Some((k, v))
        })
        .collect();

    let wfs_request = wfs::parse_wfs_request(&params)?;

    match wfs_request.request {
        wfs::WfsOperation::GetCapabilities => handle_get_capabilities(&state, &wfs_request).await,
        wfs::WfsOperation::DescribeFeatureType => {
            handle_describe_feature_type(&state, &wfs_request).await
        },
        wfs::WfsOperation::GetFeature => handle_get_feature(&state, &wfs_request, &req).await,
        // WFS-T 尚未实现 (计划后续支持)
        wfs::WfsOperation::Transaction => Err(GeoServerError::NotImplemented(
            "WFS Transaction is not implemented yet (planned for a later milestone)".to_string(),
        )),
        _ => Err(GeoServerError::BadRequest(
            "Operation not implemented".to_string(),
        )),
    }
}

// ---- 原有操作处理函数保持不变 ----

/// GeoFence: enforce layer access for a WFS typename (`ws:layer` or `layer`).
async fn enforce_geofence_typename(
    state: &AppState,
    req: &HttpRequest,
    typename: &str,
) -> Result<(), GeoServerError> {
    if !state.config.security.geofence_enabled {
        return Ok(());
    }
    let (workspace, short) = match typename.split_once(':') {
        Some((ws, rest)) => (ws.to_string(), rest.to_string()),
        None => (String::new(), typename.to_string()),
    };
    let layers_lock = state.layers.read().await;
    let resolved = layers_lock
        .iter()
        .find(|l| l.name == *typename || (l.workspace == workspace && l.name == short));
    match resolved {
        Some(layer) => {
            crate::utils::geofence::enforce_layer_access(
                state,
                req,
                &layer.workspace,
                &layer.store,
                &layer.name,
                "read",
            )
            .await
        },
        None => {
            crate::utils::geofence::enforce_layer_access(state, req, &workspace, "", &short, "read")
                .await
        },
    }
}

async fn handle_get_capabilities(
    state: &AppState,
    _request: &WfsRequest,
) -> Result<HttpResponse, GeoServerError> {
    let base_url = format!(
        "http://{}:{}",
        state.config.server.host, state.config.server.port
    );
    let capabilities = WfsCapabilities::new(&base_url);

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

async fn handle_describe_feature_type(
    state: &AppState,
    request: &WfsRequest,
) -> Result<HttpResponse, GeoServerError> {
    let type_names = request
        .type_names
        .as_ref()
        .ok_or_else(|| GeoServerError::BadRequest("TYPENAME parameter is required".to_string()))?;

    let type_name = type_names.first().ok_or_else(|| {
        GeoServerError::BadRequest("At least one TYPENAME is required".to_string())
    })?;

    // Default fallback schema (used when the layer has no resolvable store).
    let mut properties: Vec<(String, String)> = vec![
        ("id".to_string(), "xsd:string".to_string()),
        ("name".to_string(), "xsd:string".to_string()),
        ("geometry".to_string(), "xsd:string".to_string()),
    ];

    // GeoPackage layers report their real typed columns via WFS
    // DescribeFeatureType (mirrors the reference GeoServer mapping:
    // INTEGER→xsd:long, REAL→xsd:double, BOOLEAN→xsd:boolean, geometry→
    // gml:GeometryPropertyType).
    if let Some(store) = &state.store {
        let layer = store.get_layer(type_name).await.ok().flatten();
        if let Some(layer) = layer {
            if let Ok(Some(data_source)) = store.get_data_source(&layer.store).await {
                if data_source.data_source_type == DataSourceType::Geopackage {
                    let table_name = layer.native_name.clone();
                    let file_path = data_source
                        .connection
                        .as_ref()
                        .and_then(|c| c.file_path.clone());
                    if let (Some(table_name), Some(file_path)) = (table_name, file_path) {
                        if let Ok(columns) = crate::utils::geopackage::geopackage_table_columns(
                            &file_path,
                            &table_name,
                        ) {
                            // Skip the internal autoincrement `id` primary key,
                            // mirroring GeoServer (fid is not a regular attribute).
                            properties = columns
                                .into_iter()
                                .filter(|(name, _)| name != "id")
                                .map(|(name, ty)| {
                                    let xsd = wfs::sqlite_type_to_xsd(&name, &ty);
                                    (name, xsd.to_string())
                                })
                                .collect();
                        }
                    }
                }
            }
        }
    }

    let response = DescribeFeatureTypeResponse::new(type_name, properties);

    let xml = to_string(&response).map_err(|e| {
        GeoServerError::ServiceError(format!("Failed to serialize response: {}", e))
    })?;

    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema">
{}
</xs:schema>"#,
        xml
    );

    Ok(HttpResponse::Ok().content_type("application/xml").body(xml))
}

async fn handle_get_feature(
    state: &AppState,
    request: &WfsRequest,
    req: &HttpRequest,
) -> Result<HttpResponse, GeoServerError> {
    let type_names = request
        .type_names
        .as_ref()
        .ok_or_else(|| GeoServerError::BadRequest("TYPENAME parameter is required".to_string()))?;

    // GeoFence: per-request layer access (opt-in via geofence_enabled).
    for tn in type_names {
        enforce_geofence_typename(state, req, tn).await?;
    }

    let output_format = request
        .output_format
        .as_deref()
        .unwrap_or("text/xml; subtype=gml/3.1.1");

    let all_features = query_features(state, request).await?;

    let response = crate::models::FeatureCollection::new(all_features);

    // GeoJSON 输出
    if output_format.contains("json") || output_format.contains("geojson") {
        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| GeoServerError::ServiceError(e.to_string()))?;

        return Ok(HttpResponse::Ok()
            .content_type("application/json")
            .body(json));
    }

    // CSV 输出
    if output_format.contains("csv") {
        let csv = generate_csv_response(&response);
        return Ok(HttpResponse::Ok()
            .content_type("text/csv; charset=utf-8")
            .insert_header(("Content-Disposition", "attachment; filename=features.csv"))
            .body(csv));
    }

    let output_format_lower = output_format.to_lowercase();
    let type_name = type_names.first().map(|s| s.as_str()).unwrap_or("features");

    // KML 输出 (OGC KML 2.2)
    if output_format_lower.contains("kml") {
        let kml = generate_kml_response(&response, type_name);
        return Ok(HttpResponse::Ok()
            .content_type("application/vnd.google-earth.kml+xml")
            .insert_header(("Content-Disposition", "attachment; filename=features.kml"))
            .body(kml));
    }

    // Shapefile 输出 (SHAPE-ZIP)
    if output_format_lower.contains("shape") {
        let base = type_name.replace(':', "_");
        let pkg = crate::utils::shapefile_export::features_to_shapefile(&response.features)
            .map_err(GeoServerError::ServiceError)?;
        let zip = crate::utils::shapefile_export::zip_shapefile_package(&pkg, &base)
            .map_err(GeoServerError::ServiceError)?;
        return Ok(HttpResponse::Ok()
            .content_type("application/zip")
            .insert_header((
                "Content-Disposition",
                format!("attachment; filename={}.zip", base),
            ))
            .body(zip));
    }

    // GML 输出
    let (gml, content_type) = if output_format.contains("gml/2") {
        (
            generate_gml2_response(&response, None),
            "application/gml+xml; version=2.1.2",
        )
    } else if output_format.contains("gml/3.2") {
        (
            generate_gml32_response(&response, None),
            "application/gml+xml; version=3.2",
        )
    } else {
        // 默认 GML 3.1.1
        (
            generate_gml_response(&response, output_format, None),
            "application/gml+xml; version=3.1",
        )
    };

    Ok(HttpResponse::Ok().content_type(content_type).body(gml))
}

/// 查询要素 (仅过滤, 不序列化): 供 GetFeature / GetFeatureWithLock /
/// LockFeature / GetPropertyValue 复用。按 TYPENAMES 逐层查询并应用
/// FILTER / BBOX / FEATUREID / MAXFEATURES / STARTINDEX。
async fn query_features(
    state: &AppState,
    request: &WfsRequest,
) -> Result<Vec<crate::models::Feature>, GeoServerError> {
    let type_names = request
        .type_names
        .as_ref()
        .ok_or_else(|| GeoServerError::BadRequest("TYPENAME parameter is required".to_string()))?;

    let mut all_features = Vec::new();
    for type_name in type_names {
        // 从数据源只读查询要素 (强制数据源, 无数据源图层返回空)
        let features = match crate::handlers::features::query_layer_features(
            state, type_name, None, None, None,
        )
        .await
        {
            Ok(f) => f,
            // 图层不存在时跳过 (与原先 Option 语义一致)
            Err(GeoServerError::NotFound(_)) => Vec::new(),
            Err(e) => return Err(e),
        };
        let mut filtered_features = features;

        if let Some(ref filter) = request.filter {
            filtered_features.retain(|f| wfs::validate_filter(f, filter));
        }

        if let Some(ref bbox) = request.bbox {
            let bounds = bbox.to_bounds();
            filtered_features.retain(|f| match &f.geometry {
                crate::models::GeoJsonGeometry::Point { coordinates } => {
                    if coordinates.len() >= 2 {
                        bounds.contains(coordinates[0], coordinates[1])
                    } else {
                        false
                    }
                },
                _ => true,
            });
        }

        if let Some(ref feature_ids) = request.feature_id {
            filtered_features.retain(|f| feature_ids.contains(&f.id));
        }

        if let Some(max_features) = request.max_features {
            filtered_features.truncate(max_features as usize);
        }

        if let Some(start_index) = request.start_index {
            if (start_index as usize) < filtered_features.len() {
                filtered_features = filtered_features
                    .into_iter()
                    .skip(start_index as usize)
                    .collect();
            } else {
                filtered_features.clear();
            }
        }

        all_features.extend(filtered_features);
    }

    Ok(all_features)
}

/// 解析锁 TTL: EXPIRY (分钟) 优先, 否则用 `[wfs] lock_timeout_secs` (0 = 永不过期)。
fn lock_ttl(request: &WfsRequest, default_secs: u64) -> Option<std::time::Duration> {
    if let Some(expiry) = request.expiry {
        return Some(std::time::Duration::from_secs(expiry.saturating_mul(60)));
    }
    if default_secs == 0 {
        None
    } else {
        Some(std::time::Duration::from_secs(default_secs))
    }
}

/// 查询单个图层的要素 id (应用 FILTER / BBOX / FEATUREID), 供锁定操作逐层使用。
async fn query_layer_fids(
    state: &AppState,
    type_name: &str,
    request: &WfsRequest,
) -> Result<Vec<String>, GeoServerError> {
    let features =
        match crate::handlers::features::query_layer_features(state, type_name, None, None, None)
            .await
        {
            Ok(f) => f,
            Err(GeoServerError::NotFound(_)) => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
    let mut filtered = features;
    if let Some(ref filter) = request.filter {
        filtered.retain(|f| wfs::validate_filter(f, filter));
    }
    if let Some(ref bbox) = request.bbox {
        let bounds = bbox.to_bounds();
        filtered.retain(|f| match &f.geometry {
            crate::models::GeoJsonGeometry::Point { coordinates } => {
                coordinates.len() >= 2 && bounds.contains(coordinates[0], coordinates[1])
            },
            _ => true,
        });
    }
    if let Some(ref feature_ids) = request.feature_id {
        filtered.retain(|f| feature_ids.contains(&f.id));
    }
    Ok(filtered.into_iter().map(|f| f.id).collect())
}

async fn handle_get_feature_with_lock(
    state: &AppState,
    request: &WfsRequest,
    req: &HttpRequest,
) -> Result<HttpResponse, GeoServerError> {
    let type_names = request
        .type_names
        .as_ref()
        .ok_or_else(|| GeoServerError::BadRequest("TYPENAME parameter is required".to_string()))?;

    for tn in type_names {
        enforce_geofence_typename(state, req, tn).await?;
    }

    let output_format = request
        .output_format
        .as_deref()
        .unwrap_or("text/xml; subtype=gml/3.1.1");

    // 逐层加锁 (GetFeatureWithLock 与 GeoServer 一致: 锁空闲要素,
    // 已锁要素跳过, 不使整个请求失败), 然后复用 GetFeature 查询路径。
    let mut lock_id: Option<String> = None;
    let ttl = lock_ttl(request, state.config.wfs.lock_timeout_secs);
    for type_name in type_names {
        let fids = query_layer_fids(state, type_name, request).await?;
        let (lid, _locked, _skipped) = state
            .wfs_locks
            .acquire(type_name, &fids, ttl, false)
            .expect("lockAction=SOME never fails on conflicts");
        if lock_id.is_none() {
            lock_id = Some(lid);
        }
    }
    let all_features = query_features(state, request).await?;

    let response = crate::models::FeatureCollection::new(all_features);
    let lock_id = lock_id.unwrap_or_default();

    // GeoJSON 输出 (附带 lockId 字段)
    if output_format.contains("json") || output_format.contains("geojson") {
        let mut value = serde_json::to_value(&response)
            .map_err(|e| GeoServerError::ServiceError(e.to_string()))?;
        if let Some(obj) = value.as_object_mut() {
            obj.insert(
                "lockId".to_string(),
                serde_json::Value::String(lock_id.clone()),
            );
        }
        return Ok(HttpResponse::Ok()
            .content_type("application/json")
            .body(serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_string())));
    }

    // GML 输出 (响应根元素携带 lockId 属性)
    let (gml, content_type) = if output_format.contains("gml/3.2") {
        (
            generate_gml32_response(&response, Some(&lock_id)),
            "application/gml+xml; version=3.2",
        )
    } else {
        (
            generate_gml_response(&response, output_format, Some(&lock_id)),
            "application/gml+xml; version=3.1",
        )
    };

    Ok(HttpResponse::Ok().content_type(content_type).body(gml))
}

/// WFS LockFeature:
/// - 携带 LOCKID + RELEASEACTION=ALL → 释放该锁
/// - 携带 LOCKID → 续锁
/// - 否则 → 按 TYPENAMES (+FEATUREID/FILTER) 加锁, 返回 lockId
async fn handle_lock_feature(
    state: &AppState,
    request: &WfsRequest,
    req: &HttpRequest,
) -> Result<HttpResponse, GeoServerError> {
    let type_names = request
        .type_names
        .as_ref()
        .ok_or_else(|| GeoServerError::BadRequest("TYPENAME parameter is required".to_string()))?;

    for tn in type_names {
        enforce_geofence_typename(state, req, tn).await?;
    }
    let version = request.version.as_deref().unwrap_or("2.0.0").to_string();
    let ttl = lock_ttl(request, state.config.wfs.lock_timeout_secs);
    let lock_all = !request
        .lock_action
        .as_deref()
        .map(|a| a.eq_ignore_ascii_case("some"))
        .unwrap_or(false);

    // 释放 / 续锁路径 (携带 LOCKID)
    if let Some(lock_id) = request.lock_id.as_deref() {
        if request
            .release_action
            .as_deref()
            .map(|r| r.eq_ignore_ascii_case("all"))
            .unwrap_or(false)
        {
            let mut released = Vec::new();
            for tn in type_names {
                released.extend(state.wfs_locks.release(tn, lock_id));
            }
            return Ok(HttpResponse::Ok()
                .content_type("text/xml")
                .body(lock_feature_response_xml(&version, lock_id, &[], &released)));
        }

        // 续锁: 延长该 lock_id 下所有锁的过期时间
        let mut still_locked = Vec::new();
        for tn in type_names {
            if state.wfs_locks.renew(tn, lock_id, ttl) {
                still_locked.extend(state.wfs_locks.locked_features(tn, lock_id));
            }
        }
        return Ok(HttpResponse::Ok()
            .content_type("text/xml")
            .body(lock_feature_response_xml(
                &version,
                lock_id,
                &still_locked,
                &[],
            )));
    }

    // 加锁路径
    let mut locked = Vec::new();
    let mut skipped = Vec::new();
    let mut lock_id: Option<String> = None;
    for tn in type_names {
        let fids = query_layer_fids(state, tn, request).await?;
        match state.wfs_locks.acquire(tn, &fids, ttl, lock_all) {
            Ok((lid, locked_fids, skipped_fids)) => {
                if lock_id.is_none() {
                    lock_id = Some(lid);
                }
                locked.extend(locked_fids);
                skipped.extend(skipped_fids);
            },
            Err(conflicts) => {
                // lockAction=ALL 且部分要素已被锁: 整个请求失败 (与 GeoServer 一致)
                return Err(GeoServerError::BadRequest(format!(
                    "Could not lock all requested features, already locked: {}",
                    conflicts.join(", ")
                )));
            },
        }
    }
    let lock_id = lock_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    Ok(HttpResponse::Ok()
        .content_type("text/xml")
        .body(lock_feature_response_xml(
            &version, &lock_id, &locked, &skipped,
        )))
}

/// WFS LockFeatureResponse XML (1.1.0 与 2.0.0 命名空间不同)。
fn lock_feature_response_xml(
    version: &str,
    lock_id: &str,
    locked: &[String],
    not_locked: &[String],
) -> String {
    let ns = if version.starts_with("2.") {
        "http://www.opengis.net/wfs/2.0"
    } else {
        "http://www.opengis.net/wfs"
    };
    let fid = |ids: &[String]| -> String {
        ids.iter()
            .map(|f| format!("            <wfs:FeatureId fid=\"{}\"/>\n", escape_xml(f)))
            .collect()
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<wfs:LockFeatureResponse xmlns:wfs="{ns}">
  <wfs:LockId>{lock_id}</wfs:LockId>
  <wfs:FeaturesLocked>
{fid_locked}  </wfs:FeaturesLocked>
  <wfs:FeaturesNotLocked>
{fid_not_locked}  </wfs:FeaturesNotLocked>
</wfs:LockFeatureResponse>"#,
        ns = ns,
        lock_id = escape_xml(lock_id),
        fid_locked = fid(locked),
        fid_not_locked = fid(not_locked),
    )
}

/// WFS 2.0 GetPropertyValue (OGC 09-025r2 §14):
/// 按 FEATUREID / FILTER 选取要素, 返回 PROPERTYNAME 指向的属性值集合
/// (`wfs:ValueCollection`)。PROPERTYNAME 为几何列名时输出 GML 几何。
async fn handle_get_property_value(
    state: &AppState,
    request: &WfsRequest,
    req: &HttpRequest,
) -> Result<HttpResponse, GeoServerError> {
    if let Some(tns) = request.type_names.as_ref() {
        for tn in tns {
            enforce_geofence_typename(state, req, tn).await?;
        }
    }
    let property_name = request
        .property_name
        .as_ref()
        .and_then(|p| p.first())
        .ok_or_else(|| {
            GeoServerError::BadRequest("PROPERTYNAME parameter is required".to_string())
        })?;

    let features = query_features(state, request).await?;

    let is_geometry = matches!(
        property_name.to_lowercase().as_str(),
        "geometry" | "geom" | "the_geom" | "shape" | "wkb_geometry"
    );

    let mut members = String::new();
    for feature in &features {
        members.push_str("        <wfs:member>\n");
        if is_geometry {
            let geom = geometry_to_gml(&feature.geometry, "3.2");
            if geom.is_empty() {
                members.push_str("          <wfs:value/>\n");
            } else {
                members.push_str(&format!("          {}\n", geom));
            }
        } else {
            let value = feature
                .properties
                .get(property_name)
                .map(|v| escape_xml(&v.to_string()))
                .unwrap_or_default();
            members.push_str(&format!(
                "          <feature:{prop}>{value}</feature:{prop}>\n",
                prop = escape_xml(property_name),
                value = value
            ));
        }
        members.push_str("        </wfs:member>\n");
    }

    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<wfs:ValueCollection xmlns:wfs="http://www.opengis.net/wfs/2.0"
                      xmlns:gml="http://www.opengis.net/gml/3.2"
                      xmlns:feature="http://geoserver.org/feature"
                      numberMatched="{total}" numberReturned="{count}">
{members}        </wfs:ValueCollection>"#,
        total = features.len(),
        count = features.len(),
        members = members
    );

    Ok(HttpResponse::Ok()
        .content_type("application/gml+xml; version=3.2")
        .body(xml))
}

/// WFS 2.0 GetGmlObject (OGC 09-025r2 §13):
/// 按 GMLOBJECTID 返回要素对象的 GML 3.2 表示 (`wfs:GMLObjectCollection`)。
/// TYPENAMES 限定搜索图层, 缺省时搜索全部目录图层。
async fn handle_get_gml_object(
    state: &AppState,
    request: &WfsRequest,
    req: &HttpRequest,
) -> Result<HttpResponse, GeoServerError> {
    let object_ids = request.gml_object_id.as_ref().ok_or_else(|| {
        GeoServerError::BadRequest("GMLOBJECTID parameter is required".to_string())
    })?;

    let layers: Vec<String> = match request.type_names.as_ref() {
        Some(tns) => tns.clone(),
        None => state
            .list_layers()
            .await
            .into_iter()
            .map(|l| l.name)
            .collect(),
    };
    for tn in &layers {
        enforce_geofence_typename(state, req, tn).await?;
    }

    let mut members = String::new();
    for layer_name in &layers {
        let features = match crate::handlers::features::query_layer_features(
            state, layer_name, None, None, None,
        )
        .await
        {
            Ok(f) => f,
            Err(GeoServerError::NotFound(_)) => continue,
            Err(e) => return Err(e),
        };
        for feature in features {
            if object_ids.contains(&feature.id) {
                members.push_str(&format!(
                    "        <wfs:member>\n{}\n        </wfs:member>\n",
                    feature_to_gml32(&feature)
                ));
            }
        }
    }

    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<wfs:GMLObjectCollection xmlns:wfs="http://www.opengis.net/wfs/2.0"
                          xmlns:gml="http://www.opengis.net/gml/3.2"
                          xmlns:feature="http://geoserver.org/feature"
                          numberMatched="{total}" numberReturned="{count}">
{members}        </wfs:GMLObjectCollection>"#,
        total = members.len(),
        count = members.len(),
        members = members
    );

    Ok(HttpResponse::Ok()
        .content_type("application/gml+xml; version=3.2")
        .body(xml))
}

// ---- GML 序列化辅助函数 ----

pub(crate) fn generate_gml_response(
    collection: &crate::models::FeatureCollection,
    format: &str,
    lock_id: Option<&str>,
) -> String {
    let gml_version = if format.contains("3.2") {
        "3.2"
    } else {
        "3.1.1"
    };

    let mut features_xml = String::new();
    for feature in &collection.features {
        features_xml.push_str(&format!(
            r#"        <wfs:member>
            <feature:{type_name} gml:id="{id}">
                <feature:geometry>
                    {geometry}
                </feature:geometry>
                {properties}
            </feature:{type_name}>
        </wfs:member>
"#,
            type_name = "Feature",
            id = feature.id,
            geometry = geometry_to_gml(&feature.geometry, gml_version),
            properties = properties_to_gml(&feature.properties)
        ));
    }

    let lock_attr = lock_id
        .map(|id| format!(" lockId=\"{}\"", escape_xml(id)))
        .unwrap_or_default();

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<wfs:FeatureCollection xmlns:wfs="http://www.opengis.net/wfs/{gml_version}"
                        xmlns:gml="http://www.opengis.net/gml/{gml_version}"
                        xmlns:feature="http://geoserver.org/feature"
                        numberMatched="{total}" numberReturned="{count}"{lock_attr}>
{features}        </wfs:FeatureCollection>"#,
        gml_version = gml_version,
        total = collection.total_count,
        count = collection.features.len(),
        lock_attr = lock_attr,
        features = features_xml
    )
}

/// CSV 输出
fn generate_csv_response(collection: &crate::models::FeatureCollection) -> String {
    let mut csv = String::new();
    // 收集所有属性名作为表头
    let mut headers = vec!["id".to_string(), "geometry_type".to_string()];
    for feature in &collection.features {
        for key in feature.properties.keys() {
            if !headers.contains(key) {
                headers.push(key.clone());
            }
        }
    }
    // 表头也可能是用户可控的属性名: 统一做 CSV 引号转义。
    let escaped_headers: Vec<String> = headers.iter().map(|h| csv_escape(h)).collect();
    csv.push_str(&escaped_headers.join(","));
    csv.push('\n');

    for feature in &collection.features {
        let mut row = vec![
            csv_escape(&feature.id),
            format!("\"{:?}\"", std::mem::discriminant(&feature.geometry)),
        ];
        for h in headers.iter().skip(2) {
            let val = match feature.properties.get(h) {
                Some(v) => csv_escape(&v.to_string()),
                None => String::new(),
            };
            row.push(val);
        }
        csv.push_str(&row.join(","));
        csv.push('\n');
    }
    csv
}

/// CSV 字段转义: 含逗号/引号/换行时用双引号包裹, 内部引号翻倍。
fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// GML 2.1.2 输出
fn generate_gml2_response(
    collection: &crate::models::FeatureCollection,
    _lock_id: Option<&str>,
) -> String {
    let mut features_xml = String::new();
    for feature in &collection.features {
        features_xml.push_str(&format!(
            r#"        <wfs:Feature>
            <gml:boundedBy><gml:null>unknown</gml:null></gml:boundedBy>
            <feature:Feature fid="{}">
                <feature:geometry>
                    {}
                </feature:geometry>
                {}
            </feature:Feature>
        </wfs:Feature>
"#,
            feature.id,
            geometry_to_gml2(&feature.geometry),
            properties_to_gml(&feature.properties)
        ));
    }

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<wfs:FeatureCollection xmlns:wfs="http://www.opengis.net/wfs"
                        xmlns:gml="http://www.opengis.net/gml"
                        xmlns:feature="http://geoserver.org/feature"
                        numberOfFeatures="{}">
{}
        </wfs:FeatureCollection>"#,
        collection.features.len(),
        features_xml
    )
}

fn geometry_to_gml2(geometry: &crate::models::GeoJsonGeometry) -> String {
    match geometry {
        crate::models::GeoJsonGeometry::Point { coordinates } if coordinates.len() >= 2 => {
            format!(
                r#"<gml:Point srsName="EPSG:4326"><gml:coordinates>{},{},0</gml:coordinates></gml:Point>"#,
                coordinates[0], coordinates[1]
            )
        },
        _ => String::new(),
    }
}

/// GML 3.2.1 输出
fn generate_gml32_response(
    collection: &crate::models::FeatureCollection,
    lock_id: Option<&str>,
) -> String {
    let mut features_xml = String::new();
    for feature in &collection.features {
        features_xml.push_str(&format!(
            "        <wfs:member>\n{}\n        </wfs:member>\n",
            feature_to_gml32(feature)
        ));
    }

    let lock_attr = lock_id
        .map(|id| format!(" lockId=\"{}\"", escape_xml(id)))
        .unwrap_or_default();

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<wfs:FeatureCollection xmlns:wfs="http://www.opengis.net/wfs/2.0"
                        xmlns:gml="http://www.opengis.net/gml/3.2"
                        xmlns:feature="http://geoserver.org/feature"
                        numberMatched="{total}" numberReturned="{count}"{lock_attr}>
{features}
        </wfs:FeatureCollection>"#,
        total = collection.total_count,
        count = collection.features.len(),
        lock_attr = lock_attr,
        features = features_xml
    )
}

/// 单个要素的 GML 3.2 表示 (`<Feature gml:id="...">` 元素)。
fn feature_to_gml32(feature: &crate::models::Feature) -> String {
    crate::utils::gml::feature_to_gml32(feature)
}

// ---- KML 序列化辅助函数 (OGC KML 2.2) ----

/// KML `SimpleField` type for an attribute (from the first non-null value).
fn kml_field_type(name: &str, features: &[crate::models::Feature]) -> &'static str {
    for feature in features {
        if let Some(v) = feature.properties.get(name) {
            return match v {
                crate::models::PropertyValue::Integer(_) => "int",
                crate::models::PropertyValue::Number(_) => "float",
                crate::models::PropertyValue::Boolean(_) => "bool",
                _ => "string",
            };
        }
    }
    "string"
}

/// Format a `lon,lat` pair for KML `<coordinates>`.
fn kml_coord(c: &[f64]) -> String {
    if c.len() >= 2 {
        format!("{},{}", c[0], c[1])
    } else {
        String::new()
    }
}

/// Format a coordinate list `lon,lat lon,lat …` for KML `<coordinates>`.
fn kml_coord_list(coords: &[Vec<f64>]) -> String {
    coords
        .iter()
        .map(|c| kml_coord(c))
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Geometry → KML (compact form, GeoServer-style `lon,lat` ordering).
fn geometry_to_kml(geometry: &crate::models::GeoJsonGeometry) -> String {
    use crate::models::GeoJsonGeometry as G;
    match geometry {
        G::Point { coordinates } => format!(
            "<Point><coordinates>{}</coordinates></Point>",
            kml_coord(coordinates)
        ),
        G::MultiPoint { coordinates } => {
            let items: Vec<String> = coordinates
                .iter()
                .map(|c| format!("<Point><coordinates>{}</coordinates></Point>", kml_coord(c)))
                .collect();
            format!("<MultiGeometry>{}</MultiGeometry>", items.concat())
        },
        G::LineString { coordinates } => format!(
            "<LineString><coordinates>{}</coordinates></LineString>",
            kml_coord_list(coordinates)
        ),
        G::MultiLineString { coordinates } => {
            let items: Vec<String> = coordinates
                .iter()
                .map(|line| {
                    format!(
                        "<LineString><coordinates>{}</coordinates></LineString>",
                        kml_coord_list(line)
                    )
                })
                .collect();
            format!("<MultiGeometry>{}</MultiGeometry>", items.concat())
        },
        G::Polygon { coordinates } => kml_polygon(coordinates),
        G::MultiPolygon { coordinates } => {
            let items: Vec<String> = coordinates.iter().map(|poly| kml_polygon(poly)).collect();
            format!("<MultiGeometry>{}</MultiGeometry>", items.concat())
        },
        G::GeometryCollection { geometries } => {
            let items: Vec<String> = geometries.iter().map(geometry_to_kml).collect();
            format!("<MultiGeometry>{}</MultiGeometry>", items.concat())
        },
    }
}

/// A KML `<Polygon>` from rings (first ring = outer boundary, rest are holes).
fn kml_polygon(rings: &[Vec<Vec<f64>>]) -> String {
    let mut out = String::from("<Polygon>");
    if let Some(outer) = rings.first() {
        out.push_str(&format!(
            "<outerBoundaryIs><LinearRing><coordinates>{}</coordinates></LinearRing></outerBoundaryIs>",
            kml_coord_list(outer)
        ));
    }
    for ring in rings.iter().skip(1) {
        out.push_str(&format!(
            "<innerBoundaryIs><LinearRing><coordinates>{}</coordinates></LinearRing></innerBoundaryIs>",
            kml_coord_list(ring)
        ));
    }
    out.push_str("</Polygon>");
    out
}

/// Build a KML 2.2 document: a `<Document>` with a `<Schema>` of the layer's
/// attributes and one `<Placemark>` per feature.
fn generate_kml_response(collection: &crate::models::FeatureCollection, type_name: &str) -> String {
    let schema_name = type_name.replace(':', "_");

    // Attribute union, preserving insertion order.
    let mut attr_order: Vec<String> = Vec::new();
    for feature in &collection.features {
        for key in feature.properties.keys() {
            if !attr_order.iter().any(|a| a == key) {
                attr_order.push(key.clone());
            }
        }
    }

    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
    out.push_str("<kml xmlns=\"http://www.opengis.net/kml/2.2\">\n");
    out.push_str("  <Document>\n");
    out.push_str(&format!(
        "    <Schema name=\"{}\" id=\"{}\">\n",
        escape_xml(&schema_name),
        escape_xml(&schema_name)
    ));
    for attr in &attr_order {
        let field_type = kml_field_type(attr, &collection.features);
        out.push_str(&format!(
            "      <SimpleField type=\"{}\" name=\"{}\"/>\n",
            field_type,
            escape_xml(attr)
        ));
    }
    out.push_str("    </Schema>\n");
    out.push_str("    <Folder>\n");
    out.push_str(&format!("      <name>{}</name>\n", escape_xml(type_name)));
    for feature in &collection.features {
        out.push_str(&format!(
            "      <Placemark id=\"{}\">\n",
            escape_xml(&feature.id)
        ));
        out.push_str("        <ExtendedData>\n");
        out.push_str(&format!(
            "          <SchemaData schemaUrl=\"#{}\">\n",
            escape_xml(&schema_name)
        ));
        for attr in &attr_order {
            if let Some(value) = feature.properties.get(attr) {
                out.push_str(&format!(
                    "            <SimpleData name=\"{}\">{}</SimpleData>\n",
                    escape_xml(attr),
                    escape_xml(&value.to_string())
                ));
            }
        }
        out.push_str("          </SchemaData>\n");
        out.push_str("        </ExtendedData>\n");
        out.push_str(&format!("        {}\n", geometry_to_kml(&feature.geometry)));
        out.push_str("      </Placemark>\n");
    }
    out.push_str("    </Folder>\n");
    out.push_str("  </Document>\n");
    out.push_str("</kml>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_csv_escape_basic_and_special() {
        // 纯文本保持不变。
        assert_eq!(csv_escape("plain"), "plain");
        // 含逗号/引号/换行时双引号包裹, 内部引号翻倍。
        assert_eq!(csv_escape("a,b"), "\"a,b\"");
        assert_eq!(csv_escape("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(csv_escape("line\nbreak"), "\"line\nbreak\"");
    }

    #[test]
    fn test_generate_csv_response_escapes_headers_and_values() {
        let mut props = std::collections::HashMap::new();
        props.insert(
            "name".to_string(),
            crate::models::PropertyValue::String("Alice, Bob".to_string()),
        );
        let feature = crate::models::Feature::with_id(
            "f1".to_string(),
            crate::models::GeoJsonGeometry::Point {
                coordinates: vec![1.0, 2.0],
            },
            props,
        );
        let coll = crate::models::FeatureCollection {
            features: vec![feature],
            total_count: 1,
        };
        let csv = generate_csv_response(&coll);
        assert!(csv.starts_with("id,geometry_type,name\n"));
        assert!(
            csv.contains("\"Alice, Bob\""),
            "含逗号属性值应被引号包裹, 实际: {}",
            csv
        );
    }
}
