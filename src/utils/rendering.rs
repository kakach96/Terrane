use crate::models::{Bounds, Feature, GeoJsonGeometry};
use geo_types::{Geometry, LineString, Point, Polygon};
use image::codecs::png::PngEncoder;
use image::ImageEncoder;
use image::{ColorType, Rgba, RgbaImage};

#[derive(Debug, Clone, Copy)]
pub struct RenderOptions {
    pub width: u32,
    pub height: u32,
    pub transparent: bool,
    pub bg_color: Option<[u8; 4]>,
    #[allow(dead_code)]
    pub format: RenderFormat,
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum RenderFormat {
    PNG,
    JPEG,
    GIF,
}

impl Default for RenderOptions {
    fn default() -> Self {
        RenderOptions {
            width: 512,
            height: 512,
            transparent: true,
            bg_color: None,
            format: RenderFormat::PNG,
        }
    }
}

pub struct MapRenderer {
    options: RenderOptions,
    bounds: Bounds,
}

impl MapRenderer {
    pub fn new(options: RenderOptions, bounds: Bounds) -> Self {
        MapRenderer { options, bounds }
    }

    pub fn render(&self, features: Vec<(GeoJsonGeometry, Style)>) -> RgbaImage {
        let mut img = RgbaImage::new(self.options.width, self.options.height);

        if !self.options.transparent {
            let bg = self.options.bg_color.unwrap_or([255, 255, 255, 255]);
            for pixel in img.pixels_mut() {
                *pixel = Rgba(bg);
            }
        }

        if features.is_empty() && self.options.transparent {
            let bg = [230, 230, 230, 255];
            for pixel in img.pixels_mut() {
                *pixel = Rgba(bg);
            }
            self.draw_rect(
                &mut img,
                0,
                0,
                self.options.width - 1,
                self.options.height - 1,
                [200, 200, 200, 255],
                2,
            );
        }

        for (geometry, style) in features {
            self.render_feature(&mut img, &geometry, &style);
        }

        img
    }

    fn draw_rect(
        &self,
        img: &mut RgbaImage,
        x1: u32,
        y1: u32,
        x2: u32,
        y2: u32,
        color: [u8; 4],
        width: u32,
    ) {
        let min_x = x1.min(x2);
        let max_x = x1.max(x2);
        let min_y = y1.min(y2);
        let max_y = y1.max(y2);

        for x in min_x..=max_x {
            for w in 0..width {
                if y1 < self.options.height && x + w < self.options.width {
                    img.put_pixel(x + w, y1, Rgba(color));
                }
                if y2 < self.options.height && x + w < self.options.width {
                    img.put_pixel(x + w, max_y, Rgba(color));
                }
            }
        }
        for y in min_y..=max_y {
            for w in 0..width {
                if x1 < self.options.width && y + w < self.options.height {
                    img.put_pixel(x1, y + w, Rgba(color));
                }
                if max_x < self.options.width && y + w < self.options.height {
                    img.put_pixel(max_x, y + w, Rgba(color));
                }
            }
        }
    }

    fn render_feature(&self, img: &mut RgbaImage, geometry: &GeoJsonGeometry, style: &Style) {
        let geo = geometry.to_geo();
        match &geo {
            Geometry::Point(p) => self.render_point(img, p, style),
            Geometry::LineString(ls) => self.render_linestring(img, ls, style),
            Geometry::Polygon(p) => self.render_polygon(img, p, style),
            _ => {},
        }
    }

    fn render_point(&self, img: &mut RgbaImage, point: &Point<f64>, style: &Style) {
        let (cx, cy) = self.world_to_pixel(point.x(), point.y());
        let size = style.point_size.unwrap_or(6.0) as i32;
        let half = size / 2;

        let fill_color = style.parse_fill_color().unwrap_or([255, 0, 0, 255]);
        let stroke_color = style.parse_stroke_color().unwrap_or([0, 0, 0, 255]);

        match style.mark.as_deref().unwrap_or("circle") {
            "square" => self.draw_square(img, cx, cy, half, fill_color, stroke_color),
            "cross" => self.draw_cross(img, cx, cy, half, stroke_color),
            "x" | "X" => self.draw_x_mark(img, cx, cy, half, stroke_color),
            "star" => self.draw_star(img, cx, cy, half, fill_color, stroke_color),
            "triangle" => self.draw_triangle(img, cx, cy, half, fill_color, stroke_color),
            _ => self.draw_circle(img, cx, cy, half, fill_color, stroke_color),
        }
    }

    fn draw_circle(
        &self,
        img: &mut RgbaImage,
        cx: i32,
        cy: i32,
        r: i32,
        fill: [u8; 4],
        stroke: [u8; 4],
    ) {
        for dy in -r..=r {
            for dx in -r..=r {
                let dist = dx * dx + dy * dy;
                let px = cx + dx;
                let py = cy + dy;
                if px < 0
                    || px >= self.options.width as i32
                    || py < 0
                    || py >= self.options.height as i32
                {
                    continue;
                }
                if dist <= r * r {
                    if dist >= (r - 1) * (r - 1) {
                        img.put_pixel(px as u32, py as u32, Rgba(stroke));
                    } else {
                        img.put_pixel(px as u32, py as u32, Rgba(fill));
                    }
                }
            }
        }
    }

