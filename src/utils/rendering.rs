use crate::models::{Bounds, Feature, GeoJsonGeometry};
use geo_types::{Coord, LineString, Point, Polygon};
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
#[allow(clippy::upper_case_acronyms)] // PNG/JPEG/GIF are domain-standard acronyms
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

        // Z-order: stable-sort so that, within the same z-index, polygons draw
        // below lines below points (layer order is preserved by stability).
        let mut items: Vec<&(GeoJsonGeometry, Style)> = features.iter().collect();
        items.sort_by_key(|(g, s)| (s.z_index, geometry_type_rank(g)));

        // Pass 1: geometry.
        for (geometry, style) in &items {
            self.render_feature(&mut img, geometry, style);
        }

        // Pass 2: labels, with collision avoidance against already-placed ones.
        self.render_labels(&mut img, &items);

        img
    }

    /// Render all labels of the given items, skipping any whose bounding box
    /// overlaps an already-placed label (simple greedy collision avoidance).
    fn render_labels(&self, img: &mut RgbaImage, items: &[&(GeoJsonGeometry, Style)]) {
        use super::bitmap_font;
        let mut placed: Vec<(i32, i32, i32, i32)> = Vec::new();
        let w = self.options.width as i32;
        let h = self.options.height as i32;

        for (geometry, style) in items {
            let label = match &style.label {
                Some(l) if !l.text.trim().is_empty() => l,
                _ => continue,
            };
            let scale = label.scale();
            let text_w = bitmap_font::text_width(&label.text, scale) as i32;
            let text_h = bitmap_font::text_height(scale) as i32;
            let pad = (label.halo_radius.ceil() as i32).max(1);

            // Anchor: point → the point itself; line → midpoint; polygon → centroid.
            let (cx, cy) = match geometry {
                GeoJsonGeometry::Point { coordinates } if coordinates.len() >= 2 => {
                    self.world_to_pixel(coordinates[0], coordinates[1])
                },
                GeoJsonGeometry::LineString { coordinates } => {
                    if let Some((mx, my)) = midpoint(coordinates) {
                        self.world_to_pixel(mx, my)
                    } else {
                        continue;
                    }
                },
                GeoJsonGeometry::Polygon { coordinates } => {
                    if let Some((cx, cy)) = polygon_centroid(coordinates) {
                        self.world_to_pixel(cx, cy)
                    } else {
                        continue;
                    }
                },
                _ => continue,
            };

            // Bounding box (halo inflated) around the anchor.
            let x0 = cx - text_w / 2 - pad;
            let y0 = cy - text_h / 2 - pad;
            let x1 = x0 + text_w + pad * 2;
            let y1 = y0 + text_h + pad * 2;

            // Off-screen labels are skipped.
            if x1 < 0 || y1 < 0 || x0 >= w || y0 >= h {
                continue;
            }
            // Collision: skip if it overlaps any placed box.
            if placed
                .iter()
                .any(|(px0, py0, px1, py1)| x0 < *px1 && x1 > *px0 && y0 < *py1 && y1 > *py0)
            {
                continue;
            }
            placed.push((x0, y0, x1, y1));

            let text_color = label.parse_color().unwrap_or([30, 30, 30, 255]);
            let halo_color = label.parse_halo_color();

            let origin_x = cx - text_w / 2;
            let origin_y = cy - text_h / 2;

            // Halo: draw the text in the halo color around the anchor (8 offsets).
            if let Some(hc) = halo_color {
                let radius = (label.halo_radius.max(0.5)).round() as i32;
                for dy in -radius..=radius {
                    for dx in -radius..=radius {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        if dx * dx + dy * dy > radius * radius {
                            continue;
                        }
                        bitmap_font::draw_text(
                            origin_x + dx,
                            origin_y + dy,
                            &label.text,
                            scale,
                            |px, py| {
                                if px < w as u32 && py < h as u32 {
                                    blend_pixel(img, px, py, hc);
                                }
                            },
                        );
                    }
                }
            }

            // Foreground text.
            bitmap_font::draw_text(origin_x, origin_y, &label.text, scale, |px, py| {
                if px < w as u32 && py < h as u32 {
                    blend_pixel(img, px, py, text_color);
                }
            });
        }
    }

    #[allow(clippy::too_many_arguments)] // rectangle drawing primitive
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
        match geometry {
            GeoJsonGeometry::Point { coordinates } => {
                if coordinates.len() >= 2 {
                    let p = Point::new(coordinates[0], coordinates[1]);
                    self.render_point(img, &p, style);
                }
            },
            GeoJsonGeometry::LineString { coordinates } => {
                let ls = build_linestring(coordinates);
                if ls.coords().count() >= 2 {
                    self.render_linestring(img, &ls, style);
                }
            },
            GeoJsonGeometry::Polygon { coordinates } => {
                if let Some(poly) = build_polygon(coordinates) {
                    self.render_polygon(img, &poly, style);
                }
            },
            GeoJsonGeometry::MultiPoint { coordinates } => {
                for c in coordinates {
                    if c.len() >= 2 {
                        let p = Point::new(c[0], c[1]);
                        self.render_point(img, &p, style);
                    }
                }
            },
            GeoJsonGeometry::MultiLineString { coordinates } => {
                for line in coordinates {
                    let ls = build_linestring(line);
                    if ls.coords().count() >= 2 {
                        self.render_linestring(img, &ls, style);
                    }
                }
            },
            GeoJsonGeometry::MultiPolygon { coordinates } => {
                for poly in coordinates {
                    if let Some(poly) = build_polygon(poly) {
                        self.render_polygon(img, &poly, style);
                    }
                }
            },
            GeoJsonGeometry::GeometryCollection { geometries } => {
                for sub in geometries {
                    self.render_feature(img, sub, style);
                }
            },
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
                        blend_pixel(img, px as u32, py as u32, stroke);
                    } else {
                        blend_pixel(img, px as u32, py as u32, fill);
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
                    blend_pixel(img, px as u32, py as u32, stroke);
                } else {
                    blend_pixel(img, px as u32, py as u32, fill);
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
                    blend_pixel(img, px as u32, py as u32, color);
                }
                let px2 = cx + w;
                let py2 = cy + i;
                if px2 >= 0
                    && px2 < self.options.width as i32
                    && py2 >= 0
                    && py2 < self.options.height as i32
                {
                    blend_pixel(img, px2 as u32, py2 as u32, color);
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
                    blend_pixel(img, px as u32, py as u32, color);
                }
                let px2 = cx + i;
                let py2 = cy - i + w;
                if px2 >= 0
                    && px2 < self.options.width as i32
                    && py2 >= 0
                    && py2 < self.options.height as i32
                {
                    blend_pixel(img, px2 as u32, py2 as u32, color);
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
                if ((y1 <= y && y2 > y) || (y2 <= y && y1 > y)) && y1 != y2 {
                    let x = x1 as f64 + (y - y1) as f64 / (y2 - y1) as f64 * (x2 - x1) as f64;
                    xs.push(x as i32);
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
                            blend_pixel(img, x as u32, y as u32, fill);
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
            let mut color = Self::parse_color(&fill.color).unwrap_or([100, 100, 100, 128]);
            apply_opacity(&mut color, fill.opacity);

            // All rings: the exterior is filled, interior rings become holes.
            let mut rings: Vec<Vec<(i32, i32)>> = Vec::new();
            let exterior: Vec<(i32, i32)> = polygon
                .exterior()
                .coords()
                .map(|c| self.world_to_pixel(c.x, c.y))
                .collect();
            rings.push(exterior);
            for interior in polygon.interiors() {
                let ring: Vec<(i32, i32)> = interior
                    .coords()
                    .map(|c| self.world_to_pixel(c.x, c.y))
                    .collect();
                rings.push(ring);
            }
            self.fill_polygon_rings(img, &rings, color);
        }

        if let Some(stroke) = &style.stroke {
            let mut color = Self::parse_color(&stroke.color).unwrap_or([0, 0, 0, 255]);
            apply_opacity(&mut color, stroke.opacity);
            let width = stroke.width.unwrap_or(1.0) as i32;
            let dash_array = stroke.dash_array.clone();

            // Stroke the exterior ring and every interior ring.
            for ring in std::iter::once(polygon.exterior()).chain(polygon.interiors()) {
                let coords: Vec<(i32, i32)> = ring
                    .coords()
                    .map(|c| self.world_to_pixel(c.x, c.y))
                    .collect();

                if let Some(dash) = dash_array.clone() {
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
    }

    pub fn world_to_pixel(&self, x: f64, y: f64) -> (i32, i32) {
        let width = self.options.width as f64;
        let height = self.options.height as f64;
        let px = ((x - self.bounds.minx) / (self.bounds.maxx - self.bounds.minx)) * width;
        let py = height - ((y - self.bounds.miny) / (self.bounds.maxy - self.bounds.miny)) * height;
        (px as i32, py as i32)
    }

    #[allow(clippy::too_many_arguments)] // line drawing primitive
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
                        blend_pixel(img, px as u32, py as u32, color);
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

    /// Fill a polygon given by multiple rings (first = exterior, rest = holes)
    /// using the even-odd scanline rule, so interior holes stay transparent.
    fn fill_polygon_rings(&self, img: &mut RgbaImage, rings: &[Vec<(i32, i32)>], color: [u8; 4]) {
        if rings.is_empty() || rings[0].len() < 3 {
            return;
        }

        let min_y = rings
            .iter()
            .flat_map(|r| r.iter())
            .map(|p| p.1)
            .min()
            .unwrap_or(0);
        let max_y = rings
            .iter()
            .flat_map(|r| r.iter())
            .map(|p| p.1)
            .max()
            .unwrap_or(0);

        for y in min_y..=max_y {
            let mut intersections: Vec<i32> = Vec::new();
            for ring in rings {
                if ring.len() < 3 {
                    continue;
                }
                for i in 0..ring.len() {
                    let j = (i + 1) % ring.len();
                    let (x1, y1) = ring[i];
                    let (x2, y2) = ring[j];

                    if (y1 <= y && y2 > y) || (y2 <= y && y1 > y) {
                        let x_intersect =
                            x1 as f64 + (y - y1) as f64 / (y2 - y1) as f64 * (x2 - x1) as f64;
                        intersections.push(x_intersect as i32);
                    }
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
                            blend_pixel(img, x as u32, y as u32, color);
                        }
                    }
                }
            }
        }
    }

    fn parse_color(color: &str) -> Option<[u8; 4]> {
        if let Some(hex) = color.strip_prefix('#') {
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

/// Apply an opacity factor (0..1) to a color's alpha channel.
fn apply_opacity(color: &mut [u8; 4], opacity: f64) {
    let a = (opacity.clamp(0.0, 1.0) * 255.0) as u8;
    color[3] = (color[3] as u16 * a as u16 / 255) as u8;
}

/// Source-over alpha blending: composite `fg` over the existing pixel
/// (straight alpha, non-premultiplied).
fn blend_pixel(img: &mut RgbaImage, x: u32, y: u32, fg: [u8; 4]) {
    let dst = img.get_pixel(x, y).0;
    let a_fg = fg[3] as f32 / 255.0;
    if a_fg >= 1.0 {
        img.put_pixel(x, y, Rgba(fg));
        return;
    }
    if a_fg <= 0.0 {
        return;
    }
    let a_dst = dst[3] as f32 / 255.0;
    let a_out = a_fg + a_dst * (1.0 - a_fg);
    if a_out <= 0.0 {
        return;
    }
    let blend = |c_fg: u8, c_dst: u8| -> u8 {
        ((c_fg as f32 * a_fg + c_dst as f32 * a_dst * (1.0 - a_fg)) / a_out).round() as u8
    };
    let out = [
        blend(fg[0], dst[0]),
        blend(fg[1], dst[1]),
        blend(fg[2], dst[2]),
        (a_out * 255.0).round() as u8,
    ];
    img.put_pixel(x, y, Rgba(out));
}

/// Z-order rank of a geometry type: polygons at the bottom, then lines,
/// then points on top (matching GeoServer's default drawing order).
fn geometry_type_rank(g: &GeoJsonGeometry) -> i32 {
    match g {
        GeoJsonGeometry::Polygon { .. } | GeoJsonGeometry::MultiPolygon { .. } => 0,
        GeoJsonGeometry::LineString { .. } | GeoJsonGeometry::MultiLineString { .. } => 1,
        GeoJsonGeometry::Point { .. } | GeoJsonGeometry::MultiPoint { .. } => 2,
        GeoJsonGeometry::GeometryCollection { geometries } => {
            geometries.iter().map(geometry_type_rank).max().unwrap_or(0)
        },
    }
}

/// Build a `LineString` from GeoJSON coordinates.
fn build_linestring(coordinates: &[Vec<f64>]) -> LineString<f64> {
    let points: Vec<Coord<f64>> = coordinates
        .iter()
        .filter(|c| c.len() >= 2)
        .map(|c| Coord { x: c[0], y: c[1] })
        .collect();
    LineString::new(points)
}

/// Build a `Polygon` from GeoJSON rings (first ring = exterior, rest = holes).
fn build_polygon(coordinates: &[Vec<Vec<f64>>]) -> Option<Polygon<f64>> {
    let mut rings = coordinates.iter().map(|ring| build_linestring(ring));
    let exterior = rings.next()?;
    if exterior.coords().count() < 3 {
        return None;
    }
    Some(Polygon::new(exterior, rings.collect()))
}

/// Midpoint of a linestring's coordinates.
fn midpoint(coordinates: &[Vec<f64>]) -> Option<(f64, f64)> {
    if coordinates.len() < 2 {
        return None;
    }
    let a = &coordinates[0];
    let b = &coordinates[coordinates.len() / 2];
    if a.len() < 2 || b.len() < 2 {
        return None;
    }
    Some(((a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0))
}

/// Simple polygon centroid (average of the exterior ring's vertices).
fn polygon_centroid(coordinates: &[Vec<Vec<f64>>]) -> Option<(f64, f64)> {
    let ring = coordinates.first()?;
    if ring.len() < 3 {
        return None;
    }
    let n = ring.len() as f64;
    let mut sx = 0.0;
    let mut sy = 0.0;
    let mut count = 0u32;
    for c in ring {
        if c.len() >= 2 {
            sx += c[0];
            sy += c[1];
            count += 1;
        }
    }
    if count == 0 {
        None
    } else {
        Some((sx / n, sy / n))
    }
}

#[derive(Debug, Clone)]
pub struct Style {
    pub fill: Option<FillStyle>,
    pub stroke: Option<StrokeStyle>,
    pub point_size: Option<f64>,
    pub mark: Option<String>,
    /// TextSymbolizer label configuration. The text is resolved from the
    /// feature's properties (or a literal) by the style resolver before the
    /// renderer sees it.
    pub label: Option<LabelStyle>,
    /// Optional z-index (SLD `VendorOption name="z-index"` / GeoServer CSS
    /// `z-index`). Higher values draw on top. Defaults to 0.
    pub z_index: i32,
}

/// Label (TextSymbolizer) style.
#[derive(Debug, Clone)]
pub struct LabelStyle {
    /// Resolved label text (may come from a feature property or a literal).
    pub text: String,
    /// Property name the label text is read from (`ogc:PropertyName`).
    /// Resolved to `text` by the style resolver before rendering.
    pub property: Option<String>,
    /// Font size in points (rendered at ~scale 1 px/point, min 1).
    pub font_size: f64,
    /// Label fill color (#RRGGBB or #RRGGBBAA).
    pub color: String,
    /// Halo color (#RRGGBB). None disables the halo.
    pub halo_color: Option<String>,
    /// Halo radius in px (default 1).
    pub halo_radius: f64,
}

impl LabelStyle {
    pub fn parse_color(&self) -> Option<[u8; 4]> {
        Style::parse_color(&self.color)
    }

    pub fn parse_halo_color(&self) -> Option<[u8; 4]> {
        self.halo_color.as_deref().and_then(Style::parse_color)
    }

    pub fn scale(&self) -> f64 {
        (self.font_size / 12.0).max(1.0)
    }
}

impl Style {
    pub fn new() -> Self {
        Style {
            fill: Some(FillStyle::default()),
            stroke: Some(StrokeStyle::default()),
            point_size: None,
            mark: None,
            label: None,
            z_index: 0,
        }
    }

    pub fn parse_fill_color(&self) -> Option<[u8; 4]> {
        self.fill.as_ref().and_then(|f| {
            let mut c = Self::parse_color(&f.color)?;
            apply_opacity(&mut c, f.opacity);
            Some(c)
        })
    }

    pub fn parse_stroke_color(&self) -> Option<[u8; 4]> {
        self.stroke.as_ref().and_then(|s| {
            let mut c = Self::parse_color(&s.color)?;
            apply_opacity(&mut c, s.opacity);
            Some(c)
        })
    }

    fn parse_color(color: &str) -> Option<[u8; 4]> {
        if let Some(hex) = color.strip_prefix('#') {
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
            GeoJsonGeometry::Point { coordinates } if coordinates.len() >= 2 => {
                minx = minx.min(coordinates[0]);
                miny = miny.min(coordinates[1]);
                maxx = maxx.max(coordinates[0]);
                maxy = maxy.max(coordinates[1]);
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
        minx -= 0.5;
        maxx += 0.5;
    }

    if world_height < 0.01 {
        miny -= 0.5;
        maxy += 0.5;
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

    let renderer = MapRenderer::new(options, bounds);
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

/// 将要素渲染为 SVG 字符串, 遵循每要素的 SLD/CSS 样式 (填充色/透明度、
/// 描边色/线宽/虚线、点标记、标签)。
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

    svg.push_str("<rect width=\"100%\" height=\"100%\" fill=\"#f8f8f8\"/>\n");
    svg.push_str(&render_features_svg(features, bounds, width, height));
    svg.push_str("</svg>");
    svg
}

/// 渲染一组要素为 SVG 片段 (无外层 <svg> 包裹), 供 GeometryCollection
/// 递归复用 — 避免生成嵌套 SVG 文档。
fn render_features_svg(
    features: &[(GeoJsonGeometry, Style)],
    bounds: &Bounds,
    width: u32,
    height: u32,
) -> String {
    let mut svg = String::new();
    let point =
        |lon: f64, lat: f64| -> (f64, f64) { project_point(lon, lat, bounds, width, height) };
    let pts_str = |coords: &[Vec<f64>]| -> String {
        coords
            .iter()
            .filter(|c| c.len() >= 2)
            .map(|c| {
                let (sx, sy) = point(c[0], c[1]);
                format!("{},{}", sx, sy)
            })
            .collect::<Vec<_>>()
            .join(" ")
    };

    for (geometry, style) in features {
        let is_line = matches!(
            geometry,
            GeoJsonGeometry::LineString { .. } | GeoJsonGeometry::MultiLineString { .. }
        );
        let fill_attr = if is_line {
            "fill=\"none\"".to_string()
        } else {
            style
                .parse_fill_color()
                .map(|c| {
                    format!(
                        "fill=\"{}\" fill-opacity=\"{}\"",
                        rgb_hex(c),
                        (c[3] as f64 / 255.0).clamp(0.0, 1.0)
                    )
                })
                .unwrap_or_else(|| "fill=\"none\"".to_string())
        };
        let stroke_attr = style
            .parse_stroke_color()
            .map(|c| {
                let width = style.stroke.as_ref().and_then(|s| s.width).unwrap_or(1.0);
                let dash = style
                    .stroke
                    .as_ref()
                    .and_then(|s| s.dash_array.clone())
                    .map(|d| {
                        format!(
                            " stroke-dasharray=\"{}\"",
                            d.iter()
                                .map(|v| v.to_string())
                                .collect::<Vec<_>>()
                                .join(",")
                        )
                    })
                    .unwrap_or_default();
                format!(
                    "stroke=\"{}\" stroke-opacity=\"{}\" stroke-width=\"{}\"{}",
                    rgb_hex(c),
                    (c[3] as f64 / 255.0).clamp(0.0, 1.0),
                    width,
                    dash
                )
            })
            .unwrap_or_else(|| "stroke=\"none\"".to_string());

        match geometry {
            GeoJsonGeometry::Point { coordinates } => {
                if coordinates.len() >= 2 {
                    let (sx, sy) = point(coordinates[0], coordinates[1]);
                    let r = style.point_size.unwrap_or(6.0) / 2.0;
                    svg.push_str(&format!(
                        r#"<circle cx="{:.2}" cy="{:.2}" r="{:.2}" {} {}/>"#,
                        sx, sy, r, fill_attr, stroke_attr
                    ));
                }
            },
            GeoJsonGeometry::MultiPoint { coordinates } => {
                for c in coordinates {
                    if c.len() >= 2 {
                        let (sx, sy) = point(c[0], c[1]);
                        let r = style.point_size.unwrap_or(6.0) / 2.0;
                        svg.push_str(&format!(
                            r#"<circle cx="{:.2}" cy="{:.2}" r="{:.2}" {} {}/>"#,
                            sx, sy, r, fill_attr, stroke_attr
                        ));
                    }
                }
            },
            GeoJsonGeometry::LineString { coordinates } => {
                if coordinates.len() >= 2 {
                    let pts = pts_str(coordinates);
                    svg.push_str(&format!(
                        r#"<polyline points="{}" {} {}/>"#,
                        pts, fill_attr, stroke_attr
                    ));
                }
            },
            GeoJsonGeometry::MultiLineString { coordinates } => {
                for line in coordinates {
                    if line.len() >= 2 {
                        let pts = pts_str(line);
                        svg.push_str(&format!(
                            r#"<polyline points="{}" {} {}/>"#,
                            pts, fill_attr, stroke_attr
                        ));
                    }
                }
            },
            GeoJsonGeometry::Polygon { coordinates } => {
                if coordinates.is_empty() {
                    continue;
                }
                let pts = pts_str(&coordinates[0]);
                svg.push_str(&format!(
                    r#"<polygon points="{}" fill-rule="evenodd" {} {}/>"#,
                    pts, fill_attr, stroke_attr
                ));
                for ring in &coordinates[1..] {
                    let pts = pts_str(ring);
                    svg.push_str(&format!(
                        r#"<polygon points="{}" {} {}/>"#,
                        pts, fill_attr, stroke_attr
                    ));
                }
            },
            GeoJsonGeometry::MultiPolygon { coordinates } => {
                for poly in coordinates {
                    if poly.is_empty() {
                        continue;
                    }
                    let pts = pts_str(&poly[0]);
                    svg.push_str(&format!(
                        r#"<polygon points="{}" fill-rule="evenodd" {} {}/>"#,
                        pts, fill_attr, stroke_attr
                    ));
                    for ring in &poly[1..] {
                        let pts = pts_str(ring);
                        svg.push_str(&format!(
                            r#"<polygon points="{}" {} {}/>"#,
                            pts, fill_attr, stroke_attr
                        ));
                    }
                }
            },
            GeoJsonGeometry::GeometryCollection { geometries } => {
                for sub in geometries {
                    let single = [(sub.clone(), style.clone())];
                    svg.push_str(&render_features_svg(&single, bounds, width, height));
                }
            },
        }

        // 标签 (TextSymbolizer)。
        if let Some(label) = &style.label {
            if !label.text.trim().is_empty() {
                let (sx, sy) = match geometry {
                    GeoJsonGeometry::Point { coordinates } if coordinates.len() >= 2 => {
                        point(coordinates[0], coordinates[1])
                    },
                    _ => {
                        if let Some((cx, cy)) = geometry_anchor(geometry) {
                            point(cx, cy)
                        } else {
                            continue;
                        }
                    },
                };
                let color = label
                    .parse_color()
                    .map(rgb_hex)
                    .unwrap_or_else(|| "#333333".to_string());
                let halo = label
                    .parse_halo_color()
                    .map(|c| {
                        format!(
                            " stroke=\"{}\" stroke-width=\"{:.2}\"",
                            rgb_hex(c),
                            label.halo_radius * 2.0
                        )
                    })
                    .unwrap_or_default();
                let font_size = label.font_size.max(1.0);
                svg.push_str(&format!(
                    r#"<text x="{:.2}" y="{:.2}" font-family="sans-serif" font-size="{:.1}" text-anchor="middle" dominant-baseline="middle" fill="{}"{}>{}</text>"#,
                    sx, sy, font_size, color, halo, escape_xml(&label.text)
                ));
            }
        }
    }
    svg
}

/// 要素的标签锚点 (线中点 / 面质心)。
fn geometry_anchor(geometry: &GeoJsonGeometry) -> Option<(f64, f64)> {
    match geometry {
        GeoJsonGeometry::LineString { coordinates } => midpoint(coordinates),
        GeoJsonGeometry::Polygon { coordinates } => polygon_centroid(coordinates),
        _ => None,
    }
}

/// 颜色 ([r,g,b,a]) → #RRGGBB。
fn rgb_hex(c: [u8; 4]) -> String {
    format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}

/// XML 转义 (用于 SVG 文本内容)。
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
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
                let pts: Vec<String> = ring.iter().filter_map(|c| format_georss_coord(c)).collect();
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
                let pts: Vec<String> = line.iter().filter_map(|c| format_georss_coord(c)).collect();
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
                    let pts: Vec<String> =
                        ring.iter().filter_map(|c| format_georss_coord(c)).collect();
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
        assert!(rss.contains("<georss:polygon>0 0 4 0 4 4 0 4 0 0</georss:polygon>"));
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
        let xref_offset: usize =
            std::str::from_utf8(after.split(|&b| b == b'\n').next().expect("offset line"))
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

    // ------------------------------------------------------------------
    // Raster rendering engine tests (multi-geometry / opacity / holes /
    // z-order / labels).
    // ------------------------------------------------------------------

    fn renderer() -> MapRenderer {
        MapRenderer::new(
            RenderOptions {
                width: 100,
                height: 100,
                transparent: true,
                bg_color: None,
                format: RenderFormat::PNG,
            },
            Bounds::new(0.0, 0.0, 10.0, 10.0),
        )
    }

    fn is_transparent(img: &RgbaImage, x: u32, y: u32) -> bool {
        img.get_pixel(x, y).0[3] == 0
    }

    #[test]
    fn test_render_multipoint_all_drawn() {
        let geom = GeoJsonGeometry::MultiPoint {
            coordinates: vec![vec![1.0, 1.0], vec![5.0, 5.0], vec![9.0, 9.0]],
        };
        let mut style = Style::new();
        style.mark = Some("circle".to_string());
        style.point_size = Some(4.0);
        let img = renderer().render(vec![(geom, style)]);
        // Center pixels of each mark should be filled.
        for (lon, lat) in [(1.0, 1.0), (5.0, 5.0), (9.0, 9.0)] {
            let (px, py) = renderer().world_to_pixel(lon, lat);
            assert!(
                !is_transparent(&img, px as u32, py as u32),
                "mark at ({}, {}) must be drawn",
                lon,
                lat
            );
        }
    }

    #[test]
    fn test_render_multilinestring_and_geometrycollection() {
        let mls = GeoJsonGeometry::MultiLineString {
            coordinates: vec![
                vec![vec![1.0, 1.0], vec![3.0, 3.0]],
                vec![vec![7.0, 7.0], vec![9.0, 9.0]],
            ],
        };
        let coll = GeoJsonGeometry::GeometryCollection {
            geometries: vec![
                GeoJsonGeometry::MultiPolygon {
                    coordinates: vec![vec![vec![
                        vec![1.0, 1.0],
                        vec![1.0, 3.0],
                        vec![3.0, 3.0],
                        vec![3.0, 1.0],
                        vec![1.0, 1.0],
                    ]]],
                },
                point(5.0, 5.0),
            ],
        };
        let img = renderer().render(vec![(mls, Style::new()), (coll, Style::new())]);
        // MultiLineString: a pixel on the diagonal must be painted.
        let (lx, ly) = renderer().world_to_pixel(2.0, 2.0);
        assert!(!is_transparent(&img, lx as u32, ly as u32));
        // GeometryCollection: polygon fill + point mark.
        let (px, py) = renderer().world_to_pixel(2.0, 2.0);
        assert!(!is_transparent(&img, px as u32, py as u32));
        let (cx, cy) = renderer().world_to_pixel(5.0, 5.0);
        assert!(!is_transparent(&img, cx as u32, cy as u32));
    }

    #[test]
    fn test_fill_opacity_alpha_composited() {
        let geom = GeoJsonGeometry::Polygon {
            coordinates: vec![vec![
                vec![1.0, 1.0],
                vec![1.0, 4.0],
                vec![4.0, 4.0],
                vec![4.0, 1.0],
                vec![1.0, 1.0],
            ]],
        };
        // Opaque red fill at 50% opacity over a white background.
        let mut style = Style::new();
        style.fill = Some(FillStyle {
            color: "#ff0000".to_string(),
            opacity: 0.5,
        });
        style.stroke = None;
        let options = RenderOptions {
            width: 100,
            height: 100,
            transparent: false,
            bg_color: Some([255, 255, 255, 255]),
            format: RenderFormat::PNG,
        };
        let renderer = MapRenderer::new(options, Bounds::new(0.0, 0.0, 10.0, 10.0));
        let img = renderer.render(vec![(geom, style)]);
        let (ix, iy) = renderer.world_to_pixel(2.0, 2.0);
        let px = img.get_pixel(ix as u32, iy as u32).0;
        // 0.5 * 255 + 0.5 * 255(white) = ~255 red channel blended with white.
        assert!(px[0] > 200, "red channel should stay high: {}", px[0]);
        assert!(
            px[1] > 100 && px[1] < 200,
            "green channel should be mid: {}",
            px[1]
        );
        assert!(px[3] == 255);
    }

    #[test]
    fn test_polygon_holes_not_filled() {
        // Outer 0..8 square with a 2..6 hole in the middle.
        let geom = GeoJsonGeometry::Polygon {
            coordinates: vec![
                vec![
                    vec![0.0, 0.0],
                    vec![0.0, 8.0],
                    vec![8.0, 8.0],
                    vec![8.0, 0.0],
                    vec![0.0, 0.0],
                ],
                vec![
                    vec![2.0, 2.0],
                    vec![2.0, 6.0],
                    vec![6.0, 6.0],
                    vec![6.0, 2.0],
                    vec![2.0, 2.0],
                ],
            ],
        };
        let mut style = Style::new();
        style.fill = Some(FillStyle {
            color: "#0000ff".to_string(),
            opacity: 1.0,
        });
        style.stroke = None;
        let img = renderer().render(vec![(geom, style)]);
        let (outside_x, outside_y) = renderer().world_to_pixel(1.0, 1.0);
        let (hole_x, hole_y) = renderer().world_to_pixel(4.0, 4.0);
        assert!(!is_transparent(&img, outside_x as u32, outside_y as u32));
        assert!(
            is_transparent(&img, hole_x as u32, hole_y as u32),
            "polygon hole must stay transparent"
        );
    }

    #[test]
    fn test_z_order_polygon_below_point() {
        // A point exactly at the polygon center: the point mark must win.
        let poly = GeoJsonGeometry::Polygon {
            coordinates: vec![vec![
                vec![0.0, 0.0],
                vec![0.0, 8.0],
                vec![8.0, 8.0],
                vec![8.0, 0.0],
                vec![0.0, 0.0],
            ]],
        };
        let mut poly_style = Style::new();
        poly_style.fill = Some(FillStyle {
            color: "#0000ff".to_string(),
            opacity: 1.0,
        });
        poly_style.stroke = None;

        let mut point_style = Style::new();
        point_style.mark = Some("square".to_string());
        point_style.point_size = Some(6.0);
        point_style.fill = Some(FillStyle {
            color: "#ff0000".to_string(),
            opacity: 1.0,
        });
        point_style.stroke = None;

        let img = renderer().render(vec![(poly, poly_style), (point(4.0, 4.0), point_style)]);
        let (cx, cy) = renderer().world_to_pixel(4.0, 4.0);
        let px = img.get_pixel(cx as u32, cy as u32).0;
        assert!(px[0] > 200, "point (drawn last) must be on top: {:?}", px);
    }

    #[test]
    fn test_label_rendered_with_collision() {
        let mut style = Style::new();
        style.label = Some(LabelStyle {
            text: "AA".to_string(),
            property: None,
            font_size: 12.0,
            color: "#000000".to_string(),
            halo_color: Some("#ffffff".to_string()),
            halo_radius: 1.0,
        });
        // Two points 1 unit apart in a 10-unit map: labels would overlap, so
        // only the first is drawn.
        let img = renderer().render(vec![
            (point(1.0, 5.0), style.clone()),
            (point(1.1, 5.0), style.clone()),
        ]);
        // The first label's anchor area must be non-transparent.
        let (x1, y1) = renderer().world_to_pixel(1.0, 5.0);
        assert!(!is_transparent(&img, x1 as u32, y1 as u32));
    }

    #[test]
    fn test_blend_pixel_source_over() {
        let mut img = RgbaImage::new(1, 1);
        img.put_pixel(0, 0, Rgba([255, 255, 255, 255]));
        // 50% red over white → pink-ish (127 = 255 * 0.498 rounded).
        blend_pixel(&mut img, 0, 0, [255, 0, 0, 128]);
        let px = img.get_pixel(0, 0).0;
        assert_eq!(px[0], 255);
        assert_eq!(px[1], 127);
        assert_eq!(px[2], 127);
        assert_eq!(px[3], 255);
        // Fully transparent: no change.
        blend_pixel(&mut img, 0, 0, [0, 0, 0, 0]);
        assert_eq!(img.get_pixel(0, 0).0, [255, 127, 127, 255]);
    }

    // ------------------------------------------------------------------
    // SVG style-aware rendering tests.
    // ------------------------------------------------------------------

    #[test]
    fn test_svg_uses_style_colors() {
        let bounds = Bounds::new(0.0, 0.0, 10.0, 10.0);
        let mut style = Style::new();
        style.fill = Some(FillStyle {
            color: "#123456".to_string(),
            opacity: 0.5,
        });
        style.stroke = Some(StrokeStyle {
            color: "#ABCDEF".to_string(),
            width: Some(3.0),
            opacity: 1.0,
            dash_array: None,
        });
        let poly = GeoJsonGeometry::Polygon {
            coordinates: vec![vec![
                vec![1.0, 1.0],
                vec![1.0, 4.0],
                vec![4.0, 4.0],
                vec![4.0, 1.0],
                vec![1.0, 1.0],
            ]],
        };
        let svg = render_to_svg(&[(poly, style)], &bounds, 100, 100);
        // Style colors must appear verbatim; hardcoded CSS classes must not.
        assert!(
            svg.contains("fill=\"#123456\""),
            "svg must carry fill color: {}",
            svg
        );
        assert!(
            svg.contains("stroke=\"#ABCDEF\""),
            "svg must carry stroke color"
        );
        assert!(
            svg.contains("stroke-width=\"3\""),
            "svg must carry stroke width"
        );
        assert!(
            svg.contains("fill-rule=\"evenodd\""),
            "polygon holes use evenodd"
        );
        assert!(!svg.contains("class=\"fp\""), "hardcoded classes removed");
        assert!(svg.starts_with("<?xml"));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn test_svg_label_and_dash() {
        let bounds = Bounds::new(0.0, 0.0, 10.0, 10.0);
        let mut style = Style::new();
        style.stroke = Some(StrokeStyle {
            color: "#010203".to_string(),
            width: Some(2.0),
            opacity: 1.0,
            dash_array: Some(vec![4.0, 2.0]),
        });
        style.label = Some(LabelStyle {
            text: "A&B".to_string(),
            property: None,
            font_size: 14.0,
            color: "#FF0000".to_string(),
            halo_color: Some("#FFFFFF".to_string()),
            halo_radius: 2.0,
        });
        let line = GeoJsonGeometry::LineString {
            coordinates: vec![vec![1.0, 1.0], vec![9.0, 9.0]],
        };
        let svg = render_to_svg(&[(line, style)], &bounds, 100, 100);
        assert!(
            svg.contains("stroke-dasharray=\"4,2\""),
            "dash array present"
        );
        assert!(svg.contains("fill=\"none\""), "lines have no fill");
        // Label text rendered and XML-escaped.
        assert!(svg.contains("A&amp;B"), "label text escaped");
        assert!(svg.contains("<text"), "label element present");
        assert!(
            svg.contains("stroke=\"#FFFFFF\""),
            "halo color as text stroke"
        );
    }
}
