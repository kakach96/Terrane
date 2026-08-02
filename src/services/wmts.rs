//! # WMTS (Web Map Tile Service) 1.0.0 实现
//!
//! 参考 OGC 07-057r7 标准: https://www.ogc.org/standard/wmts/
//!
//! 支持操作:
//! - GetCapabilities — 返回服务元数据 XML
//! - GetTile — 返回瓦片图像 (复用底层 /tiles 端点)
//! - GetFeatureInfo — 获取要素信息

use serde::Serialize;
use crate::models::Layer;
use crate::error::GeoServerError;

// ---------------------------------------------------------------------------
// WMTS 请求定义
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct WmtsRequest {
    pub service: String,
    pub version: Option<String>,
    pub request: WmtsOperation,
}

#[derive(Debug, Clone)]
pub enum WmtsOperation {
    GetCapabilities,
    GetTile {
        layer: String,
        style: String,
        format: String,
        tile_matrix_set: String,
        tile_matrix: String,  // zoom level as string
        tile_row: u32,         // y
        tile_col: u32,         // x
    },
    GetFeatureInfo {
        layer: String,
        style: String,
        tile_matrix_set: String,
        tile_matrix: String,
        tile_row: u32,
        tile_col: u32,
        i: u32,
        j: u32,
        info_format: String,
    },
}

// ---------------------------------------------------------------------------
// WMTS GetCapabilities XML 结构
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct WmtsCapabilities {
    #[serde(rename = "@version")]
    pub version: String,
    #[serde(rename = "@xmlns")]
    pub xmlns: String,
    #[serde(rename = "@xmlns:ows")]
    pub xmlns_ows: String,
    #[serde(rename = "@xmlns:xlink")]
    pub xmlns_xlink: String,
    #[serde(rename = "@xmlns:xsi")]
    pub xmlns_xsi: String,
    #[serde(rename = "@xsi:schemaLocation")]
    pub schema_location: String,
    #[serde(rename = "ows:ServiceIdentification")]
    pub service_identification: ServiceIdentification,
    #[serde(rename = "ows:ServiceProvider")]
    pub service_provider: ServiceProvider,
    #[serde(rename = "ows:OperationsMetadata")]
    pub operations_metadata: OperationsMetadata,
    #[serde(rename = "Contents")]
    pub contents: Contents,
}

#[derive(Debug, Serialize)]
pub struct ServiceIdentification {
    #[serde(rename = "ows:Title")]
    pub title: String,
    #[serde(rename = "ows:Abstract")]
    pub abstract_text: String,
    #[serde(rename = "ows:Keywords")]
    pub keywords: Keywords,
    #[serde(rename = "ows:ServiceType")]
    pub service_type: String,
    #[serde(rename = "ows:ServiceTypeVersion")]
    pub service_type_version: String,
    #[serde(rename = "ows:Fees")]
    pub fees: String,
    #[serde(rename = "ows:AccessConstraints")]
    pub access_constraints: String,
}

#[derive(Debug, Serialize)]
pub struct Keywords {
    #[serde(rename = "ows:Keyword")]
    pub keyword: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ServiceProvider {
    #[serde(rename = "ows:ProviderName")]
    pub provider_name: String,
    #[serde(rename = "ows:ProviderSite")]
    pub provider_site: ProviderSite,
    #[serde(rename = "ows:ServiceContact")]
    pub service_contact: ServiceContact,
}

#[derive(Debug, Serialize)]
pub struct ProviderSite {
    #[serde(rename = "@xlink:href")]
    pub href: String,
}

#[derive(Debug, Serialize)]
pub struct ServiceContact {
    #[serde(rename = "ows:IndividualName")]
    pub individual_name: String,
    #[serde(rename = "ows:PositionName")]
    pub position_name: String,
    #[serde(rename = "ows:ContactInfo")]
    pub contact_info: ContactInfo,
}

#[derive(Debug, Serialize)]
pub struct ContactInfo {
    #[serde(rename = "ows:Phone")]
    pub phone: Phone,
    #[serde(rename = "ows:Address")]
    pub address: Address,
}

#[derive(Debug, Serialize)]
pub struct Phone {
    #[serde(rename = "ows:Voice")]
    pub voice: String,
}

#[derive(Debug, Serialize)]
pub struct Address {
    #[serde(rename = "ows:DeliveryPoint")]
    pub delivery_point: String,
    #[serde(rename = "ows:City")]
    pub city: String,
    #[serde(rename = "ows:AdministrativeArea")]
    pub administrative_area: String,
    #[serde(rename = "ows:PostalCode")]
    pub postal_code: String,
    #[serde(rename = "ows:Country")]
    pub country: String,
    #[serde(rename = "ows:ElectronicMailAddress")]
    pub electronic_mail_address: String,
}

#[derive(Debug, Serialize)]
pub struct OperationsMetadata {
    #[serde(rename = "ows:Operation")]
    pub operations: Vec<Operation>,
}

#[derive(Debug, Serialize)]
pub struct Operation {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "ows:DCP")]
    pub dcp: Vec<Dcp>,
}

