use super::rendering::{FillStyle, LabelStyle, StrokeStyle, Style};
use crate::models::{Feature, PropertyValue};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;

const MARK_NAMES: &[&str] = &["circle", "square", "cross", "x", "star", "triangle"];

#[derive(Debug, Clone)]
pub struct ParsedRule {
    pub name: Option<String>,
    pub min_scale: Option<f64>,
    pub max_scale: Option<f64>,
    pub filters: Vec<OgcFilter>,
    pub style: Style,
}

/// Per-rule label metadata collected while parsing a TextSymbolizer.
#[derive(Debug, Clone, Default)]
struct LabelCtx {
    /// Property name to read the label text from (`ogc:PropertyName`).
    property: Option<String>,
    /// Literal label text (`<Label>` with plain text content).
    literal: Option<String>,
    /// Font size in points (SLD `Font/CssParameter name="font-size"`).
    font_size: f64,
    /// Label fill color (SLD `TextSymbolizer/Fill`).
    color: Option<String>,
    /// Halo color (SLD `Halo/Fill`).
    halo_color: Option<String>,
    /// Halo radius in px (SLD `Halo/Radius`).
    halo_radius: f64,
}

#[derive(Debug, Clone)]
pub enum OgcFilter {
    PropertyIsEqualTo(String, String),
    PropertyIsNotEqualTo(String, String),
    PropertyIsLessThan(String, String),
    PropertyIsGreaterThan(String, String),
    PropertyIsLessThanOrEqualTo(String, String),
    PropertyIsGreaterThanOrEqualTo(String, String),
    PropertyIsLike(String, String),
    PropertyIsNull(String),
    PropertyIsBetween(String, String, String),
    And(Vec<OgcFilter>),
    Or(Vec<OgcFilter>),
    Not(Box<OgcFilter>),
}

