//! # WPS (Web Processing Service) 1.0.0 implementation
//!
//! First WPS surface for Terrane: GetCapabilities / DescribeProcess / Execute,
//! served at `/wps` (KVP GET/POST + XML Execute). The reference GeoServer at
//! :18080 has WPS disabled, so this follows the OGC WPS 1.0.0 schema directly
//! (OGC 05-007r7).
//!
//! Built-in processes (pure Rust, no external fetch):
//! - `vec:Centroid` — centroid of every input feature → point feature collection
//! - `vec:Buffer`   — point-buffer each feature by a distance → polygons
//! - `gs:Bounds`    — bounding box of the input collection → rectangle polygon

use crate::error::GeoServerError;
use crate::models::{Feature, GeoJsonGeometry};
use serde::Deserialize;
use std::collections::HashMap;

/// WPS namespace URIs used by all generated documents.
pub const WPS_NS: &str = "http://www.opengis.net/wps/1.0.0";
pub const OWS_NS: &str = "http://www.opengis.net/ows/1.1";
pub const XLINK_NS: &str = "http://www.w3.org/1999/xlink";

// ---------------------------------------------------------------------------
// Process registry
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WpsDataInputKind {
    /// Complex data (GeoJSON feature collection).
    ComplexData,
    /// A numeric literal (xsd:double).
    LiteralDouble,
    /// A string literal (xsd:string).
    LiteralString,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WpsOutputKind {
    ComplexData,
    Literal,
}

#[derive(Debug, Clone)]
pub struct WpsInputSpec {
    pub identifier: &'static str,
    pub title: &'static str,
    pub kind: WpsDataInputKind,
    pub min_occurs: u32,
    pub max_occurs: u32,
}

#[derive(Debug, Clone)]
pub struct WpsOutputSpec {
    pub identifier: &'static str,
    pub title: &'static str,
    pub kind: WpsOutputKind,
}

#[derive(Debug, Clone)]
pub struct ProcessSpec {
    pub identifier: &'static str,
    pub title: &'static str,
    pub abstract_text: &'static str,
    pub inputs: Vec<WpsInputSpec>,
    pub outputs: Vec<WpsOutputSpec>,
}

/// The built-in WPS processes.
pub fn builtin_processes() -> Vec<ProcessSpec> {
    vec![
        ProcessSpec {
            identifier: "vec:Centroid",
            title: "Centroid",
            abstract_text: "Computes the centroid of each feature and returns a new point feature collection.",
            inputs: vec![WpsInputSpec {
                identifier: "features",
                title: "Features",
                kind: WpsDataInputKind::ComplexData,
                min_occurs: 1,
                max_occurs: 1,
            }],
            outputs: vec![WpsOutputSpec {
                identifier: "result",
                title: "Result",
                kind: WpsOutputKind::ComplexData,
            }],
        },
        ProcessSpec {
            identifier: "vec:Buffer",
            title: "Buffer",
            abstract_text: "Buffers each feature by a distance. Uses a point-buffer approximation: a circle is placed around every coordinate of the input (exact for points).",
            inputs: vec![
                WpsInputSpec {
                    identifier: "features",
                    title: "Features",
                    kind: WpsDataInputKind::ComplexData,
                    min_occurs: 1,
                    max_occurs: 1,
                },
                WpsInputSpec {
                    identifier: "distance",
                    title: "Distance",
                    kind: WpsDataInputKind::LiteralDouble,
                    min_occurs: 1,
                    max_occurs: 1,
                },
            ],
            outputs: vec![WpsOutputSpec {
                identifier: "result",
                title: "Result",
                kind: WpsOutputKind::ComplexData,
            }],
        },
        ProcessSpec {
            identifier: "gs:Bounds",
            title: "Bounds",
            abstract_text: "Computes the bounding box of the input feature collection and returns it as a rectangle polygon feature.",
            inputs: vec![WpsInputSpec {
                identifier: "features",
                title: "Features",
                kind: WpsDataInputKind::ComplexData,
                min_occurs: 1,
                max_occurs: 1,
            }],
            outputs: vec![WpsOutputSpec {
                identifier: "result",
                title: "Result",
                kind: WpsOutputKind::ComplexData,
            }],
        },
    ]
}

/// Look up a built-in process by identifier (returns an owned copy so the
/// result outlives the temporary registry vector).
pub fn find_process(identifier: &str) -> Option<ProcessSpec> {
    builtin_processes()
        .into_iter()
        .find(|p| p.identifier == identifier)
}

// ---------------------------------------------------------------------------
// KVP request parsing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum WpsOperation {
    GetCapabilities,
    DescribeProcess {
        identifiers: Vec<String>,
    },
    Execute {
        identifier: String,
        data_inputs: Vec<(String, String)>,
        response_raw: bool,
        output_id: Option<String>,
    },
}

