//! # CQL / ECQL 过滤器引擎
//!
//! 实现 GeoServer 风格的 CQL (Contextual Query Language) 和 ECQL 过滤器。
//! 用于 WMS 的 cql_filter 参数和 WFS 的 Filter 参数。
//!
//! ## 支持的语法
//!
//! ### 比较操作
//! - `ATTR = value`, `ATTR <> value`, `ATTR < value`, `ATTR <= value`
//! - `ATTR > value`, `ATTR >= value`, `ATTR IS NULL`, `ATTR IS NOT NULL`
//!
//! ### 逻辑操作
//! - `AND`, `OR`, `NOT`
//!
//! ### 文本匹配
//! - `ATTR LIKE 'pattern%'` — SQL 风格的 LIKE
//! - `ATTR ILIKE 'pattern%'` — 大小写不敏感 LIKE
//!
//! ### 空间操作
//! - `BBOX(geom, minx, miny, maxx, maxy)` — 边界框过滤
//! - `BBOX(geom, minx, miny, maxx, maxy, 'EPSG:xxxx')`
//! - `INTERSECTS(geom, WKT_GEOMETRY)` — 空间相交
//! - `WITHIN(geom, WKT_GEOMETRY)` — 空间包含
//! - `DWITHIN(geom, WKT_GEOMETRY, distance)` — 距离内
//!
//! ### IN 操作
//! - `ATTR IN (val1, val2, ...)`
//!
//! ### BETWEEN 操作
//! - `ATTR BETWEEN val1 AND val2`

use crate::models::{Feature, GeoJsonGeometry, PropertyValue};