#[derive(Debug, Serialize)]
pub struct Dcp {
    #[serde(rename = "ows:HTTP")]
    pub http: HttpMethod,
}

#[derive(Debug, Serialize)]
pub struct HttpMethod {
    #[serde(rename = "ows:Get")]
    pub get: Vec<GetResource>,
}

#[derive(Debug, Serialize)]
pub struct GetResource {
    #[serde(rename = "@xlink:href")]
    pub href: String,
}

#[derive(Debug, Serialize)]
pub struct Contents {
    #[serde(rename = "TileMatrixSet")]
    pub tile_matrix_sets: Vec<TileMatrixSet>,
    #[serde(rename = "Layer")]
    pub layers: Vec<WmtsLayer>,
}

#[derive(Debug, Serialize)]
pub struct TileMatrixSet {
    #[serde(rename = "@xmlns")]
    pub xmlns: String,
    #[serde(rename = "ows:Title")]
    pub title: String,
    #[serde(rename = "ows:Abstract")]
    pub abstract_text: String,
    #[serde(rename = "ows:Identifier")]
    pub identifier: String,
    #[serde(rename = "ows:BoundingBox")]
    pub bounding_box: WmtsBoundingBox,
    #[serde(rename = "SupportedCRS")]
    pub supported_crs: String,
    #[serde(rename = "TileMatrix")]
    pub tile_matrices: Vec<TileMatrix>,
}

#[derive(Debug, Serialize)]
pub struct WmtsBoundingBox {
    #[serde(rename = "@crs")]
    pub crs: String,
    #[serde(rename = "@minx")]
    pub minx: f64,
    #[serde(rename = "@miny")]
    pub miny: f64,
    #[serde(rename = "@maxx")]
    pub maxx: f64,
    #[serde(rename = "@maxy")]
    pub maxy: f64,
}

#[derive(Debug, Serialize)]
pub struct TileMatrix {
    #[serde(rename = "ows:Identifier")]
    pub identifier: String,
    #[serde(rename = "ScaleDenominator")]
    pub scale_denominator: f64,
    #[serde(rename = "TopLeftCorner")]
    pub top_left_corner: String,
    #[serde(rename = "TileWidth")]
    pub tile_width: u32,
    #[serde(rename = "TileHeight")]
    pub tile_height: u32,
    #[serde(rename = "MatrixWidth")]
    pub matrix_width: u32,
    #[serde(rename = "MatrixHeight")]
    pub matrix_height: u32,
}

#[derive(Debug, Serialize)]
pub struct WmtsLayer {
    #[serde(rename = "ows:Title")]
    pub title: String,
    #[serde(rename = "ows:Abstract")]
    pub abstract_text: String,
    #[serde(rename = "ows:Identifier")]
    pub identifier: String,
    #[serde(rename = "ows:BoundingBox")]
    pub bounding_box: WmtsBoundingBox,
    #[serde(rename = "Style")]
    pub styles: Vec<WmtsStyle>,
    #[serde(rename = "Format")]
    pub formats: Vec<String>,
    #[serde(rename = "TileMatrixSetLink")]
    pub tile_matrix_set_links: Vec<TileMatrixSetLink>,
    #[serde(rename = "ResourceURL")]
    pub resource_urls: Vec<ResourceUrl>,
}

