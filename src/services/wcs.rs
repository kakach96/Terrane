use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WcsRequest {
    pub service: String,
    pub version: Option<String>,
    pub request: WcsOperation,
    pub coverage_id: Option<Vec<String>>,
    pub output_format: Option<String>,
    pub media_type: Option<String>,
    pub subsets: Option<Vec<Subset>>,
    pub subset_crs: Option<String>,
    pub interpolation: Option<String>,
    pub axis_labels: Option<Vec<String>>,
    pub format: Option<String>,
    pub store: Option<bool>,
    pub expiration: Option<u32>,
    pub size: Option<Vec<i64>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WcsOperation {
    #[serde(rename = "GetCapabilities")]
    GetCapabilities,
    #[serde(rename = "DescribeCoverage")]
    DescribeCoverage,
    #[serde(rename = "GetCoverage")]
    GetCoverage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subset {
    pub axis_label: String,
    pub crs: Option<String>,
    #[serde(rename = "type")]
    pub subset_type: SubsetType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SubsetType {
    #[serde(rename = "intervals")]
    Intervals { min: f64, max: f64, resolution: Option<f64> },
    #[serde(rename = "position")]
    Position { value: f64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WcsCapabilities {
    pub version: String,
    pub service: WcsServiceMetadata,
    pub capability: WcsCapability,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WcsServiceMetadata {
    pub name: String,
    pub title: String,
    pub abstract_text: Option<String>,
    pub keywords: Vec<String>,
    pub online_resource: String,
    pub fees: Option<String>,
    pub access_constraints: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WcsCapability {
    pub request: WcsRequestMetadata,
    pub exception: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WcsRequestMetadata {
    pub get_capabilities: WcsOperationMetadata,
    pub describe_coverage: WcsOperationMetadata,
    pub get_coverage: WcsGetCoverageMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WcsOperationMetadata {
    pub output_formats: Vec<String>,
    pub dcp_type: Vec<WcsDcpType>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WcsGetCoverageMetadata {
    pub output_formats: Vec<String>,
    pub dcp_type: Vec<WcsDcpType>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WcsDcpType {
    pub http: WcsHttpMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WcsHttpMetadata {
    pub get: Option<WcsOnlineResource>,
    pub post: Option<WcsOnlineResource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WcsOnlineResource {
    pub href: String,
}

impl WcsCapabilities {
    pub fn new(base_url: &str) -> Self {
        WcsCapabilities {
            version: "2.0.1".to_string(),
            service: WcsServiceMetadata {
                name: "WCS".to_string(),
                title: "Rust GeoServer WCS".to_string(),
                abstract_text: Some("Web Coverage Service implemented in Rust".to_string()),
                keywords: vec![
                    "WCS".to_string(),
                    "Web Coverage Service".to_string(),
                    "GeoServer".to_string(),
                    "GIS".to_string(),
                    "Raster".to_string(),
                ],
                online_resource: base_url.to_string(),
                fees: None,
                access_constraints: None,
            },
            capability: WcsCapability {
                request: WcsRequestMetadata {
                    get_capabilities: WcsOperationMetadata {
                        output_formats: vec!["text/xml".to_string()],
                        dcp_type: vec![WcsDcpType {
                            http: WcsHttpMetadata {
                                get: Some(WcsOnlineResource {
                                    href: format!("{}?", base_url),
                                }),
                                post: Some(WcsOnlineResource {
                                    href: base_url.to_string(),
                                }),
                            },
                        }],
                    },
                    describe_coverage: WcsOperationMetadata {
                        output_formats: vec!["text/xml".to_string()],
                        dcp_type: vec![WcsDcpType {
                            http: WcsHttpMetadata {
                                get: Some(WcsOnlineResource {
                                    href: format!("{}?", base_url),
                                }),
                                post: Some(WcsOnlineResource {
                                    href: base_url.to_string(),
                                }),
                            },
                        }],
                    },
                    get_coverage: WcsGetCoverageMetadata {
                        output_formats: vec![
                            "image/tiff".to_string(),
                            "image/png".to_string(),
                            "image/jpeg".to_string(),
                            "application/netcdf".to_string(),
                            "application/gml+xml".to_string(),
                        ],
                        dcp_type: vec![WcsDcpType {
                            http: WcsHttpMetadata {
                                get: Some(WcsOnlineResource {
                                    href: format!("{}?", base_url),
                                }),
                                post: Some(WcsOnlineResource {
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
pub struct CoverageDescription {
    pub coverage_id: String,
    pub title: String,
    pub abstract_text: Option<String>,
    pub keywords: Vec<String>,
    pub bounding_boxes: Vec<BoundingBoxMetadata>,
    pub coverage_domain: CoverageDomain,
    pub range: Range,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageDomain {
    pub spatial_domain: SpatialDomain,
    pub temporal_domain: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpatialDomain {
    pub bounding_boxes: Vec<BoundingBoxMetadata>,
    pub gridCRS: Option<GridCrs>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundingBoxMetadata {
    pub crs: String,
    pub minx: f64,
    pub miny: f64,
    pub maxx: f64,
    pub maxy: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridCrs {
    pub crs: String,
    pub grid_base_crs: Option<String>,
    pub horizontal_xy: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Range {
    pub field: Vec<Field>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    pub definition: String,
    pub description: Option<String>,
    pub unit: Option<Unit>,
    pub null_values: Vec<NullValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Unit {
    pub name: String,
    pub code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NullValue {
    pub value: String,
}

impl CoverageDescription {
    pub fn new(coverage_id: &str) -> Self {
        CoverageDescription {
            coverage_id: coverage_id.to_string(),
            title: coverage_id.to_string(),
            abstract_text: Some("Coverage description".to_string()),
            keywords: vec![],
            bounding_boxes: vec![BoundingBoxMetadata {
                crs: "EPSG:4326".to_string(),
                minx: -180.0,
                miny: -90.0,
                maxx: 180.0,
                maxy: 90.0,
            }],
            coverage_domain: CoverageDomain {
                spatial_domain: SpatialDomain {
                    bounding_boxes: vec![],
                    gridCRS: Some(GridCrs {
                        crs: "EPSG:4326".to_string(),
                        grid_base_crs: None,
                        horizontal_xy: Some(true),
                    }),
                },
                temporal_domain: None,
            },
            range: Range {
                field: vec![Field {
                    name: "raster".to_string(),
                    definition: "GridCoverage".to_string(),
                    description: None,
                    unit: Some(Unit {
                        name: "W/m**2".to_string(),
                        code: Some("nm".to_string()),
                    }),
                    null_values: vec![NullValue { value: "-9999".to_string() }],
                }],
            },
        }
    }
}

pub fn parse_wcs_request(params: &[(String, String)]) -> Result<WcsRequest, crate::error::GeoServerError> {
    let mut service = None;
    let mut version = None;
    let mut request = None;
    let mut coverage_id = None;
    let mut output_format = None;
    let mut media_type = None;
    let mut subsets = None;
    let mut subset_crs = None;
    let mut interpolation = None;
    let mut axis_labels = None;
    let format = None;
    let mut store = None;
    let mut expiration = None;
    let mut size = None;

    for (key, value) in params {
        match key.to_uppercase().as_str() {
            "SERVICE" => service = Some(value.clone()),
            "VERSION" => version = Some(value.clone()),
            "REQUEST" => {
                request = match value.to_lowercase().as_str() {
                    "getcapabilities" => Some(WcsOperation::GetCapabilities),
                    "describecoverage" => Some(WcsOperation::DescribeCoverage),
                    "getcoverage" => Some(WcsOperation::GetCoverage),
                    _ => return Err(crate::error::GeoServerError::BadRequest(format!("Unknown request: {}", value))),
                }
            }
            "COVERAGEID" | "COVERAGE_ID" => coverage_id = Some(value.split(',').map(|s| s.trim().to_string()).collect()),
            "OUTPUTFORMAT" | "FORMAT" => output_format = Some(value.clone()),
            "MEDIATYPE" => media_type = Some(value.clone()),
            "SUBSET" => {
                let parsed = parse_subset(value)?;
                if subsets.is_none() {
                    subsets = Some(vec![]);
                }
                if let Some(ref mut s) = subsets {
                    s.push(parsed);
                }
            }
            "SUBSETTINGCRS" => subset_crs = Some(value.clone()),
            "INTERPOLATION" => interpolation = Some(value.clone()),
            "AXISLABELS" => axis_labels = Some(value.split(',').map(|s| s.trim().to_string()).collect()),
            "STORE" => store = Some(value.to_lowercase() == "true"),
            "EXPIRATION" => expiration = value.parse().ok(),
            "SIZE" => {
                size = Some(value.split(',')
                    .filter_map(|s| s.trim().parse().ok())
                    .collect());
            }
            _ => {}
        }
    }

    let request = request.ok_or_else(|| crate::error::GeoServerError::BadRequest("Missing REQUEST parameter".to_string()))?;
    
    if let Some(ref svc) = service {
        if svc.to_uppercase() != "WCS" {
            return Err(crate::error::GeoServerError::BadRequest("Invalid service type".to_string()));
        }
    }

    Ok(WcsRequest {
        service: service.unwrap_or_else(|| "WCS".to_string()),
        version,
        request,
        coverage_id,
        output_format,
        media_type,
        subsets,
        subset_crs,
        interpolation,
        axis_labels,
        format,
        store,
        expiration,
        size,
    })
}

fn parse_subset(value: &str) -> Result<Subset, crate::error::GeoServerError> {
    let parts: Vec<&str> = value.split(',').collect();
    if parts.is_empty() {
        return Err(crate::error::GeoServerError::BadRequest("Invalid subset format".to_string()));
    }
    
    let axis_label = parts[0].to_string();
    let crs = parts.get(3).map(|s| s.to_string());
    
    if parts.len() >= 3 {
        if parts[1] == "min" && parts.len() >= 4 {
            Ok(Subset {
                axis_label,
                crs,
                subset_type: SubsetType::Intervals {
                    min: parts[2].parse().unwrap_or(0.0),
                    max: parts[3].parse().unwrap_or(0.0),
                    resolution: parts.get(5).and_then(|s| s.parse().ok()),
                },
            })
        } else {
            Ok(Subset {
                axis_label,
                crs,
                subset_type: SubsetType::Position {
                    value: parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0.0),
                },
            })
        }
    } else {
        Ok(Subset {
            axis_label,
            crs,
            subset_type: SubsetType::Position { value: 0.0 },
        })
    }
}
