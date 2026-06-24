use actix_web::{HttpRequest, HttpResponse, web};
use crate::services::wfs::{self, WfsRequest, WfsCapabilities, DescribeFeatureTypeResponse};
use crate::state::AppState;
use crate::error::GeoServerError;
use crate::models::{Feature, PropertyValue};
use quick_xml::se::to_string;
use tracing::info;

// ---- GET handler (原有，增加了 Transaction 的 POST 解析) ----

pub async fn handle_wfs_request(
    query: web::Query<Vec<(String, String)>>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let params = query.as_ref();
    let wfs_request = wfs::parse_wfs_request(params)?;

    match wfs_request.request {
        wfs::WfsOperation::GetCapabilities => handle_get_capabilities(&state, &wfs_request).await,
        wfs::WfsOperation::DescribeFeatureType => handle_describe_feature_type(&state, &wfs_request).await,
        wfs::WfsOperation::GetFeature => handle_get_feature(&state, &wfs_request).await,
        wfs::WfsOperation::GetFeatureWithLock => handle_get_feature_with_lock(&state, &wfs_request).await,
        wfs::WfsOperation::Transaction => handle_transaction(&state, &wfs_request).await,
        _ => Err(GeoServerError::BadRequest("Operation not implemented".to_string())),
    }
}

// ---- POST handler (解析 XML body 中的 Transaction) ----

pub async fn handle_wfs_post_request(
    req: HttpRequest,
    body: String,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let content_type = req.headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    // 检查是否为 Transaction 请求
    if body.contains("Transaction") || content_type.contains("xml") {
        return handle_transaction_xml(&state, &body).await;
    }

    // 否则尝试按 KVP 解析 POST body
    let params: Vec<(String, String)> = body.split('&')
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
        wfs::WfsOperation::DescribeFeatureType => handle_describe_feature_type(&state, &wfs_request).await,
        wfs::WfsOperation::GetFeature => handle_get_feature(&state, &wfs_request).await,
        wfs::WfsOperation::Transaction => handle_transaction(&state, &wfs_request).await,
        _ => Err(GeoServerError::BadRequest("Operation not implemented".to_string())),
    }
}

// ---- Transaction XML 处理核心 ----

async fn handle_transaction_xml(state: &AppState, xml: &str) -> Result<HttpResponse, GeoServerError> {
    info!("[WFS-T] 收到 Transaction 请求, 长度={}", xml.len());

    let transaction = wfs::parse_transaction_xml(xml)?;

    let mut total_inserted = 0u32;
    let mut total_updated = 0u32;
    let mut total_deleted = 0u32;
    let mut insert_results: Vec<String> = Vec::new();

    // 处理 Insert
    for insert_op in &transaction.inserts {
        for feature in &insert_op.features {
            let layer_name = &insert_op.type_name;
            let mut new_feature = Feature::new(feature.geometry.clone(), feature.properties.clone());
            new_feature.id = feature.id.clone();

            state.add_feature(layer_name, new_feature.clone()).await;

            // 持久化到 SQLite（如可用）
            if let Some(store) = &state.store {
                let _ = store.save_features(layer_name, &[new_feature.clone()]).await;
            }

            insert_results.push(new_feature.id.clone());
            total_inserted += 1;
        }
    }

    // 处理 Update
    for update_op in &transaction.updates {
        let layer_name = &update_op.type_name;
        let updated = {
            let mut features_lock = state.features.write().await;

            if let Some(existing) = features_lock.get_mut(layer_name) {
                for feature in existing.iter_mut() {
                    if update_op.filter.as_ref().map_or(true, |f| wfs::validate_filter(feature, f)) {
                        for prop in &update_op.properties {
                            feature.properties.insert(
                                prop.name.clone(),
                                PropertyValue::String(prop.value.clone()),
                            );
                        }
                        total_updated += 1;
                    }
                }
                existing.clone()
            } else {
                Vec::new()
            }
        };

        // 持久化
        if !updated.is_empty() {
            if let Some(store) = &state.store {
                let _ = store.save_features(layer_name, &updated).await;
            }
        }
    }

    // 处理 Delete
    for delete_op in &transaction.deletes {
        let layer_name = &delete_op.type_name;
        let mut features_lock = state.features.write().await;
        if let Some(existing) = features_lock.get_mut(layer_name) {
            let filter = &delete_op.filter;
            let before = existing.len();
            existing.retain(|f| !wfs::validate_filter(f, filter));
            total_deleted = (before - existing.len()) as u32;
        }
        drop(features_lock);

        if let Some(store) = &state.store {
            let _ = store.delete_features(layer_name).await;
            if let Some(features) = state.get_layer_features(layer_name).await {
                let _ = store.save_features(layer_name, &features).await;
            }
        }
    }

    info!("[WFS-T] Transaction 完成: inserted={}, updated={}, deleted={}",
           total_inserted, total_updated, total_deleted);

    let insert_xml: String = insert_results.iter()
        .map(|id| format!("            <wfs:Feature>wfs:{}</wfs:Feature>", id))
        .collect::<Vec<_>>()
        .join("\n");

    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<wfs:TransactionResponse xmlns:wfs="http://www.opengis.net/wfs/2.0"
                         xmlns:ogc="http://www.opengis.net/ogc"
                         version="2.0.0">
    <wfs:TransactionSummary>
        <wfs:totalInserted>{}</wfs:totalInserted>
        <wfs:totalUpdated>{}</wfs:totalUpdated>
        <wfs:totalDeleted>{}</wfs:totalDeleted>
    </wfs:TransactionSummary>
    <wfs:InsertResults>
{}
    </wfs:InsertResults>
</wfs:TransactionResponse>"#,
        total_inserted, total_updated, total_deleted, insert_xml
    );

    Ok(HttpResponse::Ok()
        .content_type("application/xml")
        .body(xml))
}