#[derive(Debug, Serialize)]
pub struct WmtsStyle {
    #[serde(rename = "ows:Identifier")]
    pub identifier: String,
    #[serde(rename = "ows:Title")]
    pub title: String,
    #[serde(rename = "ows:Abstract")]
    pub abstract_text: String,
    #[serde(rename = "LegendURL")]
    pub legend_url: Option<LegendUrl>,
}

#[derive(Debug, Serialize)]
pub struct LegendUrl {
    #[serde(rename = "@format")]
    pub format: String,
    #[serde(rename = "@xlink:href")]
    pub href: String,
    #[serde(rename = "@minx")]
    pub minx: u32,
    #[serde(rename = "@miny")]
    pub miny: u32,
    #[serde(rename = "@maxx")]
    pub maxx: u32,
    #[serde(rename = "@maxy")]
    pub maxy: u32,
}

#[derive(Debug, Serialize)]
pub struct TileMatrixSetLink {
    #[serde(rename = "TileMatrixSet")]
    pub tile_matrix_set: String,
}

#[derive(Debug, Serialize)]
pub struct ResourceUrl {
    #[serde(rename = "@format")]
    pub format: String,
    #[serde(rename = "@resourceType")]
    pub resource_type: String,
    #[serde(rename = "@template")]
    pub template_url: String,
}

// ---------------------------------------------------------------------------
// WMTS 核心实现
// ---------------------------------------------------------------------------

/// 解析 WMTS KVP 请求参数
pub fn parse_wmts_request(params: &[(String, String)]) -> Result<WmtsRequest, GeoServerError> {
    let mut service = String::new();
    let mut version = None;
    let mut request = String::new();

    // GetTile 参数
    let mut layer = String::new();
    let mut style = String::new();
    let mut format = String::new();
    let mut tile_matrix_set = String::new();
    let mut tile_matrix = String::new();
    let mut tile_row = 0u32;
    let mut tile_col = 0u32;
    let mut i = 0u32;
    let mut j = 0u32;
    let mut info_format = String::new();

    for (key, value) in params {
        let upper = key.to_uppercase();
        match upper.as_str() {
            "SERVICE" => service = value.clone(),
            "VERSION" => version = Some(value.clone()),
            "REQUEST" => request = value.to_uppercase(),
            "LAYER" => layer = value.clone(),
            "STYLE" => style = value.clone(),
            "FORMAT" => format = value.clone(),
            "TILEMATRIXSET" => tile_matrix_set = value.clone(),
            "TILEMATRIX" => tile_matrix = value.clone(),
            "TILEROW" => tile_row = value.parse().unwrap_or(0),
            "TILECOL" => tile_col = value.parse().unwrap_or(0),
            "I" => i = value.parse().unwrap_or(0),
            "J" => j = value.parse().unwrap_or(0),
            "INFOFORMAT" => info_format = value.clone(),
            _ => {}
        }
    }

    let operation = match request.as_str() {
        "GETCAPABILITIES" => WmtsOperation::GetCapabilities,
        "GETTILE" => WmtsOperation::GetTile {
            layer,
            style,
            format,
            tile_matrix_set,
            tile_matrix,
            tile_row,
            tile_col,
        },
        "GETFEATUREINFO" => WmtsOperation::GetFeatureInfo {
            layer,
            style,
            tile_matrix_set,
            tile_matrix,
            tile_row,
            tile_col,
            i,
            j,
            info_format,
        },
        _ => return Err(GeoServerError::BadRequest(format!("Unsupported WMTS operation: {}", request))),
    };

    Ok(WmtsRequest {
        service,
        version,
        request: operation,
    })
}

