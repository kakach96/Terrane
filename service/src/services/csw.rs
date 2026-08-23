//! # CSW (Catalog Service for the Web) 2.0.2 implementation
//!
//! First CSW surface for Terrane: GetCapabilities / DescribeRecord / GetRecords /
//! GetRecordById / GetDomain, served at `/csw` (KVP GET/POST + XML POST). The
//! reference GeoServer at :18080 has CSW disabled, so this follows the OGC CSW
//! 2.0.2 schema directly (OGC 07-006r1).
//!
//! Catalog records are derived from the Terrane layer catalog: every published
//! layer is exposed as a Dublin Core `csw:Record` (Summary / Brief / Full) with
//! its WGS84 bounding box. A minimal CQL constraint (`Title = '...'` /
//! `Identifier = '...'` / `Subject = '...'`, `=` and `like`) is supported.

use crate::error::GeoServerError;
use crate::models::Layer;
use crate::services::wfs;

/// CSW 2.0.2 namespace URIs.
pub const CSW_NS: &str = "http://www.opengis.net/cat/csw/2.0.2";
pub const DC_NS: &str = "http://purl.org/dc/elements/1.1/";
pub const DCT_NS: &str = "http://purl.org/dc/terms/";
pub const OWS_NS: &str = "http://www.opengis.net/ows";
pub const OGC_NS: &str = "http://www.opengis.net/ogc";
pub const GML_NS: &str = "http://www.opengis.net/gml";
pub const XLINK_NS: &str = "http://www.w3.org/1999/xlink";
pub const XSD_NS: &str = "http://www.w3.org/2001/XMLSchema";

/// Default output schema (CSW 2.0.2 `csw:Record`).
const DEFAULT_OUTPUT_SCHEMA: &str = "http://www.opengis.net/cat/csw/2.0.2";

// ---------------------------------------------------------------------------
// Request model
// ---------------------------------------------------------------------------

/// Result type of a GetRecords request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CswResultType {
    Hits,
    Results,
    Validate,
}

impl CswResultType {
    fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "hits" => CswResultType::Hits,
            "validate" => CswResultType::Validate,
            _ => CswResultType::Results,
        }
    }
}

/// Requested record element set (Dublin Core subset rendered per record).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CswElementSet {
    Summary,
    Brief,
    Full,
}

impl CswElementSet {
    fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "brief" => CswElementSet::Brief,
            "full" => CswElementSet::Full,
            _ => CswElementSet::Summary,
        }
    }
}

/// Parameters of a GetRecords request (shared by KVP and XML encodings).
#[derive(Debug, Clone)]
pub struct GetRecordsQuery {
    pub result_type: CswResultType,
    pub element_set: CswElementSet,
    pub output_schema: Option<String>,
    pub start_position: u32,
    pub max_records: u32,
    pub constraint: Option<String>,
}

impl Default for GetRecordsQuery {
    fn default() -> Self {
        GetRecordsQuery {
            result_type: CswResultType::Results,
            element_set: CswElementSet::Summary,
            output_schema: None,
            start_position: 1,
            max_records: 10,
            constraint: None,
        }
    }
}

/// A parsed CSW operation (shared dispatch target for KVP and XML encodings).
#[derive(Debug, Clone)]
pub enum CswOperation {
    GetCapabilities,
    DescribeRecord {
        typenames: Vec<String>,
        output_format: Option<String>,
    },
    GetRecords {
        typenames: Vec<String>,
        query: GetRecordsQuery,
    },
    GetRecordById {
        ids: Vec<String>,
        element_set: CswElementSet,
        output_schema: Option<String>,
    },
    GetDomain {
        parameter_name: Option<String>,
    },
}

// ---------------------------------------------------------------------------
// KVP request parsing
// ---------------------------------------------------------------------------

fn first_param<'a>(params: &'a [(String, String)], key: &str) -> Option<&'a str> {
    params
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .map(|(_, v)| v.as_str())
}

fn split_list(s: &str) -> Vec<String> {
    s.split(',').map(|t| t.trim().to_string()).collect()
}

