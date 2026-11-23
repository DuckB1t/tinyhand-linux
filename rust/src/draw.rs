use std::cell::RefCell;
use std::collections::HashMap;
use std::f64::consts::PI;
use std::fs;
use std::path::PathBuf;

use cairo::{Context, FontSlant, FontWeight, ImageSurface, Operator};
use image::imageops::FilterType;
use serde::Deserialize;

use crate::catalog::hex_rgb;
use crate::paths;

pub type Color = (f64, f64, f64, f64);

pub const WHITE: Color = (1.0, 1.0, 1.0, 1.0);
pub const SECONDARY: Color = (1.0, 1.0, 1.0, 0.62);
pub const RED: Color = (1.0, 0.26, 0.24, 1.0);
pub const ORANGE: Color = (1.0, 0.58, 0.12, 1.0);
pub const BLUE: Color = (0.15, 0.52, 0.98, 1.0);
pub const PURPLE: Color = (0.55, 0.42, 0.95, 1.0);

#[derive(Clone, Debug, Deserialize)]
pub struct Geometry {
    pub aspect: f64,
    pub file: String,
    pub hand_max_x: i32,
    pub hand_min_x: i32,
    pub hand_top: i32,
    pub rod_end: f64,
    pub rod_x: f64,
    pub size: [i32; 2],
    pub tip_x: f64,
    pub wrist: i32,
}

#[derive(Default)]
struct SurfaceCache {
    map: HashMap<String, ImageSurface>,
    order: Vec<String>,
}

pub struct HandRenderer {
    geometry: HashMap<String, Geometry>,
    thumbs: RefCell<HashMap<(String, i32), Option<ImageSurface>>>,
    surfaces: RefCell<SurfaceCache>,
}

impl HandRenderer {
    pub fn new() -> Self {
        let mut geometry = HashMap::new();
        let path = paths::asset_dir().join("manifest.json");
        if let Ok(text) = fs::read_to_string(&path) {
            if let Ok(map) = serde_json::from_str::<HashMap<String, Geometry>>(&text) {
                geometry = map;
            }
        }
        Self { geometry, thumbs: RefCell::new(HashMap::new()), surfaces: RefCell::new(SurfaceCache::default()) }
    }

    pub fn geometry(&self, id: &str) -> Option<&Geometry> {
        self.geometry.get(id)
    }

    fn asset_path(&self, id: &str) -> Option<PathBuf> {
        let geom = self.geometry.get(id)?;
        Some(paths::asset_dir().join(&geom.file))
    }

    fn surface_for(&self, id: &str) -> Option<ImageSurface> {
        if let Some(surface) = self.surfaces.borrow().map.get(id) {
            return Some(surface.clone());
        }
        let path = self.asset_path(id)?;
        let bytes = cairo_bytes(&path)?;
        let geom = self.geometry.get(id)?;
        let surface = ImageSurface::create_for_data(
            bytes, cairo::Format::ARgb32, geom.size[0], geom.size[1], geom.size[0] * 4,
        ).ok()?;
        let mut cache = self.surfaces.borrow_mut();
        cache.map.insert(id.to_string(), surface.clone());
        cache.order.retain(|x| x != id);
        cache.order.push(id.to_string());
        while cache.order.len() > 3 {
            let oldest = cache.order.remove(0);
            if oldest != id {
                cache.map.remove(&oldest);
            }
        }
        Some(surface)
    }

    pub fn thumbnail_for(&self, id: &str, size: i32) -> Option<ImageSurface> {
        let key = (id.to_string(), size);
        if let Some(value) = self.thumbs.borrow().get(&key) {
            return value.clone();
        }
        let result = self.make_thumbnail(id, size);
        self.thumbs.borrow_mut().insert(key, result.clone());
        result
    }

