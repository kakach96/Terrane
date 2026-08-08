use super::rendering::{FillStyle, StrokeStyle, Style};
use super::sld_parser::{OgcFilter, ParsedRule};
use serde_json::Value;

pub fn parse_mbstyle(json: &str) -> Vec<ParsedRule> {
    let root: Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return vec![],
    };

    let mut rules = Vec::new();

    if let Some(layers) = root.get("layers").and_then(|v| v.as_array()) {
        for layer in layers {
            let layer_type = layer.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let paint = layer.get("paint");
            let layout = layer.get("layout");
            let filter = layer.get("filter");

            let mut style = Style::new();
            match layer_type {
                "fill" => {
                    apply_fill_paint(&mut style, paint);
                },
                "line" => {
                    apply_line_paint(&mut style, paint);
                },
                "circle" => {
                    apply_circle_paint(&mut style, paint);
                },
                "background" => {
                    apply_fill_paint(&mut style, paint);
                },
                _ => {},
            }

            apply_layout(&mut style, layout);

            let feature_filters = if let Some(f) = filter {
                parse_mbstyle_filter(f)
            } else {
                vec![]
            };

            rules.push(ParsedRule {
                name: layer
                    .get("id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                min_scale: layer.get("minzoom").and_then(|v| v.as_f64()),
                max_scale: layer.get("maxzoom").and_then(|v| v.as_f64()),
                filters: feature_filters,
                style,
            });
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

fn apply_fill_paint(style: &mut Style, paint: Option<&Value>) {
    if let Some(p) = paint {
        if let Some(c) = get_color(p, "fill-color") {
            style.fill = Some(FillStyle {
                color: c,
                opacity: style.fill.as_ref().map(|f| f.opacity).unwrap_or(1.0),
            });
        }
        if let Some(o) = p.get("fill-opacity").and_then(|v| v.as_f64()) {
            let color = style
                .fill
                .as_ref()
                .map(|f| f.color.clone())
                .unwrap_or_else(|| "#808080".to_string());
            style.fill = Some(FillStyle {
                color,
                opacity: o.min(1.0).max(0.0),
            });
        }
        if let Some(c) = get_color(p, "fill-outline-color") {
            style.stroke = Some(StrokeStyle {
                color: c,
                width: style.stroke.as_ref().and_then(|s| s.width),
                opacity: style.stroke.as_ref().map(|s| s.opacity).unwrap_or(1.0),
                dash_array: style.stroke.as_ref().and_then(|s| s.dash_array.clone()),
            });
        }
    }
}

fn apply_line_paint(style: &mut Style, paint: Option<&Value>) {
    if let Some(p) = paint {
        if let Some(c) = get_color(p, "line-color") {
            let w = style.stroke.as_ref().and_then(|s| s.width);
            let o = style.stroke.as_ref().map(|s| s.opacity).unwrap_or(1.0);
            let d = style.stroke.as_ref().and_then(|s| s.dash_array.clone());
            style.stroke = Some(StrokeStyle {
                color: c,
                width: w,
                opacity: o,
                dash_array: d,
            });
        }
        if let Some(w) = p.get("line-width").and_then(|v| v.as_f64()) {
            let color = style
                .stroke
                .as_ref()
                .map(|s| s.color.clone())
                .unwrap_or_else(|| "#000000".to_string());
            let o = style.stroke.as_ref().map(|s| s.opacity).unwrap_or(1.0);
            let d = style.stroke.as_ref().and_then(|s| s.dash_array.clone());
            style.stroke = Some(StrokeStyle {
                color,
                width: Some(w),
                opacity: o,
                dash_array: d,
            });
        }
        if let Some(o) = p.get("line-opacity").and_then(|v| v.as_f64()) {
            let color = style
                .stroke
                .as_ref()
                .map(|s| s.color.clone())
                .unwrap_or_else(|| "#000000".to_string());
            let w = style.stroke.as_ref().and_then(|s| s.width);
            let d = style.stroke.as_ref().and_then(|s| s.dash_array.clone());
            style.stroke = Some(StrokeStyle {
                color,
                width: w,
                opacity: o.min(1.0).max(0.0),
                dash_array: d,
            });
        }
        if let Some(dash) = p.get("line-dasharray").and_then(|v| v.as_array()) {
            let dash_vec: Vec<f64> = dash.iter().filter_map(|v| v.as_f64()).collect();
            if !dash_vec.is_empty() {
                let color = style
                    .stroke
                    .as_ref()
                    .map(|s| s.color.clone())
                    .unwrap_or_else(|| "#000000".to_string());
                let w = style.stroke.as_ref().and_then(|s| s.width);
                let o = style.stroke.as_ref().map(|s| s.opacity).unwrap_or(1.0);
                style.stroke = Some(StrokeStyle {
                    color,
                    width: w,
                    opacity: o,
                    dash_array: Some(dash_vec),
                });
            }
        }
    }
}

fn apply_circle_paint(style: &mut Style, paint: Option<&Value>) {
    if let Some(p) = paint {
        if let Some(c) = get_color(p, "circle-color") {
            style.fill = Some(FillStyle {
                color: c,
                opacity: style.fill.as_ref().map(|f| f.opacity).unwrap_or(1.0),
            });
        }
        if let Some(o) = p.get("circle-opacity").and_then(|v| v.as_f64()) {
            let color = style
                .fill
                .as_ref()
                .map(|f| f.color.clone())
                .unwrap_or_else(|| "#FF0000".to_string());
            style.fill = Some(FillStyle {
                color,
                opacity: o.min(1.0).max(0.0),
            });
        }
        if let Some(r) = p.get("circle-radius").and_then(|v| v.as_f64()) {
            style.point_size = Some(r * 2.0);
        }
        if let Some(c) = get_color(p, "circle-stroke-color") {
            style.stroke = Some(StrokeStyle {
                color: c,
                width: style.stroke.as_ref().and_then(|s| s.width).or(Some(1.0)),
                opacity: style.stroke.as_ref().map(|s| s.opacity).unwrap_or(1.0),
                dash_array: None,
            });
        }
        if let Some(w) = p.get("circle-stroke-width").and_then(|v| v.as_f64()) {
            let color = style
                .stroke
                .as_ref()
                .map(|s| s.color.clone())
                .unwrap_or_else(|| "#000000".to_string());
            style.stroke = Some(StrokeStyle {
                color,
                width: Some(w),
                opacity: style.stroke.as_ref().map(|s| s.opacity).unwrap_or(1.0),
                dash_array: None,
            });
        }
        style.mark = Some("circle".to_string());
    }
}

fn apply_layout(_style: &mut Style, layout: Option<&Value>) {
    if let Some(l) = layout {
        if let Some(visibility) = l.get("visibility").and_then(|v| v.as_str()) {
            if visibility == "none" {}
        }
    }
}

fn get_color(paint: &Value, key: &str) -> Option<String> {
    let val = paint.get(key)?;
    match val {
        Value::String(s) => Some(s.clone()),
        Value::Array(arr) => {
            if arr.len() >= 3 {
                let r = arr[0].as_f64().unwrap_or(0.0) as u8;
                let g = arr[1].as_f64().unwrap_or(0.0) as u8;
                let b = arr[2].as_f64().unwrap_or(0.0) as u8;
                Some(format!("#{:02X}{:02X}{:02X}", r, g, b))
            } else {
                None
            }
        },
        _ => None,
    }
}

fn parse_mbstyle_filter(filter: &Value) -> Vec<OgcFilter> {
    let mut filters = Vec::new();
    if let Some(arr) = filter.as_array() {
        if arr.is_empty() {
            return filters;
        }
        let op = arr[0].as_str().unwrap_or("");
        match op {
            "==" => {
                if arr.len() >= 3 {
                    let prop = arr[1].as_str().unwrap_or("");
                    let val = value_to_string(&arr[2]);
                    filters.push(OgcFilter::PropertyIsEqualTo(prop.to_string(), val));
                }
            },
            "!=" => {
                if arr.len() >= 3 {
                    let prop = arr[1].as_str().unwrap_or("");
                    let val = value_to_string(&arr[2]);
                    filters.push(OgcFilter::PropertyIsNotEqualTo(prop.to_string(), val));
                }
            },
            "<" => {
                if arr.len() >= 3 {
                    let prop = arr[1].as_str().unwrap_or("");
                    let val = value_to_string(&arr[2]);
                    filters.push(OgcFilter::PropertyIsLessThan(prop.to_string(), val));
                }
            },
            ">" => {
                if arr.len() >= 3 {
                    let prop = arr[1].as_str().unwrap_or("");
                    let val = value_to_string(&arr[2]);
                    filters.push(OgcFilter::PropertyIsGreaterThan(prop.to_string(), val));
                }
            },
            "<=" => {
                if arr.len() >= 3 {
                    let prop = arr[1].as_str().unwrap_or("");
                    let val = value_to_string(&arr[2]);
                    filters.push(OgcFilter::PropertyIsLessThanOrEqualTo(
                        prop.to_string(),
                        val,
                    ));
                }
            },
            ">=" => {
                if arr.len() >= 3 {
                    let prop = arr[1].as_str().unwrap_or("");
                    let val = value_to_string(&arr[2]);
                    filters.push(OgcFilter::PropertyIsGreaterThanOrEqualTo(
                        prop.to_string(),
                        val,
                    ));
                }
            },
            "all" => {
                let sub: Vec<Vec<OgcFilter>> = arr[1..]
                    .iter()
                    .map(|v| parse_mbstyle_filter(v))
                    .filter(|v| !v.is_empty())
                    .collect();
                if sub.len() == 1 {
                    filters.extend(sub.into_iter().next().unwrap());
                } else if sub.len() > 1 {
                    filters.push(OgcFilter::And(sub.into_iter().flatten().collect()));
                }
            },
            "any" => {
                let sub: Vec<Vec<OgcFilter>> = arr[1..]
                    .iter()
                    .map(|v| parse_mbstyle_filter(v))
                    .filter(|v| !v.is_empty())
                    .collect();
                if sub.len() == 1 {
                    filters.extend(sub.into_iter().next().unwrap());
                } else if sub.len() > 1 {
                    filters.push(OgcFilter::Or(sub.into_iter().flatten().collect()));
                }
            },
            "none" => {
                let sub: Vec<OgcFilter> = arr[1..]
                    .iter()
                    .flat_map(|v| parse_mbstyle_filter(v))
                    .collect();
                if !sub.is_empty() {
                    filters.push(OgcFilter::Not(Box::new(OgcFilter::Or(sub))));
                }
            },
            "has" => {
                if arr.len() >= 2 {
                    let prop = arr[1].as_str().unwrap_or("");
                    filters.push(OgcFilter::PropertyIsNotEqualTo(
                        prop.to_string(),
                        String::new(),
                    ));
                }
            },
            "!has" => {
                if arr.len() >= 2 {
                    let prop = arr[1].as_str().unwrap_or("");
                    filters.push(OgcFilter::PropertyIsNull(prop.to_string()));
                }
            },
            _ => {},
        }
    }
    filters
}

fn value_to_string(val: &Value) -> String {
    match val {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        _ => val.to_string(),
    }
}

pub fn default_mbstyle(layer_name: &str) -> String {
    serde_json::json!({
        "version": 8,
        "name": layer_name,
        "layers": [
            {
                "id": layer_name,
                "type": "fill",
                "paint": {
                    "fill-color": "#6688aa",
                    "fill-opacity": 0.6,
                    "fill-outline-color": "#334455"
                }
            },
            {
                "id": format!("{}_line", layer_name),
                "type": "line",
                "paint": {
                    "line-color": "#334455",
                    "line-width": 1
                }
            },
            {
                "id": format!("{}_point", layer_name),
                "type": "circle",
                "paint": {
                    "circle-color": "#6688aa",
                    "circle-radius": 4,
                    "circle-stroke-color": "#334455",
                    "circle-stroke-width": 1
                }
            }
        ]
    })
    .to_string()
}
