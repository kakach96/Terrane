use super::rendering::{Style, FillStyle, StrokeStyle};
use super::sld_parser::{ParsedRule, OgcFilter};
use std::collections::HashMap;

pub fn parse_css(css: &str) -> Vec<ParsedRule> {
    let cleaned = strip_comments(css);
    let blocks = extract_blocks(&cleaned);
    let mut rules = Vec::new();

    for (selector_text, body_text) in blocks {
        let filters = parse_selectors(&selector_text);
        let scale_min = filters.iter().filter_map(|f| {
            if let CssSelector::ScaleLt(v) = f { Some(*v) } else { None }
        }).next();
        let scale_max = filters.iter().filter_map(|f| {
            if let CssSelector::ScaleGt(v) = f { Some(*v) } else { None }
        }).next();
        let ogc_filters: Vec<OgcFilter> = filters.iter().filter_map(|f| {
            match f {
                CssSelector::PropEq(p, v) => Some(OgcFilter::PropertyIsEqualTo(p.clone(), v.clone())),
                CssSelector::PropNeq(p, v) => Some(OgcFilter::PropertyIsNotEqualTo(p.clone(), v.clone())),
                CssSelector::PropLt(p, v) => Some(OgcFilter::PropertyIsLessThan(p.clone(), v.clone())),
                CssSelector::PropGt(p, v) => Some(OgcFilter::PropertyIsGreaterThan(p.clone(), v.clone())),
                CssSelector::PropLte(p, v) => Some(OgcFilter::PropertyIsLessThanOrEqualTo(p.clone(), v.clone())),
                CssSelector::PropGte(p, v) => Some(OgcFilter::PropertyIsGreaterThanOrEqualTo(p.clone(), v.clone())),
                _ => None,
            }
        }).collect();

        let mut style = Style::new();
        parse_properties(&body_text, &mut style);

        rules.push(ParsedRule {
            name: None,
            min_scale: scale_min,
            max_scale: scale_max,
            filters: ogc_filters,
            style,
        });
    }

    if rules.is_empty() {
        rules.push(ParsedRule {
            name: None,
            min_scale: None,
            max_scale: None,
            filters: vec![],
            style: Style::new(),
        });
    }

    rules
}

fn strip_comments(css: &str) -> String {
    let mut result = String::new();
    let chars: Vec<char> = css.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if i + 1 < chars.len() && chars[i] == '/' && chars[i + 1] == '*' {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i += 2;
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }
    result
}

fn extract_blocks(css: &str) -> Vec<(String, String)> {
    let mut blocks = Vec::new();
    let chars: Vec<char> = css.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t' || chars[i] == '\n' || chars[i] == '\r') {
            i += 1;
        }
        if i >= chars.len() { break; }

        let mut selector = String::new();
        let mut depth = 0;
        while i < chars.len() {
            if chars[i] == '{' {
                depth = 1;
                i += 1;
                break;
            }
            if chars[i] == '}' {
                i += 1;
                break;
            }
            selector.push(chars[i]);
            i += 1;
        }
        let selector = selector.trim().to_string();
        if depth == 0 { continue; }

        let mut body = String::new();
        while i < chars.len() && depth > 0 {
            if chars[i] == '{' { depth += 1; }
            if chars[i] == '}' {
                depth -= 1;
                if depth == 0 { i += 1; break; }
            }
            body.push(chars[i]);
            i += 1;
        }

        if !selector.is_empty() {
            blocks.push((selector, body.trim().to_string()));
        }
    }
    blocks
}

enum CssSelector {
    PropEq(String, String),
    PropNeq(String, String),
    PropLt(String, String),
    PropGt(String, String),
    PropLte(String, String),
    PropGte(String, String),
    ScaleLt(f64),
    ScaleGt(f64),
}

fn parse_selectors(selector_text: &str) -> Vec<CssSelector> {
    let mut selectors = Vec::new();
    let mut remaining = selector_text.trim();

    while let Some(start) = remaining.find('[') {
        let end = remaining[start..].find(']').map(|p| start + p + 1).unwrap_or(remaining.len());
        let inner = remaining[start+1..end-1].trim();
        remaining = &remaining[end..remaining.len()];

        if let Some(sel) = parse_single_selector(inner) {
            selectors.push(sel);
        }
    }

    selectors
}