/// Parse a CSW KVP request (`SERVICE=CSW&REQUEST=...`).
pub fn parse_csw_request(params: &[(String, String)]) -> Result<CswOperation, GeoServerError> {
    if let Some(svc) = first_param(params, "service") {
        if !svc.eq_ignore_ascii_case("CSW") {
            return Err(GeoServerError::BadRequest(
                "Invalid service type".to_string(),
            ));
        }
    }

    let request = first_param(params, "request")
        .ok_or_else(|| GeoServerError::BadRequest("Missing REQUEST parameter".to_string()))?
        .to_uppercase();

    match request.as_str() {
        "GETCAPABILITIES" => Ok(CswOperation::GetCapabilities),
        "DESCRIBERECORD" => Ok(CswOperation::DescribeRecord {
            typenames: first_param(params, "typenames")
                .map(split_list)
                .unwrap_or_else(|| vec!["csw:Record".to_string()]),
            output_format: first_param(params, "outputformat").map(|s| s.to_string()),
        }),
        "GETRECORDS" => Ok(CswOperation::GetRecords {
            typenames: first_param(params, "typenames")
                .map(split_list)
                .unwrap_or_else(|| vec!["csw:Record".to_string()]),
            query: GetRecordsQuery {
                result_type: CswResultType::parse(
                    first_param(params, "resulttype").unwrap_or("results"),
                ),
                element_set: CswElementSet::parse(
                    first_param(params, "elementsetname").unwrap_or("summary"),
                ),
                output_schema: first_param(params, "outputschema").map(|s| s.to_string()),
                start_position: first_param(params, "startposition")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(1),
                max_records: first_param(params, "maxrecords")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(10),
                constraint: first_param(params, "constraint").map(|s| s.to_string()),
            },
        }),
        "GETRECORDBYID" => Ok(CswOperation::GetRecordById {
            ids: first_param(params, "id")
                .map(split_list)
                .unwrap_or_default(),
            element_set: CswElementSet::parse(
                first_param(params, "elementsetname").unwrap_or("summary"),
            ),
            output_schema: first_param(params, "outputschema").map(|s| s.to_string()),
        }),
        "GETDOMAIN" => Ok(CswOperation::GetDomain {
            parameter_name: first_param(params, "parametername").map(|s| s.to_string()),
        }),
        _ => Err(GeoServerError::BadRequest(format!(
            "Unknown request: {}",
            request
        ))),
    }
}

// ---------------------------------------------------------------------------
// XML POST request parsing (reuses the shared quick-xml node tree in wfs.rs)
// ---------------------------------------------------------------------------

/// Extract a CQL string from a `csw:Constraint` node: either the inline text or
/// the first `PropertyName = Literal` pair found in a nested `ogc:Filter`.
fn extract_constraint_text(node: &wfs::XmlNode) -> Option<String> {
    if !node.text.trim().is_empty() {
        return Some(node.text.trim().to_string());
    }
    for child in &node.children {
        if let Some(cql) = filter_to_cql(child) {
            return Some(cql);
        }
    }
    None
}

fn filter_to_cql(node: &wfs::XmlNode) -> Option<String> {
    let mut props: Vec<String> = Vec::new();
    let mut lits: Vec<String> = Vec::new();
    collect_prop_lit(node, &mut props, &mut lits);
    if props.len() == 1 && !lits.is_empty() {
        Some(format!("{} = '{}'", props[0], lits[0]))
    } else {
        None
    }
}

fn collect_prop_lit(node: &wfs::XmlNode, props: &mut Vec<String>, lits: &mut Vec<String>) {
    if node.name == "PropertyName" && !node.text.trim().is_empty() {
        props.push(node.text.trim().to_string());
    }
    if node.name == "Literal" && !node.text.trim().is_empty() {
        lits.push(node.text.trim().to_string());
    }
    for child in &node.children {
        collect_prop_lit(child, props, lits);
    }
}