    fn make_thumbnail(&self, id: &str, size: i32) -> Option<ImageSurface> {
        let geom = self.geometry.get(id)?;
        let path = self.asset_path(id)?;
        let image = image::open(&path).ok()?.to_rgba8();
        let (width, height) = (image.width() as i32, image.height() as i32);
        let top = geom.hand_top.max(0) as u32;
        let bottom = ((geom.wrist + 1).min(height) as u32).max(top);
        let mut min_x = width as u32;
        let mut max_x = 0u32;
        for y in top..bottom {
            for x in 0..width as u32 {
                if image.get_pixel(x, y).0[3] > 128 {
                    if x < min_x { min_x = x; }
                    if x > max_x { max_x = x; }
                }
            }
        }
        if min_x > max_x {
            return None;
        }
        let vertical = (geom.wrist - geom.hand_top + 1).max(0) as f64;
        let side = ((max_x - min_x + 1) as f64).max(vertical) * 1.06;
        let cx = (min_x + max_x) as f64 / 2.0;
        let cy = (geom.hand_top + geom.wrist) as f64 / 2.0;
        let left = (cx - side / 2.0).max(0.0) as i32;
        let upper = (cy - side / 2.0).max(0.0) as i32;
        let right = (left as f64 + side).min(width as f64) as i32;
        let lower = (upper as f64 + side).min(height as f64) as i32;
        if right <= left || lower <= upper {
            return None;
        }
        let crop = image::imageops::crop_imm(&image, left as u32, upper as u32, (right - left) as u32, (lower - upper) as u32).to_image();
        let resized = image::imageops::resize(&crop, size as u32, size as u32, FilterType::Lanczos3);
        let data = premultiply_bgra(resized.as_raw());
        ImageSurface::create_for_data(data, cairo::Format::ARgb32, size, size, size * 4).ok()
    }

    pub fn draw(
        &self,
        ctx: &Context,
        style_id: &str,
        cursor: (f64, f64),
        grip: (f64, f64),
        hand_len: f64,
        press: f64,
        show_shadow: bool,
    ) {
        if style_id == "laser" {
            draw_laser(ctx, cursor, hand_len, press);
            return;
        }
        let Some(surface) = self.surface_for(style_id) else {
            draw_laser(ctx, cursor, hand_len, press);
            return;
        };
        let Some(geom) = self.geometry.get(style_id) else {
            draw_laser(ctx, cursor, hand_len, press);
            return;
        };

        let dx = cursor.0 - grip.0;
        let dy = cursor.1 - grip.1;
        let length = dx.hypot(dy).max(1.0);
        let ux = dx / length;
        let uy = dy / length;
        let base = (cursor.0 - ux * hand_len, cursor.1 - uy * hand_len);
        let scale = hand_len / (geom.wrist - geom.hand_top).max(1) as f64;
        let src_vx = geom.rod_x - geom.tip_x;
        let src_vy = (geom.wrist - geom.hand_top) as f64;
        let target_vx = base.0 - cursor.0;
        let target_vy = base.1 - cursor.1;
        let angle = target_vy.atan2(target_vx) - src_vy.atan2(src_vx);
        let lift = 0.06 * (hand_len / 0.13).max(1.0) * (1.0 - 0.8 * press);
        let shadow = if show_shadow { (0.15 * lift, -0.25 * lift) } else { (0.0, 0.0) };
        let width = (54.0 * scale).max(8.0);

        let _ = ctx.save();
        if show_shadow {
            ctx.set_source_rgba(0.0, 0.0, 0.0, 0.22);
            ctx.set_line_width(width + 5.0);
            line(ctx, (grip.0 + shadow.0, grip.1 + shadow.1), (base.0 + shadow.0, base.1 + shadow.1));
        }
        ctx.set_source_rgba(0.04, 0.04, 0.05, 1.0);
        ctx.set_line_width(width + 3.0);
        line(ctx, grip, base);
        ctx.set_source_rgba(0.25, 0.25, 0.28, 1.0);
        ctx.set_line_width(width);
        line(ctx, grip, base);
        ctx.set_source_rgba(0.55, 0.55, 0.58, 0.75);
        ctx.set_line_width((width * 0.16).max(1.0));
        line(
            ctx,
            (grip.0 - width * 0.22, grip.1),
            (base.0 - width * 0.22, base.1),
        );
        let _ = ctx.restore();

        if show_shadow {
            paint_crop(ctx, &surface, geom, (cursor.0 + shadow.0, cursor.1 + shadow.1), angle, scale, 0.26);
        }
        paint_crop(ctx, &surface, geom, cursor, angle, scale, 1.0);
    }
}

