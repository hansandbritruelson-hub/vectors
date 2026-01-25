use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};
use web_sys::{HtmlImageElement, Path2d};
mod psd;
mod ai;
use psd::Psd;
use ai::AiParser;
use image::{ImageOutputFormat, DynamicImage, RgbaImage};
use std::io::Cursor;
use base64::{Engine as _, engine::general_purpose};
use kurbo::{BezPath, Shape, Affine, Point};
use roxmltree;

mod tracer;
mod brush;

use tracer::Tracer;
use brush::{BrushEngine, StrokePoint};

#[wasm_bindgen]
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
pub enum ShapeType {
    Rectangle,
    Circle,
    Ellipse,
    Star,
    Polygon,
    Image,
    Path,
    Text,
    Group,
    Adjustment,
    Guide,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
pub enum HandleType {
    TopLeft, TopRight, BottomLeft, BottomRight,
    Top, Bottom, Left, Right,
    Rotate,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GradientStop {
    pub offset: f64, // 0.0 to 1.0
    pub color: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Gradient {
    pub is_radial: bool,
    pub x1: f64, pub y1: f64, // Start point (or center for radial)
    pub x2: f64, pub y2: f64, // End point (or radius point for radial)
    pub r1: f64, // Inner radius (radial only)
    pub r2: f64, // Outer radius (radial only)
    pub stops: Vec<GradientStop>,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
pub enum EffectType {
    DropShadow,
    InnerShadow,
    OuterGlow,
    InnerGlow,
    BevelEmboss,
    ColorOverlay,
    GradientOverlay,
    PatternOverlay,
    Stroke,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LayerEffect {
    pub effect_type: EffectType,
    pub enabled: bool,
    pub color: String,
    pub opacity: f64,
    pub blur: f64,
    pub x: f64,
    pub y: f64,
    pub size: f64,
    pub spread: f64,
    pub blend_mode: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct LayerStyle {
    pub effects: Vec<LayerEffect>,
}

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
    pub fill_gradient: Option<Gradient>, // New: Gradient support
    pub stroke: String,
    #[serde(skip)]
    pub stroke_gradient: Option<Gradient>, // New: Gradient support
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
    pub children: Option<Vec<VectorObject>>, // New: Grouping support
}

impl VectorObject {
    fn get_world_bounds(&self) -> (f64, f64, f64, f64) {
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
             // Add padding for brush size (max default size is around 100)
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

        // Transform: first move to center, then rotate, then move back
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
            ShapeType::Adjustment | ShapeType::Guide => {
                String::new()
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Guide {
    pub orientation: String, // "horizontal" or "vertical"
    pub position: f64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Artboard {
    pub width: f64,
    pub height: f64,
    pub background: String,
    pub guides: Vec<Guide>,
}

#[derive(Clone)]
struct EngineState {
    objects: Vec<VectorObject>,
    next_id: u32,
    selected_ids: Vec<u32>,
    artboard: Artboard,
    clip_to_artboard: bool,
    action_name: String,
}

#[wasm_bindgen]
pub struct VectorEngine {
    objects: Vec<VectorObject>,
    next_id: u32,
    selected_ids: Vec<u32>,
    pub viewport_x: f64,
    pub viewport_y: f64,
    pub viewport_zoom: f64,
    artboard: Artboard,
    pub clip_to_artboard: bool,
    pub hide_selection: bool,
    undo_stack: Vec<EngineState>,
    redo_stack: Vec<EngineState>,
    brush_engine: BrushEngine,
    brush_image_map: std::collections::HashMap<String, HtmlImageElement>,
}

#[wasm_bindgen]
impl VectorEngine {
    #[wasm_bindgen(constructor)]
    pub fn new() -> VectorEngine {
        console_error_panic_hook::set_once();
        
        VectorEngine {
            objects: Vec::new(),
            next_id: 1,
            selected_ids: Vec::new(),
            viewport_x: 0.0,
            viewport_y: 0.0,
            viewport_zoom: 1.0,
            artboard: Artboard {
                width: 800.0,
                height: 600.0,
                background: "#ffffff".to_string(),
                guides: Vec::new(),
            },
            clip_to_artboard: false,
            hide_selection: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            brush_engine: BrushEngine::new(),
            brush_image_map: std::collections::HashMap::new(),
        }
    }

    fn save_state(&mut self, action_name: &str) {
        let state = EngineState {
            objects: self.objects.clone(),
            next_id: self.next_id,
            selected_ids: self.selected_ids.clone(),
            artboard: self.artboard.clone(),
            clip_to_artboard: self.clip_to_artboard,
            action_name: action_name.to_string(),
        };
        self.undo_stack.push(state);
        if self.undo_stack.len() > 100 {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    pub fn undo(&mut self) -> bool {
        if let Some(prev_state) = self.undo_stack.pop() {
            let current_state = EngineState {
                objects: self.objects.clone(),
                next_id: self.next_id,
                selected_ids: self.selected_ids.clone(),
                artboard: self.artboard.clone(),
                clip_to_artboard: self.clip_to_artboard,
                action_name: "Redo State".to_string(),
            };
            self.redo_stack.push(current_state);

            self.objects = prev_state.objects;
            self.next_id = prev_state.next_id;
            self.selected_ids = prev_state.selected_ids;
            self.artboard = prev_state.artboard;
            self.clip_to_artboard = prev_state.clip_to_artboard;
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        if let Some(next_state) = self.redo_stack.pop() {
            let current_state = EngineState {
                objects: self.objects.clone(),
                next_id: self.next_id,
                selected_ids: self.selected_ids.clone(),
                artboard: self.artboard.clone(),
                clip_to_artboard: self.clip_to_artboard,
                action_name: "Undo State".to_string(),
            };
            self.undo_stack.push(current_state);

            self.objects = next_state.objects;
            self.next_id = next_state.next_id;
            self.selected_ids = next_state.selected_ids;
            self.artboard = next_state.artboard;
            self.clip_to_artboard = next_state.clip_to_artboard;
            true
        } else {
            false
        }
    }

    pub fn set_viewport(&mut self, x: f64, y: f64, zoom: f64) {
        self.viewport_x = x;
        self.viewport_y = y;
        self.viewport_zoom = zoom;
    }

    pub fn get_history(&self) -> String {
        let history: Vec<String> = self.undo_stack.iter().map(|s| s.action_name.clone()).collect();
        serde_json::to_string(&history).unwrap_or("[]".to_string())
    }

    pub fn execute_command(&mut self, cmd_json: &str) -> String {
        web_sys::console::log_1(&format!("Rust: execute_command: {}", cmd_json).into());
        #[derive(Deserialize)]
        struct Command {
            action: String,
            params: serde_json::Value,
        }

        let cmd: Command = match serde_json::from_str(cmd_json) {
            Ok(c) => c,
            Err(e) => return format!("{{\"error\": \"Invalid JSON: {}\"}}", e),
        };

        match cmd.action.as_str() {
            "magic_wand" => {
                let id = cmd.params["id"].as_u64().map(|v| v as u32).unwrap_or(0);
                let x = cmd.params["x"].as_f64().unwrap_or(0.0);
                let y = cmd.params["y"].as_f64().unwrap_or(0.0);
                let tolerance = cmd.params["tolerance"].as_f64().unwrap_or(30.0);
                
                if let Some(obj) = self.objects.iter().find(|o| o.id == id) {
                    if let Some(rgba) = &obj.raw_rgba {
                        let width = obj.raw_rgba_width;
                        let height = obj.raw_rgba_height;
                        
                        // Map world coords to local image pixels
                        let local_x = ((x - obj.x) / obj.width * width as f64) as i32;
                        let local_y = ((y - obj.y) / obj.height * height as f64) as i32;
                        
                        if local_x >= 0 && local_x < width as i32 && local_y >= 0 && local_y < height as i32 {
                            let start_idx = (local_y as u32 * width + local_x as u32) as usize * 4;
                            let start_r = rgba[start_idx];
                            let start_g = rgba[start_idx + 1];
                            let start_b = rgba[start_idx + 2];
                            
                            // Simple flood fill to find mask
                            let mut mask = vec![false; (width * height) as usize];
                            let mut stack = vec![(local_x, local_y)];
                            mask[(local_y as u32 * width + local_x as u32) as usize] = true;
                            
                            while let Some((cx, cy)) = stack.pop() {
                                for (dx, dy) in [(0, 1), (0, -1), (1, 0), (-1, 0)] {
                                    let nx = cx + dx;
                                    let ny = cy + dy;
                                    if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                                        let idx = (ny as u32 * width + nx as u32) as usize;
                                        if !mask[idx] {
                                            let p_idx = idx * 4;
                                            let dist = ((rgba[p_idx] as f64 - start_r as f64).powi(2) +
                                                       (rgba[p_idx+1] as f64 - start_g as f64).powi(2) +
                                                       (rgba[p_idx+2] as f64 - start_b as f64).powi(2)).sqrt();
                                            if dist <= tolerance {
                                                mask[idx] = true;
                                                stack.push((nx, ny));
                                            }
                                        }
                                    }
                                }
                            }
                            
                            // Convert mask to a Path (simplified: just return the count for now or a path)
                            // Real implementation would use the Tracer to convert mask to SVG path
                            let tracer = Tracer::new(width, height);
                            // We need a grayscale image for tracer, so we use our mask
                            let mut mask_img = vec![0u8; (width * height) as usize];
                            for i in 0..mask.len() {
                                if mask[i] { mask_img[i] = 255; }
                            }
                            let luma = image::ImageBuffer::<image::Luma<u8>, Vec<u8>>::from_raw(width, height, mask_img).unwrap();
                            let mut path_data = tracer.trace(&luma, 128);
                            
                            // Scale path to object size
                            if let Ok(mut bez) = BezPath::from_svg(&path_data) {
                                let sx = obj.width / width as f64;
                                let sy = obj.height / height as f64;
                                bez.apply_affine(Affine::scale_non_uniform(sx, sy));
                                path_data = bez.to_svg();
                            }

                            let new_id = self.add_object(ShapeType::Path, obj.x, obj.y, obj.width, obj.height, "#4facfe");
                            self.update_object(new_id, &serde_json::json!({
                                "path_data": path_data,
                                "fill": "rgba(79, 172, 254, 0.3)",
                                "stroke": "#4facfe",
                                "stroke_width": 1.0,
                                "name": "Selection Mask"
                            }));
                            
                            return format!("{{\"success\": true, \"id\": {}}}", new_id);
                        }
                    }
                }
                "{\"error\": \"Image not found or click outside image\"}".to_string()
            }
            "add_guide" => {
                let orientation = cmd.params["orientation"].as_str().unwrap_or("horizontal").to_string();
                let position = cmd.params["position"].as_f64().unwrap_or(0.0);
                self.artboard.guides.push(Guide { orientation, position });
                "{\"success\": true}".to_string()
            }
            "clear_guides" => {
                self.artboard.guides.clear();
                "{\"success\": true}".to_string()
            }
            "get_history" => self.get_history(),
            "boolean_operation" => {
                self.save_state("Boolean Operation");
                let op = cmd.params["operation"].as_str().unwrap_or("union");
                let ids: Vec<u32> = cmd.params["ids"].as_array()
                    .unwrap_or(&vec![])
                    .iter()
                    .filter_map(|v| v.as_u64().map(|id| id as u32))
                    .collect();
                
                if ids.len() < 2 {
                    return "{\"error\": \"At least 2 objects required\"}".to_string();
                }

                // Placeholder for real boolean operations using a library like clipper2
                // For now, we'll just log and return error until we add the dependency
                "{\"error\": \"Boolean operations require additional dependencies (clipper2)\"}".to_string()
            }
            "add" => {
                self.save_state("Add Object");
                let st = match cmd.params["type"].as_str() {
                    Some("Circle") => ShapeType::Circle,
                    Some("Ellipse") => ShapeType::Ellipse,
                    Some("Star") => ShapeType::Star,
                    Some("Polygon") => ShapeType::Polygon,
                    Some("Image") => ShapeType::Image,
                    Some("Path") => ShapeType::Path,
                    Some("Text") => ShapeType::Text,
                    Some("Group") => ShapeType::Group,
                    _ => ShapeType::Rectangle,
                };
                let id = self.add_object(
                    st,
                    cmd.params["x"].as_f64().unwrap_or(0.0),
                    cmd.params["y"].as_f64().unwrap_or(0.0),
                    cmd.params["width"].as_f64().unwrap_or(100.0),
                    cmd.params["height"].as_f64().unwrap_or(100.0),
                    cmd.params["fill"].as_str().unwrap_or("#4facfe"),
                );
                // Apply optional params immediately
                self.update_object(id, &cmd.params);
                format!("{{\"success\": true, \"id\": {}}}", id)
            }
            "update" => {
                if cmd.params["save_undo"].as_bool().unwrap_or(false) {
                    self.save_state("Update Object");
                }
                let mut success = false;
                if let Some(ids) = cmd.params["ids"].as_array() {
                    for id_val in ids {
                        if let Some(id) = id_val.as_u64() {
                            if self.update_object(id as u32, &cmd.params) {
                                success = true;
                            }
                        }
                    }
                } else {
                    let id = cmd.params["id"].as_u64().map(|v| v as u32).unwrap_or(0);
                    if self.update_object(id, &cmd.params) {
                        success = true;
                    }
                }
                
                if success {
                    "{\"success\": true}".to_string()
                } else {
                    "{\"error\": \"Object(s) not found\"}".to_string()
                }
            }
            "delete" => {
                if cmd.params["save_undo"].as_bool().unwrap_or(true) {
                    self.save_state("Delete Object");
                }
                let mut success = false;
                if let Some(ids) = cmd.params["ids"].as_array() {
                    for id_val in ids {
                        if let Some(id) = id_val.as_u64() {
                            if self.delete_object(id as u32) {
                                success = true;
                            }
                        }
                    }
                } else {
                    let id = cmd.params["id"].as_u64().map(|v| v as u32).unwrap_or(0);
                    if self.delete_object(id) {
                        success = true;
                    }
                }
                if success {
                    "{\"success\": true}".to_string()
                } else {
                    "{\"error\": \"Object(s) not found\"}".to_string()
                }
            }
            "duplicate" => {
                self.save_state("Duplicate Object");
                let id = cmd.params["id"].as_u64().map(|v| v as u32).unwrap_or(0);
                if let Some(pos) = self.objects.iter().position(|o| o.id == id) {
                    let mut new_obj = self.objects[pos].clone();
                    new_obj.id = self.next_id;
                    self.next_id += 1;
                    new_obj.x += 10.0;
                    new_obj.y += 10.0;
                    new_obj.name = format!("{} copy", new_obj.name);
                    let new_id = new_obj.id;
                    self.objects.insert(pos + 1, new_obj);
                    self.selected_ids = vec![new_id];
                    format!("{{\"success\": true, \"id\": {}}}", new_id)
                } else {
                    "{\"error\": \"Object not found\"}".to_string()
                }
            }
            "select" => {
                if let Some(ids) = cmd.params["ids"].as_array() {
                    self.selected_ids = ids.iter().filter_map(|v| v.as_u64().map(|id| id as u32)).collect();
                } else if let Some(id) = cmd.params["id"].as_u64() {
                    self.selected_ids = vec![id as u32];
                } else {
                    self.selected_ids.clear();
                }
                "{\"success\": true}".to_string()
            }
            "move_to_back" => {
                self.save_state("Move to Back");
                let id = cmd.params["id"].as_u64().map(|v| v as u32).unwrap_or(0);
                if let Some(pos) = self.objects.iter().position(|o| o.id == id) {
                    let obj = self.objects.remove(pos);
                    self.objects.insert(0, obj);
                    "{\"success\": true}".to_string()
                } else {
                    "{\"error\": \"Object not found\"}".to_string()
                }
            }
            "move_to_front" => {
                self.save_state("Move to Front");
                let id = cmd.params["id"].as_u64().map(|v| v as u32).unwrap_or(0);
                if let Some(pos) = self.objects.iter().position(|o| o.id == id) {
                    let obj = self.objects.remove(pos);
                    self.objects.push(obj);
                    "{\"success\": true}".to_string()
                } else {
                    "{\"error\": \"Object not found\"}".to_string()
                }
            }
            "move_forward" => {
                self.save_state("Move Forward");
                let id = cmd.params["id"].as_u64().map(|v| v as u32).unwrap_or(0);
                if let Some(pos) = self.objects.iter().position(|o| o.id == id) {
                    if pos < self.objects.len() - 1 {
                        self.objects.swap(pos, pos + 1);
                        "{\"success\": true}".to_string()
                    } else {
                        "{\"success\": true, \"message\": \"Already at front\"}".to_string()
                    }
                } else {
                    "{\"error\": \"Object not found\"}".to_string()
                }
            }
            "move_backward" => {
                self.save_state("Move Backward");
                let id = cmd.params["id"].as_u64().map(|v| v as u32).unwrap_or(0);
                if let Some(pos) = self.objects.iter().position(|o| o.id == id) {
                    if pos > 0 {
                        self.objects.swap(pos, pos - 1);
                        "{\"success\": true}".to_string()
                    } else {
                        "{\"success\": true, \"message\": \"Already at back\"}".to_string()
                    }
                } else {
                    "{\"error\": \"Object not found\"}".to_string()
                }
            }
            "set_artboard" => {
                self.save_state("Set Artboard");
                if let Some(w) = cmd.params["width"].as_f64() { self.artboard.width = w; }
                if let Some(h) = cmd.params["height"].as_f64() { self.artboard.height = h; }
                if let Some(bg) = cmd.params["background"].as_str() { self.artboard.background = bg.to_string(); }
                "{\"success\": true}".to_string()
            }
            "set_clipping" => {
                self.save_state("Set Clipping");
                if let Some(v) = cmd.params["enabled"].as_bool() { self.clip_to_artboard = v; }
                "{\"success\": true}".to_string()
            }
            "vectorize" => {
                if cmd.params["save_undo"].as_bool().unwrap_or(true) {
                    self.save_state("Vectorize Image");
                }
                let id = cmd.params["id"].as_u64().map(|v| v as u32).unwrap_or(0);
                let threshold = cmd.params["threshold"].as_f64().unwrap_or(128.0) as u8;
                
                let obj_info = if let Some(obj) = self.objects.iter().find(|o| o.id == id) {
                    if let Some(raw_image) = &obj.raw_image {
                        Some((obj.x, obj.y, obj.width, obj.height, obj.name.clone(), raw_image.clone()))
                    } else { None }
                } else { None };

                if let Some((ox, oy, ow, oh, oname, bytes)) = obj_info {
                    web_sys::console::log_1(&"Rust: Vectorizing image...".into());
                    if let Ok(img) = image::load_from_memory(&bytes) {
                        let grayscale = img.to_luma8();
                        let (width, height) = grayscale.dimensions();
                        web_sys::console::log_1(&format!("Rust: Image decoded, size: {}x{}", width, height).into());
                        
                        let tracer = Tracer::new(width, height);
                        let mut path_data = tracer.trace(&grayscale, threshold);
                        web_sys::console::log_1(&format!("Rust: Trace complete, path length: {}", path_data.len()).into());

                        if !path_data.is_empty() {
                            // Scale path from image pixels to object dimensions
                            if let Ok(mut bez) = BezPath::from_svg(&path_data) {
                                let sx = ow / width as f64;
                                let sy = oh / height as f64;
                                bez.apply_affine(Affine::scale_non_uniform(sx, sy));
                                path_data = bez.to_svg();
                            }

                            let new_id = self.add_object(
                                ShapeType::Path,
                                ox,
                                oy,
                                ow,
                                oh,
                                "#000000"
                            );
                            self.update_object(new_id, &serde_json::json!({
                                "path_data": path_data,
                                "name": format!("Traced {}", oname),
                                "fill": "transparent",
                                "stroke": "#000000",
                                "stroke_width": 1.0
                            }));
                            format!("{{\"success\": true, \"id\": {}}}", new_id)
                        } else {
                            "{{\"error\": \"No path generated\"}}".to_string()
                        }
                    } else { "{{\"error\": \"Failed to load image\"}}".to_string() }
                } else { "{{\"error\": \"Object not found or no raw image data\"}}".to_string() }
            }
            "clear" => {
                self.save_state("Clear Document");
                self.objects.clear();
                self.next_id = 1;
                self.selected_ids.clear();
                "{\"success\": true}".to_string()
            }
            "get_brushes" => {
                serde_json::to_string(&self.brush_engine.brushes).unwrap_or("[]".to_string())
            }
            "update_brush" => {
                if let Ok(updated_brush) = serde_json::from_value::<brush::Brush>(cmd.params.clone()) {
                    if let Some(brush) = self.brush_engine.brushes.iter_mut().find(|b| b.id == updated_brush.id) {
                        *brush = updated_brush;
                        "{\"success\": true}".to_string()
                    } else {
                        "{\"error\": \"Brush not found\"}".to_string()
                    }
                } else {
                    "{\"error\": \"Invalid brush JSON\"}".to_string()
                }
            }
            "create_brush_stroke" => {
                if cmd.params["save_undo"].as_bool().unwrap_or(true) {
                    self.save_state("Brush Stroke");
                }
                let brush_id = cmd.params["brush_id"].as_u64().unwrap_or(1) as u32;
                let color = cmd.params["color"].as_str().unwrap_or("#000000");
                
                let points_val = &cmd.params["points"];
                if let Some(points_arr) = points_val.as_array() {
                    let mut stroke_points = Vec::new();
                    let mut path = BezPath::new();
                    
                    for (i, p) in points_arr.iter().enumerate() {
                        let pt = Point::new(
                            p["x"].as_f64().unwrap_or(0.0),
                            p["y"].as_f64().unwrap_or(0.0),
                        );
                        if i == 0 { path.move_to(pt); } else { path.line_to(pt); }
                        
                        stroke_points.push(StrokePoint {
                            x: pt.x,
                            y: pt.y,
                            pressure: p["pressure"].as_f64().unwrap_or(1.0),
                        });
                    }
                    
                    let bbox = path.bounding_box();
                    let mut path_relative = path.clone();
                    path_relative.apply_affine(Affine::translate((-bbox.x0, -bbox.y0)));
                    
                    let stroke_points_relative: Vec<StrokePoint> = stroke_points.iter().map(|p| StrokePoint {
                        x: p.x - bbox.x0,
                        y: p.y - bbox.y0,
                        pressure: p.pressure
                    }).collect();

                    let id = self.add_object(
                        ShapeType::Path,
                        bbox.x0, bbox.y0, bbox.width().max(1.0), bbox.height().max(1.0),
                        color
                    );
                    
                    self.update_object(id, &serde_json::json!({
                        "brush_id": brush_id,
                        "stroke_points": stroke_points_relative,
                        "path_data": path_relative.to_svg(),
                        "fill": color,
                        "name": format!("Brush Stroke {}", id)
                    }));
                    
                    format!("{{\"success\": true, \"id\": {}}}", id)
                } else {
                    "{\"error\": \"Missing points array\"}".to_string()
                }
            }
            "update_brush_stroke" => {
                let id = cmd.params["id"].as_u64().unwrap_or(0) as u32;
                let points_val = &cmd.params["points"];
                if let Some(points_arr) = points_val.as_array() {
                    let mut stroke_points = Vec::new();
                    let mut path = BezPath::new();

                    for (i, p) in points_arr.iter().enumerate() {
                        let pt = Point::new(
                            p["x"].as_f64().unwrap_or(0.0),
                            p["y"].as_f64().unwrap_or(0.0),
                        );
                        if i == 0 { path.move_to(pt); } else { path.line_to(pt); }

                        stroke_points.push(StrokePoint {
                            x: pt.x,
                            y: pt.y,
                            pressure: p["pressure"].as_f64().unwrap_or(1.0),
                        });
                    }
                    
                    let bbox = path.bounding_box();
                    let mut path_relative = path.clone();
                    path_relative.apply_affine(Affine::translate((-bbox.x0, -bbox.y0)));

                    let stroke_points_relative: Vec<StrokePoint> = stroke_points.iter().map(|p| StrokePoint {
                        x: p.x - bbox.x0,
                        y: p.y - bbox.y0,
                        pressure: p.pressure
                    }).collect();

                    if self.update_object(id, &serde_json::json!({ 
                        "stroke_points": stroke_points_relative,
                        "path_data": path_relative.to_svg(),
                        "x": bbox.x0,
                        "y": bbox.y0,
                        "width": bbox.width().max(1.0),
                        "height": bbox.height().max(1.0)
                    })) {
                        "{\"success\": true}".to_string()
                    } else {
                        "{\"error\": \"Object not found\"}".to_string()
                    }
                } else {
                    "{\"error\": \"Missing points array\"}".to_string()
                }
            }
            _ => format!("{{\"error\": \"Unknown action: {}\"}}", cmd.action),
        }
    }

    pub fn register_brush(&mut self, brush_json: &str) -> u32 {
        if let Ok(mut brush) = serde_json::from_str::<brush::Brush>(brush_json) {
            let id = self.brush_engine.brushes.iter().map(|b| b.id).max().unwrap_or(0) + 1;
            brush.id = id;
            self.brush_engine.brushes.push(brush);
            id
        } else {
            0
        }
    }

    pub fn register_brush_tip(&mut self, id: &str, image: HtmlImageElement) {
        self.brush_image_map.insert(id.to_string(), image);
    }

    pub fn import_abr(&mut self, data: &[u8]) -> String {
        "{\"error\": \"ABR parsing not yet implemented\"}".to_string()
    }

    pub fn get_artboard(&self) -> String {
        serde_json::to_string(&self.artboard).unwrap_or("{}".to_string())
    }

    pub fn export_svg(&self) -> String {
        let mut defs = Vec::new();
        let mut body = String::new();

        for obj in &self.objects {
            body.push_str(&obj.to_svg_element(&mut defs));
        }

        let defs_str = if defs.is_empty() {
            String::new()
        } else {
            format!("<defs>{}</defs>", defs.join(""))
        };

        format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {} {}"><rect width="100%" height="100%" fill="{}" />{}{}</svg>"##,
            self.artboard.width, self.artboard.height, self.artboard.width, self.artboard.height,
            self.artboard.background,
            defs_str,
            body
        )
    }

    pub fn import_svg(&mut self, data: &[u8]) -> String {
        let svg_str = String::from_utf8_lossy(data);
        let doc = match roxmltree::Document::parse(&svg_str) {
            Ok(d) => d,
            Err(e) => return format!("{{\"error\": \"Failed to parse SVG: {:?}\"}}", e),
        };

        let root = doc.root_element();
        let mut width = root.attribute("width").and_then(|s| s.parse::<f64>().ok()).unwrap_or(800.0);
        let mut height = root.attribute("height").and_then(|s| s.parse::<f64>().ok()).unwrap_or(600.0);

        if let Some(viewbox) = root.attribute("viewBox") {
            let parts: Vec<f64> = viewbox.split_whitespace().filter_map(|s| s.parse::<f64>().ok()).collect();
            if parts.len() == 4 {
                width = parts[2];
                height = parts[3];
            }
        }

        let mut objects = Vec::new();
        let mut next_id = self.next_id;

        self.parse_svg_node(root, &mut objects, &mut next_id);

        self.next_id = next_id;
        
        let result = serde_json::json!({
            "width": width,
            "height": height,
            "objects": objects
        });

        result.to_string()
    }

    fn parse_svg_node(&self, node: roxmltree::Node, objects: &mut Vec<VectorObject>, next_id: &mut u32) {
        for child in node.children() {
            if !child.is_element() { continue; }

            match child.tag_name().name() {
                "rect" => {
                    let x = child.attribute("x").and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                    let y = child.attribute("y").and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                    let w = child.attribute("width").and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                    let h = child.attribute("height").and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                    let rx = child.attribute("rx").and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                    
                    let mut obj = self.create_default_object(*next_id, ShapeType::Rectangle, x, y, w, h);
                    obj.corner_radius = rx;
                    obj.name = format!("Rectangle {}", *next_id);
                    self.apply_svg_styles(child, &mut obj);
                    objects.push(obj);
                    *next_id += 1;
                }
                "circle" | "ellipse" => {
                    let cx = child.attribute("cx").and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                    let cy = child.attribute("cy").and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                    let (rx, ry) = if child.tag_name().name() == "circle" {
                        let r = child.attribute("r").and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                        (r, r)
                    } else {
                        let rx = child.attribute("rx").and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                        let ry = child.attribute("ry").and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                        (rx, ry)
                    };
                    
                    let mut obj = self.create_default_object(*next_id, ShapeType::Circle, cx - rx, cy - ry, rx * 2.0, ry * 2.0);
                    obj.name = format!("{} {}", if child.tag_name().name() == "circle" { "Circle" } else { "Ellipse" }, *next_id);
                    self.apply_svg_styles(child, &mut obj);
                    objects.push(obj);
                    *next_id += 1;
                }
                "line" => {
                    let x1 = child.attribute("x1").and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                    let y1 = child.attribute("y1").and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                    let x2 = child.attribute("x2").and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                    let y2 = child.attribute("y2").and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                    
                    let mut bez = BezPath::new();
                    bez.move_to(Point::new(x1, y1));
                    bez.line_to(Point::new(x2, y2));
                    
                    let bbox = bez.bounding_box();
                    let mut obj = self.create_default_object(*next_id, ShapeType::Path, bbox.x0, bbox.y0, bbox.width().max(1.0), bbox.height().max(1.0));
                    obj.name = format!("Line {}", *next_id);
                    
                    let mut normalized = bez.clone();
                    normalized.apply_affine(Affine::translate((-bbox.x0, -bbox.y0)));
                    obj.path_data = normalized.to_svg();
                    
                    self.apply_svg_styles(child, &mut obj);
                    objects.push(obj);
                    *next_id += 1;
                }
                "polyline" | "polygon" => {
                    let points_str = child.attribute("points").unwrap_or("");
                    let points: Vec<f64> = points_str.split(|c: char| !c.is_digit(10) && c != '.' && c != '-')
                        .filter(|s| !s.is_empty())
                        .filter_map(|s| s.parse::<f64>().ok())
                        .collect();
                    
                    if points.len() >= 4 {
                        let mut bez = BezPath::new();
                        bez.move_to(Point::new(points[0], points[1]));
                        for i in (2..points.len()).step_by(2) {
                            if i + 1 < points.len() {
                                bez.line_to(Point::new(points[i], points[i+1]));
                            }
                        }
                        if child.tag_name().name() == "polygon" {
                            bez.close_path();
                        }
                        
                        let bbox = bez.bounding_box();
                        let mut obj = self.create_default_object(*next_id, ShapeType::Path, bbox.x0, bbox.y0, bbox.width().max(1.0), bbox.height().max(1.0));
                        obj.name = format!("{} {}", if child.tag_name().name() == "polygon" { "Polygon" } else { "Polyline" }, *next_id);
                        
                        let mut normalized = bez.clone();
                        normalized.apply_affine(Affine::translate((-bbox.x0, -bbox.y0)));
                        obj.path_data = normalized.to_svg();
                        
                        self.apply_svg_styles(child, &mut obj);
                        objects.push(obj);
                        *next_id += 1;
                    }
                }
                "path" => {
                    let d = child.attribute("d").unwrap_or("").to_string();
                    if let Ok(bez) = BezPath::from_svg(&d) {
                        let bbox = bez.bounding_box();
                        let mut obj = self.create_default_object(*next_id, ShapeType::Path, bbox.x0, bbox.y0, bbox.width(), bbox.height());
                        obj.name = format!("Path {}", *next_id);
                        
                        // Normalize path data to be relative to object x,y
                        let mut normalized = bez.clone();
                        normalized.apply_affine(Affine::translate((-bbox.x0, -bbox.y0)));
                        obj.path_data = normalized.to_svg();
                        
                        self.apply_svg_styles(child, &mut obj);
                        objects.push(obj);
                        *next_id += 1;
                    }
                }
                "g" => {
                    // Flatten groups for now, but apply group styles
                    self.parse_svg_node(child, objects, next_id);
                }
                _ => {
                    // Recurse for other nodes like <svg> inside <svg> or <defs> (though we skip defs content for now)
                    if child.tag_name().name() != "defs" && child.tag_name().name() != "style" {
                        self.parse_svg_node(child, objects, next_id);
                    }
                }
            }
        }
    }

    fn apply_svg_styles(&self, node: roxmltree::Node, obj: &mut VectorObject) {
        let mut fill_val = node.attribute("fill");
        let mut stroke_val = node.attribute("stroke");
        let mut stroke_width_val = node.attribute("stroke-width");
        let mut opacity_val = node.attribute("opacity");

        if let Some(style) = node.attribute("style") {
            for part in style.split(';') {
                let kv: Vec<&str> = part.split(':').collect();
                if kv.len() == 2 {
                    match kv[0].trim() {
                        "fill" => fill_val = Some(kv[1].trim()),
                        "stroke" => stroke_val = Some(kv[1].trim()),
                        "stroke-width" => stroke_width_val = Some(kv[1].trim()),
                        "opacity" => opacity_val = Some(kv[1].trim()),
                        _ => {}
                    }
                }
            }
        }

        if let Some(fill) = fill_val {
            if fill != "none" { obj.fill = fill.to_string(); }
            else { obj.fill = "transparent".to_string(); }
        }
        if let Some(stroke) = stroke_val {
            if stroke != "none" { obj.stroke = stroke.to_string(); }
            else { obj.stroke = "transparent".to_string(); }
        }
        if let Some(sw) = stroke_width_val {
            obj.stroke_width = sw.parse::<f64>().unwrap_or(obj.stroke_width);
        }
        if let Some(op) = opacity_val {
            obj.opacity = op.parse::<f64>().unwrap_or(obj.opacity);
        }

        if let Some(transform) = node.attribute("transform") {
            if transform.contains("translate") {
                let start = transform.find("translate(").unwrap() + 10;
                let end = transform[start..].find(')').unwrap() + start;
                let parts: Vec<f64> = transform[start..end].split(|c: char| !c.is_digit(10) && c != '.' && c != '-')
                    .filter(|s| !s.is_empty())
                    .filter_map(|s| s.parse::<f64>().ok())
                    .collect();
                if !parts.is_empty() {
                    obj.x += parts[0];
                    if parts.len() > 1 { obj.y += parts[1]; }
                }
            }
            if transform.contains("rotate") {
                let start = transform.find("rotate(").unwrap() + 7;
                let end = transform[start..].find(')').unwrap() + start;
                let parts: Vec<f64> = transform[start..end].split(|c: char| !c.is_digit(10) && c != '.' && c != '-')
                    .filter(|s| !s.is_empty())
                    .filter_map(|s| s.parse::<f64>().ok())
                    .collect();
                if !parts.is_empty() {
                    obj.rotation = parts[0].to_radians();
                }
            }
        }
    }

    fn create_default_object(&self, id: u32, shape_type: ShapeType, x: f64, y: f64, width: f64, height: f64) -> VectorObject {
        VectorObject {
            id,
            shape_type,
            name: format!("{:?} {}", shape_type, id),
            x, y, width, height,
            rotation: 0.0,
            fill: "#000000".to_string(),
            stroke: "transparent".to_string(),
            stroke_width: 0.0,
            opacity: 1.0,
            visible: true, locked: false,
            blend_mode: "source-over".to_string(),
            stroke_cap: "butt".to_string(),
            stroke_join: "miter".to_string(),
            stroke_dash: Vec::new(),
            layer_style: LayerStyle::default(),
            mask_id: None,
            is_mask: false,
            sides: 4, inner_radius: 0.0, corner_radius: 0.0,
            path_data: String::new(),
            brush_id: 0, stroke_points: Vec::new(),
            text_content: String::new(), font_family: "Inter, sans-serif".to_string(), font_size: 24.0, font_weight: "normal".to_string(), text_align: "left".to_string(),
            kerning: 0.0, leading: 1.2, tracking: 0.0,
            shadow_color: "transparent".to_string(), shadow_blur: 0.0, shadow_offset_x: 0.0, shadow_offset_y: 0.0,
            sx: 0.0, sy: 0.0, sw: width.max(1.0), sh: height.max(1.0),
            brightness: 1.0, contrast: 1.0, saturate: 1.0, hue_rotate: 0.0, blur: 0.0, grayscale: 0.0, sepia: 0.0, invert: 0.0,
            raw_image: None, raw_rgba: None, raw_rgba_width: 0, raw_rgba_height: 0, image: None,
            fill_gradient: None, stroke_gradient: None, children: None,
        }
    }

    pub fn import_file(&mut self, filename: &str, data: &[u8]) -> String {
        let filename_lower = filename.to_lowercase();
        if filename_lower.ends_with(".psd") {
            self.import_psd(data)
        } else if filename_lower.ends_with(".ai") {
            self.import_ai(data)
        } else if filename_lower.ends_with(".svg") {
            self.import_svg(data)
        } else {
            "{\"error\": \"Unsupported file format\"}".to_string()
        }
    }

    fn import_psd(&mut self, data: &[u8]) -> String {
        let psd = match Psd::from_bytes(data) {
            Ok(p) => p,
            Err(e) => return format!("{{\"error\": \"Failed to parse PSD: {:?}\"}}", e),
        };

        let width = psd.width() as u32;
        let height = psd.height() as u32;
        let layers = psd.layers();
        
        web_sys::console::log_1(&format!("PSD loaded: {}x{}, layers: {}, mode: {:?}", width, height, layers.len(), psd.color_mode()).into());

        let mut imported_objects = Vec::new();
        let mut group_stack: Vec<Vec<serde_json::Value>> = vec![Vec::new()];

        // 1. ALWAYS add the composite image first (as the bottom layer)
        let composite_rgba = psd.rgba();
        if let Some(img_buffer) = RgbaImage::from_raw(width, height, composite_rgba.clone()) {
            let dyn_img = DynamicImage::ImageRgba8(img_buffer);
            let mut png_bytes: Vec<u8> = Vec::new();
            if let Ok(_) = dyn_img.write_to(&mut Cursor::new(&mut png_bytes), ImageOutputFormat::Png) {
                let id = self.next_id;
                self.next_id += 1;

                let obj = self.create_default_object(id, ShapeType::Image, 0.0, 0.0, width as f64, height as f64);
                let mut obj = obj;
                obj.name = "Composite View".to_string();
                obj.locked = true;
                obj.raw_image = Some(png_bytes.clone());
                obj.raw_rgba = Some(composite_rgba.clone());
                obj.raw_rgba_width = width;
                obj.raw_rgba_height = height;
                
                self.objects.push(obj.clone());
                
                let b64 = general_purpose::STANDARD.encode(&png_bytes);
                let data_url = format!("data:image/png;base64,{}", b64);
                let mut obj_json = serde_json::to_value(&obj).unwrap();
                obj_json["image_data_url"] = serde_json::Value::String(data_url);
                imported_objects.push(obj_json);
            }
        }

        // 2. Iterating layers
        for layer in layers.iter() {
            let l_width = layer.width() as u32;
            let l_height = layer.height() as u32;
            let name = layer.name().to_string();
            let l_x = layer.layer_left() as f64;
            let l_y = layer.layer_top() as f64;
            let opacity = layer.opacity() as f64 / 255.0;
            let visible = layer.visible();

            let blend_mode = match layer.blend_mode() {
                 "Normal" => "source-over",
                 "Multiply" => "multiply",
                 "Screen" => "screen",
                 "Overlay" => "overlay",
                 "Darken" => "darken",
                 "Lighten" => "lighten",
                 "ColorDodge" => "color-dodge",
                 "ColorBurn" => "color-burn",
                 "HardLight" => "hard-light",
                 "SoftLight" => "soft-light",
                 "Difference" => "difference",
                 "Exclusion" => "exclusion",
                 "Hue" => "hue",
                 "Saturation" => "saturation",
                 "Color" => "color",
                 "Luminosity" => "luminosity",
                 _ => "source-over",
            }.to_string();

            match layer.layer_type() {
                psd::PsdLayerType::FolderOpen | psd::PsdLayerType::FolderClosed => {
                    group_stack.push(Vec::new());
                    // We store the group info in a temporary object if we wanted to build a real tree
                    // For now, we'll just keep track of depth
                }
                psd::PsdLayerType::SectionDivider => {
                    if group_stack.len() > 1 {
                        let children = group_stack.pop().unwrap();
                        // Flatten for now or handle group object
                    }
                }
                psd::PsdLayerType::Normal => {
                    if l_width > 0 && l_height > 0 {
                        let rgba = layer.rgba();
                        if let Some(img_buffer) = RgbaImage::from_raw(l_width, l_height, rgba.clone()) {
                            let dyn_img = DynamicImage::ImageRgba8(img_buffer);
                            let mut png_bytes: Vec<u8> = Vec::new();
                            if let Ok(_) = dyn_img.write_to(&mut Cursor::new(&mut png_bytes), ImageOutputFormat::Png) {
                                let id = self.next_id;
                                self.next_id += 1;

                                let mut obj = self.create_default_object(id, ShapeType::Image, l_x, l_y, l_width as f64, l_height as f64);
                                obj.name = name;
                                obj.opacity = opacity;
                                obj.visible = visible;
                                obj.blend_mode = blend_mode;
                                obj.raw_image = Some(png_bytes.clone());
                                obj.raw_rgba = Some(rgba.clone());
                                obj.raw_rgba_width = l_width;
                                obj.raw_rgba_height = l_height;
                                
                                self.objects.push(obj.clone());
                                
                                let b64 = general_purpose::STANDARD.encode(&png_bytes);
                                let data_url = format!("data:image/png;base64,{}", b64);
                                let mut obj_json = serde_json::to_value(&obj).unwrap();
                                obj_json["image_data_url"] = serde_json::Value::String(data_url);
                                imported_objects.push(obj_json);
                            }
                        }
                    }
                }
            }
        }
        
        let response = serde_json::json!({
            "width": width,
            "height": height,
            "objects": imported_objects
        });

        serde_json::to_string(&response).unwrap_or("{\"error\": \"Serialization failed\"}".to_string())
    }

    pub fn export_psd(&self) -> Vec<u8> {
        let mut layers = Vec::new();
        for obj in &self.objects {
            if let Some(rgba) = &obj.raw_rgba {
                layers.push(psd::PsdLayer {
                    name: obj.name.clone(),
                    top: obj.y as i32,
                    left: obj.x as i32,
                    bottom: (obj.y + obj.height) as i32,
                    right: (obj.x + obj.width) as i32,
                    width: obj.raw_rgba_width,
                    height: obj.raw_rgba_height,
                    opacity: (obj.opacity * 255.0) as u8,
                    visible: obj.visible,
                    blend_mode: match obj.blend_mode.as_str() {
                        "multiply" => "Multiply".to_string(),
                        "screen" => "Screen".to_string(),
                        "overlay" => "Overlay".to_string(),
                        "darken" => "Darken".to_string(),
                        "lighten" => "Lighten".to_string(),
                        "color-dodge" => "ColorDodge".to_string(),
                        "color-burn" => "ColorBurn".to_string(),
                        "hard-light" => "HardLight".to_string(),
                        "soft-light" => "SoftLight".to_string(),
                        "difference" => "Difference".to_string(),
                        "exclusion" => "Exclusion".to_string(),
                        "hue" => "Hue".to_string(),
                        "saturation" => "Saturation".to_string(),
                        "color" => "Color".to_string(),
                        "luminosity" => "Luminosity".to_string(),
                        _ => "Normal".to_string(),
                    },
                    rgba: rgba.clone(),
                    layer_type: psd::PsdLayerType::Normal,
                });
            }
        }

        let total_pixels = (self.artboard.width * self.artboard.height) as usize;
        let mut composite_rgba = vec![255u8; total_pixels * 4];
        // Fill background color
        if let Ok(color) = u32::from_str_radix(self.artboard.background.trim_start_matches('#'), 16) {
            let r = ((color >> 16) & 0xff) as u8;
            let g = ((color >> 8) & 0xff) as u8;
            let b = (color & 0xff) as u8;
            for i in 0..total_pixels {
                composite_rgba[i * 4] = r;
                composite_rgba[i * 4 + 1] = g;
                composite_rgba[i * 4 + 2] = b;
                composite_rgba[i * 4 + 3] = 255;
            }
        }

        let psd = psd::Psd {
            width: self.artboard.width as u32,
            height: self.artboard.height as u32,
            layers,
            composite_rgba,
            color_mode: psd::ColorMode::Rgb,
        };

        psd.to_bytes().unwrap_or_default()
    }

    pub fn export_ai(&self) -> Vec<u8> {
        ai::Ai::export(self.artboard.width, self.artboard.height, &self.objects)
    }

    fn import_ai(&mut self, data: &[u8]) -> String {
        let mut parser = AiParser::new(data);
        match parser.parse() {
            Ok(ai) => {
                for obj in &ai.objects {
                    self.objects.push(obj.clone());
                }
                let response = serde_json::json!({
                    "width": ai.width,
                    "height": ai.height,
                    "objects": ai.objects
                });
                serde_json::to_string(&response).unwrap_or("{\"error\": \"Serialization failed\"}".to_string())
            }
            Err(e) => format!("{{\"error\": \"Failed to parse AI: {:?}\"}}", e),
        }
    }

    fn add_object(&mut self, shape_type: ShapeType, x: f64, y: f64, width: f64, height: f64, fill: &str) -> u32 {
        let id = self.next_id;
        let name = format!("{:?} {}", shape_type, id);
        self.objects.push(VectorObject {
            id,
            shape_type,
            name,
            x,
            y,
            width,
            height,
            rotation: 0.0,
            fill: fill.to_string(),
            stroke: "#000000".to_string(),
            stroke_width: 1.0,
            opacity: 1.0,
            visible: true,
            locked: false,
            blend_mode: "source-over".to_string(),
            stroke_cap: "butt".to_string(),
            stroke_join: "miter".to_string(),
            stroke_dash: Vec::new(),
            layer_style: LayerStyle::default(),
            mask_id: None,
            is_mask: false,
            sides: 5,
            inner_radius: 0.5,
            corner_radius: 0.0,
            path_data: String::new(),
            brush_id: 0,
            stroke_points: Vec::new(),
            text_content: "Type here...".to_string(),
            font_family: "Inter, sans-serif".to_string(),
            font_size: 24.0,
            font_weight: "normal".to_string(),
            text_align: "left".to_string(),
            kerning: 0.0,
            leading: 1.2,
            tracking: 0.0,
            shadow_color: "transparent".to_string(),
            shadow_blur: 0.0,
            shadow_offset_x: 0.0,
            shadow_offset_y: 0.0,
            sx: 0.0,
            sy: 0.0,
            sw: 0.0,
            sh: 0.0,
            brightness: 1.0,
            contrast: 1.0,
            saturate: 1.0,
            hue_rotate: 0.0,
            blur: 0.0,
            grayscale: 0.0,
            sepia: 0.0,
            invert: 0.0,
            raw_image: None,
            raw_rgba: None,
            raw_rgba_width: 0,
            raw_rgba_height: 0,
            image: None,
            fill_gradient: None,
            stroke_gradient: None,
            children: None,
        });
        self.next_id += 1;
        id
    }

    fn update_object(&mut self, id: u32, params: &serde_json::Value) -> bool {
        if let Some(obj) = self.objects.iter_mut().find(|o| o.id == id) {
            if let Some(v) = params["x"].as_f64() { obj.x = v; }
            if let Some(v) = params["y"].as_f64() { obj.y = v; }
            if let Some(v) = params["width"].as_f64() { obj.width = v; }
            if let Some(v) = params["height"].as_f64() { obj.height = v; }
            if let Some(v) = params["sx"].as_f64() { obj.sx = v; }
            if let Some(v) = params["sy"].as_f64() { obj.sy = v; }
            if let Some(v) = params["sw"].as_f64() { obj.sw = v; }
            if let Some(v) = params["sh"].as_f64() { obj.sh = v; }
            if let Some(v) = params["rotation"].as_f64() { obj.rotation = v; }
            if let Some(v) = params["fill"].as_str() { 
                obj.fill = v.to_string(); 
                obj.fill_gradient = None; // Reset gradient if solid color is set
            }
            if let Some(grad) = params["fill_gradient"].as_object() {
                // Manually deserialize Gradient from JSON Value
                let mut stops = Vec::new();
                if let Some(arr) = grad.get("stops").and_then(|s| s.as_array()) {
                    for s in arr {
                        if let (Some(offset), Some(color)) = (s["offset"].as_f64(), s["color"].as_str()) {
                            stops.push(GradientStop { offset, color: color.to_string() });
                        }
                    }
                }
                
                obj.fill_gradient = Some(Gradient {
                    is_radial: grad.get("is_radial").and_then(|v| v.as_bool()).unwrap_or(false),
                    x1: grad.get("x1").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    y1: grad.get("y1").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    x2: grad.get("x2").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    y2: grad.get("y2").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    r1: grad.get("r1").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    r2: grad.get("r2").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    stops,
                });
            }
            if let Some(v) = params["stroke"].as_str() { 
                obj.stroke = v.to_string();
                obj.stroke_gradient = None;
            }
            if let Some(grad) = params["stroke_gradient"].as_object() {
                let mut stops = Vec::new();
                if let Some(arr) = grad.get("stops").and_then(|s| s.as_array()) {
                     for s in arr {
                        if let (Some(offset), Some(color)) = (s["offset"].as_f64(), s["color"].as_str()) {
                            stops.push(GradientStop { offset, color: color.to_string() });
                        }
                    }
                }
                 obj.stroke_gradient = Some(Gradient {
                    is_radial: grad.get("is_radial").and_then(|v| v.as_bool()).unwrap_or(false),
                    x1: grad.get("x1").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    y1: grad.get("y1").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    x2: grad.get("x2").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    y2: grad.get("y2").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    r1: grad.get("r1").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    r2: grad.get("r2").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    stops,
                });
            }
            if let Some(v) = params["stroke_width"].as_f64() { obj.stroke_width = v; }
            if let Some(v) = params["opacity"].as_f64() { obj.opacity = v; }
            if let Some(v) = params["visible"].as_bool() { obj.visible = v; }
            if let Some(v) = params["blend_mode"].as_str() { obj.blend_mode = v.to_string(); }
            if let Some(v) = params["stroke_cap"].as_str() { obj.stroke_cap = v.to_string(); }
            if let Some(v) = params["stroke_join"].as_str() { obj.stroke_join = v.to_string(); }
            if let Some(arr) = params["stroke_dash"].as_array() {
                obj.stroke_dash = arr.iter().filter_map(|v| v.as_f64()).collect();
            }
            if let Some(v) = params["name"].as_str() { obj.name = v.to_string(); }
            if let Some(v) = params["locked"].as_bool() { obj.locked = v; }
            if let Some(v) = params["sides"].as_u64() { obj.sides = v as u32; }
            if let Some(v) = params["inner_radius"].as_f64() { obj.inner_radius = v; }
            if let Some(v) = params["corner_radius"].as_f64() { obj.corner_radius = v; }
            if let Some(v) = params["path_data"].as_str() { obj.path_data = v.to_string(); }
            if let Some(v) = params["brush_id"].as_u64() { obj.brush_id = v as u32; }
            if let Some(pts) = params["stroke_points"].as_array() {
                obj.stroke_points = pts.iter().map(|p| StrokePoint {
                    x: p["x"].as_f64().unwrap_or(0.0),
                    y: p["y"].as_f64().unwrap_or(0.0),
                    pressure: p["pressure"].as_f64().unwrap_or(1.0),
                }).collect();
            }
            if let Some(v) = params["brightness"].as_f64() { obj.brightness = v; }
            if let Some(v) = params["contrast"].as_f64() { obj.contrast = v; }
            if let Some(v) = params["saturate"].as_f64() { obj.saturate = v; }
            if let Some(v) = params["hue_rotate"].as_f64() { obj.hue_rotate = v; }
            if let Some(v) = params["blur"].as_f64() { obj.blur = v; }
            if let Some(v) = params["grayscale"].as_f64() { obj.grayscale = v; }
            if let Some(v) = params["sepia"].as_f64() { obj.sepia = v; }
            if let Some(v) = params["invert"].as_f64() { obj.invert = v; }
            if let Some(v) = params["text_content"].as_str() { obj.text_content = v.to_string(); }
            if let Some(v) = params["font_family"].as_str() { obj.font_family = v.to_string(); }
            if let Some(v) = params["font_size"].as_f64() { obj.font_size = v; }
            if let Some(v) = params["font_weight"].as_str() { obj.font_weight = v.to_string(); }
            if let Some(v) = params["text_align"].as_str() { obj.text_align = v.to_string(); }
            if let Some(v) = params["kerning"].as_f64() { obj.kerning = v; }
            if let Some(v) = params["leading"].as_f64() { obj.leading = v; }
            if let Some(v) = params["tracking"].as_f64() { obj.tracking = v; }
            if let Some(v) = params["shadow_color"].as_str() { obj.shadow_color = v.to_string(); }
            if let Some(v) = params["shadow_blur"].as_f64() { obj.shadow_blur = v; }
            if let Some(v) = params["shadow_offset_x"].as_f64() { obj.shadow_offset_x = v; }
            if let Some(v) = params["shadow_offset_y"].as_f64() { obj.shadow_offset_y = v; }
            if let Some(v) = params["is_mask"].as_bool() { obj.is_mask = v; }
            if let Some(v) = params["mask_id"].as_u64() { obj.mask_id = Some(v as u32); }
            if let Some(style_val) = params.get("layer_style") {
                if let Ok(style) = serde_json::from_value::<LayerStyle>(style_val.clone()) {
                    obj.layer_style = style;
                }
            }
            true
        } else {
            false
        }
    }

    fn delete_object(&mut self, id: u32) -> bool {
        let initial_len = self.objects.len();
        self.objects.retain(|o| o.id != id);
        self.selected_ids.retain(|&sid| sid != id);
        self.objects.len() < initial_len
    }

    pub fn select_point(&mut self, x: f64, y: f64, shift: bool, ignore_locked: bool) -> String {
        let tx = (x - self.viewport_x) / self.viewport_zoom;
        let ty = (y - self.viewport_y) / self.viewport_zoom;

        let mut hit_id = None;
        for obj in self.objects.iter().rev() {
            if obj.locked && !ignore_locked { continue; }
            
            // Transform point to object's local space
            let cx = obj.x + obj.width / 2.0;
            let cy = obj.y + obj.height / 2.0;
            
            // 1. Translate point to be relative to center
            let px = tx - cx;
            let py = ty - cy;
            
            // 2. Rotate point by -obj.rotation
            let cos_r = (-obj.rotation).cos();
            let sin_r = (-obj.rotation).sin();
            let rx = px * cos_r - py * sin_r;
            let ry = px * sin_r + py * cos_r;
            
            // 3. Check if point is within unrotated bounds (relative to center)
            if rx >= -obj.width / 2.0 && rx <= obj.width / 2.0 && ry >= -obj.height / 2.0 && ry <= obj.height / 2.0 {
                hit_id = Some(obj.id);
                break;
            }
        }

        if !shift {
            self.selected_ids.clear();
        }

        if let Some(id) = hit_id {
            if shift {
                if let Some(pos) = self.selected_ids.iter().position(|&x| x == id) {
                    self.selected_ids.remove(pos);
                } else {
                    self.selected_ids.push(id);
                }
            } else {
                self.selected_ids.push(id);
            }
        }

        self.get_selected_ids()
    }

    pub fn select_rect(&mut self, x: f64, y: f64, width: f64, height: f64, shift: bool, ignore_locked: bool) -> String {
        // x,y,width,height are in screen coords. Convert to world.
        // But wait, user might drag negative width/height. Normalize first.
        let mut sx = x;
        let mut sy = y;
        let mut sw = width;
        let mut sh = height;

        if sw < 0.0 { sx += sw; sw = -sw; }
        if sh < 0.0 { sy += sh; sh = -sh; }

        let x1 = (sx - self.viewport_x) / self.viewport_zoom;
        let y1 = (sy - self.viewport_y) / self.viewport_zoom;
        let x2 = (sx + sw - self.viewport_x) / self.viewport_zoom;
        let y2 = (sy + sh - self.viewport_y) / self.viewport_zoom;

        if !shift {
            self.selected_ids.clear();
        }

        for obj in &self.objects {
            if obj.locked && !ignore_locked { continue; }
            // Check if object center is in selection rect (simple approx)
            // or if object intersects. For now, let's say "fully contained" or "intersects center"
            // Let's go with "intersects" which is standard for box select.
            
            let obj_x2 = obj.x + obj.width;
            let obj_y2 = obj.y + obj.height;

            // AABB intersection
            if obj.x < x2 && obj_x2 > x1 && obj.y < y2 && obj_y2 > y1 {
                 if !self.selected_ids.contains(&obj.id) {
                     self.selected_ids.push(obj.id);
                 }
            }
        }

        self.get_selected_ids()
    }

    pub fn get_selected_ids(&self) -> String {
        serde_json::to_string(&self.selected_ids).unwrap_or("[]".to_string())
    }

    pub fn hit_test_handles(&self, x: f64, y: f64) -> String {
        let tx = (x - self.viewport_x) / self.viewport_zoom;
        let ty = (y - self.viewport_y) / self.viewport_zoom;

        // Only check the primary selected object (last one)
        if let Some(&id) = self.selected_ids.last() {
            if let Some(obj) = self.objects.iter().find(|o| o.id == id) {
                let cx = obj.x + obj.width / 2.0;
                let cy = obj.y + obj.height / 2.0;
                
                let px = tx - cx;
                let py = ty - cy;
                
                let cos_r = (-obj.rotation).cos();
                let sin_r = (-obj.rotation).sin();
                let rx = px * cos_r - py * sin_r;
                let ry = px * sin_r + py * cos_r;
                
                let local_x = rx + obj.width / 2.0;
                let local_y = ry + obj.height / 2.0;

                let handle_radius = 6.0 / self.viewport_zoom;
                let rotate_offset = -30.0 / self.viewport_zoom;

                let handles = [
                    (0.0, 0.0, HandleType::TopLeft),
                    (obj.width, 0.0, HandleType::TopRight),
                    (0.0, obj.height, HandleType::BottomLeft),
                    (obj.width, obj.height, HandleType::BottomRight),
                    (obj.width / 2.0, 0.0, HandleType::Top),
                    (obj.width / 2.0, obj.height, HandleType::Bottom),
                    (0.0, obj.height / 2.0, HandleType::Left),
                    (obj.width, obj.height / 2.0, HandleType::Right),
                    (obj.width / 2.0, rotate_offset, HandleType::Rotate),
                ];

                for (hx, hy, h_type) in handles.iter() {
                    let dist = ((local_x - hx).powi(2) + (local_y - hy).powi(2)).sqrt();
                    if dist <= handle_radius {
                        return serde_json::to_string(&(id, *h_type)).unwrap_or("null".to_string());
                    }
                }
            }
        }
        "null".to_string()
    }

    // Keep legacy for compatibility if needed, or remove? 
    // It's safer to remove and fix calls to ensure I found all usages.


    pub fn erase_image(&mut self, id: u32, x: f64, y: f64, radius: f64) -> bool {
        if let Some(obj) = self.objects.iter_mut().find(|o| o.id == id) {
            if obj.shape_type != ShapeType::Image { return false; }
            
            let pixels = match &mut obj.raw_rgba {
                Some(p) => p,
                None => return false,
            };

            let width = obj.raw_rgba_width as f64;
            let height = obj.raw_rgba_height as f64;

            // Map world x, y to local image pixels
            let cx = obj.x + obj.width / 2.0;
            let cy = obj.y + obj.height / 2.0;
            let dx = x - cx;
            let dy = y - cy;

            let cos_r = (-obj.rotation).cos();
            let sin_r = (-obj.rotation).sin();
            let lx = dx * cos_r - dy * sin_r;
            let ly = dx * sin_r + dy * cos_r;

            let px = (lx / obj.width + 0.5) * width;
            let py = (ly / obj.height + 0.5) * height;
            
            let scale_x = width / obj.width;
            let scale_y = height / obj.height;
            let p_radius = radius * (scale_x + scale_y) / 2.0;

            let r2 = p_radius * p_radius;
            let i_width = obj.raw_rgba_width as i32;
            let i_height = obj.raw_rgba_height as i32;

            let min_px = (px - p_radius).floor() as i32;
            let max_px = (px + p_radius).ceil() as i32;
            let min_py = (py - p_radius).floor() as i32;
            let max_py = (py + p_radius).ceil() as i32;

            let mut modified = false;
            for iy in min_py..max_py {
                if iy < 0 || iy >= i_height { continue; }
                for ix in min_px..max_px {
                    if ix < 0 || ix >= i_width { continue; }
                    
                    let dx_p = ix as f64 - px;
                    let dy_p = iy as f64 - py;
                    if dx_p*dx_p + dy_p*dy_p <= r2 {
                        let idx = (iy * i_width + ix) as usize * 4;
                        if pixels[idx + 3] != 0 {
                            pixels[idx + 3] = 0;
                            modified = true;
                        }
                    }
                }
            }
            return modified;
        }
        false
    }

    pub fn clone_stamp(&mut self, id: u32, src_x: f64, src_y: f64, dst_x: f64, dst_y: f64, radius: f64) -> bool {
        if let Some(obj) = self.objects.iter_mut().find(|o| o.id == id) {
            if obj.shape_type != ShapeType::Image { return false; }
            
            let width = obj.raw_rgba_width;
            let height = obj.raw_rgba_height;
            let o_x = obj.x;
            let o_y = obj.y;
            let o_w = obj.width;
            let o_h = obj.height;
            let o_rot = obj.rotation;

            // Map world src/dst to local image pixels
            let to_local = |wx: f64, wy: f64| {
                let cx = o_x + o_w / 2.0;
                let cy = o_y + o_h / 2.0;
                let dx = wx - cx;
                let dy = wy - cy;
                let cos_r = (-o_rot).cos();
                let sin_r = (-o_rot).sin();
                let lx = dx * cos_r - dy * sin_r;
                let ly = dx * sin_r + dy * cos_r;
                (
                    ((lx / o_w + 0.5) * width as f64) as i32,
                    ((ly / o_h + 0.5) * height as f64) as i32
                )
            };

            let (lsx, lsy) = to_local(src_x, src_y);
            let (ldx, ldy) = to_local(dst_x, dst_y);
            
            let scale_x = width as f64 / o_w;
            let p_radius = (radius * scale_x) as i32;
            let r2 = p_radius * p_radius;

            let pixels = match &mut obj.raw_rgba {
                Some(p) => p,
                None => return false,
            };

            let i_width = width as i32;
            let i_height = height as i32;
            let mut modified = false;
            let mut new_pixels = pixels.clone();

            for dy in -p_radius..p_radius {
                for dx in -p_radius..p_radius {
                    if dx*dx + dy*dy <= r2 {
                        let sx = lsx + dx;
                        let sy = lsy + dy;
                        let tx = ldx + dx;
                        let ty = ldy + dy;

                        if sx >= 0 && sx < i_width && sy >= 0 && sy < i_height &&
                           tx >= 0 && tx < i_width && ty >= 0 && ty < i_height {
                            let src_idx = (sy * i_width + sx) as usize * 4;
                            let dst_idx = (ty * i_width + tx) as usize * 4;
                            
                            new_pixels[dst_idx] = pixels[src_idx];
                            new_pixels[dst_idx+1] = pixels[src_idx+1];
                            new_pixels[dst_idx+2] = pixels[src_idx+2];
                            new_pixels[dst_idx+3] = pixels[src_idx+3];
                            modified = true;
                        }
                    }
                }
            }
            
            if modified {
                *pixels = new_pixels;
            }
            return modified;
        }
        false
    }

    pub fn get_image_rgba(&self, id: u32) -> Option<Vec<u8>> {
        self.objects.iter().find(|o| o.id == id).and_then(|o| o.raw_rgba.clone())
    }

    pub fn get_image_width(&self, id: u32) -> u32 {
        self.objects.iter().find(|o| o.id == id).map(|o| o.raw_rgba_width).unwrap_or(0)
    }

    pub fn get_image_height(&self, id: u32) -> u32 {
        self.objects.iter().find(|o| o.id == id).map(|o| o.raw_rgba_height).unwrap_or(0)
    }

    pub fn set_image_raw(&mut self, id: u32, data: Vec<u8>) -> bool {
        if let Some(obj) = self.objects.iter_mut().find(|o| o.id == id) {
            obj.raw_image = Some(data.clone());
            if let Ok(img) = image::load_from_memory(&data) {
                let rgba = img.to_rgba8();
                obj.raw_rgba_width = rgba.width();
                obj.raw_rgba_height = rgba.height();
                obj.raw_rgba = Some(rgba.into_raw());
            }
            true
        } else {
            false
        }
    }

    pub fn get_objects_json(&self) -> String {
        serde_json::to_string(&self.objects).unwrap_or_else(|_| "[]".to_string())
    }

    pub fn set_image_object(&mut self, id: u32, image_val: JsValue) -> bool {
        if let Some(obj) = self.objects.iter_mut().find(|o| o.id == id) {
            if let Some(image) = image_val.dyn_ref::<HtmlImageElement>() {
                if obj.sw == 0.0 { obj.sw = image.width() as f64; }
                if obj.sh == 0.0 { obj.sh = image.height() as f64; }
            } else if let Some(canvas) = image_val.dyn_ref::<web_sys::HtmlCanvasElement>() {
                if obj.sw == 0.0 { obj.sw = canvas.width() as f64; }
                if obj.sh == 0.0 { obj.sh = canvas.height() as f64; }
            }
            obj.image = Some(image_val);
            // obj.raw_image = None; // REMOVED: Keep raw_image for potential erases/vectorization
            true
        } else {
            false
        }
    }

    pub fn render(&self, ctx: &web_sys::CanvasRenderingContext2d) {
        ctx.save();
        ctx.clear_rect(0.0, 0.0, 20000.0, 20000.0);
        
        ctx.translate(self.viewport_x, self.viewport_y).unwrap();
        ctx.scale(self.viewport_zoom, self.viewport_zoom).unwrap();

        // Draw Artboard Background & Clip
        ctx.save();
        if self.clip_to_artboard {
            ctx.begin_path();
            ctx.rect(0.0, 0.0, self.artboard.width, self.artboard.height);
            ctx.clip();
        }
        
        ctx.set_fill_style(&JsValue::from_str(&self.artboard.background));
        ctx.set_shadow_color("rgba(0,0,0,0.5)");
        ctx.set_shadow_blur(20.0);
        ctx.fill_rect(0.0, 0.0, self.artboard.width, self.artboard.height);
        ctx.set_shadow_color("transparent");

        // If bottom layer is a locked image, draw checkers first
        if let Some(first) = self.objects.first() {
            if first.shape_type == ShapeType::Image && first.locked {
                self.render_checkerboard(ctx, self.artboard.width, self.artboard.height);
            }
        }

        for obj in &self.objects {
            self.render_object(ctx, obj);
        }
        
        self.render_guides(ctx);
        ctx.restore();
        
        // Render Selection Overlay
        if !self.hide_selection {
            self.render_selection_overlay(ctx);
        }

        ctx.restore();
    }

    fn render_checkerboard(&self, ctx: &web_sys::CanvasRenderingContext2d, width: f64, height: f64) {
        let size = 16.0;
        ctx.save();
        ctx.set_fill_style(&JsValue::from_str("#ffffff"));
        ctx.fill_rect(0.0, 0.0, width, height);
        ctx.set_fill_style(&JsValue::from_str("#e5e5e5"));
        
        let cols = (width / size).ceil() as i32;
        let rows = (height / size).ceil() as i32;
        
        for r in 0..rows {
            for c in 0..cols {
                if (r + c) % 2 != 0 {
                    ctx.fill_rect(c as f64 * size, r as f64 * size, size, size);
                }
            }
        }
        ctx.restore();
    }

    fn render_selection_overlay(&self, ctx: &web_sys::CanvasRenderingContext2d) {
        if self.selected_ids.is_empty() { return; }

        if self.selected_ids.len() == 1 {
             let id = self.selected_ids[0];
             if let Some(obj) = self.objects.iter().find(|o| o.id == id) {
                ctx.save();
                ctx.translate(obj.x + obj.width / 2.0, obj.y + obj.height / 2.0).unwrap();
                ctx.rotate(obj.rotation).unwrap();
                ctx.translate(-obj.width / 2.0, -obj.height / 2.0).unwrap();

                ctx.set_stroke_style(&JsValue::from_str("#4facfe"));
                ctx.set_line_width(1.5 / self.viewport_zoom);
                ctx.set_line_dash(&js_sys::Array::new()).unwrap(); 
                ctx.stroke_rect(0.0, 0.0, obj.width, obj.height);

                let handle_size = 8.0 / self.viewport_zoom;
                let rotate_offset = -30.0 / self.viewport_zoom;
                
                ctx.set_fill_style(&JsValue::from_str("#ffffff"));
                ctx.set_stroke_style(&JsValue::from_str("#4facfe"));
                ctx.set_line_width(1.0 / self.viewport_zoom);

                let handles = [
                    (0.0, 0.0), (obj.width, 0.0), (0.0, obj.height), (obj.width, obj.height),
                    (obj.width / 2.0, 0.0), (obj.width / 2.0, obj.height), 
                    (0.0, obj.height / 2.0), (obj.width, obj.height / 2.0),
                ];

                for (hx, hy) in handles {
                    ctx.begin_path();
                    ctx.rect(hx - handle_size/2.0, hy - handle_size/2.0, handle_size, handle_size);
                    ctx.fill();
                    ctx.stroke();
                }

                ctx.begin_path();
                ctx.move_to(obj.width / 2.0, 0.0);
                ctx.line_to(obj.width / 2.0, rotate_offset);
                ctx.stroke();

                ctx.begin_path();
                ctx.arc(obj.width / 2.0, rotate_offset, handle_size / 2.0, 0.0, std::f64::consts::PI * 2.0).unwrap();
                ctx.fill();
                ctx.stroke();
                
                ctx.restore();
             }
        } else {
            // Group Selection
            let mut g_min_x = f64::INFINITY;
            let mut g_min_y = f64::INFINITY;
            let mut g_max_x = f64::NEG_INFINITY;
            let mut g_max_y = f64::NEG_INFINITY;

            for id in &self.selected_ids {
                if let Some(obj) = self.objects.iter().find(|o| o.id == *id) {
                    let (min_x, min_y, max_x, max_y) = obj.get_world_bounds();
                    if min_x < g_min_x { g_min_x = min_x; }
                    if min_y < g_min_y { g_min_y = min_y; }
                    if max_x > g_max_x { g_max_x = max_x; }
                    if max_y > g_max_y { g_max_y = max_y; }
                }
            }

            if g_min_x < g_max_x && g_min_y < g_max_y {
                ctx.save();
                ctx.set_stroke_style(&JsValue::from_str("#4facfe"));
                ctx.set_line_width(1.5 / self.viewport_zoom);
                let dash = js_sys::Array::new();
                dash.push(&JsValue::from_f64(4.0 / self.viewport_zoom));
                dash.push(&JsValue::from_f64(4.0 / self.viewport_zoom));
                ctx.set_line_dash(&dash).unwrap();
                
                ctx.stroke_rect(g_min_x, g_min_y, g_max_x - g_min_x, g_max_y - g_min_y);
                ctx.restore();
            }
        }
    }

    fn render_object(&self, ctx: &web_sys::CanvasRenderingContext2d, obj: &VectorObject) {
        if !obj.visible || obj.is_mask { return; }
        
        ctx.save();

        // 1. Handle Masking
        if let Some(mask_id) = obj.mask_id {
            if let Some(mask_obj) = self.objects.iter().find(|o| o.id == mask_id) {
                // To mask, we first draw the mask shape to define the path, then clip
                ctx.save();
                // Move to mask's transform
                ctx.translate(mask_obj.x + mask_obj.width / 2.0, mask_obj.y + mask_obj.height / 2.0).unwrap();
                ctx.rotate(mask_obj.rotation).unwrap();
                ctx.translate(-mask_obj.width / 2.0, -mask_obj.height / 2.0).unwrap();
                
                self.define_object_path(ctx, mask_obj);
                ctx.restore();
                ctx.clip();
            }
        }

        ctx.set_global_alpha(obj.opacity);
        ctx.set_global_composite_operation(&obj.blend_mode).unwrap_or(());
        
        // 2. Apply Adjustment logic if it's an adjustment layer
        if obj.shape_type == ShapeType::Adjustment {
            // Adjustment layers in this engine will affect the WHOLE canvas below them
            // because of how we render sequentially.
            // We'll apply filters to the context that will persist for subsequent draws
            // but we need a way to "scope" them. 
            // For now, let's say they affect everything rendered SO FAR or everything AFTER.
            // In Photoshop, they affect layers BELOW. 
            // Since we render bottom-to-top, an adjustment layer at index N should affect
            // the result of layers 0..N-1.
            // This requires rendering 0..N-1 to a temp canvas, applying filter, then continuing.
            // Simplification for now: Adjustment layers apply a global filter to the ctx.
            let filter = format!(
                "brightness({}%) contrast({}%) saturate({}%) hue-rotate({}deg) blur({}px) grayscale({}%) sepia({}%) invert({}%)",
                obj.brightness * 100.0,
                obj.contrast * 100.0,
                obj.saturate * 100.0,
                obj.hue_rotate,
                obj.blur,
                obj.grayscale * 100.0,
                obj.sepia * 100.0,
                obj.invert * 100.0
            );
            ctx.set_filter(&filter);
            // We don't "draw" the adjustment layer itself, it just modifies the pipeline.
            // However, we might want to draw a bounding box in the UI.
            return;
        }

        // 3. Apply Layer Styles (FX) - PRE-DRAW (e.g. Drop Shadow)
        for effect in &obj.layer_style.effects {
            if !effect.enabled { continue; }
            if effect.effect_type == EffectType::DropShadow {
                ctx.set_shadow_color(&effect.color);
                ctx.set_shadow_blur(effect.blur);
                ctx.set_shadow_offset_x(effect.x);
                ctx.set_shadow_offset_y(effect.y);
            }
        }
        
        // Transform for object
        ctx.translate(obj.x + obj.width / 2.0, obj.y + obj.height / 2.0).unwrap();
        ctx.rotate(obj.rotation).unwrap();
        ctx.translate(-obj.width / 2.0, -obj.height / 2.0).unwrap();
        
        // ... rest of rendering ...

        // Recursively render children if Group
        if obj.shape_type == ShapeType::Group {
            if let Some(children) = &obj.children {
                for child in children {
                    self.render_object(ctx, child);
                }
            }
        } else {
            // Apply Fill
            if let Some(grad) = &obj.fill_gradient {
                let canvas_grad_opt = if grad.is_radial {
                    ctx.create_radial_gradient(grad.x1, grad.y1, grad.r1, grad.x2, grad.y2, grad.r2).ok()
                } else {
                    Some(ctx.create_linear_gradient(grad.x1, grad.y1, grad.x2, grad.y2))
                };

                if let Some(canvas_grad) = canvas_grad_opt {
                    for stop in &grad.stops {
                        let _ = canvas_grad.add_color_stop(stop.offset as f32, &stop.color);
                    }
                    ctx.set_fill_style(&canvas_grad);
                }
            } else {
                ctx.set_fill_style(&JsValue::from_str(&obj.fill));
            }

            // Apply Stroke
            if let Some(grad) = &obj.stroke_gradient {
                let canvas_grad_opt = if grad.is_radial {
                    ctx.create_radial_gradient(grad.x1, grad.y1, grad.r1, grad.x2, grad.y2, grad.r2).ok()
                } else {
                    Some(ctx.create_linear_gradient(grad.x1, grad.y1, grad.x2, grad.y2))
                };

                if let Some(canvas_grad) = canvas_grad_opt {
                    for stop in &grad.stops {
                        let _ = canvas_grad.add_color_stop(stop.offset as f32, &stop.color);
                    }
                    ctx.set_stroke_style(&canvas_grad);
                }
            } else {
                ctx.set_stroke_style(&JsValue::from_str(&obj.stroke));
            }
            
            ctx.set_line_width(obj.stroke_width);
            ctx.set_line_cap(&obj.stroke_cap);
            ctx.set_line_join(&obj.stroke_join);
            
            // Apply Shadow
            ctx.set_shadow_color(&obj.shadow_color);
            ctx.set_shadow_blur(obj.shadow_blur);
            ctx.set_shadow_offset_x(obj.shadow_offset_x);
            ctx.set_shadow_offset_y(obj.shadow_offset_y);

            // Set dash
            if !obj.stroke_dash.is_empty() {
                 let dash_array = js_sys::Array::new();
                 for &d in &obj.stroke_dash {
                     dash_array.push(&JsValue::from_f64(d));
                 }
                 let _ = ctx.set_line_dash(&dash_array);
            } else {
                 let _ = ctx.set_line_dash(&js_sys::Array::new());
            }

            match obj.shape_type {
                ShapeType::Rectangle => {
                    if obj.corner_radius > 0.0 {
                        let r = obj.corner_radius.min(obj.width / 2.0).min(obj.height / 2.0);
                        ctx.begin_path();
                        ctx.move_to(r, 0.0);
                        ctx.line_to(obj.width - r, 0.0);
                        ctx.arc_to(obj.width, 0.0, obj.width, r, r).unwrap();
                        ctx.line_to(obj.width, obj.height - r);
                        ctx.arc_to(obj.width, obj.height, obj.width - r, obj.height, r).unwrap();
                        ctx.line_to(r, obj.height);
                        ctx.arc_to(0.0, obj.height, 0.0, obj.height - r, r).unwrap();
                        ctx.line_to(0.0, r);
                        ctx.arc_to(0.0, 0.0, r, 0.0, r).unwrap();
                        ctx.close_path();
                        ctx.fill();
                        if obj.stroke_width > 0.0 { ctx.stroke(); }
                    } else {
                        ctx.fill_rect(0.0, 0.0, obj.width, obj.height);
                        if obj.stroke_width > 0.0 { ctx.stroke_rect(0.0, 0.0, obj.width, obj.height); }
                    }
                }
                ShapeType::Circle | ShapeType::Ellipse => {
                    ctx.begin_path();
                    let _ = ctx.ellipse(obj.width / 2.0, obj.height / 2.0, obj.width / 2.0, obj.height / 2.0, 0.0, 0.0, std::f64::consts::PI * 2.0);
                    ctx.fill();
                    if obj.stroke_width > 0.0 { ctx.stroke(); }
                }
                ShapeType::Polygon => {
                    self.draw_poly(ctx, obj.width / 2.0, obj.height / 2.0, obj.width / 2.0, obj.sides, 0.0);
                    ctx.fill();
                    if obj.stroke_width > 0.0 { ctx.stroke(); }
                }
                ShapeType::Star => {
                    self.draw_star(ctx, obj.width / 2.0, obj.height / 2.0, obj.width / 2.0, obj.inner_radius * (obj.width / 2.0), obj.sides);
                    ctx.fill();
                    if obj.stroke_width > 0.0 { ctx.stroke(); }
                }
                ShapeType::Image => {
                    if let Some(img_val) = &obj.image {
                        if let Some(img) = img_val.dyn_ref::<web_sys::HtmlImageElement>() {
                            let _ = ctx.draw_image_with_html_image_element_and_sw_and_sh_and_dx_and_dy_and_dw_and_dh(
                                img, obj.sx, obj.sy, obj.sw, obj.sh, 0.0, 0.0, obj.width, obj.height
                            );
                        } else if let Some(canvas) = img_val.dyn_ref::<web_sys::HtmlCanvasElement>() {
                            let _ = ctx.draw_image_with_html_canvas_element_and_sw_and_sh_and_dx_and_dy_and_dw_and_dh(
                                canvas, obj.sx, obj.sy, obj.sw, obj.sh, 0.0, 0.0, obj.width, obj.height
                            );
                        }
                    }
                }
                ShapeType::Path => {
                    if !obj.path_data.is_empty() {
                         if let Ok(path) = BezPath::from_svg(&obj.path_data) {
                             if obj.brush_id > 0 {
                                 if let Some(brush) = self.brush_engine.brushes.iter().find(|b| b.id == obj.brush_id) {
                                     self.brush_engine.render_stroke(ctx, brush, &path, &obj.fill, &self.brush_image_map);
                                 }
                             } else {
                                 if let Ok(p) = Path2d::new_with_path_string(&obj.path_data) {
                                     ctx.fill_with_path_2d(&p);
                                     if obj.stroke_width > 0.0 { ctx.stroke_with_path(&p); }
                                 }
                             }
                         }
                    }
                }
                ShapeType::Text => {
                    ctx.set_font(&format!("{} {}px {}", obj.font_weight, obj.font_size, obj.font_family));
                    ctx.set_text_align(&obj.text_align);
                    let _ = ctx.fill_text(&obj.text_content, 0.0, obj.font_size);
                    if obj.stroke_width > 0.0 {
                        let _ = ctx.stroke_text(&obj.text_content, 0.0, obj.font_size);
                    }
                }
                _ => {}
            }
        }

        ctx.restore();
    }

    fn define_object_path(&self, ctx: &web_sys::CanvasRenderingContext2d, obj: &VectorObject) {
        match obj.shape_type {
            ShapeType::Rectangle => {
                if obj.corner_radius > 0.0 {
                    let r = obj.corner_radius.min(obj.width / 2.0).min(obj.height / 2.0);
                    ctx.begin_path();
                    ctx.move_to(r, 0.0);
                    ctx.line_to(obj.width - r, 0.0);
                    ctx.arc_to(obj.width, 0.0, obj.width, r, r).unwrap();
                    ctx.line_to(obj.width, obj.height - r);
                    ctx.arc_to(obj.width, obj.height, obj.width - r, obj.height, r).unwrap();
                    ctx.line_to(r, obj.height);
                    ctx.arc_to(0.0, obj.height, 0.0, obj.height - r, r).unwrap();
                    ctx.line_to(0.0, r);
                    ctx.arc_to(0.0, 0.0, r, 0.0, r).unwrap();
                    ctx.close_path();
                } else {
                    ctx.begin_path();
                    ctx.rect(0.0, 0.0, obj.width, obj.height);
                }
            }
            ShapeType::Circle | ShapeType::Ellipse => {
                ctx.begin_path();
                let _ = ctx.ellipse(obj.width / 2.0, obj.height / 2.0, obj.width / 2.0, obj.height / 2.0, 0.0, 0.0, std::f64::consts::PI * 2.0);
            }
            ShapeType::Polygon => {
                self.draw_poly(ctx, obj.width / 2.0, obj.height / 2.0, obj.width / 2.0, obj.sides, 0.0);
            }
            ShapeType::Star => {
                self.draw_star(ctx, obj.width / 2.0, obj.height / 2.0, obj.width / 2.0, obj.inner_radius * (obj.width / 2.0), obj.sides);
            }
            ShapeType::Path => {
                if !obj.path_data.is_empty() {
                    if let Ok(p) = Path2d::new_with_path_string(&obj.path_data) {
                        ctx.begin_path();
                        // Path2D doesn't easily expose its segments to begin_path, 
                        // but in many browsers calling fill/stroke with Path2D is enough.
                        // For clipping, we might need a workaround or more complex path parsing.
                        // web_sys doesn't have a direct way to convert Path2D back to the main path.
                    }
                }
            }
            _ => {
                ctx.begin_path();
                ctx.rect(0.0, 0.0, obj.width, obj.height);
            }
        }
    }

    fn render_guides(&self, ctx: &web_sys::CanvasRenderingContext2d) {
        ctx.save();
        ctx.set_stroke_style(&JsValue::from_str("cyan"));
        ctx.set_line_width(1.0 / self.viewport_zoom);
        
        for guide in &self.artboard.guides {
            ctx.begin_path();
            if guide.orientation == "horizontal" {
                ctx.move_to(-10000.0, guide.position);
                ctx.line_to(10000.0, guide.position);
            } else {
                ctx.move_to(guide.position, -10000.0);
                ctx.line_to(guide.position, 10000.0);
            }
            ctx.stroke();
        }
        ctx.restore();
    }

    fn draw_poly(&self, ctx: &web_sys::CanvasRenderingContext2d, cx: f64, cy: f64, r: f64, sides: u32, rot: f64) {
        ctx.begin_path();
        for i in 0..sides {
            let angle = rot + (i as f64 * 2.0 * std::f64::consts::PI / sides as f64);
            let x = cx + r * angle.cos();
            let y = cy + r * angle.sin();
            if i == 0 { ctx.move_to(x, y); } else { ctx.line_to(x, y); }
        }
        ctx.close_path();
    }

    fn draw_star(&self, ctx: &web_sys::CanvasRenderingContext2d, cx: f64, cy: f64, r_outer: f64, r_inner: f64, points: u32) {
        ctx.begin_path();
        for i in 0..(points * 2) {
            let r = if i % 2 == 0 { r_outer } else { r_inner };
            let angle = (i as f64 * std::f64::consts::PI / points as f64) - (std::f64::consts::PI / 2.0);
            let x = cx + r * angle.cos();
            let y = cy + r * angle.sin();
            if i == 0 { ctx.move_to(x, y); } else { ctx.line_to(x, y); }
        }
        ctx.close_path();
    }
}