/// 构建 GetCapabilities XML
pub fn build_capabilities(
    base_url: &str,
    layers: &[Layer],
    api_context: &str,
) -> Result<String, GeoServerError> {
    let wmts_base = format!("{}{}/wmts", base_url, api_context);

    let capabilities = WmtsCapabilities {
        version: "1.0.0".to_string(),
        xmlns: "http://www.opengis.net/wmts/1.0".to_string(),
        xmlns_ows: "http://www.opengis.net/ows/1.1".to_string(),
        xmlns_xlink: "http://www.w3.org/1999/xlink".to_string(),
        xmlns_xsi: "http://www.w3.org/2001/XMLSchema-instance".to_string(),
        schema_location: "http://www.opengis.net/wmts/1.0 http://schemas.opengis.net/wmts/1.0/wmtsGetCapabilities_response.xsd".to_string(),
        service_identification: ServiceIdentification {
            title: "GeoFerris WMTS".to_string(),
            abstract_text: "Web Map Tile Service powered by GeoFerris".to_string(),
            keywords: Keywords {
                keyword: vec![
                    "WMTS".to_string(),
                    "Tile".to_string(),
                    "Map".to_string(),
                    "GeoFerris".to_string(),
                ],
            },
            service_type: "OGC WMTS".to_string(),
            service_type_version: "1.0.0".to_string(),
            fees: "NONE".to_string(),
            access_constraints: "NONE".to_string(),
        },
        service_provider: ServiceProvider {
            provider_name: "GeoFerris".to_string(),
            provider_site: ProviderSite {
                href: base_url.to_string(),
            },
            service_contact: ServiceContact {
                individual_name: "GeoFerris Team".to_string(),
                position_name: "System Administrator".to_string(),
                contact_info: ContactInfo {
                    phone: Phone { voice: "N/A".to_string() },
                    address: Address {
                        delivery_point: "N/A".to_string(),
                        city: "N/A".to_string(),
                        administrative_area: "N/A".to_string(),
                        postal_code: "N/A".to_string(),
                        country: "N/A".to_string(),
                        electronic_mail_address: "admin@geoserver.local".to_string(),
                    },
                },
            },
        },
        operations_metadata: OperationsMetadata {
            operations: vec![
                Operation {
                    name: "GetCapabilities".to_string(),
                    dcp: vec![Dcp {
                        http: HttpMethod {
                            get: vec![GetResource { href: format!("{}?SERVICE=WMTS&REQUEST=GetCapabilities", wmts_base) }],
                        },
                    }],
                },
                Operation {
                    name: "GetTile".to_string(),
                    dcp: vec![Dcp {
                        http: HttpMethod {
                            get: vec![GetResource { href: format!("{}?SERVICE=WMTS&REQUEST=GetTile", wmts_base) }],
                        },
                    }],
                },
            ],
        },
        contents: Contents {
            tile_matrix_sets: build_tile_matrix_sets(),
            layers: build_wmts_layers(layers, base_url, api_context),
        },
    };

    let xml = quick_xml::se::to_string(&capabilities)
        .map_err(|e| GeoServerError::ServiceError(format!("WMTS Capabilities serialization failed: {}", e)))?;

    Ok(format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>{}"#,
        xml
    ))
}

fn build_tile_matrix_sets() -> Vec<TileMatrixSet> {
    vec![
        // EPSG:4326 Gridset
        build_tile_matrix_set("EPSG:4326", "Global CRS84 Scale Set", -180.0, -90.0, 180.0, 90.0),
        // EPSG:3857 Gridset
        build_tile_matrix_set("EPSG:3857", "Google Maps Compatible", -20037508.34, -20037508.34, 20037508.34, 20037508.34),
    ]
}

