use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};
use web_sys::{HtmlImageElement, Path2d, CanvasGradient};
use psd::Psd;
use image::{ImageOutputFormat, DynamicImage, RgbaImage};
use std::io::Cursor;
use base64::{Engine as _, engine::general_purpose};
use lopdf::{Document, content::Content};
use kurbo::{BezPath, Shape, Affine};

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
    pub fill_gradient: Option<Gradient>, // New: Gradient support
    pub stroke: String,
    pub stroke_gradient: Option<Gradient>, // New: Gradient support
    pub stroke_width: f64,
    pub opacity: f64,
    pub visible: bool,
    pub locked: bool,
    pub blend_mode: String,
    pub stroke_cap: String,
    pub stroke_join: String,
    pub stroke_dash: Vec<f64>,
    // Shape specific
    pub sides: u32,
    pub inner_radius: f64,
    pub corner_radius: f64,
    pub path_data: String,
    // Text specific
    pub text_content: String,
    pub font_family: String,
    pub font_size: f64,
    pub font_weight: String,
    pub text_align: String,
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
    pub image_data_url: Option<String>,
    #[serde(skip)]
    pub image: Option<HtmlImageElement>,
    // Grouping
    pub children: Option<Vec<VectorObject>>, // New: Grouping support
}

impl VectorObject {
    fn get_world_bounds(&self) -> (f64, f64, f64, f64) {
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
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Artboard {
    pub width: f64,
    pub height: f64,
    pub background: String,
}

#[derive(Clone)]
struct EngineState {
    objects: Vec<VectorObject>,
    next_id: u32,
    selected_ids: Vec<u32>,
    artboard: Artboard,
    clip_to_artboard: bool,
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
    undo_stack: Vec<EngineState>,
    redo_stack: Vec<EngineState>,
}

#[wasm_bindgen]
impl VectorEngine {
    #[wasm_bindgen(constructor)]
    pub fn new() -> VectorEngine {
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
            },
            clip_to_artboard: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    fn save_state(&mut self) {
        let state = EngineState {
            objects: self.objects.clone(),
            next_id: self.next_id,
            selected_ids: self.selected_ids.clone(),
            artboard: self.artboard.clone(),
            clip_to_artboard: self.clip_to_artboard,
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

    pub fn execute_command(&mut self, cmd_json: &str) -> String {
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
            "add" => {
                self.save_state();
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
                    self.save_state();
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
                self.save_state();
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
                self.save_state();
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
                self.save_state();
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
                self.save_state();
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
                self.save_state();
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
                self.save_state();
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
                self.save_state();
                if let Some(w) = cmd.params["width"].as_f64() { self.artboard.width = w; }
                if let Some(h) = cmd.params["height"].as_f64() { self.artboard.height = h; }
                if let Some(bg) = cmd.params["background"].as_str() { self.artboard.background = bg.to_string(); }
                "{\"success\": true}".to_string()
            }
            "set_clipping" => {
                self.save_state();
                if let Some(v) = cmd.params["enabled"].as_bool() { self.clip_to_artboard = v; }
                "{\"success\": true}".to_string()
            }
            "clear" => {
                self.save_state();
                self.objects.clear();
                self.next_id = 1;
                self.selected_ids.clear();
                "{\"success\": true}".to_string()
            }
            _ => format!("{{\"error\": \"Unknown action: {}\"}}", cmd.action),
        }
    }

    pub fn get_artboard(&self) -> String {
        serde_json::to_string(&self.artboard).unwrap_or("{}".to_string())
    }

    pub fn import_file(&mut self, filename: &str, data: &[u8]) -> String {
        if filename.to_lowercase().ends_with(".psd") {
            self.import_psd(data)
        } else if filename.to_lowercase().ends_with(".ai") {
            self.import_ai(data)
        } else {
            "{\"error\": \"Unsupported file format\"}".to_string()
        }
    }

    fn import_psd(&mut self, data: &[u8]) -> String {
        let psd = match Psd::from_bytes(data) {
            Ok(p) => p,
            Err(_) => return "{\"error\": \"Failed to parse PSD\"}".to_string(),
        };

        let mut imported_objects = Vec::new();

        // Iterating layers
        for layer in psd.layers().iter() {
            let width = layer.width() as u32;
            let height = layer.height() as u32;
            let name = layer.name().to_string();
            
            // Skip empty layers
            if width == 0 || height == 0 { continue; }

            // Extract opacity and visibility
            let opacity = layer.opacity() as f64 / 255.0;
            let visible = layer.visible();

            // Extract blend mode via Debug format since we can't import enum directly
            let blend_mode_str = format!("{:?}", layer.blend_mode());
            let blend_mode = match blend_mode_str.as_str() {
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

            let rgba = layer.rgba();
            // Convert to PNG base64
            if let Some(img_buffer) = RgbaImage::from_raw(width, height, rgba) {
                let dyn_img = DynamicImage::ImageRgba8(img_buffer);
                let mut bytes: Vec<u8> = Vec::new();
                if dyn_img.write_to(&mut Cursor::new(&mut bytes), ImageOutputFormat::Png).is_ok() {
                    let b64 = general_purpose::STANDARD.encode(&bytes);
                    let data_url = format!("data:image/png;base64,{}", b64);
                    
                    let id = self.next_id;
                    self.next_id += 1;

                    let obj = VectorObject {
                        id,
                        shape_type: ShapeType::Image,
                        name: name.clone(),
                        x: layer.layer_left() as f64,
                        y: layer.layer_top() as f64,
                        width: width as f64,
                        height: height as f64,
                        rotation: 0.0,
                        fill: "".to_string(),
                        stroke: "".to_string(),
                        stroke_width: 0.0,
                        opacity,
                        visible, 
                        locked: false,
                        blend_mode,
                        stroke_cap: "butt".to_string(),
                        stroke_join: "miter".to_string(),
                        stroke_dash: Vec::new(),
                        sides: 4,
                        inner_radius: 0.0,
                        corner_radius: 0.0,
                        path_data: String::new(),
                        text_content: String::new(),
                        font_family: "Inter, sans-serif".to_string(),
                        font_size: 24.0,
                        font_weight: "normal".to_string(),
                        text_align: "left".to_string(),
                        shadow_color: "transparent".to_string(),
                        shadow_blur: 0.0,
                        shadow_offset_x: 0.0,
                        shadow_offset_y: 0.0,
                        sx: 0.0,
                        sy: 0.0,
                        sw: width as f64,
                        sh: height as f64,
                        brightness: 1.0,
                        contrast: 1.0,
                        saturate: 1.0,
                        hue_rotate: 0.0,
                        blur: 0.0,
                        grayscale: 0.0,
                        sepia: 0.0,
                        invert: 0.0,
                        image_data_url: Some(data_url),
                        image: None,
                        fill_gradient: None,
                        stroke_gradient: None,
                        children: None,
                    };
                    
                    self.objects.push(obj.clone());
                    imported_objects.push(obj);
                }
            }
        }
        
        match serde_json::to_string(&imported_objects) {
            Ok(s) => s,
            Err(_) => "{\"error\": \"Serialization failed\"}".to_string(),
        }
    }

    fn import_ai(&mut self, data: &[u8]) -> String {
        let doc = match Document::load_mem(data) {
             Ok(d) => d,
             Err(_) => return "{\"error\": \"Failed to parse AI/PDF file\"}".to_string(),
        };
        
        let pages = doc.get_pages();
        if pages.is_empty() {
             return "{\"error\": \"No pages found in AI file\"}".to_string();
        }
        let page_id = *pages.values().next().unwrap();
        
        let content_data = match doc.get_page_content(page_id) {
            Ok(c) => c,
            Err(_) => return "{\"error\": \"Failed to get page content\"}".to_string(),
        };
        
        let content = match Content::decode(&content_data) {
             Ok(c) => c,
             Err(_) => return "{\"error\": \"Failed to decode page content\"}".to_string(),
        };
        
        #[derive(Clone)]
        struct GraphicsState {
             transform: Affine,
             stroke_width: f64,
             stroke: String,
             fill: String,
             stroke_cap: String,
             stroke_join: String,
             stroke_dash: Vec<f64>,
        }

        impl Default for GraphicsState {
             fn default() -> Self {
                 GraphicsState {
                     transform: Affine::IDENTITY,
                     stroke_width: 1.0,
                     stroke: "#000000".to_string(),
                     fill: "#000000".to_string(),
                     stroke_cap: "butt".to_string(),
                     stroke_join: "miter".to_string(),
                     stroke_dash: Vec::new(),
                 }
             }
        }

        let mut state_stack = vec![GraphicsState::default()];
        let mut imported_objects = Vec::new();
        let mut current_path = String::new();
        let mut last_x = 0.0;
        let mut last_y = 0.0;
        
        fn get_nums(objs: &[lopdf::Object]) -> Vec<f64> {
            objs.iter().filter_map(|o| match o {
                lopdf::Object::Real(f) => Some(*f as f64),
                lopdf::Object::Integer(i) => Some(*i as f64),
                _ => None
            }).collect()
        }

        fn to_color(nums: &[f64]) -> String {
             if nums.len() == 1 {
                 let v = (nums[0] * 255.0) as u8;
                 format!("#{:02x}{:02x}{:02x}", v, v, v)
             } else if nums.len() == 3 {
                 let r = (nums[0] * 255.0) as u8;
                 let g = (nums[1] * 255.0) as u8;
                 let b = (nums[2] * 255.0) as u8;
                 format!("#{:02x}{:02x}{:02x}", r, g, b)
             } else if nums.len() == 4 {
                 let c = nums[0]; let m = nums[1]; let y = nums[2]; let k = nums[3];
                 let r = ((1.0 - c) * (1.0 - k) * 255.0) as u8;
                 let g = ((1.0 - m) * (1.0 - k) * 255.0) as u8;
                 let b = ((1.0 - y) * (1.0 - k) * 255.0) as u8;
                 format!("#{:02x}{:02x}{:02x}", r, g, b)
             } else {
                 "#000000".to_string()
             }
        }
        
        for operation in content.operations.iter() {
            let op = operation.operator.as_str();
            let args = &operation.operands;
            let nums = get_nums(args);
            
            // Get current state
            let current_state = state_stack.last_mut().unwrap();

            match op {
                "q" => { // Push state
                     state_stack.push(state_stack.last().unwrap().clone());
                },
                "Q" => { // Pop state
                     if state_stack.len() > 1 {
                         state_stack.pop();
                     }
                },
                "cm" => { // Concatenate matrix
                     if nums.len() >= 6 {
                         // PDF matrix: [a b c d e f]
                         let mat = Affine::new([nums[0], nums[1], nums[2], nums[3], nums[4], nums[5]]);
                         current_state.transform = current_state.transform * mat;
                     }
                },
                "m" => {
                    if nums.len() >= 2 {
                        current_path.push_str(&format!("M {} {} ", nums[0], -nums[1]));
                        last_x = nums[0]; last_y = nums[1];
                    }
                },
                "l" => {
                    if nums.len() >= 2 {
                        current_path.push_str(&format!("L {} {} ", nums[0], -nums[1]));
                        last_x = nums[0]; last_y = nums[1];
                    }
                },
                "c" => {
                    if nums.len() >= 6 {
                         current_path.push_str(&format!("C {} {}, {} {}, {} {} ", nums[0], -nums[1], nums[2], -nums[3], nums[4], -nums[5]));
                         last_x = nums[4]; last_y = nums[5];
                    }
                },
                "v" => {
                    if nums.len() >= 4 {
                         current_path.push_str(&format!("C {} {}, {} {}, {} {} ", last_x, -last_y, nums[0], -nums[1], nums[2], -nums[3]));
                         last_x = nums[2]; last_y = nums[3];
                    }
                },
                "y" => {
                    if nums.len() >= 4 {
                         current_path.push_str(&format!("C {} {}, {} {}, {} {} ", nums[0], -nums[1], nums[2], -nums[3], nums[2], -nums[3]));
                         last_x = nums[2]; last_y = nums[3];
                    }
                },
                "h" => {
                    current_path.push_str("Z ");
                },
                "re" => {
                    if nums.len() >= 4 {
                        let (x, y, w, h) = (nums[0], nums[1], nums[2], nums[3]);
                        current_path.push_str(&format!("M {} {} ", x, -y));
                        current_path.push_str(&format!("L {} {} ", x + w, -y));
                        current_path.push_str(&format!("L {} {} ", x + w, -(y + h)));
                        current_path.push_str(&format!("L {} {} ", x, -(y + h)));
                        current_path.push_str("Z ");
                    }
                },
                "w" => { if !nums.is_empty() { current_state.stroke_width = nums[0]; } },
                "rg" | "k" | "g" => { current_state.fill = to_color(&nums); },
                "RG" | "K" | "G" => { current_state.stroke = to_color(&nums); },
                "J" => { // Line Cap
                    if !nums.is_empty() {
                        current_state.stroke_cap = match nums[0] as i32 {
                            0 => "butt",
                            1 => "round",
                            2 => "square",
                            _ => "butt",
                        }.to_string();
                    }
                },
                "j" => { // Line Join
                    if !nums.is_empty() {
                         current_state.stroke_join = match nums[0] as i32 {
                             0 => "miter",
                             1 => "round",
                             2 => "bevel",
                             _ => "miter",
                         }.to_string();
                    }
                },
                "d" => { // Dash [array] phase
                     if args.len() >= 1 {
                         if let lopdf::Object::Array(arr) = &args[0] {
                             current_state.stroke_dash = get_nums(arr);
                         } else {
                             current_state.stroke_dash = Vec::new(); // empty array = solid
                         }
                     }
                },
                
                "S" | "s" => {
                    if !current_path.is_empty() {
                        if let Ok(mut bez) = BezPath::from_svg(&current_path) {
                            let transform = current_state.transform;
                            bez.apply_affine(transform);
                            
                            let rect = bez.bounding_box();
                            let x = rect.x0; let y = rect.y0;
                            let w = rect.width(); let h = rect.height();
                            bez.apply_affine(Affine::translate((-x, -y)));
                            let new_path = bez.to_svg();

                            let id = self.next_id; self.next_id += 1;
                            
                            // Capture state
                            let s = state_stack.last().unwrap();
                            
                            imported_objects.push(VectorObject {
                                id, shape_type: ShapeType::Path, name: format!("Path {}", id),
                                x, y, width: w, height: h,
                                rotation: 0.0, fill: "transparent".to_string(),
                                stroke: s.stroke.clone(), stroke_width: s.stroke_width,
                                stroke_cap: s.stroke_cap.clone(), stroke_join: s.stroke_join.clone(),
                                stroke_dash: s.stroke_dash.clone(),
                                blend_mode: "source-over".to_string(),
                                opacity: 1.0, visible: true, locked: false,
                                sides: 0, inner_radius: 0.0, corner_radius: 0.0,
                                path_data: new_path,
                                text_content: String::new(),
                                font_family: "Inter, sans-serif".to_string(),
                                font_size: 24.0,
                                font_weight: "normal".to_string(),
                                text_align: "left".to_string(),
                                shadow_color: "transparent".to_string(),
                                shadow_blur: 0.0,
                                shadow_offset_x: 0.0,
                                shadow_offset_y: 0.0,
                                sx: 0.0, sy: 0.0, sw: 0.0, sh: 0.0,
                                brightness: 1.0, contrast: 1.0, saturate: 1.0, hue_rotate: 0.0, blur: 0.0,
                                grayscale: 0.0, sepia: 0.0, invert: 0.0,
                                image_data_url: None, image: None,
                                fill_gradient: None, stroke_gradient: None, children: None,
                            });
                        }
                        current_path.clear();
                    }
                },
                "f" | "F" | "f*" => {
                    if !current_path.is_empty() {
                         if let Ok(mut bez) = BezPath::from_svg(&current_path) {
                            let transform = current_state.transform;
                            bez.apply_affine(transform);

                            let rect = bez.bounding_box();
                            let x = rect.x0; let y = rect.y0;
                            let w = rect.width(); let h = rect.height();
                            bez.apply_affine(Affine::translate((-x, -y)));
                            let new_path = bez.to_svg();

                            let id = self.next_id; self.next_id += 1;
                            let s = state_stack.last().unwrap();

                            imported_objects.push(VectorObject {
                                id, shape_type: ShapeType::Path, name: format!("Path {}", id),
                                x, y, width: w, height: h,
                                rotation: 0.0, fill: s.fill.clone(),
                                stroke: "transparent".to_string(), stroke_width: 0.0,
                                stroke_cap: s.stroke_cap.clone(), stroke_join: s.stroke_join.clone(),
                                stroke_dash: s.stroke_dash.clone(),
                                blend_mode: "source-over".to_string(),
                                opacity: 1.0, visible: true, locked: false,
                                sides: 0, inner_radius: 0.0, corner_radius: 0.0,
                                path_data: new_path,
                                text_content: String::new(),
                                font_family: "Inter, sans-serif".to_string(),
                                font_size: 24.0,
                                font_weight: "normal".to_string(),
                                text_align: "left".to_string(),
                                shadow_color: "transparent".to_string(),
                                shadow_blur: 0.0,
                                shadow_offset_x: 0.0,
                                shadow_offset_y: 0.0,
                                sx: 0.0, sy: 0.0, sw: 0.0, sh: 0.0,
                                brightness: 1.0, contrast: 1.0, saturate: 1.0, hue_rotate: 0.0, blur: 0.0,
                                grayscale: 0.0, sepia: 0.0, invert: 0.0,
                                image_data_url: None, image: None,
                                fill_gradient: None, stroke_gradient: None, children: None,
                            });
                        }
                        current_path.clear();
                    }
                },
                "B" | "B*" | "b" | "b*" => {
                    if !current_path.is_empty() {
                         if let Ok(mut bez) = BezPath::from_svg(&current_path) {
                            let transform = current_state.transform;
                            bez.apply_affine(transform);
                             
                            let rect = bez.bounding_box();
                            let x = rect.x0; let y = rect.y0;
                            let w = rect.width(); let h = rect.height();
                            bez.apply_affine(Affine::translate((-x, -y)));
                            let new_path = bez.to_svg();

                            let id = self.next_id; self.next_id += 1;
                            let s = state_stack.last().unwrap();
                            
                            imported_objects.push(VectorObject {
                                id, shape_type: ShapeType::Path, name: format!("Path {}", id),
                                x, y, width: w, height: h,
                                rotation: 0.0, fill: s.fill.clone(),
                                stroke: s.stroke.clone(), stroke_width: s.stroke_width,
                                stroke_cap: s.stroke_cap.clone(), stroke_join: s.stroke_join.clone(),
                                stroke_dash: s.stroke_dash.clone(),
                                blend_mode: "source-over".to_string(),
                                opacity: 1.0, visible: true, locked: false,
                                sides: 0, inner_radius: 0.0, corner_radius: 0.0,
                                path_data: new_path,
                                text_content: String::new(),
                                font_family: "Inter, sans-serif".to_string(),
                                font_size: 24.0,
                                font_weight: "normal".to_string(),
                                text_align: "left".to_string(),
                                shadow_color: "transparent".to_string(),
                                shadow_blur: 0.0,
                                shadow_offset_x: 0.0,
                                shadow_offset_y: 0.0,
                                sx: 0.0, sy: 0.0, sw: 0.0, sh: 0.0,
                                brightness: 1.0, contrast: 1.0, saturate: 1.0, hue_rotate: 0.0, blur: 0.0,
                                grayscale: 0.0, sepia: 0.0, invert: 0.0,
                                image_data_url: None, image: None,
                                fill_gradient: None, stroke_gradient: None, children: None,
                            });
                        }
                        current_path.clear();
                    }
                },
                "n" => { current_path.clear(); },
                _ => {}
            }
        }
        
        for obj in &imported_objects {
            self.objects.push(obj.clone());
        }

        match serde_json::to_string(&imported_objects) {
             Ok(s) => s,
             Err(_) => "[]".to_string(),
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
            sides: 5,
            inner_radius: 0.5,
            corner_radius: 0.0,
            path_data: String::new(),
            text_content: "Type here...".to_string(),
            font_family: "Inter, sans-serif".to_string(),
            font_size: 24.0,
            font_weight: "normal".to_string(),
            text_align: "left".to_string(),
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
            image_data_url: None,
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
            if let Some(v) = params["shadow_color"].as_str() { obj.shadow_color = v.to_string(); }
            if let Some(v) = params["shadow_blur"].as_f64() { obj.shadow_blur = v; }
            if let Some(v) = params["shadow_offset_x"].as_f64() { obj.shadow_offset_x = v; }
            if let Some(v) = params["shadow_offset_y"].as_f64() { obj.shadow_offset_y = v; }
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

    pub fn select_point(&mut self, x: f64, y: f64, shift: bool) -> String {
        let tx = (x - self.viewport_x) / self.viewport_zoom;
        let ty = (y - self.viewport_y) / self.viewport_zoom;

        let mut hit_id = None;
        for obj in self.objects.iter().rev() {
            if obj.locked { continue; }
            
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

    pub fn select_rect(&mut self, x: f64, y: f64, width: f64, height: f64, shift: bool) -> String {
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
            if obj.locked { continue; }
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


    pub fn get_objects_json(&self) -> String {
        serde_json::to_string(&self.objects).unwrap_or_else(|_| "[]".to_string())
    }

    pub fn set_image_object(&mut self, id: u32, image: HtmlImageElement) -> bool {
        if let Some(obj) = self.objects.iter_mut().find(|o| o.id == id) {
            if obj.sw == 0.0 { obj.sw = image.width() as f64; }
            if obj.sh == 0.0 { obj.sh = image.height() as f64; }
            obj.image = Some(image);
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

        for obj in &self.objects {
            self.render_object(ctx, obj);
        }
        ctx.restore();
        
        // Render Selection Overlay
        self.render_selection_overlay(ctx);

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
        if !obj.visible { return; }
        
        ctx.save();
        ctx.set_global_alpha(obj.opacity);
        ctx.set_global_composite_operation(&obj.blend_mode).unwrap_or(());
        
        // Apply Filters
        if obj.shape_type == ShapeType::Image {
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
        }
        
        // Transform for object
        ctx.translate(obj.x + obj.width / 2.0, obj.y + obj.height / 2.0).unwrap();
        ctx.rotate(obj.rotation).unwrap();
        ctx.translate(-obj.width / 2.0, -obj.height / 2.0).unwrap();

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
                    if let Some(img) = &obj.image {
                        let _ = ctx.draw_image_with_html_image_element_and_sw_and_sh_and_dx_and_dy_and_dw_and_dh(
                            img, obj.sx, obj.sy, obj.sw, obj.sh, 0.0, 0.0, obj.width, obj.height
                        );
                    }
                }
                ShapeType::Path => {
                    if !obj.path_data.is_empty() {
                         if let Ok(p) = Path2d::new_with_path_string(&obj.path_data) {
                             ctx.fill_with_path_2d(&p);
                             if obj.stroke_width > 0.0 { ctx.stroke_with_path(&p); }
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