pub fn parse_sld(xml: &str) -> Vec<ParsedRule> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut rules = Vec::new();
    let mut in_rule = false;

    let mut current_rule = ParsedRule {
        name: None,
        min_scale: None,
        max_scale: None,
        filters: vec![],
        style: Style::new(),
    };
    let mut in_polygon_symbolizer = false;
    let mut in_line_symbolizer = false;
    let mut in_point_symbolizer = false;
    let mut in_text_symbolizer = false;
    let mut in_fill = false;
    let mut in_stroke = false;
    let mut in_graphic = false;
    let mut in_mark = false;
    let mut in_ogc_filter = false;
    let mut _current_property = String::new();
    let mut current_literal = String::new();
    let mut collect_text = false;
    let mut css_param_name = String::new();
    let mut css_param_value = String::new();

    // ogc:Filter building state. Comparison operators are captured by name and
    // their PropertyName/Literal children collected; logical operators
    // (And/Or/Not) nest via a stack.
    let mut filter_stack: Vec<Vec<OgcFilter>> = Vec::new();
    let mut ogc_op: Option<String> = None;
    let mut filter_props: Vec<String> = Vec::new();
    let mut filter_literals: Vec<String> = Vec::new();

    // Label (TextSymbolizer) state.
    let mut label = LabelCtx::default();
    let mut in_label = false;
    let mut in_label_fill = false;
    let mut in_font = false;
    let mut in_halo = false;
    let mut in_halo_fill = false;
    let mut in_halo_radius = false;
    // Raw text collected inside `<Label>` (disambiguated on End events).
    let mut label_raw = String::new();
    // z-index vendor option (`<VendorOption name="z-index">5</VendorOption>`).
    let mut in_vendor_option = false;
    let mut vendor_option_name = String::new();
    let mut vendor_option_value = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let local = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let tag = local.split(':').next_back().unwrap_or(&local).to_string();

                match tag.as_str() {
                    "Rule" => {
                        in_rule = true;
                        current_rule = ParsedRule {
                            name: None,
                            min_scale: None,
                            max_scale: None,
                            filters: vec![],
                            style: Style::new(),
                        };
                    },
                    "Name" if in_rule => {
                        collect_text = true;
                    },
                    "MinScaleDenominator" if in_rule => {
                        collect_text = true;
                    },
                    "MaxScaleDenominator" if in_rule => {
                        collect_text = true;
                    },
                    "PolygonSymbolizer" => in_polygon_symbolizer = true,
                    "LineSymbolizer" => in_line_symbolizer = true,
                    "PointSymbolizer" => in_point_symbolizer = true,
                    "TextSymbolizer" => {
                        in_text_symbolizer = true;
                        label = LabelCtx::default();
                    },
                    "Fill" if in_polygon_symbolizer || in_mark => in_fill = true,
                    "Stroke" if in_polygon_symbolizer || in_line_symbolizer || in_mark => {
                        in_stroke = true
                    },
                    "Graphic" if in_point_symbolizer => in_graphic = true,
                    "Mark" if in_graphic => in_mark = true,
                    "WellKnownName" if in_mark => {
                        collect_text = true;
                    },
                    "Size" if in_graphic => {
                        collect_text = true;
                    },
                    "CssParameter" => {
                        css_param_name = e
                            .attributes()
                            .filter_map(|a| a.ok())
                            .find(|a| String::from_utf8_lossy(a.key.as_ref()) == "name")
                            .map(|a| String::from_utf8_lossy(&a.value).to_string())
                            .unwrap_or_default();
                        collect_text = true;
                    },
                    "PropertyName" if in_ogc_filter => {
                        collect_text = true;
                    },
                    "Literal" if in_ogc_filter => {
                        collect_text = true;
                    },
                    "Filter" | "ogc:Filter" => {
                        in_ogc_filter = true;
                        filter_stack.clear();
                        ogc_op = None;
                        filter_props.clear();
                        filter_literals.clear();
                    },
                    // --- ogc:Filter comparison / logical operators ---
                    "PropertyIsEqualTo"
                    | "PropertyIsNotEqualTo"
                    | "PropertyIsLessThan"
                    | "PropertyIsGreaterThan"
                    | "PropertyIsLessThanOrEqualTo"
                    | "PropertyIsGreaterThanOrEqualTo"
                    | "PropertyIsLike"
                    | "PropertyIsNull"
                    | "PropertyIsBetween" => {
                        ogc_op = Some(tag.clone());
                        filter_props.clear();
                        filter_literals.clear();
                    },
                    "And" | "Or" => {
                        filter_stack.push(Vec::new());
                    },
                    "Not" => {
                        filter_stack.push(Vec::new());
                    },
                    // --- Label (TextSymbolizer) ---
                    "Label" if in_text_symbolizer => {
                        in_label = true;
                        collect_text = true;
                    },
                    "PropertyName" if in_label => {
                        collect_text = true;
                    },
                    "Literal" if in_label => {
                        collect_text = true;
                    },
                    "Fill" if in_text_symbolizer && !in_halo => {
                        in_label_fill = true;
                    },
                    "Font" if in_text_symbolizer => {
                        in_font = true;
                    },
                    "Halo" if in_text_symbolizer => {
                        in_halo = true;
                    },
                    "Fill" if in_halo => {
                        in_halo_fill = true;
                    },
                    "Radius" if in_halo => {
                        in_halo_radius = true;
                        collect_text = true;
                    },
                    // --- z-index vendor option ---
                    "VendorOption" => {
                        in_vendor_option = true;
                        vendor_option_name = e
                            .attributes()
                            .filter_map(|a| a.ok())
                            .find(|a| String::from_utf8_lossy(a.key.as_ref()) == "name")
                            .map(|a| String::from_utf8_lossy(&a.value).to_string())
                            .unwrap_or_default();
                        collect_text = true;
                    },
                    _ => {},
                }
            },

            Ok(Event::Text(ref e)) if collect_text => {
                let text = String::from_utf8_lossy(e.as_ref()).trim().to_string();
                if !text.is_empty() {
                    if in_ogc_filter {
                        current_literal = text;
                    } else if in_label {
                        label_raw = text;
                    } else if in_halo_radius {
                        label.halo_radius = text.parse().unwrap_or(1.0);
                    } else if in_vendor_option {
                        vendor_option_value = text;
                    } else if !css_param_name.is_empty() {
                        css_param_value = text;
                    } else if (in_mark && text.len() < 20) || in_graphic {
                        current_literal = text;
                    } else {
                        // Fallback: bare element text (Rule Name,
                        // Min/MaxScaleDenominator, etc.).
                        current_literal = text;
                    }
                }
            },

            Ok(Event::End(ref e)) => {
                let local = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let tag = local.split(':').next_back().unwrap_or(&local).to_string();

                match tag.as_str() {
                    "Rule" => {
                        in_rule = false;
                        rules.push(current_rule.clone());
                    },
                    "Name" if in_rule => {
                        current_rule.name = Some(current_literal.clone());
                        current_literal.clear();
                        collect_text = false;
                    },
                    "MinScaleDenominator" => {
                        if let Ok(v) = current_literal.parse::<f64>() {
                            current_rule.min_scale = Some(v);
                        }
                        current_literal.clear();
                        collect_text = false;
                    },
                    "MaxScaleDenominator" => {
                        if let Ok(v) = current_literal.parse::<f64>() {
                            current_rule.max_scale = Some(v);
                        }
                        current_literal.clear();
                        collect_text = false;
                    },
                    "PolygonSymbolizer" => in_polygon_symbolizer = false,
                    "LineSymbolizer" => in_line_symbolizer = false,
                    "PointSymbolizer" => in_point_symbolizer = false,
                    "TextSymbolizer" => {
                        in_text_symbolizer = false;
                        // Finalize the label style from the collected context.
                        if let Some(text) = label.literal.clone().or_else(|| label.property.clone())
                        {
                            current_rule.style.label = Some(LabelStyle {
                                text,
                                property: label.property.clone(),
                                font_size: if label.font_size > 0.0 {
                                    label.font_size
                                } else {
                                    12.0
                                },
                                color: label.color.clone().unwrap_or_else(|| "#333333".to_string()),
                                halo_color: label.halo_color.clone(),
                                halo_radius: if label.halo_radius > 0.0 {
                                    label.halo_radius
                                } else {
                                    1.0
                                },
                            });
                        }
                        label = LabelCtx::default();
                    },
                    "Fill" => {
                        if in_halo_fill {
                            if !css_param_value.is_empty() {
                                apply_label_fill(
                                    &mut label,
                                    &css_param_name,
                                    &css_param_value,
                                    true,
                                );
                                css_param_value.clear();
                            }
                            in_halo_fill = false;
                        } else if in_label_fill {
                            if !css_param_value.is_empty() {
                                apply_label_fill(
                                    &mut label,
                                    &css_param_name,
                                    &css_param_value,
                                    false,
                                );
                                css_param_value.clear();
                            }
                            in_label_fill = false;
                        } else {
                            in_fill = false;
                            if !css_param_value.is_empty() {
                                apply_css_param(
                                    &mut current_rule.style,
                                    &css_param_name,
                                    &css_param_value,
                                    true,
                                );
                                css_param_value.clear();
                            }
                        }
                    },
                    "Stroke" => {
                        in_stroke = false;
                        if !css_param_value.is_empty() {
                            apply_css_param(
                                &mut current_rule.style,
                                &css_param_name,
                                &css_param_value,
                                false,
                            );
                            css_param_value.clear();
                        }
                    },
                    "Graphic" => in_graphic = false,
                    "Mark" => {
                        in_mark = false;
                        if !current_literal.is_empty() {
                            let name = current_literal.to_lowercase();
                            if MARK_NAMES.contains(&name.as_str()) {
                                current_rule.style.mark = Some(name);
                            }
                            current_literal.clear();
                            collect_text = false;
                        }
                    },
                    "WellKnownName" => {
                        current_literal.clear();
                        collect_text = false;
                    },
                    "Size" => {
                        if let Ok(s) = current_literal.parse::<f64>() {
                            current_rule.style.point_size = Some(s);
                        }
                        current_literal.clear();
                        collect_text = false;
                    },
                    "CssParameter" => {
                        if !css_param_value.is_empty() {
                            if in_fill {
                                apply_css_param(
                                    &mut current_rule.style,
                                    &css_param_name,
                                    &css_param_value,
                                    true,
                                );
                            } else if in_stroke {
                                apply_css_param(
                                    &mut current_rule.style,
                                    &css_param_name,
                                    &css_param_value,
                                    false,
                                );
                            } else if in_font {
                                apply_label_font(&mut label, &css_param_name, &css_param_value);
                            } else if in_label_fill {
                                apply_label_fill(
                                    &mut label,
                                    &css_param_name,
                                    &css_param_value,
                                    false,
                                );
                            } else if in_halo_fill {
                                apply_label_fill(
                                    &mut label,
                                    &css_param_name,
                                    &css_param_value,
                                    true,
                                );
                            }
                        }
                        css_param_name.clear();
                        css_param_value.clear();
                        collect_text = false;
                    },
                    "PropertyName" => {
                        if in_label {
                            // PropertyName inside Label → label text property.
                            if !label_raw.is_empty() {
                                label.property = Some(label_raw.clone());
                                label.literal = None;
                            }
                            label_raw.clear();
                            collect_text = false;
                        } else if in_ogc_filter {
                            if !current_literal.is_empty() {
                                filter_props.push(current_literal.clone());
                            }
                            current_literal.clear();
                            collect_text = false;
                        } else {
                            _current_property = current_literal.clone();
                            current_literal.clear();
                            collect_text = false;
                        }
                    },
                    "Literal" => {
                        if in_label {
                            if !label_raw.is_empty() {
                                label.literal = Some(label_raw.clone());
                                label.property = None;
                            }
                            label_raw.clear();
                            collect_text = false;
                        } else if in_ogc_filter {
                            if !current_literal.is_empty() {
                                filter_literals.push(current_literal.clone());
                            }
                            current_literal.clear();
                            collect_text = false;
                        } else {
                            current_literal.clear();
                            collect_text = false;
                        }
                    },
                    // --- ogc:Filter operator ends ---
                    "PropertyIsEqualTo"
                    | "PropertyIsNotEqualTo"
                    | "PropertyIsLessThan"
                    | "PropertyIsGreaterThan"
                    | "PropertyIsLessThanOrEqualTo"
                    | "PropertyIsGreaterThanOrEqualTo"
                    | "PropertyIsLike"
                    | "PropertyIsNull"
                    | "PropertyIsBetween" => {
                        if let (Some(op), Some(prop)) =
                            (ogc_op.take(), filter_props.first().cloned())
                        {
                            if let Some(f) = build_comparison_filter(&op, &prop, &filter_literals) {
                                push_filter(&mut filter_stack, &mut current_rule, f);
                            }
                        }
                        filter_props.clear();
                        filter_literals.clear();
                        collect_text = false;
                    },
                    "And" | "Or" | "Not" => {
                        if let Some(mut subs) = filter_stack.pop() {
                            if !subs.is_empty() {
                                let combined = match tag.as_str() {
                                    "And" => OgcFilter::And(subs),
                                    "Or" => OgcFilter::Or(subs),
                                    _ => OgcFilter::Not(Box::new(subs.remove(0))),
                                };
                                push_filter(&mut filter_stack, &mut current_rule, combined);
                            }
                        }
                        collect_text = false;
                    },
                    "Filter" | "ogc:Filter" => {
                        in_ogc_filter = false;
                        filter_stack.clear();
                        ogc_op = None;
                        filter_props.clear();
                        filter_literals.clear();
                    },
                    // --- Label (TextSymbolizer) ---
                    "Label" => {
                        in_label = false;
                        collect_text = false;
                        // Bare text content (no PropertyName/Literal child).
                        if label.property.is_none()
                            && label.literal.is_none()
                            && !label_raw.is_empty()
                        {
                            label.literal = Some(label_raw.clone());
                        }
                        label_raw.clear();
                    },
                    "Font" => {
                        in_font = false;
                    },
                    "Halo" => {
                        in_halo = false;
                        in_halo_radius = false;
                        collect_text = false;
                    },
                    "Radius" => {
                        in_halo_radius = false;
                        collect_text = false;
                    },
                    // --- z-index / composite vendor options ---
                    "VendorOption" => {
                        if vendor_option_name == "z-index" {
                            if let Ok(v) = vendor_option_value.trim().parse::<i32>() {
                                current_rule.style.z_index = v;
                            }
                        } else if vendor_option_name == "composite" {
                            if let Some(mode) =
                                super::rendering::CompositeOp::parse(&vendor_option_value)
                            {
                                current_rule.style.composite = mode;
                            }
                        }
                        vendor_option_name.clear();
                        vendor_option_value.clear();
                        in_vendor_option = false;
                        collect_text = false;
                    },
                    _ => {},
                }
            },

            Ok(Event::Eof) => break,
            Err(e) => {
                eprintln!("SLD parse error: {}", e);
                break;
            },
            _ => {},
        }
        buf.clear();
    }

    rules
}