/// Parse a WPS KVP request.
pub fn parse_wps_request(params: &[(String, String)]) -> Result<WpsOperation, GeoServerError> {
    let mut service = None;
    let mut request = None;
    let mut identifier = None;
    let mut identifiers = None;
    let mut data_inputs_raw = None;
    let mut response = None;
    let mut output_id = None;

    for (key, value) in params {
        match key.to_uppercase().as_str() {
            "SERVICE" => service = Some(value.clone()),
            "REQUEST" => request = Some(value.to_uppercase()),
            "IDENTIFIER" => identifier = Some(value.clone()),
            "IDENTIFIERS" => {
                identifiers = Some(value.split(',').map(|s| s.trim().to_string()).collect())
            },
            "DATAINPUTS" => data_inputs_raw = Some(value.clone()),
            "RESPONSE" => response = Some(value.to_lowercase()),
            "OUTPUTID" => output_id = Some(value.clone()),
            _ => {},
        }
    }

    if let Some(svc) = &service {
        if !svc.eq_ignore_ascii_case("WPS") {
            return Err(GeoServerError::BadRequest(
                "Invalid service type".to_string(),
            ));
        }
    }

    let request = request
        .ok_or_else(|| GeoServerError::BadRequest("Missing REQUEST parameter".to_string()))?;

    match request.as_str() {
        "GETCAPABILITIES" => Ok(WpsOperation::GetCapabilities),
        "DESCRIBEPROCESS" => {
            let ids =
                identifiers.unwrap_or_else(|| identifier.map(|i| vec![i]).unwrap_or_default());
            Ok(WpsOperation::DescribeProcess { identifiers: ids })
        },
        "EXECUTE" => {
            let identifier = identifier.ok_or_else(|| {
                GeoServerError::BadRequest("Missing IDENTIFIER parameter".to_string())
            })?;
            let data_inputs = parse_kvp_data_inputs(data_inputs_raw.as_deref().unwrap_or(""));
            let response_raw = matches!(
                response.as_deref(),
                Some("raw") | Some("rawdata") | Some("raw_data")
            );
            Ok(WpsOperation::Execute {
                identifier,
                data_inputs,
                response_raw,
                output_id,
            })
        },
        _ => Err(GeoServerError::BadRequest(format!(
            "Unknown request: {}",
            request
        ))),
    }
}

/// Parse the WPS KVP `DataInputs` value: `name=value;name=value`. A leading `@`
/// on the value is stripped (WPS attribute syntax; a `layer:` prefix is treated
/// as a layer reference by the handler).
fn parse_kvp_data_inputs(raw: &str) -> Vec<(String, String)> {
    raw.split(';')
        .filter(|s| !s.trim().is_empty())
        .filter_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            let name = parts.next()?.trim().to_string();
            let mut value = parts.next().unwrap_or("").trim().to_string();
            if let Some(stripped) = value.strip_prefix('@') {
                value = stripped.to_string();
            }
            Some((name, value))
        })
        .collect()
}

// ---------------------------------------------------------------------------
// XML generation — GetCapabilities / DescribeProcess / ExecuteResponse
// ---------------------------------------------------------------------------

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn wps_attrs() -> String {
    format!(
        r#"xmlns:wps="{}" xmlns:ows="{}" xmlns:xlink="{}""#,
        WPS_NS, OWS_NS, XLINK_NS
    )
}