fn build_tile_matrix_set(
    id: &str,
    title: &str,
    minx: f64,
    miny: f64,
    maxx: f64,
    maxy: f64,
) -> TileMatrixSet {
    let (top_left_x, top_left_y) = match id {
        "EPSG:4326" => (-180.0, 90.0),
        "EPSG:3857" => (-20037508.34, 20037508.34),
        _ => (minx, maxy),
    };

    let max_zoom = 18u32;
    let mut tile_matrices = Vec::new();

    for z in 0..=max_zoom {
        let n = 2.0_f64.powi(z as i32);
        let (matrix_width, matrix_height) = match id {
            "EPSG:4326" => (n as u32 * 2, n as u32), // 2x1 顶层
            "EPSG:3857" => (n as u32, n as u32),      // 1x1 顶层
            _ => (n as u32, n as u32),
        };

        let scale_denom = match id {
            "EPSG:4326" => 1.0 / (n * 256.0 / 360.0) * 0.00028,
            "EPSG:3857" => 156543.03 / (n * 0.00028),
            _ => 156543.03 / (n * 0.00028),
        };

        tile_matrices.push(TileMatrix {
            identifier: format!("{}:{}", id, z),
            scale_denominator: scale_denom,
            top_left_corner: format!("{} {}", top_left_x, top_left_y),
            tile_width: 256,
            tile_height: 256,
            matrix_width,
            matrix_height,
        });
    }

    TileMatrixSet {
        xmlns: "http://www.opengis.net/wmts/1.0".to_string(),
        title: title.to_string(),
        abstract_text: format!("{} tile matrix set", id),
        identifier: id.to_string(),
        bounding_box: WmtsBoundingBox {
            crs: id.to_string(),
            minx,
            miny,
            maxx,
            maxy,
        },
        supported_crs: id.to_string(),
        tile_matrices,
    }
}

fn build_wmts_layers(layers: &[Layer], base_url: &str, api_context: &str) -> Vec<WmtsLayer> {
    let tile_template = format!("{}{}/wmts/{{layer}}/{{TileMatrixSet}}/{{TileMatrix}}/{{TileCol}}/{{TileRow}}.png", base_url, api_context);
    let feature_info_template = format!("{}{}/wmts/{{layer}}/{{TileMatrixSet}}/{{TileMatrix}}/{{TileRow}}/{{TileCol}}/{{J}}/{{I}}.json", base_url, api_context);

    layers.iter().map(|layer| {
        let bounds = &layer.native_bounds;
        WmtsLayer {
            title: layer.title.clone(),
            abstract_text: layer.abstract_text.clone().unwrap_or_default(),
            identifier: layer.name.clone(),
            bounding_box: WmtsBoundingBox {
                crs: bounds.crs.to_epsg(),
                minx: bounds.bounds.minx,
                miny: bounds.bounds.miny,
                maxx: bounds.bounds.maxx,
                maxy: bounds.bounds.maxy,
            },
            styles: vec![WmtsStyle {
                identifier: "default".to_string(),
                title: "Default Style".to_string(),
                abstract_text: "Default layer style".to_string(),
                legend_url: None,
            }],
            formats: vec![
                "image/png".to_string(),
                "image/jpeg".to_string(),
            ],
            tile_matrix_set_links: vec![
                TileMatrixSetLink { tile_matrix_set: "EPSG:4326".to_string() },
                TileMatrixSetLink { tile_matrix_set: "EPSG:3857".to_string() },
            ],
            resource_urls: vec![
                ResourceUrl {
                    format: "image/png".to_string(),
                    resource_type: "tile".to_string(),
                    template_url: tile_template.clone(),
                },
                ResourceUrl {
                    format: "application/json".to_string(),
                    resource_type: "FeatureInfo".to_string(),
                    template_url: feature_info_template.clone(),
                },
            ],
        }
    }).collect()
}

/// 从 WMTS GetTile 请求参数生成瓦片 URL (复用现有 /tiles 端点)
pub fn get_tile_url(internal_base: &str, layer: &str, _style: &str, _format: &str,
                    _tile_matrix_set: &str, tile_matrix: &str, tile_row: u32, tile_col: u32) -> String {
    // 解析 zoom level
    let z = if let Some(idx) = tile_matrix.find(':') {
        &tile_matrix[idx + 1..]
    } else {
        tile_matrix
    };
    format!("{}/tiles/{}/{}/{}/{}", internal_base, layer, z, tile_col, tile_row)
}