fn parse_single_selector(s: &str) -> Option<CssSelector> {
    let s = s.trim();

    if let Some(rest) = s.strip_prefix("@scale") {
        let rest = rest.trim();
        if let Some(val) = rest.strip_prefix('<') {
            val.trim().parse::<f64>().ok().map(CssSelector::ScaleLt)
        } else if let Some(val) = rest.strip_prefix('>') {
            val.trim().parse::<f64>().ok().map(CssSelector::ScaleGt)
        } else if let Some(val) = rest.strip_prefix("<=") {
            val.trim().parse::<f64>().ok().map(CssSelector::ScaleLt)
        } else if let Some(val) = rest.strip_prefix(">=") {
            val.trim().parse::<f64>().ok().map(CssSelector::ScaleGt)
        } else {
            None
        }
    } else if let Some(pos) = s.find("!=") {
        let prop = s[..pos].trim().to_string();
        let val = s[pos+2..].trim().trim_matches('"').to_string();
        Some(CssSelector::PropNeq(prop, val))
    } else if let Some(pos) = s.find("<=") {
        let prop = s[..pos].trim().to_string();
        let val = s[pos+2..].trim().trim_matches('"').to_string();
        Some(CssSelector::PropLte(prop, val))
    } else if let Some(pos) = s.find(">=") {
        let prop = s[..pos].trim().to_string();
        let val = s[pos+2..].trim().trim_matches('"').to_string();
        Some(CssSelector::PropGte(prop, val))
    } else if let Some(pos) = s.find('<') {
        let prop = s[..pos].trim().to_string();
        let val = s[pos+1..].trim().trim_matches('"').to_string();
        Some(CssSelector::PropLt(prop, val))
    } else if let Some(pos) = s.find('>') {
        let prop = s[..pos].trim().to_string();
        let val = s[pos+1..].trim().trim_matches('"').to_string();
        Some(CssSelector::PropGt(prop, val))
    } else if let Some(pos) = s.find('=') {
        let prop = s[..pos].trim().to_string();
        let val = s[pos+1..].trim().trim_matches('"').to_string();
        Some(CssSelector::PropEq(prop, val))
    } else {
        None
    }
}

fn parse_properties(body: &str, style: &mut Style) {
    for line in body.split(';') {
        let line = line.trim();
        if line.is_empty() { continue; }
        let mut parts = line.splitn(2, ':');
        let prop = parts.next().map(|s| s.trim()).unwrap_or("");
        let val = parts.next().map(|s| s.trim()).unwrap_or("");
        if prop.is_empty() || val.is_empty() { continue; }
        apply_css_property(style, prop, val);
    }
}