fn operation_block(base_url: &str, name: &str) -> String {
    format!(
        "    <ows:Operation name=\"{}\">\n\
         <ows:DCP><ows:HTTP><ows:Get xlink:href=\"{}/wps?\"/><ows:Post xlink:href=\"{}/wps\"/></ows:HTTP></ows:DCP>\n\
         </ows:Operation>\n",
        name, base_url, base_url
    )
}

/// Build the WPS 1.0.0 GetCapabilities document.
pub fn build_capabilities(base_url: &str) -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str(&format!(
        "<wps:Capabilities service=\"WPS\" version=\"1.0.0\" xml:lang=\"en-US\" {}>\n",
        wps_attrs()
    ));
    out.push_str("  <ows:ServiceIdentification>\n");
    out.push_str("    <ows:Title>Terrane</ows:Title>\n");
    out.push_str(
        "    <ows:Abstract>Cloud-native spatial data server powered by Rust</ows:Abstract>\n",
    );
    out.push_str("    <ows:Keywords><ows:Keyword>WPS</ows:Keyword><ows:Keyword>Web Processing Service</ows:Keyword></ows:Keywords>\n");
    out.push_str("    <ows:ServiceType>WPS</ows:ServiceType>\n");
    out.push_str("    <ows:ServiceTypeVersion>1.0.0</ows:ServiceTypeVersion>\n");
    out.push_str("    <ows:Fees>NONE</ows:Fees>\n");
    out.push_str("    <ows:AccessConstraints>NONE</ows:AccessConstraints>\n");
    out.push_str("  </ows:ServiceIdentification>\n");
    out.push_str("  <ows:ServiceProvider>\n");
    out.push_str("    <ows:ProviderName>Terrane</ows:ProviderName>\n");
    out.push_str("  </ows:ServiceProvider>\n");
    out.push_str("  <ows:OperationsMetadata>\n");
    for op in ["GetCapabilities", "DescribeProcess", "Execute"] {
        out.push_str(&operation_block(base_url, op));
    }
    out.push_str("  </ows:OperationsMetadata>\n");
    out.push_str("  <wps:ProcessOfferings>\n");
    for p in builtin_processes() {
        out.push_str(&format!(
            "    <wps:Process wps:processVersion=\"1.0.0\">\n\
             <ows:Identifier>{}</ows:Identifier>\n\
             <ows:Title>{}</ows:Title>\n\
             <ows:Abstract>{}</ows:Abstract>\n\
             </wps:Process>\n",
            p.identifier,
            escape_xml(p.title),
            escape_xml(p.abstract_text)
        ));
    }
    out.push_str("  </wps:ProcessOfferings>\n");
    out.push_str("  <wps:Languages>\n");
    out.push_str("    <wps:Default><ows:Language>en-US</ows:Language></wps:Default>\n");
    out.push_str("    <wps:Supported><ows:Language>en-US</ows:Language></wps:Supported>\n");
    out.push_str("  </wps:Languages>\n");
    out.push_str("</wps:Capabilities>\n");
    out
}

fn format_block(kind: WpsDataInputKind) -> String {
    match kind {
        WpsDataInputKind::ComplexData => "        <ComplexData>\n\
                <Default><Format><MimeType>application/json</MimeType></Format></Default>\n\
                <Supported><Format><MimeType>application/json</MimeType></Format></Supported>\n\
                </ComplexData>\n"
            .to_string(),
        WpsDataInputKind::LiteralDouble => "        <LiteralData>\n\
                <ows:DataType ows:reference=\"xsd:double\"/>\n\
                <ows:AnyValue/>\n\
                </LiteralData>\n"
            .to_string(),
        WpsDataInputKind::LiteralString => "        <LiteralData>\n\
                <ows:DataType ows:reference=\"xsd:string\"/>\n\
                <ows:AnyValue/>\n\
                </LiteralData>\n"
            .to_string(),
    }
}