/// Apply a `CssParameter` to the label fill/halo color.
fn apply_label_fill(label: &mut LabelCtx, name: &str, value: &str, halo: bool) {
    if name != "fill" {
        return;
    }
    let color = if value.starts_with('#') {
        value.to_string()
    } else {
        format!("#{}", value)
    };
    if halo {
        label.halo_color = Some(color);
    } else {
        label.color = Some(color);
    }
}

/// Apply a `CssParameter` inside `Font` (font-size / font-family).
fn apply_label_font(label: &mut LabelCtx, name: &str, value: &str) {
    if name == "font-size" {
        if let Ok(v) = value.trim().parse::<f64>() {
            label.font_size = v.max(1.0);
        }
    }
}

/// Build an `OgcFilter` from a comparison operator name, its property and its
/// literals (PropertyIsBetween takes two literals).
fn build_comparison_filter(op: &str, prop: &str, literals: &[String]) -> Option<OgcFilter> {
    let lit = |i: usize| literals.get(i).cloned();
    match op {
        "PropertyIsEqualTo" => lit(0).map(|v| OgcFilter::PropertyIsEqualTo(prop.to_string(), v)),
        "PropertyIsNotEqualTo" => {
            lit(0).map(|v| OgcFilter::PropertyIsNotEqualTo(prop.to_string(), v))
        },
        "PropertyIsLessThan" => lit(0).map(|v| OgcFilter::PropertyIsLessThan(prop.to_string(), v)),
        "PropertyIsGreaterThan" => {
            lit(0).map(|v| OgcFilter::PropertyIsGreaterThan(prop.to_string(), v))
        },
        "PropertyIsLessThanOrEqualTo" => {
            lit(0).map(|v| OgcFilter::PropertyIsLessThanOrEqualTo(prop.to_string(), v))
        },
        "PropertyIsGreaterThanOrEqualTo" => {
            lit(0).map(|v| OgcFilter::PropertyIsGreaterThanOrEqualTo(prop.to_string(), v))
        },
        "PropertyIsLike" => lit(0).map(|v| OgcFilter::PropertyIsLike(prop.to_string(), v)),
        "PropertyIsNull" => Some(OgcFilter::PropertyIsNull(prop.to_string())),
        "PropertyIsBetween" => match (lit(0), lit(1)) {
            (Some(low), Some(high)) => {
                Some(OgcFilter::PropertyIsBetween(prop.to_string(), low, high))
            },
            _ => None,
        },
        _ => None,
    }
}