fn apply_css_property(style: &mut Style, prop: &str, val: &str) {
    match prop {
        "fill" => {
            let color = normalize_color(val);
            style.fill = Some(FillStyle { color: color.unwrap_or_else(|| "#808080".to_string()), opacity: style.fill.as_ref().map(|f| f.opacity).unwrap_or(1.0) });
        }
        "fill-opacity" => {
            if let Ok(opacity) = val.parse::<f64>() {
                let color = style.fill.as_ref().map(|f| f.color.clone()).unwrap_or_else(|| "#808080".to_string());
                style.fill = Some(FillStyle { color, opacity: opacity.min(1.0).max(0.0) });
            }
        }
        "stroke" => {
            if let Some(color) = normalize_color(val) {
                style.stroke = Some(StrokeStyle { color, width: style.stroke.as_ref().and_then(|s| s.width), opacity: style.stroke.as_ref().map(|s| s.opacity).unwrap_or(1.0), dash_array: style.stroke.as_ref().and_then(|s| s.dash_array.clone()) });
            }
        }
        "stroke-width" => {
            if let Ok(w) = val.parse::<f64>() {
                let color = style.stroke.as_ref().map(|s| s.color.clone()).unwrap_or_else(|| "#000000".to_string());
                let o = style.stroke.as_ref().map(|s| s.opacity).unwrap_or(1.0);
                let d = style.stroke.as_ref().and_then(|s| s.dash_array.clone());
                style.stroke = Some(StrokeStyle { color, width: Some(w), opacity: o, dash_array: d });
            }
        }
        "stroke-opacity" => {
            if let Ok(opacity) = val.parse::<f64>() {
                let color = style.stroke.as_ref().map(|s| s.color.clone()).unwrap_or_else(|| "#000000".to_string());
                let w = style.stroke.as_ref().and_then(|s| s.width);
                let d = style.stroke.as_ref().and_then(|s| s.dash_array.clone());
                style.stroke = Some(StrokeStyle { color, width: w, opacity: opacity.min(1.0).max(0.0), dash_array: d });
            }
        }
        "stroke-dasharray" => {
            let dash: Vec<f64> = val.split(|c: char| c == ' ' || c == ',')
                .filter_map(|s| s.trim().parse().ok())
                .collect();
            if !dash.is_empty() {
                let color = style.stroke.as_ref().map(|s| s.color.clone()).unwrap_or_else(|| "#000000".to_string());
                let w = style.stroke.as_ref().and_then(|s| s.width);
                let o = style.stroke.as_ref().map(|s| s.opacity).unwrap_or(1.0);
                style.stroke = Some(StrokeStyle { color, width: w, opacity: o, dash_array: Some(dash) });
            }
        }
        "mark" => {
            let clean = val.trim().strip_prefix("symbol(").and_then(|s| s.strip_suffix(')')).unwrap_or(val);
            style.mark = Some(clean.trim().to_lowercase());
        }
        "mark-size" => {
            if let Ok(s) = val.parse::<f64>() {
                style.point_size = Some(s);
            }
        }
        _ => {}
    }
}

fn normalize_color(val: &str) -> Option<String> {
    let val = val.trim();
    if val.starts_with('#') {
        Some(val.to_string())
    } else if let Some(hex) = named_color_to_hex(val) {
        Some(hex)
    } else {
        let val = val.trim_start_matches('#');
        if val.len() == 6 && val.chars().all(|c| c.is_ascii_hexdigit()) {
            Some(format!("#{}", val))
        } else {
            None
        }
    }
}

fn named_color_to_hex(name: &str) -> Option<String> {
    let mut colors = HashMap::new();
    colors.insert("red", "#FF0000");
    colors.insert("green", "#008000");
    colors.insert("blue", "#0000FF");
    colors.insert("white", "#FFFFFF");
    colors.insert("black", "#000000");
    colors.insert("yellow", "#FFFF00");
    colors.insert("orange", "#FFA500");
    colors.insert("purple", "#800080");
    colors.insert("pink", "#FFC0CB");
    colors.insert("brown", "#A52A2A");
    colors.insert("gray", "#808080");
    colors.insert("grey", "#808080");
    colors.insert("cyan", "#00FFFF");
    colors.insert("magenta", "#FF00FF");
    colors.insert("transparent", "#00000000");
    colors.get(name.to_lowercase().as_str()).map(|s| s.to_string())
}

pub fn builtin_css_styles() -> Vec<super::sld_parser::BuiltinStyle> {
    vec![
        super::sld_parser::BuiltinStyle {
            name: "css-default",
            title: "CSS 默认样式",
            sld: r#"[@scale < 500000] {
  fill: #6688aa;
  fill-opacity: 0.6;
  stroke: #334455;
  stroke-width: 1;
  mark: symbol(circle);
  mark-size: 8;
}"#,
        },
        super::sld_parser::BuiltinStyle {
            name: "css-water",
            title: "CSS 水域",
            sld: r#"* {
  fill: #bbdefb;
  fill-opacity: 0.7;
  stroke: #1976d2;
  stroke-width: 0.5;
}"#,
        },
    ]
}

pub fn default_css(layer_name: &str) -> String {
    format!(r#"/* Auto-generated CSS style for layer: {} */
* {{
  fill: #6688aa;
  fill-opacity: 0.6;
  stroke: #334455;
  stroke-width: 1;
  mark: symbol(circle);
  mark-size: 8;
}}"#, layer_name)
}
