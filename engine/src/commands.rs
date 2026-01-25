use wasm_bindgen::prelude::*;
use serde::Deserialize;
use crate::engine::VectorEngine;
use crate::types::{ShapeType, GradientStop, Gradient, LayerStyle};
use crate::objects::VectorObject;
use crate::brush::{StrokePoint, Brush};
use crate::tracer::Tracer;
use kurbo::{BezPath, Affine, Point, Shape};
use image::{DynamicImage, ImageOutputFormat, RgbaImage};
use std::io::Cursor;
use web_sys::HtmlImageElement;

#[wasm_bindgen]
impl VectorEngine {
    pub fn execute_command(&mut self, cmd_json: &str) -> String {
        #[derive(Deserialize)]
        struct Command {
            action: String,
            params: serde_json::Value,
        }

        let cmd: Command = match serde_json::from_str(cmd_json) {
            Ok(c) => c,
            Err(e) => return format!("{{\"error\": \"Invalid JSON: {}{{\"}}\"}}", e),
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
                        let local_x = ((x - obj.x) / obj.width * width as f64) as i32;
                        let local_y = ((y - obj.y) / obj.height * height as f64) as i32;
                        
                        if local_x >= 0 && local_x < width as i32 && local_y >= 0 && local_y < height as i32 {
                            let start_idx = (local_y as u32 * width + local_x as u32) as usize * 4;
                            let start_r = rgba[start_idx];
                            let start_g = rgba[start_idx + 1];
                            let start_b = rgba[start_idx + 2];
                            
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
                            
                            let tracer = Tracer::new(width, height);
                            let mut mask_img = vec![0u8; (width * height) as usize];
                            for i in 0..mask.len() {
                                if mask[i] { mask_img[i] = 255; }
                            }
                            let luma = image::ImageBuffer::<image::Luma<u8>, Vec<u8>>::from_raw(width, height, mask_img).unwrap();
                            let mut path_data = tracer.trace(&luma, 128);
                            
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
                "{ \"error\": \"Image not found or click outside image\" }".to_string()
            }
            "add_guide" => {
                let orientation = cmd.params["orientation"].as_str().unwrap_or("horizontal").to_string();
                let position = cmd.params["position"].as_f64().unwrap_or(0.0);
                self.artboard.guides.push(crate::types::Guide { orientation, position });
                "{ \"success\": true }".to_string()
            }
            "clear_guides" => {
                self.artboard.guides.clear();
                "{ \"success\": true }".to_string()
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
            
                            self.run_boolean_op(op, &ids)
                        }
            "add" => {
                self.save_state("Add Object");
                let st = match cmd.params["type"].as_str() {
                    Some("Circle") => ShapeType::Circle, Some("Ellipse") => ShapeType::Ellipse,
                    Some("Star") => ShapeType::Star, Some("Polygon") => ShapeType::Polygon,
                    Some("Image") => ShapeType::Image, Some("Path") => ShapeType::Path,
                    Some("Text") => ShapeType::Text, Some("Group") => ShapeType::Group,
                    _ => ShapeType::Rectangle,
                };
                let id = self.add_object(st, cmd.params["x"].as_f64().unwrap_or(0.0), cmd.params["y"].as_f64().unwrap_or(0.0), cmd.params["width"].as_f64().unwrap_or(100.0), cmd.params["height"].as_f64().unwrap_or(100.0), cmd.params["fill"].as_str().unwrap_or("#4facfe"));
                self.update_object(id, &cmd.params);
                format!("{{\"success\": true, \"id\": {}}}", id)
            }
            "update" => {
                if cmd.params["save_undo"].as_bool().unwrap_or(false) { self.save_state("Update Object"); }
                let mut success = false;
                if let Some(ids) = cmd.params["ids"].as_array() { 
                    for id_val in ids { if let Some(id) = id_val.as_u64() { if self.update_object(id as u32, &cmd.params) { success = true; } } }
                } else {
                    let id = cmd.params["id"].as_u64().map(|v| v as u32).unwrap_or(0);
                    if self.update_object(id, &cmd.params) { success = true; }
                }
                if success { "{ \"success\": true }".to_string() } else { "{ \"error\": \"Object(s) not found\" }".to_string() }
            }
            "delete" => {
                if cmd.params["save_undo"].as_bool().unwrap_or(true) { self.save_state("Delete Object"); }
                let mut success = false;
                if let Some(ids) = cmd.params["ids"].as_array() {
                    for id_val in ids { if let Some(id) = id_val.as_u64() { if self.delete_object(id as u32) { success = true; } } }
                } else {
                    let id = cmd.params["id"].as_u64().map(|v| v as u32).unwrap_or(0);
                    if self.delete_object(id) { success = true; }
                }
                if success { "{ \"success\": true }".to_string() } else { "{ \"error\": \"Object(s) not found\" }".to_string() }
            }
            "duplicate" => {
                self.save_state("Duplicate Object");
                let id = cmd.params["id"].as_u64().map(|v| v as u32).unwrap_or(0);
                if let Some(pos) = self.objects.iter().position(|o| o.id == id) {
                    let mut new_obj = self.objects[pos].clone();
                    new_obj.id = self.next_id; self.next_id += 1;
                    new_obj.x += 10.0; new_obj.y += 10.0;
                    new_obj.name = format!("{} copy", new_obj.name);
                    let new_id = new_obj.id;
                    self.objects.insert(pos + 1, new_obj);
                    self.selected_ids = vec![new_id];
                    format!("{{\"success\": true, \"id\": {}}}", new_id)
                } else { "{ \"error\": \"Object not found\" }".to_string() }
            }
            "select" => {
                if let Some(ids) = cmd.params["ids"].as_array() {
                    self.selected_ids = ids.iter().filter_map(|v| v.as_u64().map(|id| id as u32)).collect();
                } else if let Some(id) = cmd.params["id"].as_u64() { self.selected_ids = vec![id as u32]; }
                else { self.selected_ids.clear(); }
                "{ \"success\": true }".to_string()
            }
            "move_to_back" => {
                self.save_state("Move to Back");
                let id = cmd.params["id"].as_u64().map(|v| v as u32).unwrap_or(0);
                if let Some(pos) = self.objects.iter().position(|o| o.id == id) {
                    let obj = self.objects.remove(pos); self.objects.insert(0, obj);
                    "{ \"success\": true }".to_string()
                } else { "{ \"error\": \"Object not found\" }".to_string() }
            }
            "move_to_front" => {
                self.save_state("Move to Front");
                let id = cmd.params["id"].as_u64().map(|v| v as u32).unwrap_or(0);
                if let Some(pos) = self.objects.iter().position(|o| o.id == id) {
                    let obj = self.objects.remove(pos); self.objects.push(obj);
                    "{ \"success\": true }".to_string()
                } else { "{ \"error\": \"Object not found\" }".to_string() }
            }
            "move_forward" => {
                self.save_state("Move Forward");
                let id = cmd.params["id"].as_u64().map(|v| v as u32).unwrap_or(0);
                if let Some(pos) = self.objects.iter().position(|o| o.id == id) {
                    if pos < self.objects.len() - 1 { self.objects.swap(pos, pos + 1); "{ \"success\": true }".to_string() }
                    else { "{ \"success\": true, \"message\": \"Already at front\" }".to_string() }
                } else { "{ \"error\": \"Object not found\" }".to_string() }
            }
            "move_backward" => {
                self.save_state("Move Backward");
                let id = cmd.params["id"].as_u64().map(|v| v as u32).unwrap_or(0);
                if let Some(pos) = self.objects.iter().position(|o| o.id == id) {
                    if pos > 0 { self.objects.swap(pos, pos - 1); "{ \"success\": true }".to_string() }
                    else { "{ \"success\": true, \"message\": \"Already at back\" }".to_string() }
                } else { "{ \"error\": \"Object not found\" }".to_string() }
            }
            "set_artboard" => {
                self.save_state("Set Artboard");
                if let Some(w) = cmd.params["width"].as_f64() { self.artboard.width = w; }
                if let Some(h) = cmd.params["height"].as_f64() { self.artboard.height = h; }
                if let Some(bg) = cmd.params["background"].as_str() { self.artboard.background = bg.to_string(); }
                "{ \"success\": true }".to_string()
            }
            "set_clipping" => {
                self.save_state("Set Clipping");
                if let Some(v) = cmd.params["enabled"].as_bool() { self.clip_to_artboard = v; }
                "{ \"success\": true }".to_string()
            }
            "vectorize" => {
                if cmd.params["save_undo"].as_bool().unwrap_or(true) { self.save_state("Vectorize Image"); }
                let id = cmd.params["id"].as_u64().map(|v| v as u32).unwrap_or(0);
                let threshold = cmd.params["threshold"].as_f64().unwrap_or(128.0) as u8;
                let obj_info = if let Some(obj) = self.objects.iter().find(|o| o.id == id) { if let Some(raw_image) = &obj.raw_image { Some((obj.x, obj.y, obj.width, obj.height, obj.name.clone(), raw_image.clone())) } else { None } } else { None };
                if let Some((ox, oy, ow, oh, oname, bytes)) = obj_info {
                    if let Ok(img) = image::load_from_memory(&bytes) {
                        let grayscale = img.to_luma8(); let (width, height) = grayscale.dimensions();
                        let tracer = Tracer::new(width, height); let mut path_data = tracer.trace(&grayscale, threshold);
                        if !path_data.is_empty() {
                            if let Ok(mut bez) = BezPath::from_svg(&path_data) {
                                let sx = ow / width as f64; let sy = oh / height as f64;
                                bez.apply_affine(Affine::scale_non_uniform(sx, sy)); path_data = bez.to_svg();
                            }
                            let new_id = self.add_object(ShapeType::Path, ox, oy, ow, oh, "#000000");
                            self.update_object(new_id, &serde_json::json!({ "path_data": path_data, "name": format!("Traced {}", oname), "fill": "transparent", "stroke": "#000000", "stroke_width": 1.0 }));
                            format!("{{\"success\": true, \"id\": {}}}", new_id)
                        } else { "{ \"error\": \"No path generated\" }".to_string() }
                    } else { "{ \"error\": \"Failed to load image\" }".to_string() }
                } else { "{ \"error\": \"Object not found or no raw image data\" }".to_string() }
            }
            "clear" => {
                self.save_state("Clear Document"); self.objects.clear(); self.next_id = 1; self.selected_ids.clear();
                "{ \"success\": true }".to_string()
            }
            "get_brushes" => {
                serde_json::to_string(&self.brush_engine.brushes).unwrap_or("[]".to_string())
            }
            "update_brush" => {
                if let Ok(updated_brush) = serde_json::from_value::<Brush>(cmd.params.clone()) {
                    if let Some(brush) = self.brush_engine.brushes.iter_mut().find(|b| b.id == updated_brush.id) { *brush = updated_brush; "{ \"success\": true }".to_string() }
                    else { "{ \"error\": \"Brush not found\" }".to_string() }
                } else { "{ \"error\": \"Invalid brush JSON\" }".to_string() }
            }
            "create_brush_stroke" => {
                if cmd.params["save_undo"].as_bool().unwrap_or(true) { self.save_state("Brush Stroke"); }
                let brush_id = cmd.params["brush_id"].as_u64().unwrap_or(1) as u32;
                let color = cmd.params["color"].as_str().unwrap_or("#000000");
                if let Some(points_arr) = cmd.params["points"].as_array() {
                    let mut stroke_points = Vec::new(); let mut path = BezPath::new();
                    for (i, p) in points_arr.iter().enumerate() {
                        let pt = Point::new(p["x"].as_f64().unwrap_or(0.0), p["y"].as_f64().unwrap_or(0.0));
                        if i == 0 { path.move_to(pt); } else { path.line_to(pt); }
                        stroke_points.push(StrokePoint { x: pt.x, y: pt.y, pressure: p["pressure"].as_f64().unwrap_or(1.0) });
                    }
                    let bbox = path.bounding_box();
                    let mut path_relative = path.clone(); path_relative.apply_affine(Affine::translate((-bbox.x0, -bbox.y0)));
                    let stroke_points_relative: Vec<StrokePoint> = stroke_points.iter().map(|p| StrokePoint { x: p.x - bbox.x0, y: p.y - bbox.y0, pressure: p.pressure }).collect();
                    let id = self.add_object(ShapeType::Path, bbox.x0, bbox.y0, bbox.width().max(1.0), bbox.height().max(1.0), color);
                    self.update_object(id, &serde_json::json!({ "brush_id": brush_id, "stroke_points": stroke_points_relative, "path_data": path_relative.to_svg(), "fill": color, "name": format!("Brush Stroke {}", id) }));
                    format!("{{\"success\": true, \"id\": {}}}", id)
                } else { "{ \"error\": \"Missing points array\" }".to_string() }
            }
            "update_brush_stroke" => {
                let id = cmd.params["id"].as_u64().unwrap_or(0) as u32;
                if let Some(points_arr) = cmd.params["points"].as_array() {
                    let mut stroke_points = Vec::new(); let mut path = BezPath::new();
                    for (i, p) in points_arr.iter().enumerate() {
                        let pt = Point::new(p["x"].as_f64().unwrap_or(0.0), p["y"].as_f64().unwrap_or(0.0));
                        if i == 0 { path.move_to(pt); } else { path.line_to(pt); }
                        stroke_points.push(StrokePoint { x: pt.x, y: pt.y, pressure: p["pressure"].as_f64().unwrap_or(1.0) });
                    }
                    let bbox = path.bounding_box();
                    let mut path_relative = path.clone(); path_relative.apply_affine(Affine::translate((-bbox.x0, -bbox.y0)));
                    let stroke_points_relative: Vec<StrokePoint> = stroke_points.iter().map(|p| StrokePoint { x: p.x - bbox.x0, y: p.y - bbox.y0, pressure: p.pressure }).collect();
                    if self.update_object(id, &serde_json::json!({ "stroke_points": stroke_points_relative, "path_data": path_relative.to_svg(), "x": bbox.x0, "y": bbox.y0, "width": bbox.width().max(1.0), "height": bbox.height().max(1.0) })) { "{ \"success\": true }".to_string() }
                    else { "{ \"error\": \"Object not found\" }".to_string() }
                } else { "{ \"error\": \"Missing points array\" }".to_string() }
            }
            _ => format!("{{\"error\": \"Unknown action: {}\"}}", cmd.action),
        }
    }

