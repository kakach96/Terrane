use crate::error::GeoServerError;
use crate::models::{DataSourceType, Feature, PropertyValue};
use crate::services::wfs::{self, DescribeFeatureTypeResponse, WfsCapabilities, WfsRequest};
use crate::state::AppState;
use actix_web::{web, HttpRequest, HttpResponse};
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
        wfs::WfsOperation::DescribeFeatureType => {
            handle_describe_feature_type(&state, &wfs_request).await
        },
        wfs::WfsOperation::GetFeature => handle_get_feature(&state, &wfs_request).await,
        wfs::WfsOperation::GetFeatureWithLock => {
            handle_get_feature_with_lock(&state, &wfs_request).await
        },
        wfs::WfsOperation::Transaction => handle_transaction(&state, &wfs_request).await,
        _ => Err(GeoServerError::BadRequest(
            "Operation not implemented".to_string(),
        )),
    }
}

// ---- POST handler (解析 XML body 中的 Transaction) ----

pub async fn handle_wfs_post_request(
    req: HttpRequest,
    body: String,
    state: web::Data<AppState>,
) -> Result<HttpResponse, GeoServerError> {
    let content_type = req
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    // 检查是否为 Transaction 请求
    if body.contains("Transaction") || content_type.contains("xml") {
        return handle_transaction_xml(&state, &body).await;
    }

    // 否则尝试按 KVP 解析 POST body
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
        wfs::WfsOperation::GetFeature => handle_get_feature(&state, &wfs_request).await,
        wfs::WfsOperation::Transaction => handle_transaction(&state, &wfs_request).await,
        _ => Err(GeoServerError::BadRequest(
            "Operation not implemented".to_string(),
        )),
    }
}

// ---- Transaction XML 处理核心 ----

async fn handle_transaction_xml(
    state: &AppState,
    xml: &str,
) -> Result<HttpResponse, GeoServerError> {
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
            let mut new_feature =
                Feature::new(feature.geometry.clone(), feature.properties.clone());
            new_feature.id = feature.id.clone();

            state.add_feature(layer_name, new_feature.clone()).await;

            // 持久化到矢量数据存储（如可用）
            if let Some(bstore) = &state.vector_store {
                let _ = bstore
                    .save_features(layer_name, &[new_feature.clone()])
                    .await;
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
                    if update_op
                        .filter
                        .as_ref()
                        .map_or(true, |f| wfs::validate_filter(feature, f))
                    {
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
            if let Some(bstore) = &state.vector_store {
                let _ = bstore.save_features(layer_name, &updated).await;
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

        if let Some(bstore) = &state.vector_store {
            let _ = bstore.delete_features(layer_name).await;
            if let Some(features) = state.get_layer_features(layer_name).await {
                let _ = bstore.save_features(layer_name, &features).await;
            }
        }
    }

    info!(
        "[WFS-T] Transaction 完成: inserted={}, updated={}, deleted={}",
        total_inserted, total_updated, total_deleted
    );

    let insert_xml: String = insert_results
        .iter()
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

    Ok(HttpResponse::Ok().content_type("application/xml").body(xml))
}

// ---- 原有操作处理函数保持不变 ----

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
) -> Result<HttpResponse, GeoServerError> {
    let type_names = request
        .type_names
        .as_ref()
        .ok_or_else(|| GeoServerError::BadRequest("TYPENAME parameter is required".to_string()))?;

    let output_format = request
        .output_format
        .as_deref()
        .unwrap_or("text/xml; subtype=gml/3.1.1");

    let mut all_features = Vec::new();

    for type_name in type_names {
        if let Some(features) = state.get_layer_features(type_name).await {
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
    }

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
            generate_gml2_response(&response),
            "application/gml+xml; version=2.1.2",
        )
    } else if output_format.contains("gml/3.2") {
        (
            generate_gml32_response(&response),
            "application/gml+xml; version=3.2",
        )
    } else {
        // 默认 GML 3.1.1
        (
            generate_gml_response(&response, output_format),
            "application/gml+xml; version=3.1",
        )
    };

    Ok(HttpResponse::Ok().content_type(content_type).body(gml))
}