fn line(ctx: &Context, a: (f64, f64), b: (f64, f64)) {
    ctx.move_to(a.0, a.1);
    ctx.line_to(b.0, b.1);
    ctx.stroke().ok();
}

fn paint_crop(ctx: &Context, surface: &ImageSurface, geom: &Geometry, cursor: (f64, f64), angle: f64, scale: f64, alpha: f64) {
    let _ = ctx.save();
    ctx.translate(cursor.0, cursor.1);
    ctx.rotate(angle);
    ctx.scale(scale, scale);
    ctx.translate(-geom.tip_x, -(geom.hand_top as f64));
    ctx.rectangle(0.0, geom.hand_top as f64, geom.size[0] as f64, (geom.wrist - geom.hand_top) as f64);
    ctx.clip();
    if alpha >= 0.999 {
        let _ = ctx.set_source_surface(surface, 0.0, 0.0);
        let _ = ctx.paint();
    } else {
        ctx.push_group();
        let _ = ctx.set_source_surface(surface, 0.0, 0.0);
        let _ = ctx.paint();
        if let Ok(group) = ctx.pop_group() {
            let _ = ctx.set_source(&group);
            let _ = ctx.paint_with_alpha(alpha);
        }
    }
    let _ = ctx.restore();
}

fn premultiply_bgra(rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(rgba.len());
    for pixel in rgba.chunks_exact(4) {
        let a = pixel[3] as u16;
        let multiply = |value: u8| -> u8 { ((value as u16 * a + 127) / 255) as u8 };
        out.push(multiply(pixel[2]));
        out.push(multiply(pixel[1]));
        out.push(multiply(pixel[0]));
        out.push(pixel[3]);
    }
    out
}

fn cairo_bytes(path: &PathBuf) -> Option<Vec<u8>> {
    let image = image::open(path).ok()?.to_rgba8();
    Some(premultiply_bgra(image.as_raw()))
}

pub fn draw_laser(ctx: &Context, cursor: (f64, f64), hand_len: f64, press: f64) {
    let radius = hand_len * 0.027;
    let radius = radius.max(4.0) * (1.0 + 0.2 * press);
    let (r, g, b) = hex_rgb("#FF2920");
    let _ = ctx.save();
    ctx.set_source_rgba(r, g, b, 0.22);
    circle(ctx, cursor.0, cursor.1, radius * 2.8);
    ctx.set_source_rgba(r, g, b, 0.55);
    circle(ctx, cursor.0, cursor.1, radius * 1.7);
    ctx.set_source_rgba(r, g, b, 1.0);
    circle(ctx, cursor.0, cursor.1, radius);
    ctx.set_source_rgba(1.0, 0.86, 0.82, 0.95);
    circle(ctx, cursor.0, cursor.1, radius * 0.42);
    let _ = ctx.restore();
}

pub fn circle(ctx: &Context, cx: f64, cy: f64, radius: f64) {
    ctx.new_path();
    ctx.arc(cx, cy, radius, 0.0, 2.0 * PI);
    let _ = ctx.fill();
}

pub fn ellipse(ctx: &Context, cx: f64, cy: f64, rx: f64, ry: f64) {
    let _ = ctx.save();
    ctx.translate(cx, cy);
    ctx.scale(rx, ry);
    ctx.arc(0.0, 0.0, 1.0, 0.0, 2.0 * PI);
    let _ = ctx.restore();
}

pub fn rounded_rect(ctx: &Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    let r = r.max(0.0).min(w / 2.0).min(h / 2.0);
    ctx.new_path();
    ctx.arc(x + r, y + r, r, PI, 1.5 * PI);
    ctx.arc(x + w - r, y + r, r, 1.5 * PI, 2.0 * PI);
    ctx.arc(x + w - r, y + h - r, r, 0.0, 0.5 * PI);
    ctx.arc(x + r, y + h - r, r, 0.5 * PI, PI);
    let _ = ctx.close_path();
}

pub fn panel(ctx: &Context, x: f64, y: f64, w: f64, h: f64, radius: f64, alpha: f64) {
    rounded_rect(ctx, x, y, w, h, radius);
    ctx.set_source_rgba(0.055, 0.06, 0.075, alpha);
    let _ = ctx.fill_preserve();
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.12);
    ctx.set_line_width(1.0);
    let _ = ctx.stroke();
}