/// Push a filter onto the innermost logical container, or into the rule's
/// filters when no container is open.
fn push_filter(filter_stack: &mut [Vec<OgcFilter>], rule: &mut ParsedRule, filter: OgcFilter) {
    match filter_stack.last_mut() {
        Some(container) => container.push(filter),
        None => rule.filters.push(filter),
    }
}

fn apply_css_param(style: &mut Style, name: &str, value: &str, is_fill: bool) {
    match name {
        "fill" if is_fill => {
            let color = if value.starts_with('#') {
                value.to_string()
            } else {
                format!("#{}", value)
            };
            style.fill = Some(FillStyle {
                color,
                opacity: style.fill.as_ref().map(|f| f.opacity).unwrap_or(1.0),
            });
        },
        "fill-opacity" if is_fill => {
            if let Ok(opacity) = value.parse::<f64>() {
                let color = style
                    .fill
                    .as_ref()
                    .map(|f| f.color.clone())
                    .unwrap_or_else(|| "#808080".to_string());
                style.fill = Some(FillStyle { color, opacity });
            }
        },
        "stroke" if !is_fill => {
            let color = if value.starts_with('#') {
                value.to_string()
            } else {
                format!("#{}", value)
            };
            let w = style.stroke.as_ref().and_then(|s| s.width);
            let o = style.stroke.as_ref().map(|s| s.opacity).unwrap_or(1.0);
            let d = style.stroke.as_ref().and_then(|s| s.dash_array.clone());
            style.stroke = Some(StrokeStyle {
                color,
                width: w,
                opacity: o,
                dash_array: d,
            });
        },
        "stroke-width" if !is_fill => {
            if let Ok(w) = value.parse::<f64>() {
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
        },
        "stroke-opacity" if !is_fill => {
            if let Ok(opacity) = value.parse::<f64>() {
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
                    opacity,
                    dash_array: d,
                });
            }
        },
        "stroke-dasharray" if !is_fill => {
            let dash: Vec<f64> = value
                .split([' ', ','])
                .filter_map(|s| s.trim().parse().ok())
                .collect();
            if !dash.is_empty() {
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
                    dash_array: Some(dash),
                });
            }
        },
        _ => {},
    }
}