    pub(crate) fn add_object(&mut self, shape_type: ShapeType, x: f64, y: f64, width: f64, height: f64, fill: &str) -> u32 {
        let id = self.next_id;
        let name = format!("{:?} {}", shape_type, id);
        self.objects.push(VectorObject {
            id, shape_type, name, x, y, width, height, rotation: 0.0, fill: fill.to_string(), stroke: "#000000".to_string(), stroke_width: 1.0, opacity: 1.0, visible: true, locked: false, blend_mode: "source-over".to_string(), stroke_cap: "butt".to_string(), stroke_join: "miter".to_string(), stroke_dash: Vec::new(), layer_style: LayerStyle::default(), mask_id: None, is_mask: false, sides: 5, inner_radius: 0.5, corner_radius: 0.0, path_data: String::new(), brush_id: 0, stroke_points: Vec::new(), text_content: "Type here...".to_string(), font_family: "Inter, sans-serif".to_string(), font_size: 24.0, font_weight: "normal".to_string(), text_align: "left".to_string(), kerning: 0.0, leading: 1.2, tracking: 0.0, shadow_color: "transparent".to_string(), shadow_blur: 0.0, shadow_offset_x: 0.0, shadow_offset_y: 0.0, sx: 0.0, sy: 0.0, sw: 0.0, sh: 0.0, brightness: 1.0, contrast: 1.0, saturate: 1.0, hue_rotate: 0.0, blur: 0.0, grayscale: 0.0, sepia: 0.0, invert: 0.0, raw_image: None, raw_rgba: None, raw_rgba_width: 0, raw_rgba_height: 0, image: None, fill_gradient: None, stroke_gradient: None, children: None,
        });
        self.next_id += 1;
        id
    }