pub fn text(ctx: &Context, x: f64, y: f64, value: &str, size: f64, color: Color, bold: bool, align: &str) -> f64 {
    let weight = if bold { FontWeight::Bold } else { FontWeight::Normal };
    ctx.select_font_face("Sans", FontSlant::Normal, weight);
    ctx.set_font_size(size);
    let width = ctx.text_extents(value).map(|e| e.width()).unwrap_or(0.0);
    let mut x = x;
    if align == "center" {
        x -= width / 2.0;
    } else if align == "right" {
        x -= width;
    }
    ctx.set_source_rgba(color.0, color.1, color.2, color.3);
    ctx.move_to(x, y);
    let _ = ctx.show_text(value);
    width
}

pub fn measure_text(ctx: &Context, value: &str, size: f64, bold: bool) -> (f64, f64) {
    let weight = if bold { FontWeight::Bold } else { FontWeight::Normal };
    ctx.select_font_face("Sans", FontSlant::Normal, weight);
    ctx.set_font_size(size);
    ctx.text_extents(value).map(|e| (e.width(), e.height())).unwrap_or((0.0, 0.0))
}

pub fn ink_dot(ctx: &Context, cx: f64, cy: f64, colour: &str, radius: f64) {
    let _ = ctx.save();
    ctx.set_source_rgba(0.0, 0.0, 0.0, 0.35);
    ellipse(ctx, cx, cy + 1.5, radius + 1.5, radius + 1.5);
    let _ = ctx.fill();
    let (r, g, b) = hex_rgb(colour);
    ctx.set_source_rgb(r, g, b);
    ctx.arc(cx, cy, radius, 0.0, 2.0 * PI);
    let _ = ctx.fill();
    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.35);
    ellipse(ctx, cx - radius * 0.2, cy - radius * 0.4, radius * 0.35, radius * 0.25);
    let _ = ctx.fill();
    let _ = ctx.restore();
}

pub fn rainbow_dot(ctx: &Context, cx: f64, cy: f64, radius: f64) {
    let _ = ctx.save();
    ctx.set_source_rgba(0.0, 0.0, 0.0, 0.35);
    ctx.new_path();
    ctx.arc(cx, cy + 1.5, radius + 1.5, 0.0, 2.0 * PI);
    let _ = ctx.fill();
    for (i, colour) in ["#FF3B30", "#FF9500", "#FFCC00", "#34C759", "#0A84FF", "#AF52DE"].iter().enumerate() {
        let a0 = PI / 2.0 + i as f64 * PI / 3.0;
        let a1 = a0 + PI / 3.0;
        let (r, g, b) = hex_rgb(colour);
        ctx.set_source_rgb(r, g, b);
        ctx.move_to(cx, cy);
        ctx.arc(cx, cy, radius, a0, a1);
        let _ = ctx.close_path();
        let _ = ctx.fill();
    }
    let _ = ctx.restore();
}

pub fn laser_icon(ctx: &Context, cx: f64, cy: f64, size: f64) {
    let r = size / 2.0;
    ctx.set_source_rgba(1.0, 0.16, 0.12, 0.35);
    circle(ctx, cx, cy, r * 1.6);
    ctx.set_source_rgba(1.0, 0.16, 0.12, 1.0);
    circle(ctx, cx, cy, r * 0.65);
    ctx.set_source_rgba(1.0, 0.86, 0.82, 1.0);
    circle(ctx, cx, cy, r * 0.28);
}

pub fn app_icon(ctx: &Context, x: f64, y: f64, w: f64, h: f64) {
    rounded_rect(ctx, x, y, w, h, (w * 0.22).min(16.0));
    ctx.set_source_rgba(0.10, 0.0, 0.12, 1.0);
    let _ = ctx.fill();
    laser_icon(ctx, x + w / 2.0, y + h / 2.0, w * 0.45);
}

fn polygon(ctx: &Context, points: &[(f64, f64)], s: f64) {
    if points.is_empty() { return; }
    ctx.move_to(points[0].0 * s, points[0].1 * s);
    for point in &points[1..] {
        ctx.line_to(point.0 * s, point.1 * s);
    }
}