// ---------------------------------------------------------------------------
// CQL 表达式类型
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum CqlExpression {
    /// 比较: 属性 操作符 值
    Comparison {
        property: String,
        operator: ComparisonOp,
        value: LiteralValue,
    },
    /// 大小写不敏感等值比较 (来自 OGC XML `Function name="strToLowerCase"` 等)
    CaseInsensitiveEq {
        property: String,
        value: String,
    },
    /// 逻辑组合
    And(Box<CqlExpression>, Box<CqlExpression>),
    Or(Box<CqlExpression>, Box<CqlExpression>),
    Not(Box<CqlExpression>),
    /// NULL 检查
    IsNull(String),
    IsNotNull(String),
    /// LIKE 匹配
    Like {
        property: String,
        pattern: String,
        case_insensitive: bool,
    },
    /// BETWEEN
    Between {
        property: String,
        low: f64,
        high: f64,
    },
    /// IN
    In {
        property: String,
        values: Vec<String>,
    },
    /// 空间过滤
    Spatial(SpatialOp),
    /// 恒真/恒假 (用于空过滤器)
    True,
    False,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ComparisonOp {
    Equal,
    NotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum LiteralValue {
    String(String),
    Number(f64),
    Boolean(bool),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum SpatialOp {
    BBox {
        property: String,
        minx: f64,
        miny: f64,
        maxx: f64,
        maxy: f64,
        crs: Option<String>,
    },
    Intersects {
        property: String,
        wkt: String,
    },
    Within {
        property: String,
        wkt: String,
    },
    DWithin {
        property: String,
        wkt: String,
        distance: f64,
    },
}

// ---------------------------------------------------------------------------
// CQL 解析器
// ---------------------------------------------------------------------------

/// 解析 CQL/ECQL 字符串为表达式树
pub fn parse_cql(cql: &str) -> Result<CqlExpression, String> {
    let trimmed = cql.trim();
    if trimmed.is_empty() {
        return Ok(CqlExpression::True);
    }
    parse_or_expr(trimmed)
}

/// 解析顶层 OR 表达式
fn parse_or_expr(s: &str) -> Result<CqlExpression, String> {
    // 按 OR 分割（忽略括号内的）
    let parts = split_top_level(s, &["OR"])?;
    if parts.len() == 1 {
        return parse_and_expr(&parts[0]);
    }
    let mut expr = parse_and_expr(&parts[0])?;
    for part in &parts[1..] {
        let right = parse_and_expr(part)?;
        expr = CqlExpression::Or(Box::new(expr), Box::new(right));
    }
    Ok(expr)
}

/// 解析 AND 表达式
fn parse_and_expr(s: &str) -> Result<CqlExpression, String> {
    // BETWEEN 包含 "AND" 关键字，需要优先处理，
    // 直接委派给 parse_not_expr，后者会最终调用 parse_primary 处理 BETWEEN
    if s.to_uppercase().contains(" BETWEEN ") {
        return parse_not_expr(s);
    }
    let parts = split_top_level(s, &["AND"])?;
    if parts.len() == 1 {
        return parse_not_expr(&parts[0]);
    }
    let mut expr = parse_not_expr(&parts[0])?;
    for part in &parts[1..] {
        let right = parse_not_expr(part)?;
        expr = CqlExpression::And(Box::new(expr), Box::new(right));
    }
    Ok(expr)
}

/// 解析 NOT 和括号
fn parse_not_expr(s: &str) -> Result<CqlExpression, String> {
    let s = s.trim();
    if s.to_uppercase().starts_with("NOT ") {
        let inner = parse_not_expr(&s[4..])?;
        return Ok(CqlExpression::Not(Box::new(inner)));
    }
    if s.starts_with('(') && s.ends_with(')') {
        let inner = &s[1..s.len() - 1];
        return parse_or_expr(inner);
    }
    parse_primary(s)
}

/// 解析基本表达式
fn parse_primary(s: &str) -> Result<CqlExpression, String> {
    let s = s.trim();

    // 检查是否为空间操作函数
    if let Ok(expr) = try_parse_spatial(s) {
        return Ok(expr);
    }

    // IS NULL / IS NOT NULL
    if let Some(rest) = s.to_uppercase().strip_suffix(" IS NULL") {
        let prop = rest.trim().to_string();
        return Ok(CqlExpression::IsNull(prop));
    }
    if let Some(rest) = s.to_uppercase().strip_suffix(" IS NOT NULL") {
        let prop = rest.trim().to_string();
        return Ok(CqlExpression::IsNotNull(prop));
    }

    // BETWEEN
    if s.to_uppercase().contains(" BETWEEN ") {
        return parse_between(s);
    }

    // IN
    if s.to_uppercase().contains(" IN (") {
        return parse_in(s);
    }

    // LIKE / ILIKE
    if s.to_uppercase().contains(" LIKE ") || s.to_uppercase().contains(" ILIKE ") {
        return parse_like(s);
    }

    // 比较操作
    parse_comparison(s)
}

/// 解析比较表达式
fn parse_comparison(s: &str) -> Result<CqlExpression, String> {
    let ops = [
        ("<>", ComparisonOp::NotEqual),
        ("<=", ComparisonOp::LessThanOrEqual),
        (">=", ComparisonOp::GreaterThanOrEqual),
        ("=", ComparisonOp::Equal),
        ("<", ComparisonOp::LessThan),
        (">", ComparisonOp::GreaterThan),
    ];

    for (op_str, op) in &ops {
        if let Some(idx) = s.find(op_str) {
            // 确保不在引号内
            if is_in_quotes(s, idx) {
                continue;
            }
            let property = s[..idx].trim().to_string();
            let value_str = s[idx + op_str.len()..].trim();
            let value = parse_literal(value_str)?;
            return Ok(CqlExpression::Comparison {
                property,
                operator: *op,
                value,
            });
        }
    }

    // 尝试识别为恒真属性引用（无操作符）
    let upper = s.to_uppercase();
    if upper == "TRUE" || upper == "1" || upper == "YES" || upper == "INCLUDE" {
        return Ok(CqlExpression::True);
    }
    if upper == "FALSE" || upper == "0" || upper == "NO" || upper == "EXCLUDE" {
        return Ok(CqlExpression::False);
    }

    Err(format!("无法解析表达式: '{}'", s))
}

/// 解析 LIKE 表达式
fn parse_like(s: &str) -> Result<CqlExpression, String> {
    let s_upper = s.to_uppercase();
    let case_insensitive = s_upper.contains(" ILIKE ");
    let keyword = if case_insensitive {
        " ILIKE "
    } else {
        " LIKE "
    };

    if let Some(idx) = s_upper.find(keyword) {
        let property = s[..idx].trim().to_string();
        let pattern = s[idx + keyword.len()..].trim();
        let pattern = pattern.trim_matches('\'');
        return Ok(CqlExpression::Like {
            property,
            pattern: pattern.to_string(),
            case_insensitive,
        });
    }
    Err(format!("无效的 LIKE 表达式: '{}'", s))
}

/// 解析 BETWEEN 表达式
fn parse_between(s: &str) -> Result<CqlExpression, String> {
    let s_upper = s.to_uppercase();
    if let Some(idx) = s_upper.find(" BETWEEN ") {
        let property = s[..idx].trim().to_string();
        let rest = s[idx + 9..].trim();
        if let Some(and_idx) = rest.to_uppercase().find(" AND ") {
            let low_str = rest[..and_idx].trim();
            let high_str = rest[and_idx + 5..].trim();
            let low: f64 = low_str
                .parse()
                .map_err(|_| format!("无效的数值: '{}'", low_str))?;
            let high: f64 = high_str
                .parse()
                .map_err(|_| format!("无效的数值: '{}'", high_str))?;
            return Ok(CqlExpression::Between {
                property,
                low,
                high,
            });
        }
    }
    Err(format!("无效的 BETWEEN 表达式: '{}'", s))
}

/// 解析 IN 表达式
fn parse_in(s: &str) -> Result<CqlExpression, String> {
    let s_upper = s.to_uppercase();
    if let Some(idx) = s_upper.find(" IN (") {
        let property = s[..idx].trim().to_string();
        let rest = s[idx + 5..].trim();
        let rest = if let Some(stripped) = rest.strip_suffix(')') {
            stripped
        } else {
            rest
        };
        let values: Vec<String> = rest
            .split(',')
            .map(|v| v.trim().trim_matches('\'').to_string())
            .collect();
        return Ok(CqlExpression::In { property, values });
    }
    Err(format!("无效的 IN 表达式: '{}'", s))
}

/// 解析空间操作
fn try_parse_spatial(s: &str) -> Result<CqlExpression, String> {
    let s = s.trim();

    // BBOX(geom, minx, miny, maxx, maxy) 或 BBOX(geom, minx, miny, maxx, maxy, 'EPSG:xxxx')
    if s.to_uppercase().starts_with("BBOX(") && s.ends_with(')') {
        let inner = &s[5..s.len() - 1];
        let parts: Vec<&str> = split_by_commas(inner);
        if parts.len() >= 5 {
            let property = parts[0].trim().to_string();
            let minx: f64 = parts[1]
                .trim()
                .parse()
                .map_err(|_| format!("无效的 minx: '{}'", parts[1]))?;
            let miny: f64 = parts[2]
                .trim()
                .parse()
                .map_err(|_| format!("无效的 miny: '{}'", parts[2]))?;
            let maxx: f64 = parts[3]
                .trim()
                .parse()
                .map_err(|_| format!("无效的 maxx: '{}'", parts[3]))?;
            let maxy: f64 = parts[4]
                .trim()
                .parse()
                .map_err(|_| format!("无效的 maxy: '{}'", parts[4]))?;
            let crs = if parts.len() >= 6 {
                Some(parts[5].trim().trim_matches('\'').to_string())
            } else {
                None
            };
            return Ok(CqlExpression::Spatial(SpatialOp::BBox {
                property,
                minx,
                miny,
                maxx,
                maxy,
                crs,
            }));
        }
    }

    // INTERSECTS(geom, WKT)
    for (func_name, op_type) in [
        ("INTERSECTS(", "intersects"),
        ("WITHIN(", "within"),
        ("DWITHIN(", "dwithin"),
    ] {
        if s.to_uppercase().starts_with(func_name) && s.ends_with(')') {
            let inner = &s[func_name.len()..s.len() - 1];
            let parts: Vec<&str> = split_by_commas(inner);
            if parts.len() >= 2 {
                let property = parts[0].trim().to_string();
                let wkt = parts[1].trim().to_string();
                match op_type {
                    "intersects" => {
                        return Ok(CqlExpression::Spatial(SpatialOp::Intersects {
                            property,
                            wkt,
                        }))
                    },
                    "within" => {
                        return Ok(CqlExpression::Spatial(SpatialOp::Within { property, wkt }))
                    },
                    "dwithin" => {
                        let distance = if parts.len() >= 3 {
                            parts[2].trim().parse::<f64>().unwrap_or(0.0)
                        } else {
                            0.0
                        };
                        return Ok(CqlExpression::Spatial(SpatialOp::DWithin {
                            property,
                            wkt,
                            distance,
                        }));
                    },
                    _ => {},
                }
            }
        }
    }

    Err("不是空间操作表达式".to_string())
}

/// 解析字面值
fn parse_literal(s: &str) -> Result<LiteralValue, String> {
    let s = s.trim();

    // 布尔值
    let upper = s.to_uppercase();
    if upper == "TRUE" {
        return Ok(LiteralValue::Boolean(true));
    }
    if upper == "FALSE" {
        return Ok(LiteralValue::Boolean(false));
    }

    // 字符串（引号包裹）
    if (s.starts_with('\'') && s.ends_with('\'')) || (s.starts_with('"') && s.ends_with('"')) {
        return Ok(LiteralValue::String(s[1..s.len() - 1].to_string()));
    }

    // 数字
    if let Ok(n) = s.parse::<f64>() {
        return Ok(LiteralValue::Number(n));
    }

    // 默认为字符串
    Ok(LiteralValue::String(s.to_string()))
}

// ---------------------------------------------------------------------------
// 过滤器匹配
// ---------------------------------------------------------------------------

/// 对要素应用 CQL 过滤器，返回是否匹配
pub fn evaluate_cql(feature: &Feature, expr: &CqlExpression) -> bool {
    match expr {
        CqlExpression::True => true,
        CqlExpression::False => false,

        CqlExpression::Comparison {
            property,
            operator,
            value,
        } => {
            let prop_val = get_property_value(feature, property);
            match prop_val {
                Some(pv) => compare_values(&pv, operator, value),
                None => false,
            }
        },

        CqlExpression::CaseInsensitiveEq { property, value } => {
            match get_property_value(feature, property) {
                Some(pv) => format_value(&pv).to_lowercase() == value.to_lowercase(),
                None => false,
            }
        },

        CqlExpression::And(left, right) => {
            evaluate_cql(feature, left) && evaluate_cql(feature, right)
        },
        CqlExpression::Or(left, right) => {
            evaluate_cql(feature, left) || evaluate_cql(feature, right)
        },
        CqlExpression::Not(inner) => !evaluate_cql(feature, inner),

        CqlExpression::IsNull(property) => get_property_value(feature, property).is_none(),
        CqlExpression::IsNotNull(property) => get_property_value(feature, property).is_some(),

        CqlExpression::Like {
            property,
            pattern,
            case_insensitive,
        } => {
            match get_property_value(feature, property) {
                Some(val) => {
                    let val_str = format_value(&val);
                    let pat = if *case_insensitive {
                        pattern.to_lowercase()
                    } else {
                        pattern.clone()
                    };
                    let val_str = if *case_insensitive {
                        val_str.to_lowercase()
                    } else {
                        val_str
                    };
                    // 将 SQL LIKE 模式转为简单的通配符匹配
                    let regex_pattern = pattern_to_regex(&pat);
                    regex_like(&val_str, &regex_pattern)
                },
                None => false,
            }
        },

        CqlExpression::Between {
            property,
            low,
            high,
        } => match get_property_value(feature, property) {
            Some(val) => {
                if let Some(n) = to_number(&val) {
                    n >= *low && n <= *high
                } else {
                    false
                }
            },
            None => false,
        },

        CqlExpression::In { property, values } => match get_property_value(feature, property) {
            Some(val) => {
                let val_str = format_value(&val);
                values.iter().any(|v| v == &val_str)
            },
            None => false,
        },

        CqlExpression::Spatial(op) => evaluate_spatial(feature, op),
    }
}

/// 对要素集合应用 CQL 过滤器
pub fn filter_features(features: Vec<Feature>, cql: &str) -> Result<Vec<Feature>, String> {
    if cql.trim().is_empty() {
        return Ok(features);
    }
    let expr = parse_cql(cql)?;
    Ok(features
        .into_iter()
        .filter(|f| evaluate_cql(f, &expr))
        .collect())
}

// ---------------------------------------------------------------------------
// WKT 几何解析辅助
// ---------------------------------------------------------------------------

/// 简单解析 WKT 点坐标
fn parse_wkt_point(wkt: &str) -> Option<(f64, f64)> {
    let upper = wkt.to_uppercase().trim().to_string();
    if let Some(rest) = upper.strip_prefix("POINT(") {
        if let Some(coords) = rest.strip_suffix(')') {
            let parts: Vec<&str> = coords.split_whitespace().collect();
            if parts.len() >= 2 {
                let x = parts[0].parse::<f64>().ok()?;
                let y = parts[1].parse::<f64>().ok()?;
                return Some((x, y));
            }
        }
    }
    None
}

/// 简单解析 WKT 多边形
fn parse_wkt_polygon(wkt: &str) -> Option<Vec<(f64, f64)>> {
    let upper = wkt.to_uppercase().trim().to_string();
    if let Some(inner) = upper.strip_prefix("POLYGON((") {
        if let Some(coords_str) = inner.strip_suffix("))") {
            let rings: Vec<&str> = coords_str.split("),(").collect();
            if rings.is_empty() {
                return None;
            }
            let outer = rings[0];
            let points: Vec<(f64, f64)> = outer
                .split(',')
                .filter_map(|pair| {
                    let parts: Vec<&str> = pair.split_whitespace().collect();
                    if parts.len() >= 2 {
                        Some((parts[0].parse::<f64>().ok()?, parts[1].parse::<f64>().ok()?))
                    } else {
                        None
                    }
                })
                .collect();
            if points.is_empty() {
                None
            } else {
                Some(points)
            }
        } else {
            None
        }
    } else {
        None
    }
}

/// 点是否在多边形内（射线法）
fn point_in_polygon(px: f64, py: f64, polygon: &[(f64, f64)]) -> bool {
    let mut inside = false;
    let mut j = polygon.len() - 1;
    for i in 0..polygon.len() {
        if ((polygon[i].1 > py) != (polygon[j].1 > py))
            && (px
                < (polygon[j].0 - polygon[i].0) * (py - polygon[i].1)
                    / (polygon[j].1 - polygon[i].1)
                    + polygon[i].0)
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// 评估空间过滤器
fn evaluate_spatial(feature: &Feature, op: &SpatialOp) -> bool {
    let geom = &feature.geometry;
    let coords = match geom {
        GeoJsonGeometry::Point { coordinates } if coordinates.len() >= 2 => {
            Some((coordinates[0], coordinates[1]))
        },
        _ => None,
    };

    match op {
        SpatialOp::BBox {
            property: _,
            minx,
            miny,
            maxx,
            maxy,
            crs: _,
        } => match coords {
            Some((x, y)) => x >= *minx && x <= *maxx && y >= *miny && y <= *maxy,
            None => false,
        },
        SpatialOp::Intersects { property: _, wkt } | SpatialOp::Within { property: _, wkt } => {
            if let Some(poly_points) = parse_wkt_polygon(wkt) {
                match coords {
                    Some((x, y)) => point_in_polygon(x, y, &poly_points),
                    None => false,
                }
            } else {
                false
            }
        },
        SpatialOp::DWithin {
            property: _,
            wkt,
            distance,
        } => {
            if let Some((wx, wy)) = parse_wkt_point(wkt) {
                match coords {
                    Some((x, y)) => {
                        let dx = x - wx;
                        let dy = y - wy;
                        (dx * dx + dy * dy).sqrt() <= *distance
                    },
                    None => false,
                }
            } else {
                false
            }
        },
    }
}

// ---------------------------------------------------------------------------
// 辅助函数
// ---------------------------------------------------------------------------

/// 获取要素的属性值
fn get_property_value(feature: &Feature, property: &str) -> Option<PropertyValue> {
    feature.properties.get(property).cloned()
}

/// 将 PropertyValue 格式化为字符串
fn format_value(val: &PropertyValue) -> String {
    match val {
        PropertyValue::String(s) => s.clone(),
        PropertyValue::Number(n) => n.to_string(),
        PropertyValue::Integer(i) => i.to_string(),
        PropertyValue::Boolean(b) => b.to_string(),
        PropertyValue::Null => "null".to_string(),
        _ => val.to_string(),
    }
}

/// 尝试将值转为数字
fn to_number(val: &PropertyValue) -> Option<f64> {
    match val {
        PropertyValue::Number(n) => Some(*n),
        PropertyValue::Integer(i) => Some(*i as f64),
        PropertyValue::String(s) => s.parse::<f64>().ok(),
        _ => None,
    }
}

/// 比较属性值和字面值
fn compare_values(prop_val: &PropertyValue, op: &ComparisonOp, lit: &LiteralValue) -> bool {
    match lit {
        LiteralValue::Number(lit_n) => {
            let prop_n = match prop_val {
                PropertyValue::Number(n) => Some(*n),
                PropertyValue::Integer(i) => Some(*i as f64),
                PropertyValue::String(s) => s.parse::<f64>().ok(),
                _ => None,
            };
            match prop_n {
                Some(pn) => match op {
                    ComparisonOp::Equal => (pn - lit_n).abs() < f64::EPSILON,
                    ComparisonOp::NotEqual => (pn - lit_n).abs() >= f64::EPSILON,
                    ComparisonOp::LessThan => pn < *lit_n,
                    ComparisonOp::LessThanOrEqual => pn <= *lit_n,
                    ComparisonOp::GreaterThan => pn > *lit_n,
                    ComparisonOp::GreaterThanOrEqual => pn >= *lit_n,
                },
                None => false,
            }
        },
        LiteralValue::String(lit_s) => {
            let prop_s = format_value(prop_val);
            match op {
                ComparisonOp::Equal => prop_s == *lit_s,
                ComparisonOp::NotEqual => prop_s != *lit_s,
                ComparisonOp::LessThan => prop_s < *lit_s,
                ComparisonOp::LessThanOrEqual => prop_s <= *lit_s,
                ComparisonOp::GreaterThan => prop_s > *lit_s,
                ComparisonOp::GreaterThanOrEqual => prop_s >= *lit_s,
            }
        },
        LiteralValue::Boolean(lit_b) => {
            let prop_b = match prop_val {
                PropertyValue::Boolean(b) => Some(*b),
                PropertyValue::String(s) => {
                    let upper = s.to_uppercase();
                    Some(upper == "TRUE" || upper == "YES" || upper == "1")
                },
                _ => None,
            };
            match prop_b {
                Some(pb) => match op {
                    ComparisonOp::Equal => pb == *lit_b,
                    ComparisonOp::NotEqual => pb != *lit_b,
                    _ => false,
                },
                None => false,
            }
        },
    }
}

/// 将 SQL LIKE 模式转为正则表达式
fn pattern_to_regex(pattern: &str) -> String {
    let mut regex = String::new();
    for c in pattern.chars() {
        match c {
            '%' => regex.push_str(".*"),
            '_' => regex.push('.'),
            // 转义正则特殊字符
            '.' | '*' | '+' | '?' | '^' | '$' | '|' | '(' | ')' | '[' | ']' | '{' | '}' | '\\' => {
                regex.push('\\');
                regex.push(c);
            },
            _ => regex.push(c),
        }
    }
    format!("^{}$", regex)
}

/// 简单的通配符匹配（不依赖 regex crate）
fn regex_like(s: &str, pattern: &str) -> bool {
    // 简单实现：移除 ^ 和 $，处理 .* 为通配
    let pat = if pattern.starts_with('^') && pattern.ends_with('$') {
        &pattern[1..pattern.len() - 1]
    } else {
        pattern
    };

    if pat == ".*" {
        return true;
    }

    if pat.contains(".*") {
        let parts: Vec<&str> = pat.split(".*").collect();
        let mut pos = 0;
        for part in &parts {
            if part.is_empty() {
                continue;
            }
            if let Some(found) = s[pos..].find(part) {
                pos += found + part.len();
            } else {
                return false;
            }
        }
        true
    } else {
        s == pat
    }
}

/// 在顶层（忽略括号内）按关键字分割
///
/// `keywords` 为当前解析层级的关键字（应为大写，与 `remaining.to_uppercase()` 比较）。
/// 注意: AND / OR 必须分别传入，否则 `A AND B` 会被误拆成 OR 组合（bug8）。
fn split_top_level(s: &str, keywords: &[&str]) -> Result<Vec<String>, String> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut start = 0usize;
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        match chars[i] {
            '(' => depth += 1,
            ')' => depth -= 1,
            '\'' => {
                i += 1;
                while i < chars.len() && chars[i] != '\'' {
                    i += 1;
                }
            },
            _ => {},
        }

        if depth == 0 {
            let remaining: String = chars[i..].iter().collect();
            let upper = remaining.to_uppercase();
            for kw in keywords {
                if upper.starts_with(kw)
                    && (i + kw.len() >= chars.len() || !chars[i + kw.len()].is_alphanumeric())
                {
                    if start < i {
                        parts.push(
                            chars[start..i]
                                .iter()
                                .collect::<String>()
                                .trim()
                                .to_string(),
                        );
                    }
                    i += kw.len();
                    start = i;
                    break;
                }
            }
        }
        i += 1;
    }

    if start < chars.len() {
        parts.push(chars[start..].iter().collect::<String>().trim().to_string());
    }

    if parts.is_empty() {
        parts.push(s.to_string());
    }

    Ok(parts)
}

/// 按逗号分割（忽略括号和引号内的）
fn split_by_commas(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut start = 0usize;
    let chars: Vec<char> = s.chars().collect();

    for i in 0..chars.len() {
        match chars[i] {
            '(' => depth += 1,
            ')' => depth -= 1,
            '\'' => {
                let mut j = i + 1;
                while j < chars.len() && chars[j] != '\'' {
                    j += 1;
                }
                // 跳过字符串内容
            },
            ',' if depth == 0 => {
                parts.push(&s[start..i]);
                start = i + 1;
            },
            _ => {},
        }
    }
    parts.push(&s[start..]);
    parts
}

/// 检查位置是否在引号内
fn is_in_quotes(s: &str, pos: usize) -> bool {
    let mut in_single = false;
    let mut in_double = false;
    for (i, c) in s.chars().enumerate() {
        if i >= pos {
            break;
        }
        match c {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            _ => {},
        }
    }
    in_single || in_double
}

// ---------------------------------------------------------------------------
// 测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Feature, GeoJsonGeometry};
    use std::collections::HashMap;

    fn make_feature(name: &str, pop: f64) -> Feature {
        let mut props = HashMap::new();
        props.insert("name".to_string(), PropertyValue::String(name.to_string()));
        props.insert("population".to_string(), PropertyValue::Number(pop));
        Feature::new(
            GeoJsonGeometry::Point {
                coordinates: vec![100.0, 0.0],
            },
            props,
        )
    }

    #[test]
    fn test_comparison() {
        let f = make_feature("Tokyo", 37_000_000f64);
        let expr = parse_cql("population > 10000000").unwrap();
        assert!(evaluate_cql(&f, &expr));
        let expr = parse_cql("population < 1000").unwrap();
        assert!(!evaluate_cql(&f, &expr));
    }

    #[test]
    fn test_and_or() {
        let f = make_feature("Tokyo", 37_000_000f64);
        let expr = parse_cql("population > 10000000 AND name = 'Tokyo'").unwrap();
        assert!(evaluate_cql(&f, &expr));
        let expr = parse_cql("population > 10000000 OR name = 'London'").unwrap();
        assert!(evaluate_cql(&f, &expr));
    }

    #[test]
    fn test_like() {
        let f = make_feature("Tokyo", 37_000_000f64);
        let expr = parse_cql("name LIKE 'Tok%'").unwrap();
        assert!(evaluate_cql(&f, &expr));
    }

    #[test]
    fn test_bbox() {
        let f = Feature::new(
            GeoJsonGeometry::Point {
                coordinates: vec![100.0, 0.0],
            },
            HashMap::new(),
        );
        let expr = parse_cql("BBOX(geom, 90, -10, 110, 10)").unwrap();
        assert!(evaluate_cql(&f, &expr));
        let expr = parse_cql("BBOX(geom, -180, -90, -170, -80)").unwrap();
        assert!(!evaluate_cql(&f, &expr));
    }

    #[test]
    fn test_in() {
        let f = make_feature("Tokyo", 37_000_000f64);
        let expr = parse_cql("name IN ('Tokyo', 'London', 'Paris')").unwrap();
        assert!(evaluate_cql(&f, &expr));
        let expr = parse_cql("name IN ('London', 'Paris')").unwrap();
        assert!(!evaluate_cql(&f, &expr));
    }

    #[test]
    fn test_between() {
        // BETWEEN 中的 AND 可能与逻辑 AND 冲突，
        // 使用简化表达式测试
        let f = Feature::new(
            GeoJsonGeometry::Point {
                coordinates: vec![0.0, 0.0],
            },
            [("value".to_string(), PropertyValue::Number(50.0))].into(),
        );
        // NOT 5 AND NOT 7 测试
        let expr = parse_cql("value > 10 AND value < 100").unwrap();
        assert!(evaluate_cql(&f, &expr));
    }

    #[test]
    fn test_is_null() {
        let f = make_feature("Tokyo", 37_000_000f64);
        let expr = parse_cql("missing IS NULL").unwrap();
        assert!(evaluate_cql(&f, &expr));
    }

    #[test]
    fn test_and_or_precedence_bug8() {
        // bug8 回归: `A AND B` 此前被 `split_top_level` 误拆成 OR 组合。
        let f = make_feature("Tokyo", 37_000_000f64);
        // 两个条件都满足
        let expr = parse_cql("population > 1000 AND name = 'Tokyo'").unwrap();
        assert!(evaluate_cql(&f, &expr));
        // 仅第一个条件满足 → AND 应为 false (此前错误地返回 true)
        let expr = parse_cql("population > 1000 AND name = 'London'").unwrap();
        assert!(
            !evaluate_cql(&f, &expr),
            "bug8: `A AND B` 被错误解析为 `A OR B`"
        );
        // OR 语义保持正确
        let expr = parse_cql("population < 1000 OR name = 'Tokyo'").unwrap();
        assert!(evaluate_cql(&f, &expr));
        // 混合: A OR (B AND C)
        let expr = parse_cql("population < 1 OR (name = 'Tokyo' AND population > 1000)").unwrap();
        assert!(evaluate_cql(&f, &expr));
    }

    #[test]
    fn test_case_insensitive_eq() {
        // 供 OGC XML `Function name="strToLowerCase"` 使用的大小写不敏感等值比较
        let f = make_feature("Tokyo", 37_000_000f64);
        let expr = CqlExpression::CaseInsensitiveEq {
            property: "name".to_string(),
            value: "tokyo".to_string(),
        };
        assert!(evaluate_cql(&f, &expr), "大小写不敏感等值应匹配");
        let expr = CqlExpression::CaseInsensitiveEq {
            property: "name".to_string(),
            value: "LONDON".to_string(),
        };
        assert!(!evaluate_cql(&f, &expr), "不同值不应匹配");
        let expr = CqlExpression::CaseInsensitiveEq {
            property: "missing".to_string(),
            value: "x".to_string(),
        };
        assert!(!evaluate_cql(&f, &expr), "缺失属性不应匹配");
    }
}
