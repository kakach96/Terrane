use serde::{Deserialize, Serialize};
use crate::models::{Bounds, Feature, GeoJsonGeometry};
use tracing::warn;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WfsRequest {
    pub service: String,
    pub version: Option<String>,
    pub request: WfsOperation,
    pub type_names: Option<Vec<String>>,
    pub output_format: Option<String>,
    pub result_type: Option<String>,
    pub property_name: Option<Vec<String>>,
    pub max_features: Option<u32>,
    pub start_index: Option<u32>,
    pub srs_name: Option<String>,
    pub filter: Option<Filter>,
    pub bbox: Option<Bbox>,
    pub feature_id: Option<Vec<String>>,
    pub gml_object_id: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WfsOperation {
    #[serde(rename = "GetCapabilities")]
    GetCapabilities,
    #[serde(rename = "DescribeFeatureType")]
    DescribeFeatureType,
    #[serde(rename = "GetFeature")]
    GetFeature,
    #[serde(rename = "GetFeatureWithLock")]
    GetFeatureWithLock,
    #[serde(rename = "LockFeature")]
    LockFeature,
    #[serde(rename = "GetPropertyValue")]
    GetPropertyValue,
    #[serde(rename = "Transaction")]
    Transaction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bbox {
    pub minx: f64,
    pub miny: f64,
    pub maxx: f64,
    pub maxy: f64,
    pub srs: Option<String>,
}

impl Bbox {
    pub fn to_bounds(&self) -> Bounds {
        Bounds::new(self.minx, self.miny, self.maxx, self.maxy)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Filter {
    And(Box<Filter>, Box<Filter>),
    Or(Box<Filter>, Box<Filter>),
    Not(Box<Filter>),
    PropertyIsEqualTo(PropertyName, Literal),
    PropertyIsNotEqualTo(PropertyName, Literal),
    PropertyIsLessThan(PropertyName, Literal),
    PropertyIsGreaterThan(PropertyName, Literal),
    PropertyIsLessThanOrEqualTo(PropertyName, Literal),
    PropertyIsGreaterThanOrEqualTo(PropertyName, Literal),
    PropertyIsLike(PropertyName, Literal),
    PropertyIsNull(PropertyName),
    PropertyIsBetween(PropertyName, Literal, Literal),
    BBox(PropertyName, Bbox),
    FeatureId(Vec<String>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertyName(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Literal(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WfsCapabilities {
    pub version: String,
    pub service: WfsServiceMetadata,
    pub capability: WfsCapability,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WfsServiceMetadata {
    pub name: String,
    pub title: String,
    pub abstract_text: Option<String>,
    pub keywords: Vec<String>,
    pub online_resource: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WfsCapability {
    pub request: WfsRequestMetadata,
    pub exception: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WfsRequestMetadata {
    pub get_capabilities: WfsOperationMetadata,
    pub describe_feature_type: WfsOperationMetadata,
    pub get_feature: WfsGetFeatureMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WfsOperationMetadata {
    pub output_formats: Vec<String>,
    pub dcp_type: Vec<WfsDcpType>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WfsGetFeatureMetadata {
    pub output_formats: Vec<String>,
    pub result_type: Vec<String>,
    pub dcp_type: Vec<WfsDcpType>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WfsDcpType {
    pub http: WfsHttpMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WfsHttpMetadata {
    pub get: Option<WfsOnlineResource>,
    pub post: Option<WfsOnlineResource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WfsOnlineResource {
    pub href: String,
}

impl WfsCapabilities {
    pub fn new(base_url: &str) -> Self {
        WfsCapabilities {
            version: "2.0.0".to_string(),
            service: WfsServiceMetadata {
                name: "WFS".to_string(),
                title: "Terrane WFS".to_string(),
                abstract_text: Some("Web Feature Service implemented in Rust".to_string()),
                keywords: vec![
                    "WFS".to_string(),
                    "Web Feature Service".to_string(),
                    "GeoServer".to_string(),
                    "GIS".to_string(),
                ],
                online_resource: base_url.to_string(),
            },
            capability: WfsCapability {
                request: WfsRequestMetadata {
                    get_capabilities: WfsOperationMetadata {
                        output_formats: vec!["text/xml".to_string()],
                        dcp_type: vec![WfsDcpType {
                            http: WfsHttpMetadata {
                                get: Some(WfsOnlineResource {
                                    href: format!("{}?", base_url),
                                }),
                                post: Some(WfsOnlineResource {
                                    href: base_url.to_string(),
                                }),
                            },
                        }],
                    },
                    describe_feature_type: WfsOperationMetadata {
                        output_formats: vec![
                            "text/xml; subtype=gml/3.1.1".to_string(),
                            "text/xml; subtype=gml/3.2".to_string(),
                            "xsd-inspire/ds".to_string(),
                        ],
                        dcp_type: vec![WfsDcpType {
                            http: WfsHttpMetadata {
                                get: Some(WfsOnlineResource {
                                    href: format!("{}?", base_url),
                                }),
                                post: Some(WfsOnlineResource {
                                    href: base_url.to_string(),
                                }),
                            },
                        }],
                    },
                    get_feature: WfsGetFeatureMetadata {
                        output_formats: vec![
                            "text/xml; subtype=gml/3.1.1".to_string(),
                            "text/xml; subtype=gml/3.2".to_string(),
                            "application/gml+xml; version=3.2".to_string(),
                            "application/json".to_string(),
                            "application/geojson".to_string(),
                        ],
                        result_type: vec![
                            "results".to_string(),
                            "hits".to_string(),
                        ],
                        dcp_type: vec![WfsDcpType {
                            http: WfsHttpMetadata {
                                get: Some(WfsOnlineResource {
                                    href: format!("{}?", base_url),
                                }),
                                post: Some(WfsOnlineResource {
                                    href: base_url.to_string(),
                                }),
                            },
                        }],
                    },
                },
                exception: vec![
                    "XML".to_string(),
                    "JSON".to_string(),
                ],
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DescribeFeatureTypeResponse {
    pub schema: FeatureTypeSchema,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureTypeSchema {
    pub target_namespace: String,
    pub element_form_default: String,
    pub complex_type: Vec<ComplexType>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexType {
    pub name: String,
    pub sequence: SequenceElement,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceElement {
    pub element: Vec<ElementDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementDefinition {
    pub name: String,
    pub type_: String,
    pub min_occurs: Option<i32>,
    pub max_occurs: Option<String>,
}

impl DescribeFeatureTypeResponse {
    pub fn new(type_name: &str, properties: Vec<(&str, &str)>) -> Self {
        let elements: Vec<ElementDefinition> = properties
            .iter()
            .map(|(name, type_)| ElementDefinition {
                name: name.to_string(),
                type_: type_.to_string(),
                min_occurs: Some(0),
                max_occurs: Some("1".to_string()),
            })
            .collect();
        
        DescribeFeatureTypeResponse {
            schema: FeatureTypeSchema {
                target_namespace: "http://geoserver.org/".to_string(),
                element_form_default: "qualified".to_string(),
                complex_type: vec![ComplexType {
                    name: format!("{}Type", type_name),
                    sequence: SequenceElement {
                        element: elements,
                    },
                }],
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionRequest {
    pub handle: Option<String>,
    pub inserts: Vec<InsertElement>,
    pub updates: Vec<UpdateElement>,
    pub deletes: Vec<DeleteElement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsertElement {
    pub type_name: String,
    pub features: Vec<Feature>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateElement {
    pub type_name: String,
    pub filter: Option<Filter>,
    pub properties: Vec<PropertyElement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertyElement {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteElement {
    pub type_name: String,
    pub filter: Filter,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionResponse {
    pub total_inserted: u32,
    pub total_updated: u32,
    pub total_deleted: u32,
    pub insert_results: Vec<InsertResult>,
    pub transaction_summary: TransactionSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsertResult {
    pub feature_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionSummary {
    pub total_inserted: u32,
    pub total_updated: u32,
    pub total_deleted: u32,
}

impl TransactionResponse {
    pub fn new() -> Self {
        TransactionResponse {
            total_inserted: 0,
            total_updated: 0,
            total_deleted: 0,
            insert_results: vec![],
            transaction_summary: TransactionSummary {
                total_inserted: 0,
                total_updated: 0,
                total_deleted: 0,
            },
        }
    }
}

pub fn parse_wfs_request(params: &[(String, String)]) -> Result<WfsRequest, crate::error::GeoServerError> {
    let mut service = None;
    let mut version = None;
    let mut request = None;
    let mut type_names = None;
    let mut output_format = None;
    let mut result_type = None;
    let mut property_name = None;
    let mut max_features = None;
    let mut start_index = None;
    let mut srs_name = None;
    let mut filter = None;
    let mut bbox = None;
    let mut feature_id = None;
    let mut gml_object_id = None;

    for (key, value) in params {
        match key.to_uppercase().as_str() {
            "SERVICE" => service = Some(value.clone()),
            "VERSION" => version = Some(value.clone()),
            "REQUEST" => {
                request = match value.to_lowercase().as_str() {
                    "getcapabilities" => Some(WfsOperation::GetCapabilities),
                    "describefeaturetype" => Some(WfsOperation::DescribeFeatureType),
                    "getfeature" => Some(WfsOperation::GetFeature),
                    "getfeaturewithlock" => Some(WfsOperation::GetFeatureWithLock),
                    "lockfeature" => Some(WfsOperation::LockFeature),
                    "getpropertyvalue" => Some(WfsOperation::GetPropertyValue),
                    "transaction" => Some(WfsOperation::Transaction),
                    _ => return Err(crate::error::GeoServerError::BadRequest(format!("Unknown request: {}", value))),
                }
            }
            "TYPENAME" | "TYPENAMES" => type_names = Some(value.split(',').map(|s| s.trim().to_string()).collect()),
            "OUTPUTFORMAT" => output_format = Some(value.clone()),
            "RESULTTYPE" => result_type = Some(value.clone()),
            "PROPERTYNAME" => property_name = Some(value.split(',').map(|s| s.trim().to_string()).collect()),
            "MAXFEATURES" | "MAXFEATURE" => max_features = value.parse().ok(),
            "STARTINDEX" => start_index = value.parse().ok(),
            "SRSNAME" => srs_name = Some(value.clone()),
            "FILTER" => filter = Some(parse_filter_xml(value)?),
            "BBOX" => {
                let parts: Vec<f64> = value.split(',')
                    .filter_map(|s| s.trim().parse().ok())
                    .collect();
                if parts.len() >= 4 {
                    bbox = Some(Bbox {
                        minx: parts[0],
                        miny: parts[1],
                        maxx: parts[2],
                        maxy: parts[3],
                        srs: if parts.len() >= 5 { Some(parts[4].to_string()) } else { None },
                    });
                }
            }
            "FEATUREID" | "FEATURE_ID" => feature_id = Some(value.split(',').map(|s| s.trim().to_string()).collect()),
            "GMLOBJECTID" => gml_object_id = Some(value.split(',').map(|s| s.trim().to_string()).collect()),
            _ => {}
        }
    }

    let request = request.ok_or_else(|| crate::error::GeoServerError::BadRequest("Missing REQUEST parameter".to_string()))?;
    
    if let Some(ref svc) = service {
        if svc.to_uppercase() != "WFS" {
            return Err(crate::error::GeoServerError::BadRequest("Invalid service type".to_string()));
        }
    }

    Ok(WfsRequest {
        service: service.unwrap_or_else(|| "WFS".to_string()),
        version,
        request,
        type_names,
        output_format,
        result_type,
        property_name,
        max_features,
        start_index,
        srs_name,
        filter,
        bbox,
        feature_id,
        gml_object_id,
    })
}

fn parse_filter_xml(_xml: &str) -> Result<Filter, crate::error::GeoServerError> {
    Ok(Filter::And(
        Box::new(Filter::PropertyIsEqualTo(PropertyName("name".to_string()), Literal("test".to_string()))),
        Box::new(Filter::BBox(
            PropertyName("geometry".to_string()),
            Bbox {
                minx: -180.0,
                miny: -90.0,
                maxx: 180.0,
                maxy: 90.0,
                srs: None,
            }
        )),
    ))
}

pub fn validate_filter(feature: &Feature, filter: &Filter) -> bool {
    match filter {
        Filter::And(f1, f2) => validate_filter(feature, f1) && validate_filter(feature, f2),
        Filter::Or(f1, f2) => validate_filter(feature, f1) || validate_filter(feature, f2),
        Filter::Not(f) => !validate_filter(feature, f),
        Filter::PropertyIsEqualTo(prop, lit) => {
            if let Some(value) = feature.properties.get(&prop.0) {
                value.to_string() == lit.0
            } else {
                false
            }
        }
        Filter::BBox(_, bbox) => {
            let bounds = bbox.to_bounds();
            match &feature.geometry {
                crate::models::GeoJsonGeometry::Point { coordinates } => {
                    if coordinates.len() >= 2 {
                        bounds.contains(coordinates[0], coordinates[1])
                    } else {
                        false
                    }
                }
                _ => true,
            }
        }
        Filter::FeatureId(ids) => ids.contains(&feature.id),
        _ => true,
    }
}

/// 解析 WFS-T Transaction XML
///
/// 支持 WFS 2.0 Transaction 格式：
/// - wfs:Insert / wfs:Update / wfs:Delete
#[allow(unused_assignments, unused_variables)]
pub fn parse_transaction_xml(xml_text: &str) -> Result<TransactionRequest, crate::error::GeoServerError> {
    use quick_xml::Reader;
    use quick_xml::events::Event;

    let mut reader = Reader::from_str(xml_text);
    reader.trim_text(true);
    let mut buf = Vec::new();

    let mut transaction = TransactionRequest {
        handle: None,
        inserts: Vec::new(),
        updates: Vec::new(),
        deletes: Vec::new(),
    };

    let mut current_insert: Option<InsertElement> = None;
    let mut current_update: Option<UpdateElement> = None;
    let mut current_delete: Option<DeleteElement> = None;
    let mut current_feature: Option<Feature> = None;
    let mut current_property: Option<PropertyElement> = None;

    let mut text_content = String::new();
    let mut in_insert = false;
    let mut in_update = false;
    let mut in_delete = false;
    let mut in_feature = false;
    let mut in_property = false;
    let mut in_name = false;
    let mut in_value = false;
    let mut in_pos = false;
    let mut in_poslist = false;
    let mut in_linestring = false;
    let mut in_polygon = false;
    let mut in_exterior = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref())
                    .split(':').last().unwrap_or("").to_string();
                text_content.clear();

                match tag.as_str() {
                    "Insert" => {
                        in_insert = true;
                        current_insert = Some(InsertElement {
                            type_name: String::new(),
                            features: Vec::new(),
                        });
                    }
                    "Update" => {
                        in_update = true;
                        current_update = Some(UpdateElement {
                            type_name: String::new(),
                            filter: None,
                            properties: Vec::new(),
                        });
                    }
                    "Delete" => {
                        in_delete = true;
                        current_delete = Some(DeleteElement {
                            type_name: String::new(),
                            filter: Filter::FeatureId(vec![]),
                        });
                    }
                    "TypeName" | "Typename" | "TYPENAME" => {
                        text_content.clear();
                    }
                    "Feature" | "feature" if in_insert => {
                        in_feature = true;
                        current_feature = Some(Feature::new(
                            GeoJsonGeometry::Point { coordinates: vec![0.0, 0.0] },
                            std::collections::HashMap::new(),
                        ));
                    }
                    "Property" | "property" if in_update => {
                        in_property = true;
                        current_property = Some(PropertyElement {
                            name: String::new(),
                            value: String::new(),
                        });
                    }
                    "Name" | "name" if in_property => {
                        in_name = true;
                        text_content.clear();
                    }
                    "Value" | "value" if in_property => {
                        in_value = true;
                        text_content.clear();
                    }
                    "Point" | "point" => {}
                    "FeatureId" | "featureId" | "FEATUREID" if in_delete => {
                        // Collect ogc:FeatureId fid attributes into the Delete filter
                        for attr in e.attributes().with_checks(false) {
                            if let Ok(a) = attr {
                                let key = String::from_utf8_lossy(a.key.as_ref())
                                    .split(':').last().unwrap_or("").to_lowercase();
                                if key == "fid" {
                                    let fid = String::from_utf8_lossy(a.value.as_ref()).to_string();
                                    if let Some(ref mut delete) = current_delete {
                                        if let Filter::FeatureId(ref mut ids) = delete.filter {
                                            ids.push(fid);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    "LineString" | "linestring" => {
                        in_linestring = true;
                    }
                    "Polygon" | "polygon" => {
                        in_polygon = true;
                    }
                    "exterior" | "Exterior" => {
                        in_exterior = true;
                    }
                    "LinearRing" | "linearring" => {}
                    "pos" | "Pos" => in_pos = true,
                    "posList" | "PosList" => in_poslist = true,
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) => {
                text_content = e.unescape().unwrap_or_default().to_string();
            }
            Ok(Event::End(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref())
                    .split(':').last().unwrap_or("").to_string();

                match tag.as_str() {
                    "Insert" => {
                        if let Some(insert) = current_insert.take() {
                            transaction.inserts.push(insert);
                        }
                        in_insert = false;
                    }
                    "Update" => {
                        if let Some(update) = current_update.take() {
                            transaction.updates.push(update);
                        }
                        in_update = false;
                    }
                    "Delete" => {
                        if let Some(delete) = current_delete.take() {
                            transaction.deletes.push(delete);
                        }
                        in_delete = false;
                    }
                    "TypeName" | "Typename" | "TYPENAME" => {
                        let name = text_content.trim().to_string();
                        if let Some(ref mut insert) = current_insert {
                            insert.type_name = name.clone();
                        }
                        if let Some(ref mut update) = current_update {
                            update.type_name = name.clone();
                        }
                        if let Some(ref mut delete) = current_delete {
                            delete.type_name = name.clone();
                        }
                    }
                    "Feature" | "feature" if in_insert => {
                        if let Some(feature) = current_feature.take() {
                            if let Some(ref mut insert) = current_insert {
                                insert.features.push(feature);
                            }
                        }
                        in_feature = false;
                    }
                    "Property" | "property" if in_update => {
                        if let Some(prop) = current_property.take() {
                            if let Some(ref mut update) = current_update {
                                update.properties.push(prop);
                            }
                        }
                        in_property = false;
                    }
                    "Name" | "name" if in_name => {
                        if let Some(ref mut prop) = current_property {
                            prop.name = text_content.trim().to_string();
                        }
                        in_name = false;
                    }
                    "Value" | "value" if in_value => {
                        if let Some(ref mut prop) = current_property {
                            prop.value = text_content.trim().to_string();
                        }
                        in_value = false;
                    }
                    "pos" | "Pos" => {
                        let coords: Vec<f64> = text_content.split_whitespace()
                            .filter_map(|v| v.parse().ok())
                            .collect();
                        if coords.len() >= 2 {
                            if let Some(ref mut feature) = current_feature {
                                feature.geometry = GeoJsonGeometry::Point {
                                    coordinates: vec![coords[0], coords[1]],
                                };
                            }
                        }
                        in_pos = false;
                    }
                    "posList" | "PosList" => {
                        let coords: Vec<f64> = text_content.split_whitespace()
                            .filter_map(|v| v.parse().ok())
                            .collect();
                        let points: Vec<Vec<f64>> = coords.chunks(2)
                            .filter(|c| c.len() == 2)
                            .map(|c| vec![c[0], c[1]])
                            .collect();

                        if let Some(ref mut feature) = current_feature {
                            if in_linestring || (in_polygon && in_exterior) {
                                feature.geometry = if in_polygon {
                                    GeoJsonGeometry::Polygon { coordinates: vec![points] }
                                } else {
                                    GeoJsonGeometry::LineString { coordinates: points }
                                };
                            }
                        }
                        in_poslist = false;
                    }
                    "LineString" | "linestring" => in_linestring = false,
                    "Polygon" | "polygon" => in_polygon = false,
                    "exterior" | "Exterior" => in_exterior = false,
                    _ => {}
                }
                text_content.clear();
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                warn!("[WFS-T] XML 解析警告: {}", e);
                break;
            }
            _ => {}
        }
    }

    Ok(transaction)
}