pub fn icon(ctx: &Context, name: &str, cx: f64, cy: f64, size: f64, color: Color) {
    let _ = ctx.save();
    ctx.translate(cx, cy);
    ctx.set_source_rgba(color.0, color.1, color.2, color.3);
    ctx.set_line_width((1.2_f64).max(size * 0.09));
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);
    let s = size / 2.0;
    match name {
        "speaker" => {
            polygon(ctx, &[(-0.7, -0.3), (-0.25, -0.3), (0.1, -0.65), (0.1, 0.65), (-0.25, 0.3), (-0.7, 0.3)], s);
            let _ = ctx.close_path();
            let _ = ctx.fill();
            for radius in [0.45, 0.72] {
                ctx.new_path();
                ctx.arc(0.1 * s, 0.0, radius * s, -0.85, 0.85);
                let _ = ctx.stroke();
            }
        }
        "speaker_slash" => {
            polygon(ctx, &[(-0.7, -0.3), (-0.25, -0.3), (0.1, -0.65), (0.1, 0.65), (-0.25, 0.3), (-0.7, 0.3)], s);
            let _ = ctx.close_path();
            let _ = ctx.fill();
            line(ctx, (-0.8 * s, 0.8 * s), (0.85 * s, -0.85 * s));
        }
        "x" => {
            line(ctx, (-0.5 * s, -0.5 * s), (0.5 * s, 0.5 * s));
            line(ctx, (0.5 * s, -0.5 * s), (-0.5 * s, 0.5 * s));
        }
        "plus" => {
            line(ctx, (0.0, -0.6 * s), (0.0, 0.6 * s));
            line(ctx, (-0.6 * s, 0.0), (0.6 * s, 0.0));
        }
        "gear" => {
            ctx.new_path();
            ctx.arc(0.0, 0.0, 0.3 * s, 0.0, 2.0 * PI);
            let _ = ctx.stroke();
            for i in 0..8 {
                let a = i as f64 * PI / 4.0;
                line(ctx, (a.cos() * 0.48 * s, a.sin() * 0.48 * s), (a.cos() * 0.78 * s, a.sin() * 0.78 * s));
            }
        }
        "chevron_left" => {
            polygon(ctx, &[(0.25, -0.5), (-0.35, 0.0), (0.25, 0.5)], s);
            let _ = ctx.stroke();
        }
        "check" => {
            polygon(ctx, &[(-0.55, 0.0), (-0.1, 0.45), (0.6, -0.5)], s);
            let _ = ctx.stroke();
        }
        "arrow" => {
            line(ctx, (-0.5 * s, 0.5 * s), (0.55 * s, -0.55 * s));
            polygon(ctx, &[(0.05, -0.55), (0.55, -0.55), (0.55, -0.05)], s);
            let _ = ctx.stroke();
        }
        "waveform" => {
            for (i, height) in [0.25, 0.55, 0.9, 0.5, 0.75, 0.35].iter().enumerate() {
                let x = (-0.7 + i as f64 * 0.28) * s;
                line(ctx, (x, -height * s), (x, height * s));
            }
        }
        "sparkles" => {
            sparkle(ctx, -0.25, -0.15, 0.35, s);
            sparkle(ctx, 0.35, 0.2, 0.25, s);
        }
        "pencil" => {
            polygon(ctx, &[(-0.55, 0.35), (0.15, -0.55), (0.65, -0.05), (-0.05, 0.65)], s);
            let _ = ctx.stroke();
        }
        "hand" => {
            for (x, top) in [(-0.2, -0.4), (0.0, -0.7), (0.25, -0.45), (0.6, -0.15)] {
                line(ctx, (x * s, 0.65 * s), (x * s, top * s));
            }
            polygon(ctx, &[(-0.55, 0.2), (-0.35, 0.65), (0.55, 0.65)], s);
            let _ = ctx.close_path();
            let _ = ctx.stroke();
        }
        "info" => {
            ctx.new_path();
            ctx.arc(0.0, 0.0, 0.75 * s, 0.0, 2.0 * PI);
            let _ = ctx.stroke();
            line(ctx, (0.0, -0.35 * s), (0.0, 0.1 * s));
            circle(ctx, 0.0, 0.45 * s, 0.8);
        }
        "warning" => {
            polygon(ctx, &[(0.0, -0.7), (0.8, 0.65), (-0.8, 0.65)], s);
            let _ = ctx.stroke();
            line(ctx, (0.0, -0.15 * s), (0.0, 0.25 * s));
            circle(ctx, 0.0, 0.48 * s, 0.9);
        }
        _ => {
            ctx.new_path();
            ctx.arc(0.0, 0.0, 0.6 * s, 0.0, 2.0 * PI);
            let _ = ctx.stroke();
        }
    }
    let _ = ctx.restore();
}