pub fn match_rule(
    rule: &ParsedRule,
    feature_properties: &HashMap<String, PropertyValue>,
    scale_denom: Option<f64>,
) -> bool {
    if let Some(scale) = scale_denom {
        if let Some(min) = rule.min_scale {
            if scale < min {
                return false;
            }
        }
        if let Some(max) = rule.max_scale {
            if scale > max {
                return false;
            }
        }
    }
    if rule.filters.is_empty() {
        return true;
    }
    rule.filters
        .iter()
        .any(|f| evaluate_filter(f, feature_properties))
}

fn evaluate_filter(filter: &OgcFilter, props: &HashMap<String, PropertyValue>) -> bool {
    match filter {
        OgcFilter::PropertyIsEqualTo(prop, val) => props
            .get(prop)
            .map(|v| v.to_string() == *val)
            .unwrap_or(false),
        OgcFilter::PropertyIsNotEqualTo(prop, val) => props
            .get(prop)
            .map(|v| v.to_string() != *val)
            .unwrap_or(true),
        OgcFilter::PropertyIsLessThan(prop, val) => props
            .get(prop)
            .and_then(|v| v.to_string().parse::<f64>().ok())
            .zip(val.parse::<f64>().ok())
            .map(|(a, b)| a < b)
            .unwrap_or(false),
        OgcFilter::PropertyIsGreaterThan(prop, val) => props
            .get(prop)
            .and_then(|v| v.to_string().parse::<f64>().ok())
            .zip(val.parse::<f64>().ok())
            .map(|(a, b)| a > b)
            .unwrap_or(false),
        OgcFilter::PropertyIsLessThanOrEqualTo(prop, val) => props
            .get(prop)
            .and_then(|v| v.to_string().parse::<f64>().ok())
            .zip(val.parse::<f64>().ok())
            .map(|(a, b)| a <= b)
            .unwrap_or(false),
        OgcFilter::PropertyIsGreaterThanOrEqualTo(prop, val) => props
            .get(prop)
            .and_then(|v| v.to_string().parse::<f64>().ok())
            .zip(val.parse::<f64>().ok())
            .map(|(a, b)| a >= b)
            .unwrap_or(false),
        OgcFilter::PropertyIsLike(prop, pattern) => props
            .get(prop)
            .map(|v| wildcard_match(&v.to_string(), pattern))
            .unwrap_or(false),
        OgcFilter::PropertyIsNull(prop) => {
            !props.contains_key(prop) || matches!(props.get(prop), Some(PropertyValue::Null))
        },
        OgcFilter::PropertyIsBetween(prop, low, high) => props
            .get(prop)
            .and_then(|v| v.to_string().parse::<f64>().ok())
            .zip(low.parse::<f64>().ok())
            .zip(high.parse::<f64>().ok())
            .map(|((v, l), h)| v >= l && v <= h)
            .unwrap_or(false),
        OgcFilter::And(filters) => filters.iter().all(|f| evaluate_filter(f, props)),
        OgcFilter::Or(filters) => filters.iter().any(|f| evaluate_filter(f, props)),
        OgcFilter::Not(filter) => !evaluate_filter(filter, props),
    }
}

pub fn resolve_style(rules: &[ParsedRule], feature: &Feature, scale_denom: Option<f64>) -> Style {
    resolve_style_with_env(
        rules,
        feature,
        scale_denom,
        &std::collections::HashMap::new(),
    )
}

/// 带环境变量替换的样式解析
///
/// `env` 参数可用于 SLD 中的模板变量替换，
/// 例如 SLD 中使用 `${env('color')}` 语法，
/// 通过 WMS `ENV=color:'#FF0000'` 参数传入。
pub fn resolve_style_with_env(
    rules: &[ParsedRule],
    feature: &Feature,
    scale_denom: Option<f64>,
    env: &std::collections::HashMap<String, String>,
) -> Style {
    let props = &feature.properties;
    for rule in rules {
        if match_rule(rule, props, scale_denom) {
            let mut style = rule.style.clone();
            // 应用环境变量替换到颜色值
            apply_env_to_style(&mut style, env);
            // 解析标签文本: property → 要素属性值; literal → 原样
            if let Some(label) = style.label.as_mut() {
                if let Some(prop) = label.property.as_deref() {
                    if let Some(v) = props.get(prop) {
                        label.text = v.to_string();
                    } else {
                        label.text = String::new();
                    }
                }
            }
            return style;
        }
    }
    Style::default()
}

/// 将环境变量替换应用到样式颜色值中
fn apply_env_to_style(
    style: &mut super::rendering::Style,
    env: &std::collections::HashMap<String, String>,
) {
    // 遍历环境变量，将匹配的 hex 颜色替换到样式中
    for val in env.values() {
        if val.starts_with('#') && val.len() == 7 {
            // 验证是否为有效 hex 颜色
            let _valid = val.len() == 7 && val[1..].chars().all(|c| c.is_ascii_hexdigit());

            if let Some(ref mut fill) = style.fill {
                fill.color = val.clone();
            }
            if let Some(ref mut stroke) = style.stroke {
                stroke.color = val.clone();
            }
        }
    }
}

pub struct BuiltinStyle {
    pub name: &'static str,
    pub title: &'static str,
    pub sld: &'static str,
}