/// Parse a CSW XML POST request body.
pub fn parse_csw_post(xml: &str) -> Result<CswOperation, GeoServerError> {
    let roots = wfs::parse_xml_nodes(xml).map_err(GeoServerError::BadRequest)?;
    let root = roots
        .first()
        .ok_or_else(|| GeoServerError::BadRequest("Empty CSW request".to_string()))?;

    match root.name.as_str() {
        "GetCapabilities" => Ok(CswOperation::GetCapabilities),
        "DescribeRecord" => {
            let typenames: Vec<String> = root
                .children_named("TypeName")
                .into_iter()
                .map(|c| c.text.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            Ok(CswOperation::DescribeRecord {
                typenames: if typenames.is_empty() {
                    vec!["csw:Record".to_string()]
                } else {
                    typenames
                },
                output_format: root.attr("outputFormat").map(|s| s.to_string()),
            })
        },
        "GetRecords" => {
            let mut query = GetRecordsQuery {
                result_type: CswResultType::parse(root.attr("resultType").unwrap_or("results")),
                output_schema: root.attr("outputSchema").map(|s| s.to_string()),
                start_position: root
                    .attr("startPosition")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(1),
                max_records: root
                    .attr("maxRecords")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(10),
                ..GetRecordsQuery::default()
            };
            let mut typenames = vec!["csw:Record".to_string()];

            for qnode in root.children_named("Query") {
                if let Some(tn) = qnode.attr("typeNames") {
                    typenames = split_list(tn);
                }
                if let Some(es) = qnode.children_named("ElementSetName").first() {
                    query.element_set = CswElementSet::parse(es.text.trim());
                }
                if let Some(con) = qnode.children_named("Constraint").first() {
                    query.constraint = extract_constraint_text(con);
                }
            }

            Ok(CswOperation::GetRecords { typenames, query })
        },
        "GetRecordById" => {
            let ids: Vec<String> = root
                .children_named("Id")
                .into_iter()
                .map(|c| c.text.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            Ok(CswOperation::GetRecordById {
                ids,
                element_set: CswElementSet::parse(root.attr("elementSetName").unwrap_or("summary")),
                output_schema: root.attr("outputSchema").map(|s| s.to_string()),
            })
        },
        "GetDomain" => Ok(CswOperation::GetDomain {
            parameter_name: root
                .children_named("ParameterName")
                .first()
                .map(|c| c.text.trim().to_string()),
        }),
        other => Err(GeoServerError::BadRequest(format!(
            "Unknown CSW request: {}",
            other
        ))),
    }
}

// ---------------------------------------------------------------------------
// XML generation
// ---------------------------------------------------------------------------

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn csw_attrs() -> String {
    format!(
        r#"xmlns:csw="{}" xmlns:dc="{}" xmlns:dct="{}" xmlns:ows="{}" xmlns:ogc="{}" xmlns:gml="{}" xmlns:xlink="{}" xmlns:xsd="{}""#,
        CSW_NS, DC_NS, DCT_NS, OWS_NS, OGC_NS, GML_NS, XLINK_NS, XSD_NS
    )
}

/// Build the CSW 2.0.2 GetCapabilities document.
pub fn build_capabilities(base_url: &str) -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str(&format!(
        "<csw:Capabilities service=\"CSW\" version=\"2.0.2\" {}>\n",
        csw_attrs()
    ));
    out.push_str("  <ows:ServiceIdentification>\n");
    out.push_str("    <ows:Title>Terrane CSW</ows:Title>\n");
    out.push_str(
        "    <ows:Abstract>Cloud-native spatial data server powered by Rust — Catalog Service for the Web</ows:Abstract>\n",
    );
    out.push_str("    <ows:Keywords><ows:Keyword>CSW</ows:Keyword><ows:Keyword>Catalog Service</ows:Keyword></ows:Keywords>\n");
    out.push_str("    <ows:ServiceType>CSW</ows:ServiceType>\n");
    out.push_str("    <ows:ServiceTypeVersion>2.0.2</ows:ServiceTypeVersion>\n");
    out.push_str("    <ows:Fees>NONE</ows:Fees>\n");
    out.push_str("    <ows:AccessConstraints>NONE</ows:AccessConstraints>\n");
    out.push_str("  </ows:ServiceIdentification>\n");
    out.push_str("  <ows:ServiceProvider>\n");
    out.push_str("    <ows:ProviderName>Terrane</ows:ProviderName>\n");
    out.push_str("  </ows:ServiceProvider>\n");
    out.push_str("  <ows:OperationsMetadata>\n");
    for op in [
        "GetCapabilities",
        "DescribeRecord",
        "GetRecords",
        "GetRecordById",
        "GetDomain",
    ] {
        out.push_str(&format!(
            "    <ows:Operation name=\"{}\">\n\
             <ows:DCP><ows:HTTP><ows:Get xlink:href=\"{}/csw?\"/><ows:Post xlink:href=\"{}/csw\"/></ows:HTTP></ows:DCP>\n\
             <ows:Parameter name=\"service\"><ows:Value>CSW</ows:Value></ows:Parameter>\n\
             <ows:Parameter name=\"version\"><ows:Value>2.0.2</ows:Value></ows:Parameter>\n\
             </ows:Operation>\n",
            op, base_url, base_url
        ));
    }
    out.push_str("  </ows:OperationsMetadata>\n");
    out.push_str("  <csw:FilterCapabilities>\n");
    out.push_str("    <ogc:Filter_Capabilities>\n");
    out.push_str("      <ogc:Scalar_Capabilities>\n");
    out.push_str("        <ogc:LogicalOperators/>\n");
    out.push_str("        <ogc:ComparisonOperators>\n");
    for op in ["LessThan", "GreaterThan", "EqualTo", "Like", "Between"] {
        out.push_str(&format!("          <ogc:{} />\n", op));
    }
    out.push_str("        </ogc:ComparisonOperators>\n");
    out.push_str("      </ogc:Scalar_Capabilities>\n");
    out.push_str("      <ogc:Spatial_Capabilities>\n");
    out.push_str("        <ogc:SpatialOperators>\n");
    out.push_str("          <ogc:BBOX/>\n");
    out.push_str("        </ogc:SpatialOperators>\n");
    out.push_str("      </ogc:Spatial_Capabilities>\n");
    out.push_str("    </ogc:Filter_Capabilities>\n");
    out.push_str("  </csw:FilterCapabilities>\n");
    out.push_str("</csw:Capabilities>\n");
    out
}

fn bbox_xml(layer: &Layer) -> String {
    let b = &layer.lat_lon_bounds.bounds;
    format!(
        "    <ows:BoundingBox crs=\"urn:ogc:def:crs:OGC:2:84\">\n\
         <ows:LowerCorner>{} {}</ows:LowerCorner>\n\
         <ows:UpperCorner>{} {}</ows:UpperCorner>\n\
         </ows:BoundingBox>\n",
        b.minx, b.miny, b.maxx, b.maxy
    )
}

/// Render a single catalog record (Summary / Brief / Full Dublin Core subset).
pub fn record_xml(base_url: &str, layer: &Layer, element_set: CswElementSet) -> String {
    let ident = format!(
        "    <dc:identifier>{}</dc:identifier>\n",
        escape_xml(&layer.name)
    );
    let title = format!("    <dc:title>{}</dc:title>\n", escape_xml(&layer.title));
    let rtype = "    <dc:type>dataset</dc:type>\n";
    let bbox = bbox_xml(layer);

    match element_set {
        CswElementSet::Summary => format!(
            "  <csw:SummaryRecord>\n{}{}{}{}</csw:SummaryRecord>\n",
            ident, title, rtype, bbox
        ),
        CswElementSet::Brief => {
            let format = "    <dc:format>application/xml</dc:format>\n";
            format!(
                "  <csw:BriefRecord>\n{}{}{}{}{}</csw:BriefRecord>\n",
                ident, title, rtype, format, bbox
            )
        },
        CswElementSet::Full => {
            let subject = format!(
                "    <dc:subject>{}</dc:subject>\n",
                escape_xml(&layer.workspace)
            );
            let format = "    <dc:format>application/xml</dc:format>\n";
            let references = format!(
                "    <dct:references xlink:href=\"{}/geoserver/layers/{}\"/>\n",
                base_url,
                escape_xml(&layer.name)
            );
            format!(
                "  <csw:Record>\n{}{}{}{}{}{}{}</csw:Record>\n",
                ident, title, subject, rtype, format, references, bbox
            )
        },
    }
}

/// Apply a minimal CQL constraint to the catalog. Supported fields: title /
/// identifier / subject, operators `=` and `like` (case-insensitive). Unknown
/// fields or unparseable constraints return the full catalog (lenient).
fn apply_constraint(layers: &[Layer], constraint: &str) -> Vec<Layer> {
    let c = constraint.trim();
    if c.is_empty() {
        return layers.to_vec();
    }
    let lower = c.to_lowercase();
    let (field, op, value) = if let Some(pos) = lower.find(" like ") {
        let (f, v) = c.split_at(pos);
        let v = v[..].trim_start_matches(" like ").trim();
        (f.trim(), "like", unquote(v))
    } else if let Some(pos) = lower.find(" = ") {
        let (f, v) = c.split_at(pos);
        let v = v[..].trim_start_matches(" = ").trim();
        (f.trim(), "eq", unquote(v))
    } else {
        return layers.to_vec();
    };

    layers
        .iter()
        .filter(|layer| {
            let field_value = match normalize_field(field) {
                Some("title") => layer.title.to_lowercase(),
                Some("identifier") => layer.name.to_lowercase(),
                Some("subject") => layer.workspace.to_lowercase(),
                _ => return true, // unknown field → keep
            };
            let needle = value.to_lowercase();
            match op {
                "like" => field_value.contains(needle.trim_matches('%')),
                _ => field_value == needle,
            }
        })
        .cloned()
        .collect()
}

fn unquote(s: &str) -> String {
    let t = s.trim();
    if t.len() >= 2 {
        let bytes = t.as_bytes();
        if (bytes[0] == b'\'' && bytes[t.len() - 1] == b'\'')
            || (bytes[0] == b'"' && bytes[t.len() - 1] == b'"')
        {
            return t[1..t.len() - 1].to_string();
        }
    }
    t.to_string()
}

fn normalize_field(field: &str) -> Option<&str> {
    let f = field
        .trim()
        .trim_start_matches("dc:")
        .trim_start_matches("dct:")
        .to_lowercase();
    match f.as_str() {
        "title" => Some("title"),
        "identifier" | "id" => Some("identifier"),
        "subject" | "workspace" => Some("subject"),
        _ => None,
    }
}

/// Build the CSW GetRecordsResponse document.
pub fn build_get_records(base_url: &str, layers: &[Layer], query: &GetRecordsQuery) -> String {
    let filtered = match &query.constraint {
        Some(c) => apply_constraint(layers, c),
        None => layers.to_vec(),
    };
    let matched = filtered.len();
    let start = query.start_position.max(1) as usize;
    let max_records = query.max_records as usize;
    let page: Vec<&Layer> = filtered.iter().skip(start - 1).take(max_records).collect();
    // resultType=hits reports the matched count only (no records are returned).
    let returned = if query.result_type == CswResultType::Hits {
        0
    } else {
        page.len()
    };
    let next_record = if start + returned <= matched && returned > 0 {
        (start + returned) as u32
    } else {
        0
    };
    let timestamp = chrono::Utc::now().to_rfc3339();
    let element_set = match query.element_set {
        CswElementSet::Summary => "summary",
        CswElementSet::Brief => "brief",
        CswElementSet::Full => "full",
    };

    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str(&format!("<csw:GetRecordsResponse {}>\n", csw_attrs()));
    out.push_str(&format!(
        "  <csw:SearchStatus timestamp=\"{}\"/>\n",
        timestamp
    ));
    out.push_str(&format!(
        "  <csw:SearchResults numberOfRecordsMatched=\"{}\" numberOfRecordsReturned=\"{}\" nextRecord=\"{}\" elementSet=\"{}\">\n",
        matched, returned, next_record, element_set
    ));

    if query.result_type != CswResultType::Hits {
        for layer in page {
            out.push_str(&record_xml(base_url, layer, query.element_set));
        }
    }
    out.push_str("  </csw:SearchResults>\n");
    out.push_str("</csw:GetRecordsResponse>\n");
    out
}

/// Build the CSW GetRecordByIdResponse document.
pub fn build_get_record_by_id(
    base_url: &str,
    layers: &[Layer],
    ids: &[String],
    element_set: CswElementSet,
) -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str(&format!("<csw:GetRecordByIdResponse {}>\n", csw_attrs()));
    for id in ids {
        if let Some(layer) = layers.iter().find(|l| l.name == *id) {
            out.push_str(&record_xml(base_url, layer, element_set));
        }
    }
    out.push_str("</csw:GetRecordByIdResponse>\n");
    out
}

