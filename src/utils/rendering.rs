use image::{Rgba, RgbaImage, ColorType};
use image::codecs::png::PngEncoder;
use geo_types::{Point, LineString, Polygon, Geometry};
use crate::models::{Bounds, GeoJsonGeometry, Feature};

#[derive(Debug, Clone, Copy)]
pub struct RenderOptions {
    pub width: u32,
    pub height: u32,
    pub transparent: bool,
    pub bg_color: Option<[u8; 4]>,
    pub format: RenderFormat,
}

#[derive(Debug, Clone, Copy)]
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
        
        for (geometry, style) in features {
            self.render_feature(&mut img, &geometry, &style);
        }
        
        img
    }

    fn render_feature(&self, img: &mut RgbaImage, geometry: &GeoJsonGeometry, style: &Style) {
        let geo = geometry.to_geo();
        match &geo {
            Geometry::Point(p) => self.render_point(img, p, style),
            Geometry::LineString(ls) => self.render_linestring(img, ls, style),
            Geometry::Polygon(p) => self.render_polygon(img, p, style),
            _ => {}
        }
    }

    fn render_point(&self, img: &mut RgbaImage, point: &Point<f64>, style: &Style) {
        let (px, py) = self.world_to_pixel(point.x(), point.y());
        if px >= 0 && px < self.options.width as i32 && py >= 0 && py < self.options.height as i32 {
            let color = style.parse_fill_color().unwrap_or([255, 0, 0, 255]);
            for dy in -2..=2 {
                for dx in -2..=2 {
                    if dx * dx + dy * dy <= 4 {
                        let x = (px + dx) as u32;
                        let y = (py + dy) as u32;
                        if x < self.options.width && y < self.options.height {
                            img.put_pixel(x, y, Rgba(color));
                        }
                    }
                }
            }
        }
    }

    fn render_linestring(&self, img: &mut RgbaImage, ls: &LineString<f64>, style: &Style) {
        let color = style.parse_stroke_color().unwrap_or([0, 0, 0, 255]);
        let width = style.stroke.as_ref().and_then(|s| s.width).unwrap_or(1.0) as i32;
        
        let coords: Vec<(i32, i32)> = ls.coords()
            .map(|c| self.world_to_pixel(c.x, c.y))
            .collect();
        
        for i in 0..coords.len().saturating_sub(1) {
            self.draw_line(img, coords[i].0, coords[i].1, coords[i + 1].0, coords[i + 1].1, color, width);
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
            
            let coords: Vec<(i32, i32)> = polygon.exterior().coords()
                .map(|c| self.world_to_pixel(c.x, c.y))
                .collect();
            
            for i in 0..coords.len().saturating_sub(1) {
                self.draw_line(img, coords[i].0, coords[i].1, coords[i + 1].0, coords[i + 1].1, color, width);
            }
        }
    }

    fn world_to_pixel(&self, x: f64, y: f64) -> (i32, i32) {
        let width = self.options.width as f64;
        let height = self.options.height as f64;
        
        let px = ((x - self.bounds.minx) / (self.bounds.maxx - self.bounds.minx)) * width;
        let py = height - ((y - self.bounds.miny) / (self.bounds.maxy - self.bounds.miny)) * height;
        
        (px as i32, py as i32)
    }

    fn draw_line(&self, img: &mut RgbaImage, x0: i32, y0: i32, x1: i32, y1: i32, color: [u8; 4], width: i32) {
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
                    if px >= 0 && px < self.options.width as i32 && py >= 0 && py < self.options.height as i32 {
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
                    let x_intersect = x1 as f64 + (y - y1) as f64 / (y2 - y1) as f64 * (x2 - x1) as f64;
                    intersections.push(x_intersect as i32);
                }
            }
            
            intersections.sort();
            
            for i in (0..intersections.len()).step_by(2) {
                if i + 1 < intersections.len() {
                    let x_start = intersections[i];
                    let x_end = intersections[i + 1];
                    for x in x_start..=x_end {
                        if x >= 0 && x < self.options.width as i32 && y >= 0 && y < self.options.height as i32 {
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
}

impl Style {
    pub fn new() -> Self {
        Style {
            fill: Some(FillStyle::default()),
            stroke: Some(StrokeStyle::default()),
        }
    }
    
    pub fn parse_fill_color(&self) -> Option<[u8; 4]> {
        self.fill.as_ref().and_then(|f| Self::parse_color(&f.color))
    }
    
    pub fn parse_stroke_color(&self) -> Option<[u8; 4]> {
        self.stroke.as_ref().and_then(|s| Self::parse_color(&s.color))
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
}

impl Default for Style {
    fn default() -> Self {
        Style::new()
    }
}

#[derive(Debug, Clone)]
pub struct FillStyle {
    pub color: String,
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
    pub opacity: f64,
}

impl Default for StrokeStyle {
    fn default() -> Self {
        StrokeStyle {
            color: "#000000".to_string(),
            width: Some(1.0),
            opacity: 1.0,
        }
    }
}

pub fn render_map(features: &[Feature], img_width: u32, img_height: u32) -> Vec<u8> {
    if features.is_empty() {
        let img = RgbaImage::new(img_width, img_height);
        let mut buf = Vec::new();
        PngEncoder::new(&mut buf).encode(&img, img_width, img_height, ColorType::Rgba8).unwrap();
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
            }
            GeoJsonGeometry::LineString { coordinates } => {
                for coord in coordinates {
                    if coord.len() >= 2 {
                        minx = minx.min(coord[0]);
                        miny = miny.min(coord[1]);
                        maxx = maxx.max(coord[0]);
                        maxy = maxy.max(coord[1]);
                    }
                }
            }
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
            }
            _ => {}
        }
    }

    let mut world_width = maxx - minx;
    let mut world_height = maxy - miny;
    
    if world_width < 0.01 {
        world_width = 1.0;
        minx = minx - 0.5;
        maxx = maxx + 0.5;
    }
    
    if world_height < 0.01 {
        world_height = 1.0;
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
    PngEncoder::new(&mut buf).encode(&img, img_width, img_height, ColorType::Rgba8).unwrap();
    buf
}