pub fn builtin_styles() -> Vec<BuiltinStyle> {
    vec![
        BuiltinStyle {
            name: "default",
            title: "默认样式",
            sld: r#"<?xml version="1.0" encoding="UTF-8"?>
<StyledLayerDescriptor version="1.0.0" xmlns="http://www.opengis.net/sld">
  <NamedLayer><Name>default</Name><UserStyle><FeatureTypeStyle><Rule>
    <PolygonSymbolizer><Fill><CssParameter name="fill">#6688aa</CssParameter><CssParameter name="fill-opacity">0.6</CssParameter></Fill><Stroke><CssParameter name="stroke">#334455</CssParameter><CssParameter name="stroke-width">1</CssParameter></Stroke></PolygonSymbolizer>
    <LineSymbolizer><Stroke><CssParameter name="stroke">#334455</CssParameter><CssParameter name="stroke-width">1</CssParameter></Stroke></LineSymbolizer>
    <PointSymbolizer><Graphic><Mark><WellKnownName>circle</WellKnownName><Fill><CssParameter name="fill">#6688aa</CssParameter></Fill><Stroke><CssParameter name="stroke">#334455</CssParameter><CssParameter name="stroke-width">1</CssParameter></Stroke></Mark><Size>8</Size></Graphic></PointSymbolizer>
  </Rule></FeatureTypeStyle></UserStyle></NamedLayer>
</StyledLayerDescriptor>"#,
        },
        BuiltinStyle {
            name: "red-polygon",
            title: "红色面",
            sld: r#"<?xml version="1.0" encoding="UTF-8"?>
<StyledLayerDescriptor version="1.0.0" xmlns="http://www.opengis.net/sld">
  <NamedLayer><Name>red-polygon</Name><UserStyle><FeatureTypeStyle><Rule>
    <PolygonSymbolizer><Fill><CssParameter name="fill">#e53935</CssParameter><CssParameter name="fill-opacity">0.5</CssParameter></Fill><Stroke><CssParameter name="stroke">#b71c1c</CssParameter><CssParameter name="stroke-width">1</CssParameter></Stroke></PolygonSymbolizer>
  </Rule></FeatureTypeStyle></UserStyle></NamedLayer>
</StyledLayerDescriptor>"#,
        },
        BuiltinStyle {
            name: "blue-line",
            title: "蓝色线",
            sld: r#"<?xml version="1.0" encoding="UTF-8"?>
<StyledLayerDescriptor version="1.0.0" xmlns="http://www.opengis.net/sld">
  <NamedLayer><Name>blue-line</Name><UserStyle><FeatureTypeStyle><Rule>
    <LineSymbolizer><Stroke><CssParameter name="stroke">#1e88e5</CssParameter><CssParameter name="stroke-width">2</CssParameter></Stroke></LineSymbolizer>
  </Rule></FeatureTypeStyle></UserStyle></NamedLayer>
</StyledLayerDescriptor>"#,
        },
        BuiltinStyle {
            name: "green-point",
            title: "绿色点",
            sld: r#"<?xml version="1.0" encoding="UTF-8"?>
<StyledLayerDescriptor version="1.0.0" xmlns="http://www.opengis.net/sld">
  <NamedLayer><Name>green-point</Name><UserStyle><FeatureTypeStyle><Rule>
    <PointSymbolizer><Graphic><Mark><WellKnownName>circle</WellKnownName><Fill><CssParameter name="fill">#43a047</CssParameter></Fill><Stroke><CssParameter name="stroke">#2e7d32</CssParameter><CssParameter name="stroke-width">1</CssParameter></Stroke></Mark><Size>10</Size></Graphic></PointSymbolizer>
  </Rule></FeatureTypeStyle></UserStyle></NamedLayer>
</StyledLayerDescriptor>"#,
        },
        BuiltinStyle {
            name: "water",
            title: "水域",
            sld: r#"<?xml version="1.0" encoding="UTF-8"?>
<StyledLayerDescriptor version="1.0.0" xmlns="http://www.opengis.net/sld">
  <NamedLayer><Name>water</Name><UserStyle><FeatureTypeStyle><Rule>
    <PolygonSymbolizer><Fill><CssParameter name="fill">#bbdefb</CssParameter><CssParameter name="fill-opacity">0.7</CssParameter></Fill><Stroke><CssParameter name="stroke">#1976d2</CssParameter><CssParameter name="stroke-width">0.5</CssParameter></Stroke></PolygonSymbolizer>
  </Rule></FeatureTypeStyle></UserStyle></NamedLayer>
</StyledLayerDescriptor>"#,
        },
        BuiltinStyle {
            name: "roads",
            title: "道路",
            sld: r#"<?xml version="1.0" encoding="UTF-8"?>
<StyledLayerDescriptor version="1.0.0" xmlns="http://www.opengis.net/sld">
  <NamedLayer><Name>roads</Name><UserStyle><FeatureTypeStyle><Rule>
    <LineSymbolizer><Stroke><CssParameter name="stroke">#ff8f00</CssParameter><CssParameter name="stroke-width">1.5</CssParameter></Stroke></LineSymbolizer>
  </Rule></FeatureTypeStyle></UserStyle></NamedLayer>
</StyledLayerDescriptor>"#,
        },
        BuiltinStyle {
            name: "buildings",
            title: "建筑",
            sld: r#"<?xml version="1.0" encoding="UTF-8"?>
<StyledLayerDescriptor version="1.0.0" xmlns="http://www.opengis.net/sld">
  <NamedLayer><Name>buildings</Name><UserStyle><FeatureTypeStyle><Rule>
    <PolygonSymbolizer><Fill><CssParameter name="fill">#eeeeee</CssParameter><CssParameter name="fill-opacity">1</CssParameter></Fill><Stroke><CssParameter name="stroke">#424242</CssParameter><CssParameter name="stroke-width">1</CssParameter></Stroke></PolygonSymbolizer>
  </Rule></FeatureTypeStyle></UserStyle></NamedLayer>
</StyledLayerDescriptor>"#,
        },
        BuiltinStyle {
            name: "land-use",
            title: "土地利用",
            sld: r#"<?xml version="1.0" encoding="UTF-8"?>
<StyledLayerDescriptor version="1.0.0" xmlns="http://www.opengis.net/sld">
  <NamedLayer><Name>land-use</Name><UserStyle><FeatureTypeStyle><Rule>
    <PolygonSymbolizer><Fill><CssParameter name="fill">#a5d6a7</CssParameter><CssParameter name="fill-opacity">0.5</CssParameter></Fill><Stroke><CssParameter name="stroke">#388e3c</CssParameter><CssParameter name="stroke-width">0.8</CssParameter></Stroke></PolygonSymbolizer>
  </Rule></FeatureTypeStyle></UserStyle></NamedLayer>
</StyledLayerDescriptor>"#,
        },
        BuiltinStyle {
            name: "orange-point",
            title: "橙色点",
            sld: r#"<?xml version="1.0" encoding="UTF-8"?>
<StyledLayerDescriptor version="1.0.0" xmlns="http://www.opengis.net/sld">
  <NamedLayer><Name>orange-point</Name><UserStyle><FeatureTypeStyle><Rule>
    <PointSymbolizer><Graphic><Mark><WellKnownName>square</WellKnownName><Fill><CssParameter name="fill">#ff9800</CssParameter></Fill><Stroke><CssParameter name="stroke">#e65100</CssParameter><CssParameter name="stroke-width">1</CssParameter></Stroke></Mark><Size>8</Size></Graphic></PointSymbolizer>
  </Rule></FeatureTypeStyle></UserStyle></NamedLayer>
</StyledLayerDescriptor>"#,
        },
        BuiltinStyle {
            name: "purple-polygon",
            title: "紫色面",
            sld: r#"<?xml version="1.0" encoding="UTF-8"?>
<StyledLayerDescriptor version="1.0.0" xmlns="http://www.opengis.net/sld">
  <NamedLayer><Name>purple-polygon</Name><UserStyle><FeatureTypeStyle><Rule>
    <PolygonSymbolizer><Fill><CssParameter name="fill">#ce93d8</CssParameter><CssParameter name="fill-opacity">0.5</CssParameter></Fill><Stroke><CssParameter name="stroke">#8e24aa</CssParameter><CssParameter name="stroke-width">1</CssParameter></Stroke></PolygonSymbolizer>
  </Rule></FeatureTypeStyle></UserStyle></NamedLayer>
</StyledLayerDescriptor>"#,
        },
    ]
}