    fn draw_square(
        &self,
        img: &mut RgbaImage,
        cx: i32,
        cy: i32,
        half: i32,
        fill: [u8; 4],
        stroke: [u8; 4],
    ) {
        for dy in -half..=half {
            for dx in -half..=half {
                let px = cx + dx;
                let py = cy + dy;
                if px < 0
                    || px >= self.options.width as i32
                    || py < 0
                    || py >= self.options.height as i32
                {
                    continue;
                }
                if dx.abs() == half || dy.abs() == half {
                    img.put_pixel(px as u32, py as u32, Rgba(stroke));
                } else {
                    img.put_pixel(px as u32, py as u32, Rgba(fill));
                }
            }
        }
    }

    fn draw_cross(&self, img: &mut RgbaImage, cx: i32, cy: i32, half: i32, color: [u8; 4]) {
        for i in -half..=half {
            for w in -1..=1 {
                let px = cx + i;
                let py = cy + w;
                if px >= 0
                    && px < self.options.width as i32
                    && py >= 0
                    && py < self.options.height as i32
                {
                    img.put_pixel(px as u32, py as u32, Rgba(color));
                }
                let px2 = cx + w;
                let py2 = cy + i;
                if px2 >= 0
                    && px2 < self.options.width as i32
                    && py2 >= 0
                    && py2 < self.options.height as i32
                {
                    img.put_pixel(px2 as u32, py2 as u32, Rgba(color));
                }
            }
        }
    }

    fn draw_x_mark(&self, img: &mut RgbaImage, cx: i32, cy: i32, half: i32, color: [u8; 4]) {
        for i in -half..=half {
            for w in -1..=1 {
                let px = cx + i;
                let py = cy + i + w;
                if px >= 0
                    && px < self.options.width as i32
                    && py >= 0
                    && py < self.options.height as i32
                {
                    img.put_pixel(px as u32, py as u32, Rgba(color));
                }
                let px2 = cx + i;
                let py2 = cy - i + w;
                if px2 >= 0
                    && px2 < self.options.width as i32
                    && py2 >= 0
                    && py2 < self.options.height as i32
                {
                    img.put_pixel(px2 as u32, py2 as u32, Rgba(color));
                }
            }
        }
    }

    fn draw_star(
        &self,
        img: &mut RgbaImage,
        cx: i32,
        cy: i32,
        half: i32,
        _fill: [u8; 4],
        stroke: [u8; 4],
    ) {
        let outer = half;
        let inner = half * 2 / 5;
        let points = [
            (0.0, 1.0),
            (0.2245, 0.3090),
            (0.9511, 0.3090),
            (0.3633, -0.1180),
            (0.5878, -0.8090),
            (0.0, -0.3820),
            (-0.5878, -0.8090),
            (-0.3633, -0.1180),
            (-0.9511, 0.3090),
            (-0.2245, 0.3090),
        ];
        for i in 0..points.len() {
            let j = (i + 1) % points.len();
            let x1 = cx
                + (outer as f64 * points[i].0
                    + (outer - inner) as f64 * (points[j].0 - points[i].0))
                    as i32;
            let y1 = cy
                + (outer as f64 * points[i].1
                    + (outer - inner) as f64 * (points[j].1 - points[i].1))
                    as i32;
            let x2 = cx + (outer as f64 * points[j].0) as i32;
            let y2 = cy + (outer as f64 * points[j].1) as i32;
            self.draw_line(img, x1, y1, x2, y2, stroke, 1);
            let x3 = cx + (inner as f64 * points[i].0) as i32;
            let y3 = cy + (inner as f64 * points[i].1) as i32;
            self.draw_line(img, x3, y3, x1, y1, stroke, 1);
        }
    }

    fn draw_triangle(
        &self,
        img: &mut RgbaImage,
        cx: i32,
        cy: i32,
        half: i32,
        fill: [u8; 4],
        stroke: [u8; 4],
    ) {
        let pts = [
            (cx, cy - half),
            (cx - half, cy + half),
            (cx + half, cy + half),
        ];
        self.draw_line(img, pts[0].0, pts[0].1, pts[1].0, pts[1].1, stroke, 1);
        self.draw_line(img, pts[1].0, pts[1].1, pts[2].0, pts[2].1, stroke, 1);
        self.draw_line(img, pts[2].0, pts[2].1, pts[0].0, pts[0].1, stroke, 1);
        let min_y = pts.iter().map(|p| p.1).min().unwrap_or(0);
        let max_y = pts.iter().map(|p| p.1).max().unwrap_or(0);
        for y in min_y..=max_y {
            let mut xs: Vec<i32> = Vec::new();
            for i in 0..3 {
                let j = (i + 1) % 3;
                let (x1, y1) = pts[i];
                let (x2, y2) = pts[j];
                if (y1 <= y && y2 > y) || (y2 <= y && y1 > y) {
                    if y1 != y2 {
                        let x = x1 as f64 + (y - y1) as f64 / (y2 - y1) as f64 * (x2 - x1) as f64;
                        xs.push(x as i32);
                    }
                }
            }
            xs.sort();
            for pair in xs.chunks(2) {
                if pair.len() == 2 {
                    for x in pair[0]..=pair[1] {
                        if x >= 0
                            && x < self.options.width as i32
                            && y >= 0
                            && y < self.options.height as i32
                        {
                            img.put_pixel(x as u32, y as u32, Rgba(fill));
                        }
                    }
                }
            }
        }
    }