/// Build the CSW DescribeRecordResponse document (a simplified inline schema
/// per requested type name).
pub fn build_describe_record(typenames: &[String]) -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str(&format!("<csw:DescribeRecordResponse {}>\n", csw_attrs()));
    for tn in typenames {
        out.push_str(&format!(
            "  <csw:SchemaComponent targetNamespace=\"{}\" parentSchema=\"http://schemas.opengis.net/csw/2.0.2/record.xsd\" schemaLanguage=\"XMLSCHEMA\">\n",
            CSW_NS
        ));
        out.push_str(&describe_schema(tn));
        out.push_str("  </csw:SchemaComponent>\n");
    }
    out.push_str("</csw:DescribeRecordResponse>\n");
    out
}

fn describe_schema(typename: &str) -> String {
    match typename {
        "gmd:MD_Metadata" => {
            "    <csw:DescribeRecord>\n      <xsd:schema xmlns:xsd=\"http://www.w3.org/2001/XMLSchema\">\n        <xsd:complexType name=\"MD_Metadata\">\n          <xsd:sequence>\n            <xsd:element name=\"fileIdentifier\" type=\"xsd:string\"/>\n            <xsd:element name=\"language\" type=\"xsd:string\"/>\n            <xsd:element name=\"title\" type=\"xsd:string\"/>\n          </xsd:sequence>\n        </xsd:complexType>\n      </xsd:schema>\n    </csw:DescribeRecord>\n"
                .to_string()
        },
        _ => {
            "    <csw:DescribeRecord>\n      <xsd:schema xmlns:xsd=\"http://www.w3.org/2001/XMLSchema\">\n        <xsd:complexType name=\"Record\">\n          <xsd:sequence>\n            <xsd:element name=\"identifier\" type=\"xsd:string\"/>\n            <xsd:element name=\"title\" type=\"xsd:string\"/>\n            <xsd:element name=\"subject\" type=\"xsd:string\"/>\n            <xsd:element name=\"type\" type=\"xsd:string\"/>\n            <xsd:element name=\"format\" type=\"xsd:string\"/>\n          </xsd:sequence>\n        </xsd:complexType>\n      </xsd:schema>\n    </csw:DescribeRecord>\n"
                .to_string()
        },
    }
}

