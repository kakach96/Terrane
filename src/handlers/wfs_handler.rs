use actix_web::{HttpResponse, web};
use crate::services::wfs::{self, WfsRequest, WfsCapabilities, DescribeFeatureTypeResponse};
use crate::state::AppState;
use crate::error::GeoServerError;
use quick_xml::se::to_string;

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
    let transaction_response = wfs::TransactionResponse::new();
    
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<wfs:TransactionResponse xmlns:wfs="http://www.opengis.net/wfs/2.0">
    <wfs:TransactionSummary>
        <wfs:totalInserted>{}</wfs:totalInserted>
        <wfs:totalUpdated>{}</wfs:totalUpdated>
        <wfs:totalDeleted>{}</wfs:totalDeleted>
    </wfs:TransactionSummary>
    <wfs:InsertResults>
    </wfs:InsertResults>
</wfs:TransactionResponse>"#,
        transaction_response.total_inserted,
        transaction_response.total_updated,
        transaction_response.total_deleted
    );
    
    Ok(HttpResponse::Ok()
        .content_type("application/xml")
        .body(xml))
}

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