    fn render_linestring(&self, img: &mut RgbaImage, ls: &LineString<f64>, style: &Style) {
        let color = style.parse_stroke_color().unwrap_or([0, 0, 0, 255]);
        let width = style.stroke.as_ref().and_then(|s| s.width).unwrap_or(1.0) as i32;
        let dash_array = style.stroke.as_ref().and_then(|s| s.dash_array.clone());

        let coords: Vec<(i32, i32)> = ls.coords().map(|c| self.world_to_pixel(c.x, c.y)).collect();

        if let Some(dash) = dash_array {
            self.draw_line_dashed(img, &coords, color, width, &dash);
        } else {
            for i in 0..coords.len().saturating_sub(1) {
                self.draw_line(
                    img,
                    coords[i].0,
                    coords[i].1,
                    coords[i + 1].0,
                    coords[i + 1].1,
                    color,
                    width,
                );
            }
        }
    }

    fn draw_line_dashed(
        &self,
        img: &mut RgbaImage,
        coords: &[(i32, i32)],
        color: [u8; 4],
        width: i32,
        dash: &[f64],
    ) {
        let mut dash_idx = 0;
        let mut dash_remaining = dash[0].max(1.0);
        let mut drawing = true;

        for seg in 0..coords.len().saturating_sub(1) {
            let (x0, y0) = coords[seg];
            let (x1, y1) = coords[seg + 1];
            let dx = (x1 - x0) as f64;
            let dy = (y1 - y0) as f64;
            let seg_len = (dx * dx + dy * dy).sqrt();
            if seg_len < 0.5 {
                continue;
            }

            let mut pos = 0.0_f64;
            while pos < seg_len {
                let remaining_in_seg = seg_len - pos;
                let draw_len = dash_remaining.min(remaining_in_seg);
                let t0 = pos / seg_len;
                let t1 = (pos + draw_len) / seg_len;
                let sx0 = x0 + (dx * t0) as i32;
                let sy0 = y0 + (dy * t0) as i32;
                let sx1 = x0 + (dx * t1) as i32;
                let sy1 = y0 + (dy * t1) as i32;

                if drawing {
                    self.draw_line(img, sx0, sy0, sx1, sy1, color, width);
                }

                pos += draw_len;
                dash_remaining -= draw_len;
                if dash_remaining <= 0.5 {
                    dash_idx = (dash_idx + 1) % dash.len();
                    dash_remaining = dash[dash_idx].max(1.0);
                    drawing = !drawing;
                }
            }
        }
    }

    fn render_polygon(&self, img: &mut RgbaImage, polygon: &Polygon<f64>, style: &Style) {
        if let Some(fill) = &style.fill {
            let color = Self::parse_color(&fill.color).unwrap_or([100, 100, 100, 128]);
            let exterior = polygon.exterior();
            let mut pixels = Vec::new();
            for coord in exterior.coords() {
                let (px, py) = self.world_to_pixel(coord.x, coord.y);
                pixels.push((px, py));
            }
            self.fill_polygon(img, &pixels, color);
        }

        if let Some(stroke) = &style.stroke {
            let color = Self::parse_color(&stroke.color).unwrap_or([0, 0, 0, 255]);
            let width = stroke.width.unwrap_or(1.0) as i32;
            let dash_array = stroke.dash_array.clone();

            let coords: Vec<(i32, i32)> = polygon
                .exterior()
                .coords()
                .map(|c| self.world_to_pixel(c.x, c.y))
                .collect();

            if let Some(dash) = dash_array {
                self.draw_line_dashed(img, &coords, color, width, &dash);
            } else {
                for i in 0..coords.len().saturating_sub(1) {
                    self.draw_line(
                        img,
                        coords[i].0,
                        coords[i].1,
                        coords[i + 1].0,
                        coords[i + 1].1,
                        color,
                        width,
                    );
                }
            }
        }
    }

    pub fn world_to_pixel(&self, x: f64, y: f64) -> (i32, i32) {
        let width = self.options.width as f64;
        let height = self.options.height as f64;
        let px = ((x - self.bounds.minx) / (self.bounds.maxx - self.bounds.minx)) * width;
        let py = height - ((y - self.bounds.miny) / (self.bounds.maxy - self.bounds.miny)) * height;
        (px as i32, py as i32)
    }

