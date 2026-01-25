use wasm_bindgen::prelude::*;
use crate::engine::VectorEngine;
use crate::types::{ShapeType, Artboard};
use crate::objects::VectorObject;
use kurbo::{BezPath, Point, Shape, Affine};

#[wasm_bindgen]
impl VectorEngine {
    pub fn export_svg(&self) -> String {
        let mut defs = Vec::new();
        let mut body = String::new();
        for obj in &self.objects { body.push_str(&obj.to_svg_element(&mut defs)); }
        let defs_str = if defs.is_empty() { String::new() } else { format!("<defs>{}</defs>", defs.join("")) };
        format!(
            r##"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\"><rect width=\"100%\" height=\"100%\" fill=\"{}\" />{}{}</svg>"##,
            self.artboard.width, self.artboard.height, self.artboard.width, self.artboard.height,
            self.artboard.background, defs_str, body
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
            if parts.len() == 4 { width = parts[2]; height = parts[3]; }
        }
        let mut objects = Vec::new();
        let mut next_id = self.next_id;
        self.parse_svg_node(root, &mut objects, &mut next_id);
        self.next_id = next_id;
        let result = serde_json::json!({ "width": width, "height": height, "objects": objects });
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
                    let mut obj = self.create_default_object(*next_id, ShapeType::Rectangle, x, y, w, h);
                    obj.corner_radius = child.attribute("rx").and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                    obj.name = format!("Rectangle {}", *next_id);
                    self.apply_svg_styles(child, &mut obj); objects.push(obj); *next_id += 1;
                }
                "circle" | "ellipse" => {
                    let cx = child.attribute("cx").and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                    let cy = child.attribute("cy").and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                    let (rx, ry) = if child.tag_name().name() == "circle" {
                        let r = child.attribute("r").and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0); (r, r)
                    } else {
                        (child.attribute("rx").and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0), child.attribute("ry").and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0))
                    };
                    let mut obj = self.create_default_object(*next_id, ShapeType::Circle, cx - rx, cy - ry, rx * 2.0, ry * 2.0);
                    obj.name = format!("{} {}", if child.tag_name().name() == "circle" { "Circle" } else { "Ellipse" }, *next_id);
                    self.apply_svg_styles(child, &mut obj); objects.push(obj); *next_id += 1;
                }
                "path" => {
                    let d = child.attribute("d").unwrap_or("").to_string();
                    if let Ok(bez) = BezPath::from_svg(&d) {
                        let bbox = bez.bounding_box();
                        let mut obj = self.create_default_object(*next_id, ShapeType::Path, bbox.x0, bbox.y0, bbox.width(), bbox.height());
                        obj.name = format!("Path {}", *next_id);
                        let mut normalized = bez.clone(); normalized.apply_affine(Affine::translate((-bbox.x0, -bbox.y0)));
                        obj.path_data = normalized.to_svg();
                        self.apply_svg_styles(child, &mut obj); objects.push(obj); *next_id += 1;
                    }
                }
                "g" => { self.parse_svg_node(child, objects, next_id); }
                _ => { if child.tag_name().name() != "defs" && child.tag_name().name() != "style" { self.parse_svg_node(child, objects, next_id); } }
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
                    match kv[0].trim() { "fill" => fill_val = Some(kv[1].trim()), "stroke" => stroke_val = Some(kv[1].trim()), "stroke-width" => stroke_width_val = Some(kv[1].trim()), "opacity" => opacity_val = Some(kv[1].trim()), _ => {} }
                }
            }
        }
        if let Some(fill) = fill_val { if fill != "none" { obj.fill = fill.to_string(); } else { obj.fill = "transparent".to_string(); } }
        if let Some(stroke) = stroke_val { if stroke != "none" { obj.stroke = stroke.to_string(); } else { obj.stroke = "transparent".to_string(); } }
        if let Some(sw) = stroke_width_val { obj.stroke_width = sw.parse::<f64>().unwrap_or(obj.stroke_width); }
        if let Some(op) = opacity_val { obj.opacity = op.parse::<f64>().unwrap_or(obj.opacity); }
    }
}