async fn handle_get_feature_with_lock(
    state: &AppState,
    request: &WfsRequest,
) -> Result<HttpResponse, GeoServerError> {
    let response = handle_get_feature(state, request).await?;
    Ok(response)
}

async fn handle_transaction(
    _state: &AppState,
    _request: &WfsRequest,
) -> Result<HttpResponse, GeoServerError> {
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

    Ok(HttpResponse::Ok().content_type("application/xml").body(xml))
}

// ---- GML 序列化辅助函数 ----

fn generate_gml_response(collection: &crate::models::FeatureCollection, format: &str) -> String {
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
                format!(
                    r#"<gml:Point srsName="EPSG:4326"><gml:pos>{} {}</gml:pos></gml:Point>"#,
                    coordinates[0], coordinates[1]
                )
            } else {
                String::new()
            }
        },
        crate::models::GeoJsonGeometry::LineString { coordinates } => {
            let points: Vec<String> = coordinates
                .iter()
                .filter(|c| c.len() >= 2)
                .map(|c| format!("{} {}", c[0], c[1]))
                .collect();
            format!(
                r#"<gml:LineString srsName="EPSG:4326"><gml:posList>{}</gml:posList></gml:LineString>"#,
                points.join(" ")
            )
        },
        crate::models::GeoJsonGeometry::Polygon { coordinates } => {
            if let Some(exterior) = coordinates.first() {
                let points: Vec<String> = exterior
                    .iter()
                    .filter(|c| c.len() >= 2)
                    .map(|c| format!("{} {}", c[0], c[1]))
                    .collect();
                format!(
                    r#"<gml:Polygon srsName="EPSG:4326"><gml:exterior><gml:LinearRing><gml:posList>{}</gml:posList></gml:LinearRing></gml:exterior></gml:Polygon>"#,
                    points.join(" ")
                )
            } else {
                String::new()
            }
        },
        _ => String::new(),
    }
}

fn properties_to_gml(
    properties: &std::collections::HashMap<String, crate::models::PropertyValue>,
) -> String {
    let mut xml = String::new();
    for (key, value) in properties {
        xml.push_str(&format!(
            "                <feature:{}>{}</feature:{}>\n",
            key,
            escape_xml(&value.to_string()),
            key
        ));
    }
    xml
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
    csv.push_str(&headers.join(","));
    csv.push('\n');

    for feature in &collection.features {
        let mut row = vec![
            feature.id.clone(),
            format!("\"{:?}\"", std::mem::discriminant(&feature.geometry)),
        ];
        for h in headers.iter().skip(2) {
            let val = match feature.properties.get(h) {
                Some(v) => {
                    let s = v.to_string();
                    if s.contains(',') || s.contains('"') {
                        format!("\"{}\"", s.replace('"', "\"\""))
                    } else {
                        s
                    }
                },
                None => String::new(),
            };
            row.push(val);
        }
        csv.push_str(&row.join(","));
        csv.push('\n');
    }
    csv
}

/// GML 2.1.2 输出
fn generate_gml2_response(collection: &crate::models::FeatureCollection) -> String {
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
fn generate_gml32_response(collection: &crate::models::FeatureCollection) -> String {
    let mut features_xml = String::new();
    for feature in &collection.features {
        features_xml.push_str(&format!(
            r#"        <wfs:member>
            <Feature gml:id="{}">
                <feature:geometry>
                    {}
                </feature:geometry>
                {}
            </Feature>
        </wfs:member>
"#,
            feature.id,
            geometry_to_gml(&feature.geometry, "3.2"),
            properties_to_gml(&feature.properties)
        ));
    }

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<wfs:FeatureCollection xmlns:wfs="http://www.opengis.net/wfs/2.0"
                        xmlns:gml="http://www.opengis.net/gml/3.2"
                        xmlns:feature="http://geoserver.org/feature"
                        numberMatched="{}" numberReturned="{}">
{}
        </wfs:FeatureCollection>"#,
        collection.total_count,
        collection.features.len(),
        features_xml
    )
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
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