/// Build the CSW GetDomainResponse document for a parameter.
pub fn build_get_domain(parameter_name: &str) -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str(&format!("<csw:GetDomainResponse {}>\n", csw_attrs()));
    out.push_str("  <csw:DomainValues>\n");
    out.push_str(&format!(
        "    <csw:ParameterName>{}</csw:ParameterName>\n",
        escape_xml(parameter_name)
    ));
    out.push_str("    <csw:ListOfValues>\n");
    for v in ["hits", "results", "validate"] {
        out.push_str(&format!("      <csw:Value>{}</csw:Value>\n", v));
    }
    out.push_str("    </csw:ListOfValues>\n");
    out.push_str("  </csw:DomainValues>\n");
    out.push_str("</csw:GetDomainResponse>\n");
    out
}

/// Build an OWS exception report.
pub fn build_exception(exception_code: &str, text: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <ows:ExceptionReport xmlns:ows=\"{}\" version=\"1.0.0\">\n\
         <ows:Exception exceptionCode=\"{}\">\n\
         <ows:ExceptionText>{}</ows:ExceptionText>\n\
         </ows:Exception>\n\
         </ows:ExceptionReport>\n",
        OWS_NS,
        exception_code,
        escape_xml(text)
    )
}

#[allow(dead_code)]
fn _schema_default() -> &'static str {
    DEFAULT_OUTPUT_SCHEMA
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{BoundingBox, Bounds, CoordinateReferenceSystem};

    fn test_layer(name: &str, title: &str, workspace: &str) -> Layer {
        let mut layer = Layer::new(
            name.to_string(),
            title.to_string(),
            workspace.to_string(),
            "shapes".to_string(),
            CoordinateReferenceSystem::EPSG4326,
        );
        layer.lat_lon_bounds = BoundingBox::new(
            CoordinateReferenceSystem::EPSG4326,
            Bounds::new(-10.0, -5.0, 10.0, 5.0),
        );
        layer
    }

    fn params(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn test_parse_kvp_get_records() {
        let op = parse_csw_request(&params(&[
            ("service", "CSW"),
            ("version", "2.0.2"),
            ("request", "GetRecords"),
            ("typeNames", "csw:Record"),
            ("resultType", "results"),
            ("elementSetName", "brief"),
            ("maxRecords", "5"),
            ("startPosition", "2"),
        ]))
        .unwrap();
        match op {
            CswOperation::GetRecords { typenames, query } => {
                assert_eq!(typenames, vec!["csw:Record"]);
                assert_eq!(query.result_type, CswResultType::Results);
                assert_eq!(query.element_set, CswElementSet::Brief);
                assert_eq!(query.max_records, 5);
                assert_eq!(query.start_position, 2);
            },
            _ => panic!("expected GetRecords"),
        }
    }

    #[test]
    fn test_parse_kvp_operations() {
        // GetCapabilities
        assert!(matches!(
            parse_csw_request(&params(&[
                ("service", "CSW"),
                ("request", "GetCapabilities")
            ]))
            .unwrap(),
            CswOperation::GetCapabilities
        ));
        // DescribeRecord with typeNames
        match parse_csw_request(&params(&[
            ("service", "CSW"),
            ("request", "DescribeRecord"),
            ("typeNames", "csw:Record,gmd:MD_Metadata"),
        ]))
        .unwrap()
        {
            CswOperation::DescribeRecord { typenames, .. } => {
                assert_eq!(typenames.len(), 2);
                assert_eq!(typenames[1], "gmd:MD_Metadata");
            },
            _ => panic!("expected DescribeRecord"),
        }
        // GetRecordById
        match parse_csw_request(&params(&[
            ("service", "CSW"),
            ("request", "GetRecordById"),
            ("id", "world,usa"),
        ]))
        .unwrap()
        {
            CswOperation::GetRecordById { ids, .. } => assert_eq!(ids, vec!["world", "usa"]),
            _ => panic!("expected GetRecordById"),
        }
        // wrong service
        assert!(parse_csw_request(&params(&[
            ("service", "WFS"),
            ("request", "GetCapabilities")
        ]))
        .is_err());
    }

    #[test]
    fn test_parse_csw_post_get_records_xml() {
        let xml = r#"<?xml version="1.0"?>
        <csw:GetRecords service="CSW" version="2.0.2" resultType="results"
            outputSchema="http://www.opengis.net/cat/csw/2.0.2"
            startPosition="1" maxRecords="25"
            xmlns:csw="http://www.opengis.net/cat/csw/2.0.2">
          <csw:Query typeNames="csw:Record">
            <csw:ElementSetName>full</csw:ElementSetName>
            <csw:Constraint version="1.1.0">
              <ogc:Filter xmlns:ogc="http://www.opengis.net/ogc">
                <ogc:PropertyIsEqualTo>
                  <ogc:PropertyName>Title</ogc:PropertyName>
                  <ogc:Literal>World</ogc:Literal>
                </ogc:PropertyIsEqualTo>
              </ogc:Filter>
            </csw:Constraint>
          </csw:Query>
        </csw:GetRecords>"#;
        match parse_csw_post(xml).unwrap() {
            CswOperation::GetRecords { typenames, query } => {
                assert_eq!(typenames, vec!["csw:Record"]);
                assert_eq!(query.element_set, CswElementSet::Full);
                assert_eq!(query.max_records, 25);
                assert!(query
                    .constraint
                    .as_deref()
                    .map(|c| c.contains("Title") && c.contains("World"))
                    .unwrap_or(false));
            },
            _ => panic!("expected GetRecords"),
        }
    }

    #[test]
    fn test_parse_csw_post_get_record_by_id_xml() {
        let xml = r#"<csw:GetRecordById xmlns:csw="http://www.opengis.net/cat/csw/2.0.2"
            service="CSW" version="2.0.2" outputSchema="http://www.opengis.net/cat/csw/2.0.2"
            elementSetName="brief">
          <csw:Id>world</csw:Id>
        </csw:GetRecordById>"#;
        match parse_csw_post(xml).unwrap() {
            CswOperation::GetRecordById {
                ids, element_set, ..
            } => {
                assert_eq!(ids, vec!["world"]);
                assert_eq!(element_set, CswElementSet::Brief);
            },
            _ => panic!("expected GetRecordById"),
        }
    }

    #[test]
    fn test_build_capabilities_structure() {
        let xml = build_capabilities("http://127.0.0.1:8080");
        assert!(xml.contains("<csw:Capabilities service=\"CSW\" version=\"2.0.2\""));
        assert!(xml.contains("<ows:OperationsMetadata>"));
        for op in [
            "GetCapabilities",
            "DescribeRecord",
            "GetRecords",
            "GetRecordById",
            "GetDomain",
        ] {
            assert!(xml.contains(&format!("<ows:Operation name=\"{}\">", op)));
        }
        assert!(xml.contains("<csw:FilterCapabilities>"));
        assert!(xml.contains("http://127.0.0.1:8080/csw?"));
    }

    #[test]
    fn test_apply_constraint() {
        let layers = vec![
            test_layer("world", "World", "default"),
            test_layer("usa", "USA States", "default"),
            test_layer("parks", "Parks", "gis"),
        ];
        // exact title
        let r = apply_constraint(&layers, "Title = 'World'");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].name, "world");
        // like on title (case-insensitive)
        let r = apply_constraint(&layers, "dc:title like 'usa%'");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].name, "usa");
        // subject match
        let r = apply_constraint(&layers, "Subject = 'gis'");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].name, "parks");
        // unknown field → full catalog (lenient)
        let r = apply_constraint(&layers, "bogus = 'x'");
        assert_eq!(r.len(), 3);
    }

    #[test]
    fn test_build_get_records_paging_and_elements() {
        let layers = vec![
            test_layer("world", "World", "default"),
            test_layer("usa", "USA", "default"),
            test_layer("parks", "Parks", "gis"),
        ];
        // summary + paging (page 2, size 2 → records at positions 2,3, nextRecord 0)
        let query = GetRecordsQuery {
            start_position: 2,
            max_records: 2,
            ..GetRecordsQuery::default()
        };
        let xml = build_get_records("http://x", &layers, &query);
        assert!(xml.contains("numberOfRecordsMatched=\"3\""));
        assert!(xml.contains("numberOfRecordsReturned=\"2\""));
        assert!(xml.contains("nextRecord=\"0\""));
        assert!(xml.contains("<csw:SummaryRecord>"));
        assert!(xml.contains("<dc:identifier>usa</dc:identifier>"));
        assert!(xml.contains("<dc:identifier>parks</dc:identifier>"));
        assert!(!xml.contains("<dc:format>")); // summary has no format
                                               // hits result type → no records
        let hits = GetRecordsQuery {
            result_type: CswResultType::Hits,
            ..GetRecordsQuery::default()
        };
        let xml = build_get_records("http://x", &layers, &hits);
        assert!(xml.contains("numberOfRecordsMatched=\"3\""));
        assert!(xml.contains("numberOfRecordsReturned=\"0\""));
        assert!(!xml.contains("<csw:SummaryRecord>"));
        // full element set → csw:Record with subject + references
        let full = GetRecordsQuery {
            element_set: CswElementSet::Full,
            max_records: 1,
            ..GetRecordsQuery::default()
        };
        let xml = build_get_records("http://x", &layers, &full);
        assert!(xml.contains("<csw:Record>"));
        assert!(xml.contains("<dc:subject>default</dc:subject>"));
        assert!(xml.contains("<dct:references"));
    }

    #[test]
    fn test_record_xml_bbox() {
        let layer = test_layer("world", "World", "default");
        let xml = record_xml("http://x", &layer, CswElementSet::Summary);
        assert!(xml.contains("<ows:BoundingBox crs=\"urn:ogc:def:crs:OGC:2:84\">"));
        assert!(xml.contains("<ows:LowerCorner>-10 -5</ows:LowerCorner>"));
        assert!(xml.contains("<ows:UpperCorner>10 5</ows:UpperCorner>"));
    }

    #[test]
    fn test_build_exception() {
        let xml = build_exception("InvalidParameterValue", "bad <request>");
        assert!(xml.contains("<ows:ExceptionReport"));
        assert!(xml.contains("exceptionCode=\"InvalidParameterValue\""));
        assert!(xml.contains("bad &lt;request&gt;"));
    }
}
