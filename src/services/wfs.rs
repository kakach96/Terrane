use crate::models::{Bounds, Feature, GeoJsonGeometry};
use crate::utils::cql_filter::{evaluate_cql, parse_cql, CqlExpression};
use serde::{Deserialize, Serialize};
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
    /// GeoServer 厂商扩展: ECQL / CQL 表达式 (来自 FILTER=ECQL 或 CQL_FILTER 参数)
    Cql(Box<CqlExpression>),
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
                        result_type: vec!["results".to_string(), "hits".to_string()],
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
                exception: vec!["XML".to_string(), "JSON".to_string()],
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
                    sequence: SequenceElement { element: elements },
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

pub fn parse_wfs_request(
    params: &[(String, String)],
) -> Result<WfsRequest, crate::error::GeoServerError> {
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
                    _ => {
                        return Err(crate::error::GeoServerError::BadRequest(format!(
                            "Unknown request: {}",
                            value
                        )))
                    },
                }
            },
            "TYPENAME" | "TYPENAMES" => {
                type_names = Some(value.split(',').map(|s| s.trim().to_string()).collect())
            },
            "OUTPUTFORMAT" => output_format = Some(value.clone()),
            "RESULTTYPE" => result_type = Some(value.clone()),
            "PROPERTYNAME" => {
                property_name = Some(value.split(',').map(|s| s.trim().to_string()).collect())
            },
            "MAXFEATURES" | "MAXFEATURE" => max_features = value.parse().ok(),
            "STARTINDEX" => start_index = value.parse().ok(),
            "SRSNAME" => srs_name = Some(value.clone()),
            "FILTER" => filter = Some(parse_filter(value)?),
            "CQL_FILTER" => {
                let expr = parse_cql(value).map_err(|e| {
                    crate::error::GeoServerError::BadRequest(format!("Invalid CQL_FILTER: {}", e))
                })?;
                filter = Some(Filter::Cql(Box::new(expr)));
            },
            "BBOX" => {
                let parts: Vec<f64> = value
                    .split(',')
                    .filter_map(|s| s.trim().parse().ok())
                    .collect();
                if parts.len() >= 4 {
                    bbox = Some(Bbox {
                        minx: parts[0],
                        miny: parts[1],
                        maxx: parts[2],
                        maxy: parts[3],
                        srs: if parts.len() >= 5 {
                            Some(parts[4].to_string())
                        } else {
                            None
                        },
                    });
                }
            },
            "FEATUREID" | "FEATURE_ID" => {
                feature_id = Some(value.split(',').map(|s| s.trim().to_string()).collect())
            },
            "GMLOBJECTID" => {
                gml_object_id = Some(value.split(',').map(|s| s.trim().to_string()).collect())
            },
            _ => {},
        }
    }

    let request = request.ok_or_else(|| {
        crate::error::GeoServerError::BadRequest("Missing REQUEST parameter".to_string())
    })?;

    if let Some(ref svc) = service {
        if svc.to_uppercase() != "WFS" {
            return Err(crate::error::GeoServerError::BadRequest(
                "Invalid service type".to_string(),
            ));
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

/// 解析 WFS `FILTER` 参数。
///
/// 与参考 GeoServer 行为一致，支持两种编码:
/// - OGC XML Filter 编码 (WFS 1.0/1.1/2.0 标准): `<Filter>...</Filter>`
/// - ECQL / CQL (GeoServer 厂商扩展): `name='x' AND bbox(geom, ...)`
pub fn parse_filter(value: &str) -> Result<Filter, crate::error::GeoServerError> {
    let trimmed = value.trim();
    if trimmed.starts_with('<') {
        return parse_ogc_filter_xml(trimmed);
    }
    let expr = parse_cql(trimmed).map_err(|e| {
        crate::error::GeoServerError::BadRequest(format!("Invalid FILTER expression: {}", e))
    })?;
    Ok(Filter::Cql(Box::new(expr)))
}

// ---------------------------------------------------------------------------
// OGC XML Filter 编码解析
// ---------------------------------------------------------------------------

/// 简化的 XML 节点树，由 quick-xml 事件构建
struct XmlNode {
    name: String,
    attrs: Vec<(String, String)>,
    text: String,
    children: Vec<XmlNode>,
}

impl XmlNode {
    fn attr(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
}

/// 去掉命名空间前缀，返回本地元素名
fn xml_local_name(raw: &[u8]) -> String {
    let s = String::from_utf8_lossy(raw).to_string();
    s.rsplit(':').next().unwrap_or(&s).to_string()
}

/// 将 XML 字符串解析为节点树
fn parse_xml_nodes(xml: &str) -> Result<Vec<XmlNode>, String> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    let mut buf = Vec::new();

    let mut stack: Vec<XmlNode> = Vec::new();
    let mut roots: Vec<XmlNode> = Vec::new();
    let mut text_buf = String::new();

    let flush_text = |stack: &mut Vec<XmlNode>, text_buf: &mut String| {
        if !text_buf.is_empty() {
            if let Some(top) = stack.last_mut() {
                top.text.push_str(text_buf.trim());
                top.text.push(' ');
            }
            text_buf.clear();
        }
    };
    let attach = |stack: &mut Vec<XmlNode>, roots: &mut Vec<XmlNode>, node: XmlNode| {
        if let Some(parent) = stack.last_mut() {
            parent.children.push(node);
        } else {
            roots.push(node);
        }
    };

    loop {
        match reader.read_event_into(&mut buf) {
            Err(e) => return Err(format!("XML parse error: {}", e)),
            Ok(Event::Eof) => break,
            Ok(Event::Start(ref e)) => {
                flush_text(&mut stack, &mut text_buf);
                let name = xml_local_name(e.name().as_ref());
                let attrs = e
                    .attributes()
                    .filter_map(|a| a.ok())
                    .map(|a| {
                        (
                            xml_local_name(a.key.as_ref()),
                            String::from_utf8_lossy(&a.value).to_string(),
                        )
                    })
                    .collect();
                stack.push(XmlNode {
                    name,
                    attrs,
                    text: String::new(),
                    children: vec![],
                });
            },
            Ok(Event::Empty(ref e)) => {
                flush_text(&mut stack, &mut text_buf);
                let name = xml_local_name(e.name().as_ref());
                let attrs = e
                    .attributes()
                    .filter_map(|a| a.ok())
                    .map(|a| {
                        (
                            xml_local_name(a.key.as_ref()),
                            String::from_utf8_lossy(&a.value).to_string(),
                        )
                    })
                    .collect();
                attach(
                    &mut stack,
                    &mut roots,
                    XmlNode {
                        name,
                        attrs,
                        text: String::new(),
                        children: vec![],
                    },
                );
            },
            Ok(Event::Text(ref e)) => {
                text_buf.push_str(String::from_utf8_lossy(e.as_ref()).trim());
            },
            Ok(Event::End(_)) => {
                flush_text(&mut stack, &mut text_buf);
                if let Some(node) = stack.pop() {
                    attach(&mut stack, &mut roots, node);
                }
            },
            _ => {},
        }
    }
    Ok(roots)
}

/// 将 XML 节点树转换为 OGC Filter
fn node_to_filter(node: &XmlNode) -> Result<Filter, String> {
    match node.name.as_str() {
        "Filter" => {
            let child = node
                .children
                .first()
                .ok_or_else(|| "empty <Filter> element".to_string())?;
            node_to_filter(child)
        },
        "And" | "Or" => {
            let is_and = node.name == "And";
            let children: Vec<Filter> = node
                .children
                .iter()
                .map(node_to_filter)
                .collect::<Result<_, _>>()?;
            if children.is_empty() {
                return Err(format!("empty <{}> element", node.name));
            }
            let mut iter = children.into_iter();
            let first = iter.next().unwrap();
            Ok(iter.fold(first, |acc, f| {
                if is_and {
                    Filter::And(Box::new(acc), Box::new(f))
                } else {
                    Filter::Or(Box::new(acc), Box::new(f))
                }
            }))
        },
        "Not" => {
            let child = node
                .children
                .first()
                .ok_or_else(|| "empty <Not> element".to_string())?;
            Ok(Filter::Not(Box::new(node_to_filter(child)?)))
        },
        "PropertyIsEqualTo" => comparison_filter(node, CompareOp::Equal),
        "PropertyIsNotEqualTo" => comparison_filter(node, CompareOp::NotEqual),
        "PropertyIsLessThan" => comparison_filter(node, CompareOp::LessThan),
        "PropertyIsGreaterThan" => comparison_filter(node, CompareOp::GreaterThan),
        "PropertyIsLessThanOrEqualTo" => comparison_filter(node, CompareOp::LessThanOrEqual),
        "PropertyIsGreaterThanOrEqualTo" => comparison_filter(node, CompareOp::GreaterThanOrEqual),
        "PropertyIsLike" => {
            let prop = child_text(node, "PropertyName")?;
            let pat = child_text(node, "Literal")?;
            Ok(Filter::PropertyIsLike(PropertyName(prop), Literal(pat)))
        },
        "PropertyIsNull" => {
            let prop = child_text(node, "PropertyName")?;
            Ok(Filter::PropertyIsNull(PropertyName(prop)))
        },
        "PropertyIsBetween" => {
            let prop = child_text(node, "PropertyName")?;
            let lits: Vec<String> = node
                .children
                .iter()
                .filter(|c| c.name == "Literal")
                .map(|c| c.text.trim().to_string())
                .collect();
            if lits.len() < 2 {
                return Err("<PropertyIsBetween> 需要两个 <Literal>".to_string());
            }
            Ok(Filter::PropertyIsBetween(
                PropertyName(prop),
                Literal(lits[0].clone()),
                Literal(lits[1].clone()),
            ))
        },
        "BBOX" => {
            let prop = child_text(node, "PropertyName")?;
            let (minx, miny, maxx, maxy) = parse_bbox_corners(node)?;
            Ok(Filter::BBox(
                PropertyName(prop),
                Bbox {
                    minx,
                    miny,
                    maxx,
                    maxy,
                    srs: None,
                },
            ))
        },
        "FeatureId" => {
            let fid = node.attr("fid").unwrap_or("").to_string();
            if fid.is_empty() {
                return Err("<FeatureId> 缺少 fid 属性".to_string());
            }
            Ok(Filter::FeatureId(vec![fid]))
        },
        other => Err(format!("不支持的 OGC Filter 元素: {}", other)),
    }
}

/// 获取指定名称子元素的文本内容
fn child_text(node: &XmlNode, name: &str) -> Result<String, String> {
    node.children
        .iter()
        .find(|c| c.name == name)
        .map(|c| c.text.trim().to_string())
        .ok_or_else(|| format!("<{}> 缺少 <{}> 子元素", node.name, name))
}

/// 比较操作符节点 → Filter
fn comparison_filter(node: &XmlNode, op: CompareOp) -> Result<Filter, String> {
    let prop = child_text(node, "PropertyName")?;
    let lit = child_text(node, "Literal")?;
    Ok(match op {
        CompareOp::Equal => Filter::PropertyIsEqualTo(PropertyName(prop), Literal(lit)),
        CompareOp::NotEqual => Filter::PropertyIsNotEqualTo(PropertyName(prop), Literal(lit)),
        CompareOp::LessThan => Filter::PropertyIsLessThan(PropertyName(prop), Literal(lit)),
        CompareOp::GreaterThan => Filter::PropertyIsGreaterThan(PropertyName(prop), Literal(lit)),
        CompareOp::LessThanOrEqual => {
            Filter::PropertyIsLessThanOrEqualTo(PropertyName(prop), Literal(lit))
        },
        CompareOp::GreaterThanOrEqual => {
            Filter::PropertyIsGreaterThanOrEqualTo(PropertyName(prop), Literal(lit))
        },
    })
}

/// 从 BBOX 节点的 Envelope (lowerCorner/upperCorner) 提取角点
fn parse_bbox_corners(node: &XmlNode) -> Result<(f64, f64, f64, f64), String> {
    let envelope = node
        .children
        .iter()
        .find(|c| c.name == "Envelope")
        .ok_or_else(|| "<BBOX> 缺少 <Envelope>".to_string())?;
    let lower = child_text(envelope, "lowerCorner")?;
    let upper = child_text(envelope, "upperCorner")?;
    let parse_pair = |s: &str| -> Result<(f64, f64), String> {
        let parts: Vec<f64> = s
            .split_whitespace()
            .filter_map(|p| p.parse().ok())
            .collect();
        if parts.len() >= 2 {
            Ok((parts[0], parts[1]))
        } else {
            Err(format!("无效的角点: '{}'", s))
        }
    };
    let (minx, miny) = parse_pair(&lower)?;
    let (maxx, maxy) = parse_pair(&upper)?;
    Ok((minx, miny, maxx, maxy))
}

fn parse_ogc_filter_xml(xml: &str) -> Result<Filter, crate::error::GeoServerError> {
    let roots = parse_xml_nodes(xml).map_err(|e| {
        crate::error::GeoServerError::BadRequest(format!("Invalid FILTER XML: {}", e))
    })?;
    let node = roots
        .first()
        .ok_or_else(|| crate::error::GeoServerError::BadRequest("Empty FILTER XML".to_string()))?;
    node_to_filter(node)
        .map_err(|e| crate::error::GeoServerError::BadRequest(format!("Invalid FILTER XML: {}", e)))
}

/// 属性比较操作符
#[derive(Clone, Copy)]
enum CompareOp {
    Equal,
    NotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
}

/// 属性比较: 双方都能解析为数值时按数值比较, 否则按字符串比较
fn compare_property(feature: &Feature, property: &str, literal: &str, op: CompareOp) -> bool {
    let Some(val) = feature.properties.get(property) else {
        return false;
    };
    let val_str = val.to_string();
    if let (Ok(n), Ok(l)) = (val_str.parse::<f64>(), literal.parse::<f64>()) {
        return match op {
            CompareOp::Equal => n == l,
            CompareOp::NotEqual => n != l,
            CompareOp::LessThan => n < l,
            CompareOp::LessThanOrEqual => n <= l,
            CompareOp::GreaterThan => n > l,
            CompareOp::GreaterThanOrEqual => n >= l,
        };
    }
    match op {
        CompareOp::Equal => val_str == literal,
        CompareOp::NotEqual => val_str != literal,
        _ => false,
    }
}

/// SQL 风格通配符匹配 (`%` 任意多个字符, `_` 单个字符)
fn wildcard_match(text: &str, pattern: &str) -> bool {
    let mut ti = 0;
    let mut pi = 0;
    let text_bytes = text.as_bytes();
    let pat_bytes = pattern.as_bytes();
    let mut star = None;

    while ti < text_bytes.len() {
        if pi < pat_bytes.len() && (pat_bytes[pi] == b'_' || pat_bytes[pi] == text_bytes[ti]) {
            ti += 1;
            pi += 1;
        } else if pi < pat_bytes.len() && pat_bytes[pi] == b'%' {
            star = Some((ti, pi));
            pi += 1;
        } else if let Some((st, sp)) = star {
            ti = st + 1;
            pi = sp + 1;
            star = Some((ti, pi));
        } else {
            return false;
        }
    }

    while pi < pat_bytes.len() && pat_bytes[pi] == b'%' {
        pi += 1;
    }

    pi == pat_bytes.len()
}

/// 对要素应用 OGC/ECQL 过滤器，返回是否匹配
pub fn validate_filter(feature: &Feature, filter: &Filter) -> bool {
    match filter {
        Filter::And(f1, f2) => validate_filter(feature, f1) && validate_filter(feature, f2),
        Filter::Or(f1, f2) => validate_filter(feature, f1) || validate_filter(feature, f2),
        Filter::Not(f) => !validate_filter(feature, f),
        Filter::PropertyIsEqualTo(prop, lit) => {
            compare_property(feature, &prop.0, &lit.0, CompareOp::Equal)
        },
        Filter::PropertyIsNotEqualTo(prop, lit) => {
            compare_property(feature, &prop.0, &lit.0, CompareOp::NotEqual)
        },
        Filter::PropertyIsLessThan(prop, lit) => {
            compare_property(feature, &prop.0, &lit.0, CompareOp::LessThan)
        },
        Filter::PropertyIsGreaterThan(prop, lit) => {
            compare_property(feature, &prop.0, &lit.0, CompareOp::GreaterThan)
        },
        Filter::PropertyIsLessThanOrEqualTo(prop, lit) => {
            compare_property(feature, &prop.0, &lit.0, CompareOp::LessThanOrEqual)
        },
        Filter::PropertyIsGreaterThanOrEqualTo(prop, lit) => {
            compare_property(feature, &prop.0, &lit.0, CompareOp::GreaterThanOrEqual)
        },
        Filter::PropertyIsLike(prop, lit) => match feature.properties.get(&prop.0) {
            Some(val) => wildcard_match(&val.to_string(), &lit.0),
            None => false,
        },
        Filter::PropertyIsNull(prop) => {
            !feature.properties.contains_key(&prop.0)
                || matches!(
                    feature.properties.get(&prop.0),
                    Some(crate::models::PropertyValue::Null)
                )
        },
        Filter::PropertyIsBetween(prop, low, high) => feature
            .properties
            .get(&prop.0)
            .and_then(|v| v.to_string().parse::<f64>().ok())
            .zip(low.0.parse::<f64>().ok())
            .zip(high.0.parse::<f64>().ok())
            .map(|((v, l), h)| v >= l && v <= h)
            .unwrap_or(false),
        Filter::BBox(_, bbox) => {
            let bounds = bbox.to_bounds();
            match &feature.geometry {
                crate::models::GeoJsonGeometry::Point { coordinates } => {
                    if coordinates.len() >= 2 {
                        bounds.contains(coordinates[0], coordinates[1])
                    } else {
                        false
                    }
                },
                _ => true,
            }
        },
        Filter::FeatureId(ids) => ids.contains(&feature.id),
        Filter::Cql(expr) => evaluate_cql(feature, expr),
    }
}

/// 解析 WFS-T Transaction XML
///
/// 支持 WFS 2.0 Transaction 格式：
/// - wfs:Insert / wfs:Update / wfs:Delete
#[allow(unused_assignments, unused_variables)]
pub fn parse_transaction_xml(
    xml_text: &str,
) -> Result<TransactionRequest, crate::error::GeoServerError> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

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
                    .split(':')
                    .last()
                    .unwrap_or("")
                    .to_string();
                text_content.clear();

                match tag.as_str() {
                    "Insert" => {
                        in_insert = true;
                        current_insert = Some(InsertElement {
                            type_name: String::new(),
                            features: Vec::new(),
                        });
                    },
                    "Update" => {
                        in_update = true;
                        current_update = Some(UpdateElement {
                            type_name: String::new(),
                            filter: None,
                            properties: Vec::new(),
                        });
                    },
                    "Delete" => {
                        in_delete = true;
                        current_delete = Some(DeleteElement {
                            type_name: String::new(),
                            filter: Filter::FeatureId(vec![]),
                        });
                    },
                    "TypeName" | "Typename" | "TYPENAME" => {
                        text_content.clear();
                    },
                    "Feature" | "feature" if in_insert => {
                        in_feature = true;
                        current_feature = Some(Feature::new(
                            GeoJsonGeometry::Point {
                                coordinates: vec![0.0, 0.0],
                            },
                            std::collections::HashMap::new(),
                        ));
                    },
                    "Property" | "property" if in_update => {
                        in_property = true;
                        current_property = Some(PropertyElement {
                            name: String::new(),
                            value: String::new(),
                        });
                    },
                    "Name" | "name" if in_property => {
                        in_name = true;
                        text_content.clear();
                    },
                    "Value" | "value" if in_property => {
                        in_value = true;
                        text_content.clear();
                    },
                    "Point" | "point" => {},
                    "FeatureId" | "featureId" | "FEATUREID" if in_delete => {
                        // Collect ogc:FeatureId fid attributes into the Delete filter
                        for attr in e.attributes().with_checks(false) {
                            if let Ok(a) = attr {
                                let key = String::from_utf8_lossy(a.key.as_ref())
                                    .split(':')
                                    .last()
                                    .unwrap_or("")
                                    .to_lowercase();
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
                    },
                    "LineString" | "linestring" => {
                        in_linestring = true;
                    },
                    "Polygon" | "polygon" => {
                        in_polygon = true;
                    },
                    "exterior" | "Exterior" => {
                        in_exterior = true;
                    },
                    "LinearRing" | "linearring" => {},
                    "pos" | "Pos" => in_pos = true,
                    "posList" | "PosList" => in_poslist = true,
                    _ => {},
                }
            },
            Ok(Event::Text(ref e)) => {
                text_content = e.unescape().unwrap_or_default().to_string();
            },
            Ok(Event::End(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref())
                    .split(':')
                    .last()
                    .unwrap_or("")
                    .to_string();

                match tag.as_str() {
                    "Insert" => {
                        if let Some(insert) = current_insert.take() {
                            transaction.inserts.push(insert);
                        }
                        in_insert = false;
                    },
                    "Update" => {
                        if let Some(update) = current_update.take() {
                            transaction.updates.push(update);
                        }
                        in_update = false;
                    },
                    "Delete" => {
                        if let Some(delete) = current_delete.take() {
                            transaction.deletes.push(delete);
                        }
                        in_delete = false;
                    },
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
                    },
                    "Feature" | "feature" if in_insert => {
                        if let Some(feature) = current_feature.take() {
                            if let Some(ref mut insert) = current_insert {
                                insert.features.push(feature);
                            }
                        }
                        in_feature = false;
                    },
                    "Property" | "property" if in_update => {
                        if let Some(prop) = current_property.take() {
                            if let Some(ref mut update) = current_update {
                                update.properties.push(prop);
                            }
                        }
                        in_property = false;
                    },
                    "Name" | "name" if in_name => {
                        if let Some(ref mut prop) = current_property {
                            prop.name = text_content.trim().to_string();
                        }
                        in_name = false;
                    },
                    "Value" | "value" if in_value => {
                        if let Some(ref mut prop) = current_property {
                            prop.value = text_content.trim().to_string();
                        }
                        in_value = false;
                    },
                    "pos" | "Pos" => {
                        let coords: Vec<f64> = text_content
                            .split_whitespace()
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
                    },
                    "posList" | "PosList" => {
                        let coords: Vec<f64> = text_content
                            .split_whitespace()
                            .filter_map(|v| v.parse().ok())
                            .collect();
                        let points: Vec<Vec<f64>> = coords
                            .chunks(2)
                            .filter(|c| c.len() == 2)
                            .map(|c| vec![c[0], c[1]])
                            .collect();

                        if let Some(ref mut feature) = current_feature {
                            if in_linestring || (in_polygon && in_exterior) {
                                feature.geometry = if in_polygon {
                                    GeoJsonGeometry::Polygon {
                                        coordinates: vec![points],
                                    }
                                } else {
                                    GeoJsonGeometry::LineString {
                                        coordinates: points,
                                    }
                                };
                            }
                        }
                        in_poslist = false;
                    },
                    "LineString" | "linestring" => in_linestring = false,
                    "Polygon" | "polygon" => in_polygon = false,
                    "exterior" | "Exterior" => in_exterior = false,
                    _ => {},
                }
                text_content.clear();
            },
            Ok(Event::Eof) => break,
            Err(e) => {
                warn!("[WFS-T] XML 解析警告: {}", e);
                break;
            },
            _ => {},
        }
    }

    Ok(transaction)
}