fn output_block(kind: WpsOutputKind) -> String {
    match kind {
        WpsOutputKind::ComplexData => "        <ComplexData>\n\
                <Default><Format><MimeType>application/json</MimeType></Format></Default>\n\
                <Supported><Format><MimeType>application/json</MimeType></Format></Supported>\n\
                </ComplexData>\n"
            .to_string(),
        WpsOutputKind::Literal => {
            "        <LiteralData><ows:DataType ows:reference=\"xsd:string\"/></LiteralData>\n"
                .to_string()
        },
    }
}

/// Build the WPS 1.0.0 DescribeProcess document for the given identifiers
/// (empty = all built-in processes). Unknown identifiers are rejected.
pub fn build_process_descriptions(identifiers: &[String]) -> Result<String, GeoServerError> {
    let specs: Vec<ProcessSpec> = if identifiers.is_empty() {
        builtin_processes()
    } else {
        identifiers
            .iter()
            .map(|id| {
                find_process(id)
                    .ok_or_else(|| GeoServerError::BadRequest(format!("Unknown process: {}", id)))
            })
            .collect::<Result<_, _>>()?
    };

    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str(&format!(
        "<wps:ProcessDescriptions service=\"WPS\" version=\"1.0.0\" xml:lang=\"en-US\" {}>\n",
        wps_attrs()
    ));
    for spec in specs {
        out.push_str(&format!(
            "  <ProcessDescription wps:processVersion=\"1.0.0\" storeSupported=\"true\" statusSupported=\"true\">\n"
        ));
        out.push_str(&format!(
            "    <ows:Identifier>{}</ows:Identifier>\n",
            spec.identifier
        ));
        out.push_str(&format!(
            "    <ows:Title>{}</ows:Title>\n",
            escape_xml(spec.title)
        ));
        out.push_str(&format!(
            "    <ows:Abstract>{}</ows:Abstract>\n",
            escape_xml(spec.abstract_text)
        ));
        out.push_str("    <DataInputs>\n");
        for input in &spec.inputs {
            out.push_str(&format!(
                "      <Input minOccurs=\"{}\" maxOccurs=\"{}\">\n",
                input.min_occurs, input.max_occurs
            ));
            out.push_str(&format!(
                "        <ows:Identifier>{}</ows:Identifier>\n",
                input.identifier
            ));
            out.push_str(&format!(
                "        <ows:Title>{}</ows:Title>\n",
                escape_xml(input.title)
            ));
            out.push_str(&format_block(input.kind));
            out.push_str("      </Input>\n");
        }
        out.push_str("    </DataInputs>\n");
        out.push_str("    <ProcessOutputs>\n");
        for output in &spec.outputs {
            out.push_str("      <Output>\n");
            out.push_str(&format!(
                "        <ows:Identifier>{}</ows:Identifier>\n",
                output.identifier
            ));
            out.push_str(&format!(
                "        <ows:Title>{}</ows:Title>\n",
                escape_xml(output.title)
            ));
            out.push_str(&output_block(output.kind));
            out.push_str("      </Output>\n");
        }
        out.push_str("    </ProcessOutputs>\n");
        out.push_str("  </ProcessDescription>\n");
    }
    out.push_str("</wps:ProcessDescriptions>\n");
    Ok(out)
}

// ---------------------------------------------------------------------------
// Execute
// ---------------------------------------------------------------------------

/// A fully-resolved process input (features already loaded by the handler).
#[derive(Debug, Clone)]
pub enum ResolvedInput {
    Features(Vec<Feature>),
    Literal(String),
}

/// An Execute output value.
#[derive(Debug, Clone)]
pub enum OutputValue {
    GeoJson(serde_json::Value),
    Literal(String),
}

/// The outcome of running a process.
#[derive(Debug, Clone)]
pub struct WpsResult {
    pub output_id: String,
    pub output_title: &'static str,
    pub value: OutputValue,
}

/// Serialize a feature list as a proper GeoJSON FeatureCollection.
pub fn features_to_geojson(features: &[Feature]) -> serde_json::Value {
    let items: Vec<serde_json::Value> = features
        .iter()
        .map(|f| {
            serde_json::json!({
                "type": "Feature",
                "id": f.id,
                "geometry": f.geometry,
                "properties": f.properties,
            })
        })
        .collect();
    serde_json::json!({ "type": "FeatureCollection", "features": items })
}