// ---- 原有操作处理函数保持不变 ----

async fn handle_get_capabilities(state: &AppState, _request: &WfsRequest) -> Result<HttpResponse, GeoServerError> {
    let base_url = format!("http://{}:{}", state.config.server.host, state.config.server.port);
    let capabilities = WfsCapabilities::new(&base_url);

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

async fn handle_describe_feature_type(_state: &AppState, request: &WfsRequest) -> Result<HttpResponse, GeoServerError> {
    let type_names = request.type_names.as_ref()
        .ok_or_else(|| GeoServerError::BadRequest("TYPENAME parameter is required".to_string()))?;

    let type_name = type_names.first()
        .ok_or_else(|| GeoServerError::BadRequest("At least one TYPENAME is required".to_string()))?;

    let properties: Vec<(&str, &str)> = vec![
        ("id", "xsd:string"),
        ("name", "xsd:string"),
        ("geometry", "xsd:string"),
    ];

    let response = DescribeFeatureTypeResponse::new(type_name, properties);

    let xml = to_string(&response)
        .map_err(|e| GeoServerError::ServiceError(format!("Failed to serialize response: {}", e)))?;

    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema">
{}
</xs:schema>"#,
        xml
    );

    Ok(HttpResponse::Ok()
        .content_type("application/xml")
        .body(xml))
}

async fn handle_get_feature(state: &AppState, request: &WfsRequest) -> Result<HttpResponse, GeoServerError> {
    let type_names = request.type_names.as_ref()
        .ok_or_else(|| GeoServerError::BadRequest("TYPENAME parameter is required".to_string()))?;

    let output_format = request.output_format.as_deref().unwrap_or("text/xml; subtype=gml/3.1.1");

    let mut all_features = Vec::new();

    for type_name in type_names {
        if let Some(features) = state.get_layer_features(type_name).await {
            let mut filtered_features = features;

            if let Some(ref filter) = request.filter {
                filtered_features.retain(|f| wfs::validate_filter(f, filter));
            }

            if let Some(ref bbox) = request.bbox {
                let bounds = bbox.to_bounds();
                filtered_features.retain(|f| {
                    match &f.geometry {
                        crate::models::GeoJsonGeometry::Point { coordinates } => {
                            if coordinates.len() >= 2 {
                                bounds.contains(coordinates[0], coordinates[1])
                            } else {
                                false
                            }
                        }
                        _ => true,
                    }
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
                    filtered_features = filtered_features.into_iter().skip(start_index as usize).collect();
                } else {
                    filtered_features.clear();
                }
            }

            all_features.extend(filtered_features);
        }
    }

    let response = crate::models::FeatureCollection::new(all_features);

    if output_format.contains("json") || output_format.contains("geojson") {
        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| GeoServerError::ServiceError(e.to_string()))?;

        Ok(HttpResponse::Ok()
            .content_type("application/json")
            .body(json))
    } else {
        let gml_response = generate_gml_response(&response, output_format);

        Ok(HttpResponse::Ok()
            .content_type("application/xml")
            .body(gml_response))
    }
}

