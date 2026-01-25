use serde::{Serialize, Deserialize};
use wasm_bindgen::prelude::*;
use crate::types::{ShapeType, Gradient, LayerStyle};
use crate::brush::StrokePoint;
use base64::{Engine as _, engine::general_purpose};

#[derive(Serialize, Deserialize, Clone)]
pub struct VectorObject {
    pub id: u32,
    pub shape_type: ShapeType,
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub rotation: f64, // in radians
    pub fill: String,
    #[serde(skip)]
    pub fill_gradient: Option<Gradient>,
    pub stroke: String,
    #[serde(skip)]
    pub stroke_gradient: Option<Gradient>,
    pub stroke_width: f64,
    pub opacity: f64,
    pub visible: bool,
    pub locked: bool,
    pub blend_mode: String,
    pub stroke_cap: String,
    pub stroke_join: String,
    pub stroke_dash: Vec<f64>,
    // Layer Styles (FX)
    pub layer_style: LayerStyle,
    // Masking
    pub mask_id: Option<u32>,
    pub is_mask: bool,
    // Shape specific
    pub sides: u32,
    pub inner_radius: f64,
    pub corner_radius: f64,
    pub path_data: String,
    // Brush specific
    pub brush_id: u32,
    pub stroke_points: Vec<StrokePoint>,
    // Text specific
    pub text_content: String,
    pub font_family: String,
    pub font_size: f64,
    pub font_weight: String,
    pub text_align: String,
    pub kerning: f64,
    pub leading: f64,
    pub tracking: f64,
    pub shadow_color: String,
    pub shadow_blur: f64,
    pub shadow_offset_x: f64,
    pub shadow_offset_y: f64,
    // Source rect for cropping
    pub sx: f64,
    pub sy: f64,
    pub sw: f64,
    pub sh: f64,
    pub brightness: f64,
    pub contrast: f64,
    pub saturate: f64,
    pub hue_rotate: f64,
    pub blur: f64,
    pub grayscale: f64,
    pub sepia: f64,
    pub invert: f64,
    #[serde(skip)]
    pub raw_image: Option<Vec<u8>>,
    #[serde(skip)]
    pub raw_rgba: Option<Vec<u8>>,
    pub raw_rgba_width: u32,
    pub raw_rgba_height: u32,
    #[serde(skip)]
    pub image: Option<JsValue>,
    // Grouping
    pub children: Option<Vec<VectorObject>>,
}

impl VectorObject {
    pub fn get_world_bounds(&self) -> (f64, f64, f64, f64) {
        if self.brush_id > 0 && !self.stroke_points.is_empty() {
             let mut min_x = f64::INFINITY;
             let mut min_y = f64::INFINITY;
             let mut max_x = f64::NEG_INFINITY;
             let mut max_y = f64::NEG_INFINITY;
             for p in &self.stroke_points {
                 if p.x < min_x { min_x = p.x; }
                 if p.x > max_x { max_x = p.x; }
                 if p.y < min_y { min_y = p.y; }
                 if p.y > max_y { max_y = p.y; }
             }
             return (min_x - 50.0, min_y - 50.0, max_x + 50.0, max_y + 50.0);
        }

        let hw = self.width / 2.0;
        let hh = self.height / 2.0;
        let corners = [
            (-hw, -hh), (hw, -hh), (hw, hh), (-hw, hh)
        ];
        
        let cos_r = self.rotation.cos();
        let sin_r = self.rotation.sin();
        
        let cx = self.x + hw;
        let cy = self.y + hh;

        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;

        for (px, py) in corners {
            let rx = px * cos_r - py * sin_r;
            let ry = px * sin_r + py * cos_r;
            let wx = cx + rx;
            let wy = cy + ry;
            
            if wx < min_x { min_x = wx; }
            if wx > max_x { max_x = wx; }
            if wy < min_y { min_y = wy; }
            if wy > max_y { max_y = wy; }
        }
        (min_x, min_y, max_x, max_y)
    }