/// Parse a feature collection from JSON (accepts both proper GeoJSON and the
/// Terrane `{features, total_count}` shape).
pub fn parse_feature_collection_json(s: &str) -> Result<Vec<Feature>, String> {
    #[derive(Deserialize)]
    struct Fc {
        features: Vec<Feature>,
    }
    let fc: Fc =
        serde_json::from_str(s).map_err(|e| format!("invalid feature collection JSON: {}", e))?;
    Ok(fc.features)
}

/// Run a built-in process with resolved inputs.
pub fn run_process(
    spec: &ProcessSpec,
    inputs: &HashMap<String, ResolvedInput>,
) -> Result<WpsResult, String> {
    let get_features = |name: &str| -> Result<Vec<Feature>, String> {
        match inputs.get(name) {
            Some(ResolvedInput::Features(f)) => Ok(f.clone()),
            Some(ResolvedInput::Literal(s)) => parse_feature_collection_json(s),
            None => Err(format!("Missing input '{}'", name)),
        }
    };

    let build = |features: Vec<Feature>| WpsResult {
        output_id: "result".to_string(),
        output_title: "Result",
        value: OutputValue::GeoJson(features_to_geojson(&features)),
    };

    match spec.identifier {
        "vec:Centroid" => {
            let features = get_features("features")?;
            let out: Vec<Feature> = features
                .iter()
                .filter_map(|f| {
                    crate::utils::geometry::centroid_geometry(&f.geometry)
                        .map(|g| Feature::with_id(f.id.clone(), g, f.properties.clone()))
                })
                .collect();
            Ok(build(out))
        },
        "vec:Buffer" => {
            let features = get_features("features")?;
            let distance: f64 = match inputs.get("distance") {
                Some(ResolvedInput::Literal(s)) => s
                    .trim()
                    .parse()
                    .map_err(|_| format!("invalid distance literal: {}", s))?,
                _ => return Err("Missing input 'distance'".to_string()),
            };
            let out: Vec<Feature> = features
                .iter()
                .filter_map(|f| {
                    crate::utils::geometry::buffer_geometry(&f.geometry, distance)
                        .map(|g| Feature::with_id(f.id.clone(), g, f.properties.clone()))
                })
                .collect();
            Ok(build(out))
        },
        "gs:Bounds" => {
            let features = get_features("features")?;
            let bounds = crate::utils::geometry::calculate_bounds(
                features.iter().map(|f| f.geometry.to_geo()),
            )
            .unwrap_or_else(|| crate::models::Bounds::new(-180.0, -90.0, 180.0, 90.0));
            let ring = vec![
                vec![bounds.minx, bounds.miny],
                vec![bounds.maxx, bounds.miny],
                vec![bounds.maxx, bounds.maxy],
                vec![bounds.minx, bounds.maxy],
                vec![bounds.minx, bounds.miny],
            ];
            let geom = GeoJsonGeometry::Polygon {
                coordinates: vec![ring],
            };
            let out = vec![Feature::with_id("bounds".to_string(), geom, HashMap::new())];
            Ok(build(out))
        },
        _ => Err(format!("Process not implemented: {}", spec.identifier)),
    }
}

/// A parsed Execute XML input.
#[derive(Debug, Clone)]
pub struct ExecuteXmlInput {
    pub identifier: String,
    pub literal: Option<String>,
    pub complex_data: Option<String>,
    pub reference: Option<String>,
}

/// A parsed WPS Execute request (XML).
#[derive(Debug, Clone)]
pub struct ExecuteXmlRequest {
    pub identifier: String,
    pub inputs: Vec<ExecuteXmlInput>,
    pub response_raw: bool,
    pub output_id: Option<String>,
}

fn first_identifier(node: &crate::services::wfs::XmlNode) -> Option<String> {
    node.children_named("Identifier")
        .first()
        .map(|i| i.text.trim().to_string())
}