    fn draw_line(
        &self,
        img: &mut RgbaImage,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        color: [u8; 4],
        width: i32,
    ) {
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        let mut x = x0;
        let mut y = y0;

        loop {
            for wdx in -width..=width {
                for wdy in -width..=width {
                    let px = x + wdx;
                    let py = y + wdy;
                    if px >= 0
                        && px < self.options.width as i32
                        && py >= 0
                        && py < self.options.height as i32
                    {
                        img.put_pixel(px as u32, py as u32, Rgba(color));
                    }
                }
            }

            if x == x1 && y == y1 {
                break;
            }

            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    fn fill_polygon(&self, img: &mut RgbaImage, points: &[(i32, i32)], color: [u8; 4]) {
        if points.len() < 3 {
            return;
        }

        let min_y = points.iter().map(|p| p.1).min().unwrap_or(0);
        let max_y = points.iter().map(|p| p.1).max().unwrap_or(0);

        for y in min_y..=max_y {
            let mut intersections: Vec<i32> = Vec::new();
            for i in 0..points.len() {
                let j = (i + 1) % points.len();
                let (x1, y1) = points[i];
                let (x2, y2) = points[j];

                if (y1 <= y && y2 > y) || (y2 <= y && y1 > y) {
                    let x_intersect =
                        x1 as f64 + (y - y1) as f64 / (y2 - y1) as f64 * (x2 - x1) as f64;
                    intersections.push(x_intersect as i32);
                }
            }

            intersections.sort();

            for i in (0..intersections.len()).step_by(2) {
                if i + 1 < intersections.len() {
                    let x_start = intersections[i];
                    let x_end = intersections[i + 1];
                    for x in x_start..=x_end {
                        if x >= 0
                            && x < self.options.width as i32
                            && y >= 0
                            && y < self.options.height as i32
                        {
                            img.put_pixel(x as u32, y as u32, Rgba(color));
                        }
                    }
                }
            }
        }
    }

    fn parse_color(color: &str) -> Option<[u8; 4]> {
        if color.starts_with('#') {
            let hex = &color[1..];
            if hex.len() == 6 {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                return Some([r, g, b, 255]);
            } else if hex.len() == 8 {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
                return Some([r, g, b, a]);
            }
        }
        None
    }
}

#[derive(Debug, Clone)]
pub struct Style {
    pub fill: Option<FillStyle>,
    pub stroke: Option<StrokeStyle>,
    pub point_size: Option<f64>,
    pub mark: Option<String>,
}

impl Style {
    pub fn new() -> Self {
        Style {
            fill: Some(FillStyle::default()),
            stroke: Some(StrokeStyle::default()),
            point_size: None,
            mark: None,
        }
    }

    pub fn parse_fill_color(&self) -> Option<[u8; 4]> {
        self.fill.as_ref().and_then(|f| Self::parse_color(&f.color))
    }

    pub fn parse_stroke_color(&self) -> Option<[u8; 4]> {
        self.stroke
            .as_ref()
            .and_then(|s| Self::parse_color(&s.color))
    }

    fn parse_color(color: &str) -> Option<[u8; 4]> {
        if color.starts_with('#') {
            let hex = &color[1..];
            if hex.len() == 6 {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                return Some([r, g, b, 255]);
            }
        }
        Some([100, 100, 100, 255])
    }

    pub fn parse_color_named(color: &str) -> Option<[u8; 4]> {
        Self::parse_color(color)
    }
}

impl Default for Style {
    fn default() -> Self {
        Style::new()
    }
}

#[derive(Debug, Clone)]
pub struct FillStyle {
    pub color: String,
    #[allow(dead_code)]
    pub opacity: f64,
}

impl Default for FillStyle {
    fn default() -> Self {
        FillStyle {
            color: "#808080".to_string(),
            opacity: 1.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StrokeStyle {
    pub color: String,
    pub width: Option<f64>,
    #[allow(dead_code)]
    pub opacity: f64,
    pub dash_array: Option<Vec<f64>>,
}

impl Default for StrokeStyle {
    fn default() -> Self {
        StrokeStyle {
            color: "#000000".to_string(),
            width: Some(1.0),
            opacity: 1.0,
            dash_array: None,
        }
    }
}

pub fn render_map(features: &[Feature], img_width: u32, img_height: u32) -> Vec<u8> {
    if features.is_empty() {
        let img = RgbaImage::new(img_width, img_height);
        let mut buf = Vec::new();
        PngEncoder::new(&mut buf)
            .write_image(img.as_raw(), img_width, img_height, ColorType::Rgba8)
            .unwrap();
        return buf;
    }

    let mut minx = f64::MAX;
    let mut miny = f64::MAX;
    let mut maxx = f64::MIN;
    let mut maxy = f64::MIN;

    for feature in features {
        match &feature.geometry {
            GeoJsonGeometry::Point { coordinates } => {
                if coordinates.len() >= 2 {
                    minx = minx.min(coordinates[0]);
                    miny = miny.min(coordinates[1]);
                    maxx = maxx.max(coordinates[0]);
                    maxy = maxy.max(coordinates[1]);
                }
            },
            GeoJsonGeometry::LineString { coordinates } => {
                for coord in coordinates {
                    if coord.len() >= 2 {
                        minx = minx.min(coord[0]);
                        miny = miny.min(coord[1]);
                        maxx = maxx.max(coord[0]);
                        maxy = maxy.max(coord[1]);
                    }
                }
            },
            GeoJsonGeometry::Polygon { coordinates } => {
                for ring in coordinates {
                    for coord in ring {
                        if coord.len() >= 2 {
                            minx = minx.min(coord[0]);
                            miny = miny.min(coord[1]);
                            maxx = maxx.max(coord[0]);
                            maxy = maxy.max(coord[1]);
                        }
                    }
                }
            },
            _ => {},
        }
    }

    let mut world_width = maxx - minx;
    let world_height = maxy - miny;

    if world_width < 0.01 {
        world_width = 1.0;
        minx = minx - 0.5;
        maxx = maxx + 0.5;
    }

    if world_height < 0.01 {
        miny = miny - 0.5;
        maxy = maxy + 0.5;
    }

    let padding = world_width * 0.1;
    let bounds = Bounds::new(
        minx - padding,
        miny - padding,
        maxx + padding,
        maxy + padding,
    );

    let options = RenderOptions {
        width: img_width,
        height: img_height,
        transparent: false,
        bg_color: Some([255, 255, 255, 255]),
        format: RenderFormat::PNG,
    };

    let renderer = MapRenderer::new(options.clone(), bounds);
    let features_with_style: Vec<(GeoJsonGeometry, Style)> = features
        .iter()
        .map(|f| (f.geometry.clone(), Style::default()))
        .collect();

    let img = renderer.render(features_with_style);

    let mut buf = Vec::new();
    PngEncoder::new(&mut buf)
        .write_image(img.as_raw(), img_width, img_height, ColorType::Rgba8)
        .unwrap();
    buf
}

// ---------------------------------------------------------------------------
// SVG 渲染 — 将地图渲染为 SVG 矢量格式
// ---------------------------------------------------------------------------

/// 将要素渲染为 SVG 字符串
pub fn render_to_svg(
    features: &[(GeoJsonGeometry, Style)],
    bounds: &Bounds,
    width: u32,
    height: u32,
) -> String {
    let mut svg = String::new();
    svg.push_str(&format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {} {}" width="{}" height="{}">
"#,
        width, height, width, height
    ));

    svg.push_str(
        r#"<defs><style type="text/css"><![CDATA[
    .fp { fill: #1565c0; stroke: #0d47a1; stroke-width: 2; }
    .fl { fill: none; stroke: #1565c0; stroke-width: 2; }
    .fg { fill: #bbdefb; stroke: #1565c0; stroke-width: 1.5; fill-opacity: 0.6; }
]]></style></defs>"#,
    );

    svg.push_str(&format!(
        "<rect width=\"100%\" height=\"100%\" fill=\"#f8f8f8\"/>\n"
    ));

    for (geometry, _style) in features {
        match geometry {
            GeoJsonGeometry::Point { coordinates } if coordinates.len() >= 2 => {
                let (sx, sy) = project_point(coordinates[0], coordinates[1], bounds, width, height);
                svg.push_str(&format!(
                    r#"<circle cx="{}" cy="{}" r="4" class="fp"/>"#,
                    sx, sy
                ));
            },
            GeoJsonGeometry::MultiPoint { coordinates } => {
                for c in coordinates {
                    if c.len() >= 2 {
                        let (sx, sy) = project_point(c[0], c[1], bounds, width, height);
                        svg.push_str(&format!(
                            r#"<circle cx="{}" cy="{}" r="3" class="fp"/>"#,
                            sx, sy
                        ));
                    }
                }
            },
            GeoJsonGeometry::LineString { coordinates } => {
                if coordinates.len() >= 2 {
                    let pts: String = coordinates
                        .iter()
                        .filter(|c| c.len() >= 2)
                        .map(|c| {
                            let (sx, sy) = project_point(c[0], c[1], bounds, width, height);
                            format!("{},{}", sx, sy)
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                    svg.push_str(&format!(r#"<polyline points="{}" class="fl"/>"#, pts));
                }
            },
            GeoJsonGeometry::Polygon { coordinates } => {
                for ring in coordinates {
                    if ring.len() >= 3 {
                        let pts: String = ring
                            .iter()
                            .filter(|c| c.len() >= 2)
                            .map(|c| {
                                let (sx, sy) = project_point(c[0], c[1], bounds, width, height);
                                format!("{},{}", sx, sy)
                            })
                            .collect::<Vec<_>>()
                            .join(" ");
                        svg.push_str(&format!(r#"<polygon points="{}" class="fg"/>"#, pts));
                    }
                }
            },
            GeoJsonGeometry::MultiPolygon { coordinates } => {
                for poly in coordinates {
                    for ring in poly {
                        if ring.len() >= 3 {
                            let pts: String = ring
                                .iter()
                                .filter(|c| c.len() >= 2)
                                .map(|c| {
                                    let (sx, sy) = project_point(c[0], c[1], bounds, width, height);
                                    format!("{},{}", sx, sy)
                                })
                                .collect::<Vec<_>>()
                                .join(" ");
                            svg.push_str(&format!(r#"<polygon points="{}" class="fg"/>"#, pts));
                        }
                    }
                }
            },
            _ => {},
        }
        svg.push('\n');
    }
    svg.push_str("</svg>");
    svg
}

/// 地理坐标 → 屏幕坐标
fn project_point(lon: f64, lat: f64, bounds: &Bounds, width: u32, height: u32) -> (f64, f64) {
    let sx = (lon - bounds.minx) / (bounds.maxx - bounds.minx) * width as f64;
    let sy = (bounds.maxy - lat) / (bounds.maxy - bounds.miny) * height as f64;
    (sx, sy)
}

// ---------------------------------------------------------------------------
// KML 渲染 — 将要素输出为 KML 格式
// ---------------------------------------------------------------------------

/// 将要素渲染为 KML 字符串
pub fn render_to_kml(features: &[(GeoJsonGeometry, Style)], layer_name: &str) -> String {
    let mut kml = String::new();
    kml.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    kml.push_str(&format!(
        r#"<kml xmlns="http://www.opengis.net/kml/2.2">
<Document><name>{}</name><open>1</open>"#,
        layer_name
    ));

    for (i, (geometry, _style)) in features.iter().enumerate() {
        kml.push_str(&format!(
            r#"<Placemark><name>Feature {}</name>{}</Placemark>"#,
            i,
            geometry_to_kml(geometry)
        ));
    }

    kml.push_str(
        r#"<Style id="defaultStyle">
<LineStyle><color>ff0000ff</color><width>2</width></LineStyle>
<PolyStyle><color>400000ff</color></PolyStyle>
</Style>"#,
    );

    kml.push_str("</Document></kml>");
    kml
}

fn geometry_to_kml(g: &GeoJsonGeometry) -> String {
    match g {
        GeoJsonGeometry::Point { coordinates } if coordinates.len() >= 2 => {
            format!(
                "<Point><coordinates>{},{},0</coordinates></Point>",
                coordinates[0], coordinates[1]
            )
        },
        GeoJsonGeometry::LineString { coordinates } => {
            let c: Vec<String> = coordinates
                .iter()
                .filter(|c| c.len() >= 2)
                .map(|c| format!("{},{},0", c[0], c[1]))
                .collect();
            format!(
                "<LineString><coordinates>{}</coordinates></LineString>",
                c.join(" ")
            )
        },
        GeoJsonGeometry::Polygon { coordinates } => {
            let rings: Vec<String> = coordinates
                .iter()
                .map(|ring| {
                    let c: Vec<String> = ring
                        .iter()
                        .filter(|c| c.len() >= 2)
                        .map(|c| format!("{},{},0", c[0], c[1]))
                        .collect();
                    format!(
                        "<LinearRing><coordinates>{}</coordinates></LinearRing>",
                        c.join(" ")
                    )
                })
                .collect();
            format!("<Polygon>{}</Polygon>", rings.join(""))
        },
        _ => String::new(),
    }
}

// ---------------------------------------------------------------------------
// GeoRSS 渲染 — 将要素输出为 RSS 2.0 + GeoRSS
// ---------------------------------------------------------------------------

/// Convert a `(lon, lat)` coordinate pair to GeoRSS's `lat lon` ordering.
fn format_georss_coord(coordinates: &[f64]) -> Option<String> {
    if coordinates.len() >= 2 {
        Some(format!("{} {}", coordinates[1], coordinates[0]))
    } else {
        None
    }
}

/// Render features as an RSS 2.0 feed with the GeoRSS namespace. Points become
/// `<georss:point>`, lines `<georss:line>` and polygons `<georss:polygon>`,
/// always in `lat lon` order (GeoRSS convention, matching GeoServer).
pub fn render_to_georss(features: &[(GeoJsonGeometry, Style)], layer_name: &str) -> String {
    let mut rss = String::new();
    rss.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    rss.push_str(&format!(
        r#"<rss xmlns:georss="http://www.georss.org/georss" version="2.0">
<channel><title>{}</title><description>{}</description>"#,
        layer_name, layer_name
    ));

    for (i, (geometry, _style)) in features.iter().enumerate() {
        rss.push_str(&format!(r#"<item><title>Feature {}</title>"#, i));
        rss.push_str(&geometry_to_georss(geometry));
        rss.push_str("</item>");
    }

    rss.push_str("</channel></rss>");
    rss
}

fn geometry_to_georss(g: &GeoJsonGeometry) -> String {
    match g {
        GeoJsonGeometry::Point { coordinates } => {
            if let Some(c) = format_georss_coord(coordinates) {
                format!("<georss:point>{}</georss:point>", c)
            } else {
                String::new()
            }
        },
        GeoJsonGeometry::LineString { coordinates } => {
            let pts: Vec<String> = coordinates
                .iter()
                .filter_map(|c| format_georss_coord(c))
                .collect();
            if !pts.is_empty() {
                format!("<georss:line>{}</georss:line>", pts.join(" "))
            } else {
                String::new()
            }
        },
        GeoJsonGeometry::Polygon { coordinates } => {
            // GeoRSS polygon uses the (closed) exterior ring only.
            if let Some(ring) = coordinates.first() {
                let pts: Vec<String> = ring
                    .iter()
                    .filter_map(|c| format_georss_coord(c))
                    .collect();
                if !pts.is_empty() {
                    format!("<georss:polygon>{}</georss:polygon>", pts.join(" "))
                } else {
                    String::new()
                }
            } else {
                String::new()
            }
        },
        // GeoRSS has no multi-geometry element: fall back to the first member.
        GeoJsonGeometry::MultiPoint { coordinates } => {
            if let Some(c) = coordinates.first().and_then(|c| format_georss_coord(c)) {
                format!("<georss:point>{}</georss:point>", c)
            } else {
                String::new()
            }
        },
        GeoJsonGeometry::MultiLineString { coordinates } => {
            if let Some(line) = coordinates.first() {
                let pts: Vec<String> = line
                    .iter()
                    .filter_map(|c| format_georss_coord(c))
                    .collect();
                if !pts.is_empty() {
                    format!("<georss:line>{}</georss:line>", pts.join(" "))
                } else {
                    String::new()
                }
            } else {
                String::new()
            }
        },
        GeoJsonGeometry::MultiPolygon { coordinates } => {
            if let Some(poly) = coordinates.first() {
                if let Some(ring) = poly.first() {
                    let pts: Vec<String> = ring
                        .iter()
                        .filter_map(|c| format_georss_coord(c))
                        .collect();
                    if !pts.is_empty() {
                        format!("<georss:polygon>{}</georss:polygon>", pts.join(" "))
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                }
            } else {
                String::new()
            }
        },
        _ => String::new(),
    }
}

// ---------------------------------------------------------------------------
// PDF 渲染 — 将渲染好的地图图像封装为单页 PDF
// ---------------------------------------------------------------------------

/// Render features as a single-page PDF with the rendered map embedded as a
/// FlateDecode-compressed RGB image (reuses the `MapRenderer` pipeline).
pub fn render_to_pdf(
    features: &[(GeoJsonGeometry, Style)],
    bounds: &Bounds,
    width: u32,
    height: u32,
) -> Vec<u8> {
    // Render the map onto an opaque (white) background.
    let options = RenderOptions {
        width,
        height,
        transparent: false,
        bg_color: None,
        format: RenderFormat::PNG,
    };
    let renderer = MapRenderer::new(options, bounds.clone());
    let img = renderer.render(features.to_vec());

    // RgbaImage -> raw RGB bytes (drop alpha).
    let raw = img.as_raw();
    let mut rgb = Vec::with_capacity(raw.len() / 4 * 3);
    for px in raw.chunks_exact(4) {
        rgb.push(px[0]);
        rgb.push(px[1]);
        rgb.push(px[2]);
    }

    // zlib-compress the samples (PDF /FlateDecode filter).
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    use std::io::Write;
    let _ = encoder.write_all(&rgb);
    let compressed = encoder.finish().unwrap_or_default();

    build_single_page_pdf(width, height, &compressed)
}

/// Assemble a minimal one-page PDF document embedding a FlateDecode image
/// XObject, with a correct xref table and trailer.
fn build_single_page_pdf(width: u32, height: u32, image_data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut offsets: Vec<usize> = Vec::new();

    out.extend_from_slice(b"%PDF-1.4\n");

    // Object 1: document catalog.
    offsets.push(out.len());
    out.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");

    // Object 2: page tree root.
    offsets.push(out.len());
    out.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");

    // Object 3: single page.
    offsets.push(out.len());
    let page = format!(
        "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Resources << /XObject << /Im0 5 0 R >> >> /Contents 4 0 R >>\nendobj\n",
        width, height
    );
    out.extend_from_slice(page.as_bytes());

    // Object 4: content stream that places the image full-page.
    offsets.push(out.len());
    let content = format!("q {} 0 0 {} 0 0 cm /Im0 Do Q", width, height);
    let content_obj = format!(
        "4 0 obj\n<< /Length {} >>\nstream\n{}\nendstream\nendobj\n",
        content.len(),
        content
    );
    out.extend_from_slice(content_obj.as_bytes());

    // Object 5: image XObject (FlateDecode-compressed RGB samples).
    offsets.push(out.len());
    let image_header = format!(
        "5 0 obj\n<< /Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode /Length {} >>\nstream\n",
        width, height, image_data.len()
    );
    out.extend_from_slice(image_header.as_bytes());
    out.extend_from_slice(image_data);
    out.extend_from_slice(b"\nendstream\nendobj\n");

    // Cross-reference table (objects 0..5) + trailer.
    let xref_offset = out.len();
    out.extend_from_slice(b"xref\n0 6\n0000000000 65535 f \n");
    for off in &offsets {
        out.extend_from_slice(format!("{:010} 00000 n \n", off).as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
            xref_offset
        )
        .as_bytes(),
    );

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(lon: f64, lat: f64) -> GeoJsonGeometry {
        GeoJsonGeometry::Point {
            coordinates: vec![lon, lat],
        }
    }

    #[test]
    fn test_render_to_georss_point_latlon_order() {
        let features = vec![(point(103.8, 44.3), Style::default())];
        let rss = render_to_georss(&features, "archsites");
        assert!(rss.starts_with(r#"<?xml version="1.0" encoding="UTF-8"?>"#));
        assert!(rss.contains(r#"<rss xmlns:georss="http://www.georss.org/georss" version="2.0">"#));
        assert!(rss.contains("<channel><title>archsites</title>"));
        assert!(rss.contains("<item><title>Feature 0</title>"));
        // GeoRSS orders as "lat lon" (not "lon lat").
        assert!(rss.contains("<georss:point>44.3 103.8</georss:point>"));
        assert!(rss.ends_with("</channel></rss>"));
    }

    #[test]
    fn test_render_to_georss_line_and_polygon() {
        let line = GeoJsonGeometry::LineString {
            coordinates: vec![vec![0.0, 1.0], vec![2.0, 3.0], vec![4.0, 5.0]],
        };
        let polygon = GeoJsonGeometry::Polygon {
            coordinates: vec![vec![
                vec![0.0, 0.0],
                vec![0.0, 4.0],
                vec![4.0, 4.0],
                vec![4.0, 0.0],
                vec![0.0, 0.0],
            ]],
        };
        let features = vec![(line, Style::default()), (polygon, Style::default())];
        let rss = render_to_georss(&features, "shapes");
        assert!(rss.contains("<georss:line>1 0 3 2 5 4</georss:line>"));
        assert!(rss.contains(
            "<georss:polygon>0 0 4 0 4 4 0 4 0 0</georss:polygon>"
        ));
    }

    fn find_bytes(hay: &[u8], needle: &[u8]) -> Option<usize> {
        hay.windows(needle.len()).position(|w| w == needle)
    }

    #[test]
    fn test_render_to_pdf_valid_structure() {
        let bounds = Bounds::new(-180.0, -90.0, 180.0, 90.0);
        let features = vec![(point(0.0, 0.0), Style::default())];
        let pdf = render_to_pdf(&features, &bounds, 64, 64);
        assert!(pdf.starts_with(b"%PDF-1.4"));
        assert!(find_bytes(&pdf, b"/Type /Catalog").is_some());
        assert!(find_bytes(&pdf, b"/Type /Page").is_some());
        assert!(find_bytes(&pdf, b"/Subtype /Image").is_some());
        assert!(find_bytes(&pdf, b"/Filter /FlateDecode").is_some());
        assert!(pdf.ends_with(b"%%EOF\n"));

        // Every object offset in the xref table must point at "N 0 obj".
        // Parse as raw bytes because the image stream is binary.
        let startxref_pos = find_bytes(&pdf, b"startxref\n").expect("startxref");
        let after = &pdf[startxref_pos + b"startxref\n".len()..];
        let xref_offset: usize = std::str::from_utf8(
            after.split(|&b| b == b'\n').next().expect("offset line"),
        )
        .expect("utf8 offset")
        .trim()
        .parse()
        .expect("xref offset");

        let xref = &pdf[xref_offset..];
        assert!(xref.starts_with(b"xref\n0 6\n"));
        let mut cursor = xref_offset + b"xref\n0 6\n".len();
        let mut expected_obj = 0usize;
        for line_no in 0..6 {
            let line_end = pdf[cursor..]
                .iter()
                .position(|&b| b == b'\n')
                .map(|p| cursor + p)
                .expect("line end");
            let line = &pdf[cursor..line_end];
            cursor = line_end + 1;
            if line_no == 0 {
                // Object 0: free head entry.
                assert!(line.starts_with(b"0000000000 65535 f"));
                expected_obj = 1;
                continue;
            }
            let off: usize = std::str::from_utf8(&line[..10])
                .expect("utf8 entry")
                .parse()
                .expect("offset number");
            let expected = format!("{} 0 obj", expected_obj);
            assert_eq!(
                &pdf[off..off + expected.len()],
                expected.as_bytes(),
                "xref 条目 {} 应指向 '{} 0 obj'",
                expected_obj,
                expected_obj
            );
            expected_obj += 1;
        }
        // Image stream must contain actual FlateDecode data.
        assert!(pdf.len() > 200, "PDF 应包含压缩图像数据");
    }
}
