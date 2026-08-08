use super::rendering::{FillStyle, StrokeStyle, Style};
use super::sld_parser::{OgcFilter, ParsedRule};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
struct YsldRoot {
    #[allow(dead_code)]
    name: Option<String>,
    #[allow(dead_code)]
    title: Option<String>,
    #[serde(rename = "feature-styles")]
    feature_styles: Option<Vec<YsldFeatureStyle>>,
}

#[derive(Debug, Deserialize)]
struct YsldFeatureStyle {
    #[allow(dead_code)]
    name: Option<String>,
    rules: Option<Vec<YsldRule>>,
}

#[derive(Debug, Deserialize)]
struct YsldRule {
    #[allow(dead_code)]
    name: Option<String>,
    scale: Option<Vec<f64>>,
    filter: Option<serde_json::Value>,
    symbolizers: Option<Vec<HashMap<String, serde_json::Value>>>,
}

pub fn parse_ysld(ysld: &str) -> Vec<ParsedRule> {
    let root: YsldRoot = match serde_yaml::from_str(ysld) {
        Ok(r) => r,
        Err(_) => return vec![],
    };

    let mut rules = Vec::new();

    if let Some(feature_styles) = root.feature_styles {
        for fs in feature_styles {
            if let Some(rule_defs) = fs.rules {
                for rd in rule_defs {
                    let mut rule = ParsedRule {
                        name: rd.name.clone(),
                        min_scale: None,
                        max_scale: None,
                        filters: vec![],
                        style: Style::new(),
                    };

                    if let Some(ref scale) = rd.scale {
                        if scale.len() >= 2 {
                            rule.min_scale = Some(scale[0]);
                            rule.max_scale = Some(scale[1]);
                        }
                    }

                    if let Some(ref filter_val) = rd.filter {
                        rule.filters = parse_ysld_filters(filter_val);
                    }

                    if let Some(ref symbolizers) = rd.symbolizers {
                        for sym in symbolizers {
                            apply_ysld_symbolizer(&mut rule.style, sym);
                        }
                    }

                    rules.push(rule);
                }
            }
        }
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

fn parse_ysld_filters(val: &serde_json::Value) -> Vec<OgcFilter> {
    let mut filters = Vec::new();
    if let Some(arr) = val.as_array() {
        for item in arr {
            if let Some(obj) = item.as_object() {
                if let Some(prop) = obj.get("property").and_then(|v| v.as_str()) {
                    for (key, val) in obj {
                        match key.as_str() {
                            "greater-than" => {
                                if let Some(v) = val.as_str() {
                                    filters.push(OgcFilter::PropertyIsGreaterThan(
                                        prop.to_string(),
                                        v.to_string(),
                                    ));
                                }
                            },
                            "less-than" => {
                                if let Some(v) = val.as_str() {
                                    filters.push(OgcFilter::PropertyIsLessThan(
                                        prop.to_string(),
                                        v.to_string(),
                                    ));
                                }
                            },
                            "equals" => {
                                if let Some(v) = val.as_str() {
                                    filters.push(OgcFilter::PropertyIsEqualTo(
                                        prop.to_string(),
                                        v.to_string(),
                                    ));
                                }
                            },
                            "not-equal" => {
                                if let Some(v) = val.as_str() {
                                    filters.push(OgcFilter::PropertyIsNotEqualTo(
                                        prop.to_string(),
                                        v.to_string(),
                                    ));
                                }
                            },
                            "like" => {
                                if let Some(v) = val.as_str() {
                                    filters.push(OgcFilter::PropertyIsLike(
                                        prop.to_string(),
                                        v.to_string(),
                                    ));
                                }
                            },
                            "between" => {
                                if let Some(arr) = val.as_array() {
                                    if arr.len() >= 2 {
                                        if let (Some(low), Some(high)) =
                                            (arr[0].as_str(), arr[1].as_str())
                                        {
                                            filters.push(OgcFilter::PropertyIsBetween(
                                                prop.to_string(),
                                                low.to_string(),
                                                high.to_string(),
                                            ));
                                        }
                                    }
                                }
                            },
                            _ => {},
                        }
                    }
                }
            }
        }
    }
    filters
}

fn apply_ysld_symbolizer(style: &mut Style, sym: &HashMap<String, serde_json::Value>) {
    for (sym_type, props) in sym {
        match sym_type.as_str() {
            "polygon" => {
                if let Some(obj) = props.as_object() {
                    apply_ysld_polygon(style, obj);
                }
            },
            "line" => {
                if let Some(obj) = props.as_object() {
                    apply_ysld_line(style, obj);
                }
            },
            "point" => {
                if let Some(obj) = props.as_object() {
                    apply_ysld_point(style, obj);
                }
            },
            _ => {},
        }
    }
}

fn apply_ysld_polygon(style: &mut Style, props: &serde_json::Map<String, serde_json::Value>) {
    let mut fill_color = style
        .fill
        .as_ref()
        .map(|f| f.color.clone())
        .unwrap_or_else(|| "#808080".to_string());
    let mut fill_opacity = style.fill.as_ref().map(|f| f.opacity).unwrap_or(1.0);
    let mut stroke_color = style
        .stroke
        .as_ref()
        .map(|s| s.color.clone())
        .unwrap_or_else(|| "#000000".to_string());
    let mut stroke_width = style.stroke.as_ref().and_then(|s| s.width);
    let mut stroke_opacity = style.stroke.as_ref().map(|s| s.opacity).unwrap_or(1.0);

    for (key, val) in props {
        match key.as_str() {
            "fill-color" => {
                if let Some(s) = val.as_str() {
                    fill_color = s.to_string();
                }
            },
            "fill-opacity" => {
                if let Some(n) = val.as_f64() {
                    fill_opacity = n.min(1.0).max(0.0);
                }
            },
            "stroke-color" => {
                if let Some(s) = val.as_str() {
                    stroke_color = s.to_string();
                }
            },
            "stroke-width" => {
                if let Some(n) = val.as_f64() {
                    stroke_width = Some(n);
                }
            },
            "stroke-opacity" => {
                if let Some(n) = val.as_f64() {
                    stroke_opacity = n.min(1.0).max(0.0);
                }
            },
            _ => {},
        }
    }

    style.fill = Some(FillStyle {
        color: fill_color,
        opacity: fill_opacity,
    });
    style.stroke = Some(StrokeStyle {
        color: stroke_color,
        width: stroke_width,
        opacity: stroke_opacity,
        dash_array: style.stroke.as_ref().and_then(|s| s.dash_array.clone()),
    });
}

fn apply_ysld_line(style: &mut Style, props: &serde_json::Map<String, serde_json::Value>) {
    let mut stroke_color = style
        .stroke
        .as_ref()
        .map(|s| s.color.clone())
        .unwrap_or_else(|| "#000000".to_string());
    let mut stroke_width = style.stroke.as_ref().and_then(|s| s.width);
    let mut stroke_opacity = style.stroke.as_ref().map(|s| s.opacity).unwrap_or(1.0);

    for (key, val) in props {
        match key.as_str() {
            "stroke-color" => {
                if let Some(s) = val.as_str() {
                    stroke_color = s.to_string();
                }
            },
            "stroke-width" => {
                if let Some(n) = val.as_f64() {
                    stroke_width = Some(n);
                }
            },
            "stroke-opacity" => {
                if let Some(n) = val.as_f64() {
                    stroke_opacity = n.min(1.0).max(0.0);
                }
            },
            _ => {},
        }
    }

    style.stroke = Some(StrokeStyle {
        color: stroke_color,
        width: stroke_width,
        opacity: stroke_opacity,
        dash_array: style.stroke.as_ref().and_then(|s| s.dash_array.clone()),
    });
}

fn apply_ysld_point(style: &mut Style, props: &serde_json::Map<String, serde_json::Value>) {
    let mut fill_color = style
        .fill
        .as_ref()
        .map(|f| f.color.clone())
        .unwrap_or_else(|| "#FF0000".to_string());
    let mut fill_opacity = style.fill.as_ref().map(|f| f.opacity).unwrap_or(1.0);
    let mut stroke_color = style
        .stroke
        .as_ref()
        .map(|s| s.color.clone())
        .unwrap_or_else(|| "#000000".to_string());
    let mut stroke_width = style.stroke.as_ref().and_then(|s| s.width);
    let mut mark = style.mark.clone();
    let mut point_size = style.point_size;

    for (key, val) in props {
        match key.as_str() {
            "fill-color" => {
                if let Some(s) = val.as_str() {
                    fill_color = s.to_string();
                }
            },
            "fill-opacity" => {
                if let Some(n) = val.as_f64() {
                    fill_opacity = n.min(1.0).max(0.0);
                }
            },
            "stroke-color" => {
                if let Some(s) = val.as_str() {
                    stroke_color = s.to_string();
                }
            },
            "stroke-width" => {
                if let Some(n) = val.as_f64() {
                    stroke_width = Some(n);
                }
            },
            "mark" => {
                if let Some(s) = val.as_str() {
                    mark = Some(s.to_lowercase());
                }
            },
            "mark-size" => {
                if let Some(n) = val.as_f64() {
                    point_size = Some(n);
                }
            },
            "size" => {
                if let Some(n) = val.as_f64() {
                    point_size = Some(n);
                }
            },
            _ => {},
        }
    }

    style.fill = Some(FillStyle {
        color: fill_color,
        opacity: fill_opacity,
    });
    if stroke_width.is_some() || stroke_color != "#000000" {
        style.stroke = Some(StrokeStyle {
            color: stroke_color,
            width: stroke_width,
            opacity: 1.0,
            dash_array: None,
        });
    }
    style.mark = mark;
    style.point_size = point_size;
}

pub fn default_ysld(layer_name: &str) -> String {
    format!(
        "name: \"{}\"\ntitle: \"{} Style\"\nfeature-styles:\n- name: default\n  rules:\n  - symbolizers:\n    - polygon:\n        fill-color: \"#6688aa\"\n        fill-opacity: 0.6\n        stroke-color: \"#334455\"\n        stroke-width: 1\n    - line:\n        stroke-color: \"#334455\"\n        stroke-width: 1\n    - point:\n        mark: circle\n        mark-size: 8\n        fill-color: \"#6688aa\"\n        stroke-color: \"#334455\"\n        stroke-width: 1\n",
        layer_name, layer_name
    )
}