    pub fn to_svg_element(&self, defs: &mut Vec<String>) -> String {
        if !self.visible { return String::new(); }

        let mut attrs = Vec::new();
        let transform = format!("translate({} {}) rotate({}) translate({} {})",
            self.x + self.width / 2.0, 
            self.y + self.height / 2.0,
            self.rotation.to_degrees(),
            -self.width / 2.0,
            -self.height / 2.0
        );

        attrs.push(format!(r##"transform="{}"##, transform));
        
        if self.opacity < 1.0 {
            attrs.push(format!(r##"opacity="{}"##, self.opacity));
        }
        
        if self.blend_mode != "source-over" {
            attrs.push(format!(r##"style="mix-blend-mode: {}"##, self.blend_mode));
        }

        // Fill
        if let Some(grad) = &self.fill_gradient {
            let grad_id = format!("grad_fill_{}", self.id);
            let mut grad_svg = if grad.is_radial {
                format!(r##"<radialGradient id="{}" cx="{}" cy="{}" r="{}" fx="{}" fy="{}" gradientUnits="userSpaceOnUse">"##,
                    grad_id, grad.x1, grad.y1, grad.r2, grad.x1, grad.y1)
            } else {
                format!(r##"<linearGradient id="{}" x1="{}" y1="{}" x2="{}" y2="{}" gradientUnits="userSpaceOnUse">"##,
                    grad_id, grad.x1, grad.y1, grad.x2, grad.y2)
            };
            for stop in &grad.stops {
                grad_svg.push_str(&format!(r##"<stop offset="{}" stop-color="{}" />"##, stop.offset, stop.color));
            }
            if grad.is_radial {
                grad_svg.push_str("</radialGradient>");
            } else {
                grad_svg.push_str("</linearGradient>");
            }
            defs.push(grad_svg);
            attrs.push(format!(r##"fill="url(#{})"##, grad_id));
        } else {
            let fill = if self.fill == "transparent" { "none".to_string() } else if self.fill.is_empty() { "none".to_string() } else { self.fill.clone() };
            attrs.push(format!(r##"fill="{}"##, fill));
        }

        // Stroke
        if self.stroke_width > 0.0 && self.stroke != "transparent" && !self.stroke.is_empty() {
            if let Some(grad) = &self.stroke_gradient {
                let grad_id = format!("grad_stroke_{}", self.id);
                let mut grad_svg = if grad.is_radial {
                    format!(r##"<radialGradient id="{}" cx="{}" cy="{}" r="{}" fx="{}" fy="{}" gradientUnits="userSpaceOnUse">"##,
                        grad_id, grad.x1, grad.y1, grad.r2, grad.x1, grad.y1)
                } else {
                    format!(r##"<linearGradient id="{}" x1="{}" y1="{}" x2="{}" y2="{}" gradientUnits="userSpaceOnUse">"##,
                        grad_id, grad.x1, grad.y1, grad.x2, grad.y2)
                };
                for stop in &grad.stops {
                    grad_svg.push_str(&format!(r##"<stop offset="{}" stop-color="{}" />"##, stop.offset, stop.color));
                }
                if grad.is_radial {
                    grad_svg.push_str("</radialGradient>");
                } else {
                    grad_svg.push_str("</linearGradient>");
                }
                defs.push(grad_svg);
                attrs.push(format!(r##"stroke="url(#{})"##, grad_id));
            } else {
                attrs.push(format!(r##"stroke="{}"##, self.stroke));
            }
            attrs.push(format!(r##"stroke-width="{}"##, self.stroke_width));
            attrs.push(format!(r##"stroke-linecap="{}"##, self.stroke_cap));
            attrs.push(format!(r##"stroke-linejoin="{}"##, self.stroke_join));
            if !self.stroke_dash.is_empty() {
                let dash = self.stroke_dash.iter().map(|d| d.to_string()).collect::<Vec<_>>().join(" ");
                attrs.push(format!(r##"stroke-dasharray="{}"##, dash));
            }
        } else {
            attrs.push(r##"stroke="none""##.to_string());
        }

        let attr_str = attrs.join(" ");

        match self.shape_type {
            ShapeType::Rectangle => {
                if self.corner_radius > 0.0 {
                    format!(r##"<rect width="{}" height="{}" rx="{}" ry="{}" {} />"##, 
                        self.width, self.height, self.corner_radius, self.corner_radius, attr_str)
                } else {
                    format!(r##"<rect width="{}" height="{}" {} />"##, 
                        self.width, self.height, attr_str)
                }
            }
            ShapeType::Circle | ShapeType::Ellipse => {
                format!(r##"<ellipse cx="{}" cy="{}" rx="{}" ry="{}" {} />"##, 
                    self.width / 2.0, self.height / 2.0, self.width / 2.0, self.height / 2.0, attr_str)
            }
            ShapeType::Path => {
                format!(r##"<path d="{}" {} />"##, self.path_data, attr_str)
            }
            ShapeType::Polygon => {
                let mut points = Vec::new();
                let cx = self.width / 2.0;
                let cy = self.height / 2.0;
                let r = self.width / 2.0;
                for i in 0..self.sides {
                    let angle = (i as f64 * 2.0 * std::f64::consts::PI / self.sides as f64) - (std::f64::consts::PI / 2.0);
                    let x = cx + r * angle.cos();
                    let y = cy + r * angle.sin();
                    points.push(format!("{},{}", x, y));
                }
                format!(r##"<polygon points="{}" {} />"##, points.join(" "), attr_str)
            }
            ShapeType::Star => {
                let mut points = Vec::new();
                let cx = self.width / 2.0;
                let cy = self.height / 2.0;
                let r_outer = self.width / 2.0;
                let r_inner = self.inner_radius * (self.width / 2.0);
                for i in 0..(self.sides * 2) {
                    let r = if i % 2 == 0 { r_outer } else { r_inner };
                    let angle = (i as f64 * std::f64::consts::PI / self.sides as f64) - (std::f64::consts::PI / 2.0);
                    let x = cx + r * angle.cos();
                    let y = cy + r * angle.sin();
                    points.push(format!("{},{}", x, y));
                }
                format!(r##"<polygon points="{}" {} />"##, points.join(" "), attr_str)
            }
            ShapeType::Text => {
                format!(r##"<text x="0" y="{}" font-family="{}" font-size="{}" font-weight="{}" text-anchor="{}" {}>{}</text>"##,
                    self.font_size, self.font_family, self.font_size, self.font_weight, 
                    if self.text_align == "left" { "start" } else if self.text_align == "right" { "end" } else { "middle" },
                    attr_str, self.text_content)
            }
            ShapeType::Group => {
                let mut inner = String::new();
                if let Some(children) = &self.children {
                    for child in children {
                        inner.push_str(&child.to_svg_element(defs));
                    }
                }
                format!(r##"<g {}>{}</g>"##, attr_str, inner)
            }
            ShapeType::Image => {
                if let Some(raw_image) = &self.raw_image {
                    let b64 = general_purpose::STANDARD.encode(raw_image);
                    format!(r##"<image width="{}" height="{}" href="data:image/png;base64,{}" {} />"##,
                        self.width, self.height, b64, attr_str)
                } else {
                    format!(r##"<rect width="{}" height="{}" fill="#ccc" {} />"##, self.width, self.height, attr_str)
                }
            }
            _ => String::new()
        }
    }
}
