use crate::error::TerraneError;
use crate::models::{Bounds, Layer};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WmsRequest {
    pub service: String,
    pub version: Option<String>,
    pub request: WmsOperation,
    pub layers: Option<Vec<String>>,
    pub styles: Option<Vec<String>>,
    pub crs: Option<String>,
    pub bbox: Option<Bbox>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub format: Option<String>,
    pub transparent: Option<bool>,
    pub bgcolor: Option<String>,
    pub exceptions: Option<String>,
    pub time: Option<String>,
    pub elevation: Option<String>,
    pub query_layers: Option<Vec<String>>,
    pub info_format: Option<String>,
    pub feature_count: Option<u32>,
    pub i: Option<f64>,
    pub j: Option<f64>,
    pub sld: Option<String>,
    pub sld_body: Option<String>,
    /// GeoServer Vendor 参数: CQL 过滤器
    pub cql_filter: Option<String>,
    /// GeoServer Vendor 参数: 环境变量替换
    pub env: Option<String>,
    /// GeoServer Vendor 参数: 要素 ID 过滤
    pub feature_id: Option<String>,
    /// GeoServer Vendor 参数: 地图旋转角度
    pub angle: Option<f64>,
    /// GetLegendGraphic 参数: 比例尺分母 (SCALE) — 仅显示匹配规则的图例
    pub scale: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WmsOperation {
    #[serde(rename = "GetCapabilities")]
    GetCapabilities,
    #[serde(rename = "GetMap")]
    GetMap,
    #[serde(rename = "GetFeatureInfo")]
    GetFeatureInfo,
    #[serde(rename = "DescribeLayer")]
    DescribeLayer,
    #[serde(rename = "GetLegendGraphic")]
    GetLegendGraphic,
    #[serde(rename = "GetStyles")]
    GetStyles,
    #[serde(rename = "PutStyles")]
    PutStyles,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bbox {
    pub minx: f64,
    pub miny: f64,
    pub maxx: f64,
    pub maxy: f64,
}

impl Bbox {
    pub fn to_bounds(&self) -> Bounds {
        Bounds::new(self.minx, self.miny, self.maxx, self.maxy)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WmsCapabilities {
    pub version: String,
    pub service: ServiceMetadata,
    pub capability: Capability,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceMetadata {
    pub name: String,
    pub title: String,
    pub abstract_text: Option<String>,
    pub keywords: Vec<String>,
    pub online_resource: String,
    pub contact_information: Option<ContactInformation>,
    pub fees: Option<String>,
    pub access_constraints: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactInformation {
    pub contact_person_primary: Option<ContactPersonPrimary>,
    pub contact_position: Option<String>,
    pub contact_address: Option<ContactAddress>,
    pub contact_voice_telephone: Option<String>,
    pub contact_facsimile_telephone: Option<String>,
    pub contact_electronic_mail_address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactPersonPrimary {
    pub contact_person: Option<String>,
    pub contact_organization: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactAddress {
    pub address_type: Option<String>,
    pub address: Option<String>,
    pub city: Option<String>,
    pub state_or_province: Option<String>,
    pub post_code: Option<String>,
    pub country: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    pub request: RequestMetadata,
    pub exception: Vec<String>,
    pub layers: Vec<LayerCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestMetadata {
    pub get_capabilities: OperationMetadata,
    pub get_map: OperationMetadata,
    pub get_feature_info: Option<OperationMetadata>,
    pub get_legend_graphic: Option<OperationMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationMetadata {
    pub formats: Vec<String>,
    pub dcp_type: Vec<DcpType>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DcpType {
    pub http: HttpMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpMetadata {
    pub get: Option<OnlineResource>,
    pub post: Option<OnlineResource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnlineResource {
    pub href: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerCapability {
    pub name: Option<String>,
    pub title: String,
    pub abstract_text: Option<String>,
    pub keyword_list: Vec<String>,
    pub crs: Vec<String>,
    pub bounding_box: Vec<BoundingBoxMetadata>,
    pub layer_limit: Option<u32>,
    pub queryable: bool,
    pub cascaded: Option<u32>,
    pub opaque: Option<bool>,
    pub no_subsets: Option<bool>,
    pub fixed_width: Option<u32>,
    pub fixed_height: Option<u32>,
    pub styles: Vec<StyleMetadata>,
    pub min_scale_denominator: Option<f64>,
    pub max_scale_denominator: Option<f64>,
    pub scale_hint: Option<ScaleHint>,
    pub attributes: Option<Vec<String>>,
    pub authority_urls: Option<Vec<AuthorityUrl>>,
    pub metadata_urls: Option<Vec<MetadataUrl>>,
    pub data_urls: Option<Vec<DataUrl>>,
    pub layers: Vec<LayerCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundingBoxMetadata {
    pub crs: String,
    pub minx: f64,
    pub miny: f64,
    pub maxx: f64,
    pub maxy: f64,
    pub resx: Option<f64>,
    pub resy: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScaleHint {
    pub min: f64,
    pub max: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleMetadata {
    pub name: String,
    pub title: Option<String>,
    pub abstract_text: Option<String>,
    pub legend_url: Option<LegendUrl>,
    pub style_sheet_url: Option<StyleSheetUrl>,
    pub style_url: Option<StyleUrl>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegendUrl {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub format: String,
    pub online_resource: OnlineResource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleSheetUrl {
    pub format: String,
    pub online_resource: OnlineResource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleUrl {
    pub online_resource: OnlineResource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorityUrl {
    pub name: String,
    pub href: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataUrl {
    pub format: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataUrl {
    pub format: String,
    pub url: String,
}

impl WmsCapabilities {
    pub fn new(base_url: &str) -> Self {
        WmsCapabilities {
            version: "1.3.0".to_string(),
            service: ServiceMetadata {
                name: "WMS".to_string(),
                title: "Terrane".to_string(),
                abstract_text: Some(
                    "Cloud-native, high-performance spatial data server, powered by Rust"
                        .to_string(),
                ),
                keywords: vec![
                    "WMS".to_string(),
                    "Web Map Service".to_string(),
                    "Terrane".to_string(),
                    "GIS".to_string(),
                    "Rust".to_string(),
                ],
                online_resource: base_url.to_string(),
                contact_information: None,
                fees: None,
                access_constraints: None,
            },
            capability: Capability {
                request: RequestMetadata {
                    get_capabilities: OperationMetadata {
                        formats: vec![
                            "text/xml".to_string(),
                            "application/vnd.ogc.wms_xml".to_string(),
                        ],
                        dcp_type: vec![DcpType {
                            http: HttpMetadata {
                                get: Some(OnlineResource {
                                    href: format!("{}?", base_url),
                                }),
                                post: Some(OnlineResource {
                                    href: base_url.to_string(),
                                }),
                            },
                        }],
                    },
                    get_map: OperationMetadata {
                        formats: vec![
                            "image/png".to_string(),
                            "image/jpeg".to_string(),
                            "image/gif".to_string(),
                            "image/webp".to_string(),
                            "image/tiff".to_string(),
                            "image/svg+xml".to_string(),
                            "application/vnd.google-earth.kml+xml".to_string(),
                            "application/json".to_string(),
                            "application/pdf".to_string(),
                            "application/rss+xml".to_string(),
                            "application/atom+xml".to_string(),
                            "application/gml+xml; version=3.2".to_string(),
                            "application/json;type=utfgrid".to_string(),
                        ],
                        dcp_type: vec![DcpType {
                            http: HttpMetadata {
                                get: Some(OnlineResource {
                                    href: format!("{}?", base_url),
                                }),
                                post: Some(OnlineResource {
                                    href: base_url.to_string(),
                                }),
                            },
                        }],
                    },
                    get_feature_info: Some(OperationMetadata {
                        formats: vec![
                            "text/plain".to_string(),
                            "application/vnd.ogc.gml".to_string(),
                            "text/html".to_string(),
                            "application/json".to_string(),
                        ],
                        dcp_type: vec![DcpType {
                            http: HttpMetadata {
                                get: Some(OnlineResource {
                                    href: format!("{}?", base_url),
                                }),
                                post: Some(OnlineResource {
                                    href: base_url.to_string(),
                                }),
                            },
                        }],
                    }),
                    get_legend_graphic: Some(OperationMetadata {
                        formats: vec![
                            "image/png".to_string(),
                            "image/jpeg".to_string(),
                            "image/gif".to_string(),
                        ],
                        dcp_type: vec![DcpType {
                            http: HttpMetadata {
                                get: Some(OnlineResource {
                                    href: format!("{}?", base_url),
                                }),
                                post: None,
                            },
                        }],
                    }),
                },
                exception: vec![
                    "application/vnd.ogc.se_xml".to_string(),
                    "application/vnd.ogc.se_inimage".to_string(),
                    "application/vnd.ogc.se_blank".to_string(),
                ],
                layers: vec![],
            },
        }
    }

    pub fn add_layer(&mut self, layer: &Layer) {
        let layer_cap = LayerCapability {
            name: Some(layer.name.clone()),
            title: layer.title.clone(),
            abstract_text: layer.abstract_text.clone(),
            keyword_list: vec![],
            crs: vec![layer.srs.to_epsg()],
            bounding_box: vec![BoundingBoxMetadata {
                crs: layer.srs.to_epsg(),
                minx: layer.native_bounds.bounds.minx,
                miny: layer.native_bounds.bounds.miny,
                maxx: layer.native_bounds.bounds.maxx,
                maxy: layer.native_bounds.bounds.maxy,
                resx: None,
                resy: None,
            }],
            layer_limit: None,
            queryable: true,
            cascaded: None,
            opaque: None,
            no_subsets: None,
            fixed_width: None,
            fixed_height: None,
            styles: layer
                .styles
                .iter()
                .map(|s| StyleMetadata {
                    name: s.name.clone(),
                    title: Some(s.name.clone()),
                    abstract_text: None,
                    legend_url: None,
                    style_sheet_url: None,
                    style_url: None,
                })
                .collect(),
            min_scale_denominator: None,
            max_scale_denominator: None,
            scale_hint: None,
            attributes: None,
            authority_urls: None,
            metadata_urls: None,
            data_urls: None,
            layers: vec![],
        };

        self.capability.layers.push(layer_cap);
    }
}

pub fn parse_wms_request(params: &[(String, String)]) -> Result<WmsRequest, TerraneError> {
    let mut service = None;
    let mut version = None;
    let mut request = None;
    let mut layers = None;
    let mut styles = None;
    let mut crs = None;
    let mut bbox = None;
    let mut width = None;
    let mut height = None;
    let mut format = None;
    let mut transparent = None;
    let mut bgcolor = None;
    let mut exceptions = None;
    let mut time = None;
    let mut elevation = None;
    let mut query_layers = None;
    let mut info_format = None;
    let mut feature_count = None;
    let mut i = None;
    let mut j = None;
    let mut sld = None;
    let mut sld_body = None;
    let mut cql_filter = None;
    let mut env = None;
    let mut feature_id = None;
    let mut angle = None;
    let mut scale = None;

    for (key, value) in params {
        match key.to_uppercase().as_str() {
            "SERVICE" => service = Some(value.clone()),
            "VERSION" => version = Some(value.clone()),
            "REQUEST" => {
                request = match value.to_lowercase().as_str() {
                    "getcapabilities" => Some(WmsOperation::GetCapabilities),
                    "getmap" => Some(WmsOperation::GetMap),
                    "getfeatureinfo" => Some(WmsOperation::GetFeatureInfo),
                    "describelayer" => Some(WmsOperation::DescribeLayer),
                    "getlegendgraphic" => Some(WmsOperation::GetLegendGraphic),
                    "getstyles" => Some(WmsOperation::GetStyles),
                    "putstyles" => Some(WmsOperation::PutStyles),
                    _ => {
                        return Err(TerraneError::BadRequest(format!(
                            "Unknown request: {}",
                            value
                        )))
                    },
                }
            },
            "LAYERS" => layers = Some(value.split(',').map(|s| s.trim().to_string()).collect()),
            "STYLES" => styles = Some(value.split(',').map(|s| s.trim().to_string()).collect()),
            "CRS" | "SRS" => crs = Some(value.clone()),
            "BBOX" => {
                let parts: Vec<f64> = value
                    .split(',')
                    .filter_map(|s| s.trim().parse().ok())
                    .collect();
                if parts.len() == 4 {
                    let raw = Bbox {
                        minx: parts[0],
                        miny: parts[1],
                        maxx: parts[2],
                        maxy: parts[3],
                    };
                    bbox = Some(normalize_bbox(raw, version.as_deref(), crs.as_deref()));
                }
            },
            "WIDTH" => width = value.parse().ok(),
            "HEIGHT" => height = value.parse().ok(),
            "FORMAT" => format = Some(value.clone()),
            "TRANSPARENT" => transparent = Some(value.to_uppercase() == "TRUE"),
            "BGCOLOR" => bgcolor = Some(value.clone()),
            "EXCEPTIONS" => exceptions = Some(value.clone()),
            "TIME" => time = Some(value.clone()),
            "ELEVATION" => elevation = Some(value.clone()),
            "QUERY_LAYERS" => {
                query_layers = Some(value.split(',').map(|s| s.trim().to_string()).collect())
            },
            "INFO_FORMAT" => info_format = Some(value.clone()),
            "FEATURE_COUNT" => feature_count = value.parse().ok(),
            "I" => i = value.parse().ok(),
            "X" => i = value.parse().ok(),
            "J" => j = value.parse().ok(),
            "Y" => j = value.parse().ok(),
            "SLD" => sld = Some(value.clone()),
            "SLD_BODY" | "SLDBODY" => sld_body = Some(value.clone()),
            // GeoServer Vendor 参数
            "CQL_FILTER" => cql_filter = Some(value.clone()),
            "ENV" => env = Some(value.clone()),
            "FEATUREID" | "FEATURE_ID" => feature_id = Some(value.clone()),
            "ANGLE" => angle = value.parse().ok(),
            // GetLegendGraphic 比例尺过滤参数
            "SCALE" => scale = value.parse().ok(),
            _ => {},
        }
    }

    let request =
        request.ok_or_else(|| TerraneError::BadRequest("Missing REQUEST parameter".to_string()))?;

    if let Some(ref svc) = service {
        if svc.to_uppercase() != "WMS" {
            return Err(TerraneError::BadRequest("Invalid service type".to_string()));
        }
    }

    Ok(WmsRequest {
        service: service.unwrap_or_else(|| "WMS".to_string()),
        version,
        request,
        layers,
        styles,
        crs,
        bbox,
        width,
        height,
        format,
        transparent,
        bgcolor,
        exceptions,
        time,
        elevation,
        query_layers,
        info_format,
        feature_count,
        i,
        j,
        sld,
        sld_body,
        cql_filter,
        env,
        feature_id,
        angle,
        scale,
    })
}

/// WMS 1.3.0 对地理坐标系（EPSG:4326）要求 BBOX 轴序为 lat,lon
/// 需要转换为内部的 lon,lat 表示
fn normalize_bbox(raw: Bbox, version: Option<&str>, crs: Option<&str>) -> Bbox {
    let is_130 = version.is_some_and(|v| v.starts_with("1.3"));
    let is_geographic = crs.is_some_and(|c| {
        let upper = c.to_uppercase();
        upper == "EPSG:4326" || upper == "4326" || upper == "CRS:84"
    });

    if is_130 && is_geographic {
        // WMS 1.3.0 要求 BBOX = minLat, minLon, maxLat, maxLon
        // 内部使用 minx=lon, miny=lat，所以需要交换
        tracing::debug!(
            "[normalize_bbox] WMS 1.3.0 地理CRS: 交换轴序 ({}, {}, {}, {}) -> ({}, {}, {}, {})",
            raw.minx,
            raw.miny,
            raw.maxx,
            raw.maxy,
            raw.miny,
            raw.minx,
            raw.maxy,
            raw.maxx
        );
        Bbox {
            minx: raw.miny, // 纬度 → 经度
            miny: raw.minx, // 经度 → 纬度
            maxx: raw.maxy,
            maxy: raw.maxx,
        }
    } else {
        raw
    }
}

pub fn format_wms_exception(
    err: &TerraneError,
    exceptions: Option<&str>,
    width: u32,
    height: u32,
) -> (Vec<u8>, &'static str) {
    let msg = format!("{}", err);
    let fmt = exceptions.unwrap_or("application/vnd.ogc.se_xml");

    match fmt {
        "application/vnd.ogc.se_inimage" => {
            let mut img = image::RgbaImage::new(width.max(1), height.max(1));
            for pixel in img.pixels_mut() {
                *pixel = image::Rgba([255, 255, 255, 255]);
            }
            let mut buf = Vec::new();
            use image::codecs::png::PngEncoder;
            use image::ImageEncoder;
            PngEncoder::new(&mut buf)
                .write_image(
                    img.as_raw(),
                    width.max(1),
                    height.max(1),
                    image::ColorType::Rgba8.into(),
                )
                .ok();
            (buf, "image/png")
        },
        "application/vnd.ogc.se_blank" => {
            let img = image::RgbaImage::new(width.max(1), height.max(1));
            let mut buf = Vec::new();
            use image::codecs::png::PngEncoder;
            use image::ImageEncoder;
            PngEncoder::new(&mut buf)
                .write_image(
                    img.as_raw(),
                    width.max(1),
                    height.max(1),
                    image::ColorType::Rgba8.into(),
                )
                .ok();
            (buf, "image/png")
        },
        _ => {
            let xml = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<ServiceExceptionReport version="1.3.0" xmlns="http://www.opengis.net/ogc">
  <ServiceException code="{}">{}</ServiceException>
</ServiceExceptionReport>"#,
                "InvalidRequest",
                msg.replace('&', "&amp;")
                    .replace('<', "&lt;")
                    .replace('>', "&gt;")
            );
            (xml.into_bytes(), "application/vnd.ogc.se_xml")
        },
    }
}

pub fn validate_wms_get_map_request(req: &WmsRequest) -> Result<(), TerraneError> {
    if req.layers.is_none() || req.layers.as_ref().unwrap().is_empty() {
        return Err(TerraneError::BadRequest(
            "LAYERS parameter is required".to_string(),
        ));
    }

    if req.bbox.is_none() {
        return Err(TerraneError::BadRequest(
            "BBOX parameter is required".to_string(),
        ));
    }

    let width = req.width.unwrap_or(512);
    let height = req.height.unwrap_or(512);

    if width == 0 || width > 4096 {
        return Err(TerraneError::BadRequest(
            "Invalid WIDTH parameter (must be between 1 and 4096)".to_string(),
        ));
    }

    if height == 0 || height > 4096 {
        return Err(TerraneError::BadRequest(
            "Invalid HEIGHT parameter (must be between 1 and 4096)".to_string(),
        ));
    }

    if req.format.is_none() {
        return Err(TerraneError::BadRequest(
            "FORMAT parameter is required".to_string(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_scale_parameter() {
        let params = vec![
            ("SERVICE".to_string(), "WMS".to_string()),
            ("REQUEST".to_string(), "GetLegendGraphic".to_string()),
            ("LAYER".to_string(), "roads".to_string()),
            ("SCALE".to_string(), "25000".to_string()),
            ("FORMAT".to_string(), "image/png".to_string()),
            ("WIDTH".to_string(), "60".to_string()),
        ];
        let req = parse_wms_request(&params).expect("parse");
        assert_eq!(req.scale, Some(25000.0));
        assert_eq!(req.width, Some(60));
    }

    #[test]
    fn test_parse_angle_parameter() {
        let params = vec![
            ("SERVICE".to_string(), "WMS".to_string()),
            ("REQUEST".to_string(), "GetMap".to_string()),
            ("LAYERS".to_string(), "a,b".to_string()),
            ("BBOX".to_string(), "0,0,10,10".to_string()),
            ("WIDTH".to_string(), "100".to_string()),
            ("HEIGHT".to_string(), "100".to_string()),
            ("FORMAT".to_string(), "image/png".to_string()),
            ("ANGLE".to_string(), "30.5".to_string()),
        ];
        let req = parse_wms_request(&params).expect("parse");
        assert_eq!(req.angle, Some(30.5));
    }
}