/// Parse a WPS 1.0.0 Execute request XML body (minimal supported subset:
/// `<wps:DataInputs><wps:Input>` with LiteralData / ComplexData / Reference).
pub fn parse_execute_xml(xml: &str) -> Result<ExecuteXmlRequest, String> {
    let roots = crate::services::wfs::parse_xml_nodes(xml)?;
    let root = roots.first().ok_or("empty Execute XML")?;

    let identifier = first_identifier(root).ok_or("missing process identifier")?;

    let mut inputs = Vec::new();
    for di in root.children_named("DataInputs") {
        for input in di.children_named("Input") {
            let name = first_identifier(input).unwrap_or_default();
            let mut literal = None;
            let mut complex_data = None;
            let mut reference = None;
            for data in input.children_named("Data") {
                if let Some(l) = data.children_named("LiteralData").first() {
                    literal = Some(l.text.trim().to_string());
                }
                if let Some(c) = data.children_named("ComplexData").first() {
                    complex_data = Some(c.text.trim().to_string());
                }
            }
            for r in input.children_named("Reference") {
                reference = r.attr("href").map(|s| s.to_string());
            }
            inputs.push(ExecuteXmlInput {
                identifier: name,
                literal,
                complex_data,
                reference,
            });
        }
    }

    let mut response_raw = false;
    let mut output_id = None;
    for rf in root.children_named("ResponseForm") {
        let raw_outputs = rf.children_named("RawDataOutput");
        if !raw_outputs.is_empty() {
            response_raw = true;
            output_id = raw_outputs
                .first()
                .and_then(|o| first_identifier(o))
                .or(output_id);
        } else if let Some(doc) = rf.children_named("ResponseDocument").first() {
            for o in doc.children_named("Output") {
                if output_id.is_none() {
                    output_id = first_identifier(o);
                }
            }
        }
    }

    Ok(ExecuteXmlRequest {
        identifier,
        inputs,
        response_raw,
        output_id,
    })
}

/// Build a WPS 1.0.0 ExecuteResponse XML document wrapping a GeoJSON output.
pub fn build_execute_response(base_url: &str, spec: &ProcessSpec, result: &WpsResult) -> String {
    let json = match &result.value {
        OutputValue::GeoJson(v) => serde_json::to_string(v).unwrap_or_default(),
        OutputValue::Literal(s) => escape_xml(s),
    };
    let json_escaped = escape_xml(&json);
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <wps:ExecuteResponse service=\"WPS\" version=\"1.0.0\" xml:lang=\"en-US\" serviceInstance=\"{}/wps?\" {}>\n\
         \x20 <wps:Process wps:processVersion=\"1.0.0\">\n\
         \x20   <ows:Identifier>{}</ows:Identifier>\n\
         \x20   <ows:Title>{}</ows:Title>\n\
         \x20 </wps:Process>\n\
         \x20 <wps:Status creationTime=\"\"><wps:ProcessSucceeded>Process {} succeeded</wps:ProcessSucceeded></wps:Status>\n\
         \x20 <wps:ProcessOutputs>\n\
         \x20   <wps:Output>\n\
         \x20     <ows:Identifier>{}</ows:Identifier>\n\
         \x20     <ows:Title>{}</ows:Title>\n\
         \x20     <wps:Data><wps:ComplexData mimeType=\"application/json\">{}</wps:ComplexData></wps:Data>\n\
         \x20   </wps:Output>\n\
         \x20 </wps:ProcessOutputs>\n\
         </wps:ExecuteResponse>\n",
        base_url,
        wps_attrs(),
        spec.identifier,
        escape_xml(spec.title),
        spec.identifier,
        result.output_id,
        escape_xml(result.output_title),
        json_escaped,
    )
}

