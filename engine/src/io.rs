use wasm_bindgen::prelude::*;
use crate::engine::VectorEngine;
use crate::types::{ShapeType, Artboard};
use crate::objects::VectorObject;
use crate::psd::{Psd, PsdLayer, PsdLayerType, ColorMode};
use crate::ai::{Ai, AiParser};
use image::{RgbaImage, DynamicImage, ImageOutputFormat};
use std::io::Cursor;
use base64::{Engine as _, engine::general_purpose};

#[wasm_bindgen]
impl VectorEngine {
    pub fn import_file(&mut self, filename: &str, data: &[u8]) -> String {
        let filename_lower = filename.to_lowercase();
        if filename_lower.ends_with(".psd") { self.import_psd(data) }
        else if filename_lower.ends_with(".ai") { self.import_ai(data) }
        else if filename_lower.ends_with(".svg") { self.import_svg(data) }
        else { "{\"error\": \"Unsupported file format\"}".to_string() }
    }

        fn import_psd(&mut self, data: &[u8]) -> String {

            let psd = match Psd::from_bytes(data) {

                Ok(p) => p,

                Err(e) => return format!("{{\"error\": \"Failed to parse PSD: {:?}\"}}", e),

            };

    

            let width = psd.width() as u32;
        let height = psd.height() as u32;
        let layers = psd.layers();
        let mut imported_objects = Vec::new();
        let composite_rgba = psd.rgba();
        if let Some(img_buffer) = RgbaImage::from_raw(width, height, composite_rgba.clone()) {
            let dyn_img = DynamicImage::ImageRgba8(img_buffer);
            let mut png_bytes: Vec<u8> = Vec::new();
            if let Ok(_) = dyn_img.write_to(&mut Cursor::new(&mut png_bytes), ImageOutputFormat::Png) {
                let id = self.next_id; self.next_id += 1;
                let mut obj = self.create_default_object(id, ShapeType::Image, 0.0, 0.0, width as f64, height as f64);
                obj.name = "Composite View".to_string(); obj.locked = true; obj.raw_image = Some(png_bytes.clone()); obj.raw_rgba = Some(composite_rgba.clone()); obj.raw_rgba_width = width; obj.raw_rgba_height = height;
                self.objects.push(obj.clone());
                let b64 = general_purpose::STANDARD.encode(&png_bytes);
                let data_url = format!("data:image/png;base64,{}", b64);
                let mut obj_json = serde_json::to_value(&obj).unwrap();
                obj_json["image_data_url"] = serde_json::Value::String(data_url);
                imported_objects.push(obj_json);
            }
        }
        for layer in layers.iter() {
            let l_width = layer.width() as u32;
            let l_height = layer.height() as u32;
            let name = layer.name().to_string();
            let l_x = layer.layer_left() as f64;
            let l_y = layer.layer_top() as f64;
            let opacity = layer.opacity() as f64 / 255.0;
            let visible = layer.visible();
            let blend_mode = match layer.blend_mode() {
                "Normal" => "source-over", "Multiply" => "multiply", "Screen" => "screen", "Overlay" => "overlay", "Darken" => "darken", "Lighten" => "lighten", "ColorDodge" => "color-dodge", "ColorBurn" => "color-burn", "HardLight" => "hard-light", "SoftLight" => "soft-light", "Difference" => "difference", "Exclusion" => "exclusion", "Hue" => "hue", "Saturation" => "saturation", "Color" => "color", "Luminosity" => "luminosity", _ => "source-over",
            }.to_string();
            if let PsdLayerType::Normal = layer.layer_type() {
                if l_width > 0 && l_height > 0 {
                    let rgba = layer.rgba();
                    if let Some(img_buffer) = RgbaImage::from_raw(l_width, l_height, rgba.clone()) {
                        let dyn_img = DynamicImage::ImageRgba8(img_buffer);
                        let mut png_bytes: Vec<u8> = Vec::new();
                        if let Ok(_) = dyn_img.write_to(&mut Cursor::new(&mut png_bytes), ImageOutputFormat::Png) {
                            let id = self.next_id; self.next_id += 1;
                            let mut obj = self.create_default_object(id, ShapeType::Image, l_x, l_y, l_width as f64, l_height as f64);
                            obj.name = name; obj.opacity = opacity; obj.visible = visible; obj.blend_mode = blend_mode; obj.raw_image = Some(png_bytes.clone()); obj.raw_rgba = Some(rgba.clone()); obj.raw_rgba_width = l_width; obj.raw_rgba_height = l_height;
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
        let response = serde_json::json!({ "width": width, "height": height, "objects": imported_objects });
        serde_json::to_string(&response).unwrap_or("{\"error\": \"Serialization failed\"}".to_string())
    }

    pub fn export_psd(&self) -> Vec<u8> {
        let mut layers = Vec::new();
        for obj in &self.objects {
            if let Some(rgba) = &obj.raw_rgba {
                layers.push(PsdLayer {
                    name: obj.name.clone(), top: obj.y as i32, left: obj.x as i32, bottom: (obj.y + obj.height) as i32, right: (obj.x + obj.width) as i32,
                    width: obj.raw_rgba_width, height: obj.raw_rgba_height, opacity: (obj.opacity * 255.0) as u8, visible: obj.visible,
                    blend_mode: match obj.blend_mode.as_str() { "multiply" => "Multiply".to_string(), "screen" => "Screen".to_string(), "overlay" => "Overlay".to_string(), "darken" => "Darken".to_string(), "lighten" => "Lighten".to_string(), "color-dodge" => "ColorDodge".to_string(), "color-burn" => "ColorBurn".to_string(), "hard-light" => "HardLight".to_string(), "soft-light" => "SoftLight".to_string(), "difference" => "Difference".to_string(), "exclusion" => "Exclusion".to_string(), "hue" => "Hue".to_string(), "saturation" => "Saturation".to_string(), "color" => "Color".to_string(), "luminosity" => "Luminosity".to_string(), _ => "Normal".to_string(), },
                    rgba: rgba.clone(), layer_type: PsdLayerType::Normal,
                });
            }
        }
        let total_pixels = (self.artboard.width * self.artboard.height) as usize;
        let mut composite_rgba = vec![255u8; total_pixels * 4];
        if let Ok(color) = u32::from_str_radix(self.artboard.background.trim_start_matches('#'), 16) {
            let r = ((color >> 16) & 0xff) as u8; let g = ((color >> 8) & 0xff) as u8; let b = (color & 0xff) as u8;
            for i in 0..total_pixels { composite_rgba[i * 4] = r; composite_rgba[i * 4 + 1] = g; composite_rgba[i * 4 + 2] = b; composite_rgba[i * 4 + 3] = 255; }
        }
        let psd = Psd { width: self.artboard.width as u32, height: self.artboard.height as u32, layers, composite_rgba, color_mode: ColorMode::Rgb };
        psd.to_bytes().unwrap_or_default()
    }

    pub fn export_ai(&self) -> Vec<u8> { Ai::export(self.artboard.width, self.artboard.height, &self.objects) }

    fn import_ai(&mut self, data: &[u8]) -> String {
        let mut parser = AiParser::new(data);
        match parser.parse() {
            Ok(ai) => {
                for obj in &ai.objects { self.objects.push(obj.clone()); }
                let response = serde_json::json!({ "width": ai.width, "height": ai.height, "objects": ai.objects });
                serde_json::to_string(&response).unwrap_or("{\"error\": \"Serialization failed\"}".to_string())
            }
            Err(e) => format!("{{\"error\": \"Failed to parse AI: {:?}\"}}", e),
        }
    }

    pub(crate) fn create_default_object(&self, id: u32, shape_type: ShapeType, x: f64, y: f64, width: f64, height: f64) -> VectorObject {
        VectorObject {
            id, shape_type, name: format!("{:?} {}", shape_type, id), x, y, width, height, rotation: 0.0, fill: "#000000".to_string(), stroke: "transparent".to_string(), stroke_width: 0.0, opacity: 1.0, visible: true, locked: false, blend_mode: "source-over".to_string(), stroke_cap: "butt".to_string(), stroke_join: "miter".to_string(), stroke_dash: Vec::new(), layer_style: crate::types::LayerStyle::default(), mask_id: None, is_mask: false, sides: 4, inner_radius: 0.0, corner_radius: 0.0, path_data: String::new(), brush_id: 0, stroke_points: Vec::new(), text_content: String::new(), font_family: "Inter, sans-serif".to_string(), font_size: 24.0, font_weight: "normal".to_string(), text_align: "left".to_string(), kerning: 0.0, leading: 1.2, tracking: 0.0, shadow_color: "transparent".to_string(), shadow_blur: 0.0, shadow_offset_x: 0.0, shadow_offset_y: 0.0, sx: 0.0, sy: 0.0, sw: width.max(1.0), sh: height.max(1.0), brightness: 1.0, contrast: 1.0, saturate: 1.0, hue_rotate: 0.0, blur: 0.0, grayscale: 0.0, sepia: 0.0, invert: 0.0, raw_image: None, raw_rgba: None, raw_rgba_width: 0, raw_rgba_height: 0, image: None, fill_gradient: None, stroke_gradient: None, children: None,
        }
    }
}