pub fn default_sld(layer_name: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<StyledLayerDescriptor version="1.0.0"
  xmlns="http://www.opengis.net/sld"
  xmlns:ogc="http://www.opengis.net/ogc"
  xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
  <NamedLayer>
    <Name>{name}</Name>
    <UserStyle>
      <FeatureTypeStyle>
        <Rule>
          <PolygonSymbolizer>
            <Fill>
              <CssParameter name="fill">#6688aa</CssParameter>
              <CssParameter name="fill-opacity">0.6</CssParameter>
            </Fill>
            <Stroke>
              <CssParameter name="stroke">#334455</CssParameter>
              <CssParameter name="stroke-width">1</CssParameter>
            </Stroke>
          </PolygonSymbolizer>
          <LineSymbolizer>
            <Stroke>
              <CssParameter name="stroke">#334455</CssParameter>
              <CssParameter name="stroke-width">1</CssParameter>
            </Stroke>
          </LineSymbolizer>
          <PointSymbolizer>
            <Graphic>
              <Mark>
                <WellKnownName>circle</WellKnownName>
                <Fill>
                  <CssParameter name="fill">#6688aa</CssParameter>
                </Fill>
                <Stroke>
                  <CssParameter name="stroke">#334455</CssParameter>
                  <CssParameter name="stroke-width">1</CssParameter>
                </Stroke>
              </Mark>
              <Size>8</Size>
            </Graphic>
          </PointSymbolizer>
        </Rule>
      </FeatureTypeStyle>
    </UserStyle>
  </NamedLayer>
</StyledLayerDescriptor>"#,
        name = layer_name
    )
}

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

#[cfg(test)]
mod tests {
    use super::*;

    fn feature_with(name: &str, pop: &str) -> Feature {
        let mut props = HashMap::new();
        props.insert("name".to_string(), PropertyValue::String(name.to_string()));
        props.insert(
            "population".to_string(),
            PropertyValue::String(pop.to_string()),
        );
        Feature::new(
            crate::models::GeoJsonGeometry::Point {
                coordinates: vec![0.0, 0.0],
            },
            props,
        )
    }