fn sparkle(ctx: &Context, ox: f64, oy: f64, radius: f64, s: f64) {
    let points = [
        (ox, oy - radius),
        (ox + radius * 0.3, oy - radius * 0.3),
        (ox + radius, oy),
        (ox + radius * 0.3, oy + radius * 0.3),
        (ox, oy + radius),
        (ox - radius * 0.3, oy + radius * 0.3),
        (ox - radius, oy),
        (ox - radius * 0.3, oy - radius * 0.3),
    ];
    ctx.new_path();
    for (i, (x, y)) in points.iter().enumerate() {
        let px = x * s;
        let py = y * s;
        if i == 0 { ctx.move_to(px, py); } else { ctx.line_to(px, py); }
    }
    let _ = ctx.close_path();
    let _ = ctx.stroke();
}

pub fn draw_stroke(ctx: &Context, points: &[(f64, f64)], color: (f64, f64, f64), width: f64, alpha: f64, arrow: bool) {
    if points.len() < 2 { return; }
    let _ = ctx.save();
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);

    ctx.set_source_rgba(0.0, 0.0, 0.0, 0.18 * alpha);
    ctx.set_line_width(width + 3.0);
    stroke_path(ctx, points);
    ctx.set_source_rgba(color.0, color.1, color.2, 0.98 * alpha);
    ctx.set_line_width(width);
    stroke_path(ctx, points);

    if arrow {
        let end = *points.last().unwrap();
        let mut back = points[points.len() - 2];
        let size = (14.0_f64).max(width * 3.2);
        for candidate in points[..points.len() - 1].iter().rev() {
            if (candidate.0 - end.0).hypot(candidate.1 - end.1) >= size * 1.2 {
                back = *candidate;
                break;
            }
        }
        let angle = (end.1 - back.1).atan2(end.0 - back.0);
        let length = size.min((back.0 - end.0).hypot(back.1 - end.1) * 0.8);
        for side in [-1.0, 1.0] {
            let a = angle + PI + side * PI / 7.0;
            ctx.move_to(end.0, end.1);
            ctx.line_to(end.0 + a.cos() * length, end.1 + a.sin() * length);
        }
        let _ = ctx.stroke();
    }
    let _ = ctx.restore();
}

fn stroke_path(ctx: &Context, points: &[(f64, f64)]) {
    ctx.new_path();
    ctx.move_to(points[0].0, points[0].1);
    if points.len() == 2 {
        ctx.line_to(points[1].0, points[1].1);
    } else {
        let mut previous = points[0];
        for i in 1..points.len() - 1 {
            let control = points[i];
            let mid = ((points[i].0 + points[i + 1].0) / 2.0, (points[i].1 + points[i + 1].1) / 2.0);
            let cp1 = (previous.0 + (control.0 - previous.0) * 2.0 / 3.0, previous.1 + (control.1 - previous.1) * 2.0 / 3.0);
            let cp2 = (mid.0 + (control.0 - mid.0) * 2.0 / 3.0, mid.1 + (control.1 - mid.1) * 2.0 / 3.0);
            ctx.curve_to(cp1.0, cp1.1, cp2.0, cp2.1, mid.0, mid.1);
            previous = mid;
        }
        let last = points[points.len() - 1];
        ctx.line_to(last.0, last.1);
    }
    let _ = ctx.stroke();
}

pub fn clear_surface(ctx: &Context) {
    ctx.set_operator(Operator::Clear);
    let _ = ctx.paint();
    ctx.set_operator(Operator::Over);
}