/// A WPS exception report (ows:ExceptionReport).
pub fn build_exception(exception_code: &str, text: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <ows:ExceptionReport xmlns:ows=\"{}\" version=\"1.0.0\">\n\
         \x20 <ows:Exception exceptionCode=\"{}\">\n\
         \x20   <ows:ExceptionText>{}</ows:ExceptionText>\n\
         \x20 </ows:Exception>\n\
         </ows:ExceptionReport>\n",
        OWS_NS,
        exception_code,
        escape_xml(text)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_kvp_data_inputs() {
        let inputs = parse_kvp_data_inputs("features=layer:world;distance=2");
        assert_eq!(
            inputs,
            vec![
                ("features".to_string(), "layer:world".to_string()),
                ("distance".to_string(), "2".to_string())
            ]
        );
        // `@` attribute prefix is stripped (WPS reference syntax).
        let refs = parse_kvp_data_inputs("features=@layer:world");
        assert_eq!(
            refs,
            vec![("features".to_string(), "layer:world".to_string())]
        );
        // Empty / malformed entries are skipped.
        assert!(parse_kvp_data_inputs("").is_empty());
        assert!(parse_kvp_data_inputs(";;").is_empty());
    }

    #[test]
    fn test_parse_wps_kvp_operations() {
        let op = parse_wps_request(&[
            ("service".to_string(), "WPS".to_string()),
            ("request".to_string(), "GetCapabilities".to_string()),
        ])
        .unwrap();
        assert!(matches!(op, WpsOperation::GetCapabilities));

        let op = parse_wps_request(&[
            ("SERVICE".to_string(), "WPS".to_string()),
            ("REQUEST".to_string(), "DescribeProcess".to_string()),
            ("IDENTIFIER".to_string(), "vec:Buffer".to_string()),
        ])
        .unwrap();
        match op {
            WpsOperation::DescribeProcess { identifiers } => {
                assert_eq!(identifiers, vec!["vec:Buffer".to_string()]);
            },
            _ => panic!("expected DescribeProcess"),
        }

        let op = parse_wps_request(&[
            ("SERVICE".to_string(), "WPS".to_string()),
            ("REQUEST".to_string(), "Execute".to_string()),
            ("IDENTIFIER".to_string(), "vec:Centroid".to_string()),
            ("DATAINPUTS".to_string(), "features=layer:world".to_string()),
            ("RESPONSE".to_string(), "raw".to_string()),
        ])
        .unwrap();
        match op {
            WpsOperation::Execute {
                identifier,
                data_inputs,
                response_raw,
                ..
            } => {
                assert_eq!(identifier, "vec:Centroid");
                assert_eq!(
                    data_inputs,
                    vec![("features".to_string(), "layer:world".to_string())]
                );
                assert!(response_raw);
            },
            _ => panic!("expected Execute"),
        }

        // Wrong service → error.
        assert!(parse_wps_request(&[
            ("SERVICE".to_string(), "WFS".to_string()),
            ("REQUEST".to_string(), "GetCapabilities".to_string()),
        ])
        .is_err());
    }

    #[test]
    fn test_parse_execute_xml() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<wps:Execute service="WPS" version="1.0.0" xmlns:wps="http://www.opengis.net/wps/1.0.0" xmlns:ows="http://www.opengis.net/ows/1.1" xmlns:xlink="http://www.w3.org/1999/xlink">
  <ows:Identifier>vec:Buffer</ows:Identifier>
  <wps:DataInputs>
    <wps:Input><ows:Identifier>features</ows:Identifier><wps:Reference xlink:href="layer:world"/></wps:Input>
    <wps:Input><ows:Identifier>distance</ows:Identifier><wps:Data><wps:LiteralData>3.5</wps:LiteralData></wps:Data></wps:Input>
  </wps:DataInputs>
  <wps:ResponseForm><wps:RawDataOutput><ows:Identifier>result</ows:Identifier></wps:RawDataOutput></wps:ResponseForm>
</wps:Execute>"#;
        let req = parse_execute_xml(xml).unwrap();
        assert_eq!(req.identifier, "vec:Buffer");
        assert_eq!(req.inputs.len(), 2);
        assert_eq!(req.inputs[0].identifier, "features");
        assert_eq!(req.inputs[0].reference.as_deref(), Some("layer:world"));
        assert_eq!(req.inputs[1].identifier, "distance");
        assert_eq!(req.inputs[1].literal.as_deref(), Some("3.5"));
        assert!(req.response_raw);
        assert_eq!(req.output_id.as_deref(), Some("result"));
    }

    #[test]
    fn test_build_capabilities_structure() {
        let doc = build_capabilities("http://127.0.0.1:8080");
        assert!(doc.contains("<wps:Capabilities service=\"WPS\" version=\"1.0.0\""));
        assert!(doc.contains("<ows:OperationsMetadata>"));
        assert!(doc.contains("<ows:Operation name=\"Execute\""));
        assert!(doc.contains("<wps:ProcessOfferings>"));
        assert!(doc.contains("<ows:Identifier>vec:Centroid</ows:Identifier>"));
        assert!(doc.contains("<ows:Identifier>gs:Bounds</ows:Identifier>"));
        assert!(doc.contains("<wps:Languages>"));
    }

    #[test]
    fn test_features_to_geojson() {
        let mut props = HashMap::new();
        props.insert(
            "name".to_string(),
            crate::models::PropertyValue::String("a".to_string()),
        );
        let features = vec![Feature::with_id(
            "f1".to_string(),
            GeoJsonGeometry::Point {
                coordinates: vec![1.0, 2.0],
            },
            props,
        )];
        let v = features_to_geojson(&features);
        assert_eq!(v["type"], "FeatureCollection");
        assert_eq!(v["features"][0]["type"], "Feature");
        assert_eq!(v["features"][0]["geometry"]["type"], "Point");
        assert_eq!(v["features"][0]["properties"]["name"], "a");
    }

    #[test]
    fn test_run_process_centroid_and_bounds() {
        let mut props = HashMap::new();
        props.insert("n".to_string(), crate::models::PropertyValue::Integer(7));
        let square = Feature::with_id(
            "s1".to_string(),
            GeoJsonGeometry::Polygon {
                coordinates: vec![vec![
                    vec![0.0, 0.0],
                    vec![4.0, 0.0],
                    vec![4.0, 4.0],
                    vec![0.0, 4.0],
                    vec![0.0, 0.0],
                ]],
            },
            props,
        );

        let mut inputs = HashMap::new();
        inputs.insert(
            "features".to_string(),
            ResolvedInput::Features(vec![square.clone()]),
        );
        let spec = find_process("vec:Centroid").unwrap();
        let out = run_process(&spec, &inputs).unwrap();
        match &out.value {
            OutputValue::GeoJson(v) => {
                let geom = &v["features"][0]["geometry"];
                assert_eq!(geom["type"], "Point");
                let c = geom["coordinates"].as_array().unwrap();
                assert!((c[0].as_f64().unwrap() - 2.0).abs() < 0.01);
                assert!((c[1].as_f64().unwrap() - 2.0).abs() < 0.01);
            },
            _ => panic!("expected GeoJSON output"),
        }

        // Bounds of two points.
        let mut inputs2 = HashMap::new();
        inputs2.insert(
            "features".to_string(),
            ResolvedInput::Features(vec![
                Feature::with_id(
                    "a".to_string(),
                    GeoJsonGeometry::Point {
                        coordinates: vec![1.0, 2.0],
                    },
                    HashMap::new(),
                ),
                Feature::with_id(
                    "b".to_string(),
                    GeoJsonGeometry::Point {
                        coordinates: vec![5.0, 8.0],
                    },
                    HashMap::new(),
                ),
            ]),
        );
        let spec2 = find_process("gs:Bounds").unwrap();
        let out2 = run_process(&spec2, &inputs2).unwrap();
        match &out2.value {
            OutputValue::GeoJson(v) => {
                let ring = v["features"][0]["geometry"]["coordinates"][0]
                    .as_array()
                    .unwrap();
                assert!(ring
                    .iter()
                    .any(|c| c[0].as_f64().unwrap() == 1.0 && c[1].as_f64().unwrap() == 2.0));
                assert!(ring
                    .iter()
                    .any(|c| c[0].as_f64().unwrap() == 5.0 && c[1].as_f64().unwrap() == 8.0));
            },
            _ => panic!("expected GeoJSON output"),
        }
    }
}