    #[test]
    fn test_parse_text_symbolizer_property_label() {
        let sld = r#"<?xml version="1.0" encoding="UTF-8"?>
<StyledLayerDescriptor version="1.0.0" xmlns="http://www.opengis.net/sld">
  <NamedLayer><Name>cities</Name><UserStyle><FeatureTypeStyle><Rule>
    <TextSymbolizer>
      <Label><ogc:PropertyName>name</ogc:PropertyName></Label>
      <Font>
        <CssParameter name="font-family">Arial</CssParameter>
        <CssParameter name="font-size">14</CssParameter>
      </Font>
      <Fill><CssParameter name="fill">#FF0000</CssParameter></Fill>
      <Halo>
        <Radius>3</Radius>
        <Fill><CssParameter name="fill">#FFFFFF</CssParameter></Fill>
      </Halo>
    </TextSymbolizer>
  </Rule></FeatureTypeStyle></UserStyle></NamedLayer>
</StyledLayerDescriptor>"#;
        let rules = parse_sld(sld);
        assert_eq!(rules.len(), 1);
        let label = rules[0].style.label.as_ref().expect("label parsed");
        assert_eq!(label.property.as_deref(), Some("name"));
        assert_eq!(label.font_size, 14.0);
        assert_eq!(label.color, "#FF0000");
        assert_eq!(label.halo_color.as_deref(), Some("#FFFFFF"));
        assert_eq!(label.halo_radius, 3.0);
    }

    #[test]
    fn test_parse_text_symbolizer_literal_label() {
        let sld = r#"<?xml version="1.0" encoding="UTF-8"?>
<StyledLayerDescriptor version="1.0.0" xmlns="http://www.opengis.net/sld">
  <NamedLayer><Name>cities</Name><UserStyle><FeatureTypeStyle><Rule>
    <TextSymbolizer>
      <Label>Hello</Label>
    </TextSymbolizer>
  </Rule></FeatureTypeStyle></UserStyle></NamedLayer>
</StyledLayerDescriptor>"#;
        let rules = parse_sld(sld);
        let label = rules[0].style.label.as_ref().expect("label parsed");
        assert_eq!(label.text, "Hello");
        assert!(label.property.is_none());
    }

    #[test]
    fn test_resolve_label_from_feature_property() {
        let sld = r#"<?xml version="1.0" encoding="UTF-8"?>
<StyledLayerDescriptor version="1.0.0" xmlns="http://www.opengis.net/sld">
  <NamedLayer><Name>cities</Name><UserStyle><FeatureTypeStyle><Rule>
    <TextSymbolizer>
      <Label><ogc:PropertyName>name</ogc:PropertyName></Label>
      <Font><CssParameter name="font-size">12</CssParameter></Font>
    </TextSymbolizer>
  </Rule></FeatureTypeStyle></UserStyle></NamedLayer>
</StyledLayerDescriptor>"#;
        let rules = parse_sld(sld);
        let feature = feature_with("Beijing", "1000");
        let style = resolve_style(&rules, &feature, None);
        let label = style.label.expect("label resolved");
        assert_eq!(label.text, "Beijing");
    }

    #[test]
    fn test_parse_z_index_vendor_option() {
        let sld = r#"<?xml version="1.0" encoding="UTF-8"?>
<StyledLayerDescriptor version="1.0.0" xmlns="http://www.opengis.net/sld">
  <NamedLayer><Name>x</Name><UserStyle><FeatureTypeStyle><Rule>
    <VendorOption name="z-index">7</VendorOption>
    <PolygonSymbolizer><Fill><CssParameter name="fill">#112233</CssParameter></Fill></PolygonSymbolizer>
  </Rule></FeatureTypeStyle></UserStyle></NamedLayer>
</StyledLayerDescriptor>"#;
        let rules = parse_sld(sld);
        assert_eq!(rules[0].style.z_index, 7);
    }

    #[test]
    fn test_parse_composite_vendor_option() {
        let sld = r#"<?xml version="1.0" encoding="UTF-8"?>
<StyledLayerDescriptor version="1.0.0" xmlns="http://www.opengis.net/sld">
  <NamedLayer><Name>x</Name><UserStyle><FeatureTypeStyle><Rule>
    <VendorOption name="composite">multiply</VendorOption>
    <PolygonSymbolizer><Fill><CssParameter name="fill">#112233</CssParameter></Fill></PolygonSymbolizer>
  </Rule></FeatureTypeStyle></UserStyle></NamedLayer>
</StyledLayerDescriptor>"#;
        let rules = parse_sld(sld);
        assert_eq!(
            rules[0].style.composite,
            crate::utils::rendering::CompositeOp::Multiply
        );
    }

    #[test]
    fn test_rule_filters_and_scale() {
        let sld = r#"<?xml version="1.0" encoding="UTF-8"?>
<StyledLayerDescriptor version="1.0.0" xmlns="http://www.opengis.net/sld">
  <NamedLayer><Name>x</Name><UserStyle><FeatureTypeStyle>
    <Rule>
      <MinScaleDenominator>1000</MinScaleDenominator>
      <MaxScaleDenominator>50000</MaxScaleDenominator>
      <ogc:Filter><ogc:PropertyIsEqualTo><ogc:PropertyName>type</ogc:PropertyName><ogc:Literal>city</ogc:Literal></ogc:PropertyIsEqualTo></ogc:Filter>
      <PolygonSymbolizer><Fill><CssParameter name="fill">#112233</CssParameter></Fill></PolygonSymbolizer>
    </Rule>
  </FeatureTypeStyle></UserStyle></NamedLayer>
</StyledLayerDescriptor>"#;
        let rules = parse_sld(sld);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].min_scale, Some(1000.0));
        assert_eq!(rules[0].max_scale, Some(50000.0));
        assert!(!rules[0].filters.is_empty());
    }
}
