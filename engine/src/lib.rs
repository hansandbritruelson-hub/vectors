use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};
use web_sys::{HtmlImageElement, Path2d};
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
    pub stroke: String,
    pub stroke_width: f64,
    pub opacity: f64,
    pub visible: bool,
    pub locked: bool,
    pub blend_mode: String, // New: CSS blend mode
    pub stroke_cap: String, // New: "butt", "round", "square"
    pub stroke_join: String, // New: "miter", "round", "bevel"
    pub stroke_dash: Vec<f64>, // New: Array of dash lengths
    // Shape specific
    pub sides: u32,       // For Polygon
    pub inner_radius: f64, // For Star (0.0 to 1.0 ratio)
    pub corner_radius: f64, // For Rectangle
    pub path_data: String, // For Path (SVG d)
    // Source rect for cropping
    pub sx: f64,
    pub sy: f64,
    pub sw: f64,
    pub sh: f64,
    pub image_data_url: Option<String>, // Base64 data for transport
    #[serde(skip)]
    pub image: Option<HtmlImageElement>,
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
    selected_id: Option<u32>,
    artboard: Artboard,
    clip_to_artboard: bool,
}

#[wasm_bindgen]
pub struct VectorEngine {
    objects: Vec<VectorObject>,
    next_id: u32,
    selected_id: Option<u32>,
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
            selected_id: None,
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
            selected_id: self.selected_id,
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
                selected_id: self.selected_id,
                artboard: self.artboard.clone(),
                clip_to_artboard: self.clip_to_artboard,
            };
            self.redo_stack.push(current_state);

            self.objects = prev_state.objects;
            self.next_id = prev_state.next_id;
            self.selected_id = prev_state.selected_id;
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
                selected_id: self.selected_id,
                artboard: self.artboard.clone(),
                clip_to_artboard: self.clip_to_artboard,
            };
            self.undo_stack.push(current_state);

            self.objects = next_state.objects;
            self.next_id = next_state.next_id;
            self.selected_id = next_state.selected_id;
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
                let id = cmd.params["id"].as_u64().map(|v| v as u32).unwrap_or(0);
                if self.update_object(id, &cmd.params) {
                    "{{\"success\": true}}".to_string()
                } else {
                    "{{\"error\": \"Object not found\"}}".to_string()
                }
            }
            "delete" => {
                self.save_state();
                let id = cmd.params["id"].as_u64().map(|v| v as u32).unwrap_or(0);
                if self.delete_object(id) {
                    "{{\"success\": true}}".to_string()
                } else {
                    "{{\"error\": \"Object not found\"}}".to_string()
                }
            }
            "select" => {
                let id = cmd.params["id"].as_u64().map(|v| v as u32);
                self.selected_id = id;
                "{{\"success\": true}}".to_string()
            }
            "move_to_back" => {
                self.save_state();
                let id = cmd.params["id"].as_u64().map(|v| v as u32).unwrap_or(0);
                if let Some(pos) = self.objects.iter().position(|o| o.id == id) {
                    let obj = self.objects.remove(pos);
                    self.objects.insert(0, obj);
                    "{{\"success\": true}}".to_string()
                } else {
                    "{{\"error\": \"Object not found\"}}".to_string()
                }
            }
            "move_to_front" => {
                self.save_state();
                let id = cmd.params["id"].as_u64().map(|v| v as u32).unwrap_or(0);
                if let Some(pos) = self.objects.iter().position(|o| o.id == id) {
                    let obj = self.objects.remove(pos);
                    self.objects.push(obj);
                    "{{\"success\": true}}".to_string()
                } else {
                    "{{\"error\": \"Object not found\"}}".to_string()
                }
            }
            "set_artboard" => {
                self.save_state();
                if let Some(w) = cmd.params["width"].as_f64() { self.artboard.width = w; }
                if let Some(h) = cmd.params["height"].as_f64() { self.artboard.height = h; }
                if let Some(bg) = cmd.params["background"].as_str() { self.artboard.background = bg.to_string(); }
                "{{\"success\": true}}".to_string()
            }
            "set_clipping" => {
                self.save_state();
                if let Some(v) = cmd.params["enabled"].as_bool() { self.clip_to_artboard = v; }
                "{{\"success\": true}}".to_string()
            }
            "clear" => {
                self.save_state();
                self.objects.clear();
                self.next_id = 1;
                self.selected_id = None;
                "{{\"success\": true}}".to_string()
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
            "{{\"error\": \"Unsupported file format\"}}".to_string()
        }
    }

    fn import_psd(&mut self, data: &[u8]) -> String {
        let psd = match Psd::from_bytes(data) {
            Ok(p) => p,
            Err(_) => return "{{\"error\": \"Failed to parse PSD\"}}".to_string(),
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
                        sx: 0.0,
                        sy: 0.0,
                        sw: width as f64,
                        sh: height as f64,
                        image_data_url: Some(data_url),
                        image: None,
                    };
                    
                    self.objects.push(obj.clone());
                    imported_objects.push(obj);
                }
            }
        }
        
        match serde_json::to_string(&imported_objects) {
            Ok(s) => s,
            Err(_) => "{{\"error\": \"Serialization failed\"}}".to_string(),
        }
    }

    fn import_ai(&mut self, data: &[u8]) -> String {
        let doc = match Document::load_mem(data) {
             Ok(d) => d,
             Err(_) => return "{{\"error\": \"Failed to parse AI/PDF file\"}}".to_string(),
        };
        
        let pages = doc.get_pages();
        if pages.is_empty() {
             return "{{\"error\": \"No pages found in AI file\"}}".to_string();
        }
        let page_id = *pages.values().next().unwrap();
        
        let content_data = match doc.get_page_content(page_id) {
            Ok(c) => c,
            Err(_) => return "{{\"error\": \"Failed to get page content\"}}".to_string(),
        };
        
        let content = match Content::decode(&content_data) {
             Ok(c) => c,
             Err(_) => return "{{\"error\": \"Failed to decode page content\"}}".to_string(),
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
                                sx: 0.0, sy: 0.0, sw: 0.0, sh: 0.0, image_data_url: None, image: None,
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
                                sx: 0.0, sy: 0.0, sw: 0.0, sh: 0.0, image_data_url: None, image: None,
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
                                sx: 0.0, sy: 0.0, sw: 0.0, sh: 0.0, image_data_url: None, image: None,
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
            sx: 0.0,
            sy: 0.0,
            sw: 0.0,
            sh: 0.0,
            image_data_url: None,
            image: None,
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
            if let Some(v) = params["fill"].as_str() { obj.fill = v.to_string(); }
            if let Some(v) = params["stroke"].as_str() { obj.stroke = v.to_string(); }
            if let Some(v) = params["stroke_width"].as_f64() { obj.stroke_width = v; }
            if let Some(v) = params["opacity"].as_f64() { obj.opacity = v; }
            if let Some(v) = params["name"].as_str() { obj.name = v.to_string(); }
            if let Some(v) = params["locked"].as_bool() { obj.locked = v; }
            if let Some(v) = params["sides"].as_u64() { obj.sides = v as u32; }
            if let Some(v) = params["inner_radius"].as_f64() { obj.inner_radius = v; }
            if let Some(v) = params["corner_radius"].as_f64() { obj.corner_radius = v; }
            if let Some(v) = params["path_data"].as_str() { obj.path_data = v.to_string(); }
            true
        } else {
            false
        }
    }

    fn delete_object(&mut self, id: u32) -> bool {
        let initial_len = self.objects.len();
        self.objects.retain(|o| o.id != id);
        if self.selected_id == Some(id) {
            self.selected_id = None;
        }
        self.objects.len() < initial_len
    }

    pub fn select_at(&mut self, x: f64, y: f64) -> Option<u32> {
        // Adjust x,y for viewport
        let tx = (x - self.viewport_x) / self.viewport_zoom;
        let ty = (y - self.viewport_y) / self.viewport_zoom;

        for obj in self.objects.iter().rev() {
            if obj.locked { continue; }
            // Simple AABB for now, ideally would handle rotation
            if tx >= obj.x && tx <= obj.x + obj.width && ty >= obj.y && ty <= obj.y + obj.height {
                self.selected_id = Some(obj.id);
                return Some(obj.id);
            }
        }
        self.selected_id = None;
        None
    }

    pub fn get_selected_id(&self) -> i32 {
        self.selected_id.map(|id| id as i32).unwrap_or(-1)
    }

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
            if !obj.visible { continue; }
            
            ctx.save();
            ctx.set_global_alpha(obj.opacity);
            ctx.set_global_composite_operation(&obj.blend_mode).unwrap_or(());
            
            // Transform for object
            ctx.translate(obj.x + obj.width / 2.0, obj.y + obj.height / 2.0).unwrap();
            ctx.rotate(obj.rotation).unwrap();
            ctx.translate(-obj.width / 2.0, -obj.height / 2.0).unwrap();

            ctx.set_fill_style(&JsValue::from_str(&obj.fill));
            ctx.set_stroke_style(&JsValue::from_str(&obj.stroke));
            ctx.set_line_width(obj.stroke_width);
            ctx.set_line_cap(&obj.stroke_cap);
            ctx.set_line_join(&obj.stroke_join);
            
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
            }

            // Draw selection highlight (relative to object space)
            if Some(obj.id) == self.selected_id {
                ctx.set_stroke_style(&JsValue::from_str("#4facfe"));
                ctx.set_line_width(2.0 / self.viewport_zoom);
                ctx.set_line_dash(&js_sys::Array::new()).unwrap(); // Reset dash for selection
                ctx.stroke_rect(-2.0, -2.0, obj.width + 4.0, obj.height + 4.0);
            }

            ctx.restore();
        }
        ctx.restore();
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
