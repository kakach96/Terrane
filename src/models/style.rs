use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Style {
    pub name: String,
    pub title: String,
    pub abstract_text: Option<String>,
    pub style_format: StyleFormat,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StyleFormat {
    SLD,
    CSS,
    YSLD,
    MBStyle,
}

impl std::fmt::Display for StyleFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StyleFormat::SLD => write!(f, "SLD"),
            StyleFormat::CSS => write!(f, "CSS"),
            StyleFormat::YSLD => write!(f, "YSLD"),
            StyleFormat::MBStyle => write!(f, "MBStyle"),
        }
    }
}

impl Style {
    pub fn new(name: String, title: String, content: String) -> Self {
        Style {
            name,
            title,
            abstract_text: None,
            style_format: StyleFormat::SLD,
            content,
        }
    }
}

/// Auto-detect style format from content
pub fn detect_style_format(content: &str) -> StyleFormat {
    let trimmed = content.trim();
    if trimmed.starts_with("<?xml") || trimmed.starts_with("<StyledLayerDescriptor") {
        StyleFormat::SLD
    } else if trimmed.starts_with('{') {
        if trimmed.contains("\"version\"") && trimmed.contains("\"layers\"") {
            StyleFormat::MBStyle
        } else {
            StyleFormat::SLD
        }
    } else if trimmed.starts_with("name:") || trimmed.starts_with("title:") || trimmed.starts_with("feature-styles:") {
        StyleFormat::YSLD
    } else {
        StyleFormat::CSS
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SldStyle {
    pub named_layers: Vec<NamedLayer>,
    pub user_styles: Vec<UserStyle>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamedLayer {
    pub name: String,
    pub named_styles: Vec<NamedStyle>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamedStyle {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserStyle {
    pub name: Option<String>,
    pub title: Option<String>,
    pub abstract_text: Option<String>,
    pub feature_type_styles: Vec<FeatureTypeStyle>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureTypeStyle {
    pub name: Option<String>,
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub name: Option<String>,
    pub title: Option<String>,
    pub abstract_text: Option<String>,
    pub filters: Vec<ogc_filter::Filter>,
    pub point_symbolizer: Option<PointSymbolizer>,
    pub line_symbolizer: Option<LineSymbolizer>,
    pub polygon_symbolizer: Option<PolygonSymbolizer>,
    pub text_symbolizer: Option<TextSymbolizer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointSymbolizer {
    pub graphic: Option<Graphic>,
    pub geometry: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineSymbolizer {
    pub stroke: Option<Stroke>,
    pub geometry: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolygonSymbolizer {
    pub fill: Option<Fill>,
    pub stroke: Option<Stroke>,
    pub geometry: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextSymbolizer {
    pub label: String,
    pub fill: Option<Fill>,
    pub font: Option<Font>,
    pub geometry: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Graphic {
    pub marks: Vec<Mark>,
    pub size: Option<f64>,
    pub opacity: Option<f64>,
    pub rotation: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mark {
    pub well_known_name: WellKnownName,
    pub fill: Option<Fill>,
    pub stroke: Option<Stroke>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WellKnownName {
    Circle,
    Square,
    Triangle,
    Star,
    Cross,
    X,
    #[serde(rename = "shape://vertline")]
    VertLine,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fill {
    pub fill: Option<String>,
    pub fill_opacity: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stroke {
    pub stroke: Option<String>,
    pub stroke_width: Option<f64>,
    pub stroke_opacity: Option<f64>,
    pub stroke_linecap: Option<String>,
    pub stroke_linejoin: Option<String>,
    pub stroke_dasharray: Option<Vec<f64>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Font {
    pub font_family: Vec<String>,
    pub font_size: Option<f64>,
    pub font_style: Option<String>,
    pub font_weight: Option<String>,
}

pub mod ogc_filter {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum Filter {
        PropertyIsEqualTo(PropertyName, Expression),
        PropertyIsNotEqualTo(PropertyName, Expression),
        PropertyIsLessThan(PropertyName, Expression),
        PropertyIsGreaterThan(PropertyName, Expression),
        PropertyIsLessThanOrEqualTo(PropertyName, Expression),
        PropertyIsGreaterThanOrEqualTo(PropertyName, Expression),
        PropertyIsLike(PropertyName, String),
        PropertyIsNull(PropertyName),
        PropertyIsBetween(PropertyName, Expression, Expression),
        And(Box<Filter>, Box<Filter>),
        Or(Box<Filter>, Box<Filter>),
        Not(Box<Filter>),
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct PropertyName(pub String);

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(untagged)]
    pub enum Expression {
        Literal(String),
        Property(PropertyName),
        Math(Box<MathExpression>),
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MathExpression {
        pub op: String,
        pub exprs: Vec<Expression>,
    }
}