    pub(crate) fn update_object(&mut self, id: u32, params: &serde_json::Value) -> bool {
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
            if let Some(v) = params["fill"].as_str() { obj.fill = v.to_string(); obj.fill_gradient = None; }
            if let Some(grad) = params["fill_gradient"].as_object() {
                let mut stops = Vec::new();
                if let Some(arr) = grad.get("stops").and_then(|s| s.as_array()) { for s in arr { if let (Some(offset), Some(color)) = (s["offset"].as_f64(), s["color"].as_str()) { stops.push(GradientStop { offset, color: color.to_string() }); } } }
                obj.fill_gradient = Some(Gradient {
                    is_radial: grad.get("is_radial").and_then(|v| v.as_bool()).unwrap_or(false),
                    x1: grad.get("x1").and_then(|v| v.as_f64()).unwrap_or(0.0), y1: grad.get("y1").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    x2: grad.get("x2").and_then(|v| v.as_f64()).unwrap_or(0.0), y2: grad.get("y2").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    r1: grad.get("r1").and_then(|v| v.as_f64()).unwrap_or(0.0), r2: grad.get("r2").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    stops,
                });
            }
            if let Some(v) = params["stroke"].as_str() { obj.stroke = v.to_string(); obj.stroke_gradient = None; }
            if let Some(grad) = params["stroke_gradient"].as_object() {
                let mut stops = Vec::new();
                if let Some(arr) = grad.get("stops").and_then(|s| s.as_array()) { for s in arr { if let (Some(offset), Some(color)) = (s["offset"].as_f64(), s["color"].as_str()) { stops.push(GradientStop { offset, color: color.to_string() }); } } }
                 obj.stroke_gradient = Some(Gradient {
                    is_radial: grad.get("is_radial").and_then(|v| v.as_bool()).unwrap_or(false),
                    x1: grad.get("x1").and_then(|v| v.as_f64()).unwrap_or(0.0), y1: grad.get("y1").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    x2: grad.get("x2").and_then(|v| v.as_f64()).unwrap_or(0.0), y2: grad.get("y2").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    r1: grad.get("r1").and_then(|v| v.as_f64()).unwrap_or(0.0), r2: grad.get("r2").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    stops,
                });
            }
            if let Some(v) = params["stroke_width"].as_f64() { obj.stroke_width = v; }
            if let Some(v) = params["opacity"].as_f64() { obj.opacity = v; }
            if let Some(v) = params["visible"].as_bool() { obj.visible = v; }
            if let Some(v) = params["blend_mode"].as_str() { obj.blend_mode = v.to_string(); }
            if let Some(v) = params["stroke_cap"].as_str() { obj.stroke_cap = v.to_string(); }
            if let Some(v) = params["stroke_join"].as_str() { obj.stroke_join = v.to_string(); }
            if let Some(arr) = params["stroke_dash"].as_array() { obj.stroke_dash = arr.iter().filter_map(|v| v.as_f64()).collect(); }
            if let Some(v) = params["name"].as_str() { obj.name = v.to_string(); }
            if let Some(v) = params["locked"].as_bool() { obj.locked = v; }
            if let Some(v) = params["sides"].as_u64() { obj.sides = v as u32; }
            if let Some(v) = params["inner_radius"].as_f64() { obj.inner_radius = v; }
            if let Some(v) = params["corner_radius"].as_f64() { obj.corner_radius = v; }
            if let Some(v) = params["path_data"].as_str() { obj.path_data = v.to_string(); }
            if let Some(v) = params["brush_id"].as_u64() { obj.brush_id = v as u32; }
            if let Some(pts) = params["stroke_points"].as_array() {
                obj.stroke_points = pts.iter().map(|p| StrokePoint { x: p["x"].as_f64().unwrap_or(0.0), y: p["y"].as_f64().unwrap_or(0.0), pressure: p["pressure"].as_f64().unwrap_or(1.0) }).collect();
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
            if let Some(style_val) = params.get("layer_style") { if let Ok(style) = serde_json::from_value::<LayerStyle>(style_val.clone()) { obj.layer_style = style; } }
            true
        } else { false }
    }

    pub(crate) fn delete_object(&mut self, id: u32) -> bool {
        let initial_len = self.objects.len();
        self.objects.retain(|o| o.id != id);
        self.selected_ids.retain(|&sid| sid != id);
        self.objects.len() < initial_len
    }

    pub fn register_brush(&mut self, brush_json: &str) -> u32 {
        if let Ok(mut brush) = serde_json::from_str::<Brush>(brush_json) {
            let id = self.brush_engine.brushes.iter().map(|b| b.id).max().unwrap_or(0) + 1;
            brush.id = id; self.brush_engine.brushes.push(brush); id
        } else { 0 }
    }

    pub fn register_brush_tip(&mut self, id: &str, image: HtmlImageElement) { self.brush_image_map.insert(id.to_string(), image); }
}