async fn handle_get_feature_with_lock(state: &AppState, request: &WfsRequest) -> Result<HttpResponse, GeoServerError> {
    let response = handle_get_feature(state, request).await?;
    Ok(response)
}

async fn handle_transaction(_state: &AppState, _request: &WfsRequest) -> Result<HttpResponse, GeoServerError> {
    // GET 方式的 Transaction 不支持操作体，返回空结果
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<wfs:TransactionResponse xmlns:wfs="http://www.opengis.net/wfs/2.0"
                         version="2.0.0">
    <wfs:TransactionSummary>
        <wfs:totalInserted>0</wfs:totalInserted>
        <wfs:totalUpdated>0</wfs:totalUpdated>
        <wfs:totalDeleted>0</wfs:totalDeleted>
    </wfs:TransactionSummary>
</wfs:TransactionResponse>"#
    );

    Ok(HttpResponse::Ok()
        .content_type("application/xml")
        .body(xml))
}

// ---- GML 序列化辅助函数 ----

fn generate_gml_response(collection: &crate::models::FeatureCollection, format: &str) -> String {
    let gml_version = if format.contains("3.2") { "3.2" } else { "3.1.1" };
    
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
    
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<wfs:FeatureCollection xmlns:wfs="http://www.opengis.net/wfs/{gml_version}"
                        xmlns:gml="http://www.opengis.net/gml/{gml_version}"
                        xmlns:feature="http://geoserver.org/feature"
                        numberMatched="{total}" numberReturned="{count}">
{features}        </wfs:FeatureCollection>"#,
        gml_version = gml_version,
        total = collection.total_count,
        count = collection.features.len(),
        features = features_xml
    )
}

fn geometry_to_gml(geometry: &crate::models::GeoJsonGeometry, _version: &str) -> String {
    match geometry {
        crate::models::GeoJsonGeometry::Point { coordinates } => {
            if coordinates.len() >= 2 {
                format!(r#"<gml:Point srsName="EPSG:4326"><gml:pos>{} {}</gml:pos></gml:Point>"#,
                    coordinates[0], coordinates[1])
            } else {
                String::new()
            }
        }
        crate::models::GeoJsonGeometry::LineString { coordinates } => {
            let points: Vec<String> = coordinates.iter()
                .filter(|c| c.len() >= 2)
                .map(|c| format!("{} {}", c[0], c[1]))
                .collect();
            format!(r#"<gml:LineString srsName="EPSG:4326"><gml:posList>{}</gml:posList></gml:LineString>"#,
                points.join(" "))
        }
        crate::models::GeoJsonGeometry::Polygon { coordinates } => {
            if let Some(exterior) = coordinates.first() {
                let points: Vec<String> = exterior.iter()
                    .filter(|c| c.len() >= 2)
                    .map(|c| format!("{} {}", c[0], c[1]))
                    .collect();
                format!(r#"<gml:Polygon srsName="EPSG:4326"><gml:exterior><gml:LinearRing><gml:posList>{}</gml:posList></gml:LinearRing></gml:exterior></gml:Polygon>"#,
                    points.join(" "))
            } else {
                String::new()
            }
        }
        _ => String::new(),
    }
}

fn properties_to_gml(properties: &std::collections::HashMap<String, crate::models::PropertyValue>) -> String {
    let mut xml = String::new();
    for (key, value) in properties {
        xml.push_str(&format!("                <feature:{}>{}</feature:{}>\n",
            key,
            value.to_string(),
            key
        ));
    }
    xml
}
