use kurbo::{ BezPath, Affine, Shape };
use crate::{VectorObject, ShapeType};
use std::collections::HashMap;

pub struct Ai {
    pub width: f64,
    pub height: f64,
    pub objects: Vec<VectorObject>,
}

struct PdfWriter {
    buffer: Vec<u8>,
    offsets: Vec<usize>,
}

impl PdfWriter {
    fn new() -> Self {
        let mut buffer = Vec::new();
        buffer.extend_from_slice(b"%PDF-1.4\n");
        buffer.extend_from_slice(b"%\xE2\xE3\xCF\xD3\n");
        buffer.extend_from_slice(b"%AI12_FileFormatLevel: 3\n");
        PdfWriter {
            buffer,
            offsets: Vec::new(),
        }
    }

    fn start_obj(&mut self) -> usize {
        let id = self.offsets.len() + 1;
        self.offsets.push(self.buffer.len());
        self.buffer.extend_from_slice(format!("{} 0 obj\n", id).as_bytes());
        id
    }

    fn end_obj(&mut self) {
        self.buffer.extend_from_slice(b"endobj\n");
    }

    fn write_raw(&mut self, data: &[u8]) {
        self.buffer.extend_from_slice(data);
    }

    fn finish(mut self) -> Vec<u8> {
        let xref_pos = self.buffer.len();
        self.buffer.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", self.offsets.len() + 1).as_bytes());
        for offset in &self.offsets {
            self.buffer.extend_from_slice(format!("{:010} 00000 n \n", offset).as_bytes());
        }
        let size = self.offsets.len() + 1;
        self.buffer.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF", size, xref_pos).as_bytes());
        self.buffer
    }
}

impl Ai {
    pub fn export(width: f64, height: f64, objects: &[VectorObject]) -> Vec<u8> {
        let mut writer = PdfWriter::new();
        
        let _catalog_id = writer.start_obj();
        writer.write_raw(b"<< /Type /Catalog /Pages 2 0 R >>\n");
        writer.end_obj();
        
        let pages_id = writer.start_obj();
        writer.write_raw(b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>\n");
        writer.end_obj();

        // Collect unique opacities and images
        let mut opacities: HashMap<String, f64> = HashMap::new();
        let mut raw_images: HashMap<u32, Vec<u8>> = HashMap::new();
        
        fn collect_resources(objs: &[VectorObject], opacities: &mut HashMap<String, f64>, images: &mut HashMap<u32, Vec<u8>>) {
            for obj in objs {
                if obj.opacity < 1.0 {
                    let key = format!("GS{}", (obj.opacity * 1000.0) as i32);
                    opacities.insert(key, obj.opacity);
                }
                if obj.shape_type == ShapeType::Image {
                    if let Some(rgba) = &obj.raw_rgba {
                        images.insert(obj.id, rgba.clone());
                    }
                }
                if let Some(children) = &obj.children {
                    collect_resources(children, opacities, images);
                }
            }
        }
        collect_resources(objects, &mut opacities, &mut raw_images);

        // Write image objects
        let mut image_map: HashMap<u32, usize> = HashMap::new();
        for (obj_id, rgba) in raw_images {
            let img_id = writer.start_obj();
            let width = objects.iter().find(|o| o.id == obj_id).map(|o| o.raw_rgba_width).unwrap_or(1);
            let height = objects.iter().find(|o| o.id == obj_id).map(|o| o.raw_rgba_height).unwrap_or(1);
            
            let mut rgb = Vec::with_capacity((width * height * 3) as usize);
            for i in 0..(width * height) as usize {
                rgb.push(rgba[i * 4]);
                rgb.push(rgba[i * 4 + 1]);
                rgb.push(rgba[i * 4 + 2]);
            }
            
            use std::io::Write;
            let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
            encoder.write_all(&rgb).unwrap();
            let compressed_rgb = encoder.finish().unwrap();

            writer.write_raw(format!("<< /Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode /Length {} >>\nstream\n", 
                width, height, compressed_rgb.len()).as_bytes());
            writer.write_raw(&compressed_rgb);
            writer.write_raw(b"\nendstream\n");
            writer.end_obj();
            image_map.insert(obj_id, img_id);
        }

        let _page_id = writer.start_obj();
        let mut page_dict = format!(
            "<< /Type /Page /Parent {} 0 R /MediaBox [0 0 {} {}] /Contents {} 0 R /Resources << /Font << /F1 << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> >>",
            pages_id, width, height, writer.offsets.len() + 1
        );
        
        if !opacities.is_empty() {
            page_dict.push_str(" /ExtGState << ");
            let mut keys: Vec<_> = opacities.keys().collect();
            keys.sort();
            for key in keys {
                page_dict.push_str(&format!("/{} << /Type /ExtGState /ca {} /CA {} >> ", key, opacities[key], opacities[key]));
            }
            page_dict.push_str(">>");
        }

        if !image_map.is_empty() {
            page_dict.push_str(" /XObject << ");
            for (obj_id, pdf_id) in &image_map {
                page_dict.push_str(&format!("/Im{} {} 0 R ", obj_id, pdf_id));
            }
            page_dict.push_str(">>");
        }
        
        page_dict.push_str(" >> >>\n");
        writer.write_raw(page_dict.as_bytes());
        writer.end_obj();
        
        let mut content = Vec::new();
        fn write_objects(objs: &[VectorObject], content: &mut Vec<u8>, height: f64, opacities: &HashMap<String, f64>, image_map: &HashMap<u32, usize>) {
            for obj in objs {
                if !obj.visible { continue; }
                content.extend_from_slice(b"q\n");
                
                if obj.opacity < 1.0 {
                    let key = format!("GS{}", (obj.opacity * 1000.0) as i32);
                    content.extend_from_slice(format!("/{} gs\n", key).as_bytes());
                }

                if obj.shape_type == ShapeType::Group {
                    if let Some(children) = &obj.children {
                        content.extend_from_slice(format!("1 0 0 1 {} {} cm\n", obj.x, -obj.y).as_bytes());
                        write_objects(children, content, height, opacities, image_map);
                    }
                } else if obj.shape_type == ShapeType::Image {
                    if image_map.contains_key(&obj.id) {
                        content.extend_from_slice(format!("{} 0 0 {} {} {} cm\n", 
                            obj.width, obj.height, obj.x, height - obj.y - obj.height).as_bytes());
                        content.extend_from_slice(format!("/Im{} Do\n", obj.id).as_bytes());
                    }
                } else if obj.shape_type == ShapeType::Text {
                    content.extend_from_slice(b"BT\n");
                    content.extend_from_slice(format!("/F1 {} Tf\n", obj.font_size).as_bytes());
                    let cos_r = obj.rotation.cos();
                    let sin_r = obj.rotation.sin();
                    content.extend_from_slice(format!("{} {} {} {} {} {} Tm\n", 
                        cos_r, -sin_r, sin_r, cos_r, obj.x, height - obj.y).as_bytes());
                    content.extend_from_slice(format!("({})
", obj.text_content.replace("(", "\\(").replace(")", "\\)")).as_bytes());
                    content.extend_from_slice(b"ET\n");
                } else {
                    let fill_rgb = self::parse_hex_color(&obj.fill);
                    let stroke_rgb = self::parse_hex_color(&obj.stroke);
                    
                    content.extend_from_slice(format!("{} {} {} rg\n", fill_rgb.0 as f32 / 255.0, fill_rgb.1 as f32 / 255.0, fill_rgb.2 as f32 / 255.0).as_bytes());
                    content.extend_from_slice(format!("{} {} {} RG\n", stroke_rgb.0 as f32 / 255.0, stroke_rgb.1 as f32 / 255.0, stroke_rgb.2 as f32 / 255.0).as_bytes());
                    content.extend_from_slice(format!("{} w\n", obj.stroke_width).as_bytes());
                    
                    let path_data = match obj.shape_type {
                        ShapeType::Rectangle => {
                            format!("M 0 0 L {} 0 L {} {} L 0 {} Z", obj.width, obj.width, obj.height, obj.height)
                        }
                        ShapeType::Circle | ShapeType::Ellipse => {
                            let r = obj.width / 2.0;
                            let ry = obj.height / 2.0;
                            let kappa = 0.552284749831;
                            let ox = r * kappa;
                            let oy = ry * kappa;
                            format!("M {} {} C {} {} {} {} {} {} C {} {} {} {} {} {} C {} {} {} {} {} {} C {} {} {} {} {} {} Z",
                                0.0, ry,
                                0.0, ry - oy, r - ox, 0.0, r, 0.0,
                                r + ox, 0.0, obj.width, ry - oy, obj.width, ry,
                                obj.width, ry + oy, r + ox, obj.height, r, obj.height,
                                r - ox, obj.height, 0.0, ry + oy, 0.0, ry
                            )
                        }
                        ShapeType::Polygon => {
                            let mut p = String::new();
                            let cx = obj.width / 2.0;
                            let cy = obj.height / 2.0;
                            let r = obj.width / 2.0;
                            for i in 0..obj.sides {
                                let angle = (i as f64 * 2.0 * std::f64::consts::PI / obj.sides as f64) - (std::f64::consts::PI / 2.0);
                                let x = cx + r * angle.cos();
                                let y = cy + r * angle.sin();
                                if i == 0 { p.push_str(&format!("M {} {} ", x, y)); } 
                                else { p.push_str(&format!("L {} {} ", x, y)); }
                            }
                            p.push_str("Z");
                            p
                        }
                        ShapeType::Star => {
                            let mut p = String::new();
                            let cx = obj.width / 2.0;
                            let cy = obj.height / 2.0;
                            let r_outer = obj.width / 2.0;
                            let r_inner = obj.inner_radius * (obj.width / 2.0);
                            for i in 0..(obj.sides * 2) {
                                let r = if i % 2 == 0 { r_outer } else { r_inner };
                                let angle = (i as f64 * std::f64::consts::PI / obj.sides as f64) - (std::f64::consts::PI / 2.0);
                                let x = cx + r * angle.cos();
                                let y = cy + r * angle.sin();
                                if i == 0 { p.push_str(&format!("M {} {} ", x, y)); } 
                                else { p.push_str(&format!("L {} {} ", x, y)); }
                            }
                            p.push_str("Z");
                            p
                        }
                        ShapeType::Path => obj.path_data.clone(),
                        _ => String::new(),
                    };

                    if let Ok(mut path) = BezPath::from_svg(&path_data) {
                        let transform = Affine::translate((obj.x + obj.width / 2.0, obj.y + obj.height / 2.0))
                            * Affine::rotate(obj.rotation)
                            * Affine::translate((-obj.width / 2.0, -obj.height / 2.0));
                        
                        path.apply_affine(transform);
                        
                        for el in path.iter() {
                            match el {
                                kurbo::PathEl::MoveTo(p) => {
                                    content.extend_from_slice(format!("{} {} m\n", p.x, height - p.y).as_bytes());
                                }
                                kurbo::PathEl::LineTo(p) => {
                                    content.extend_from_slice(format!("{} {} l\n", p.x, height - p.y).as_bytes());
                                }
                                kurbo::PathEl::CurveTo(p1, p2, p3) => {
                                    content.extend_from_slice(format!("{} {} {} {} {} {} c\n", 
                                        p1.x, height - p1.y,
                                        p2.x, height - p2.y,
                                        p3.x, height - p3.y
                                    ).as_bytes());
                                }
                                kurbo::PathEl::QuadTo(p1, p2) => {
                                    content.extend_from_slice(format!("{} {} {} {} l\n", p1.x, height - p1.y, p2.x, height - p2.y).as_bytes());
                                }
                                kurbo::PathEl::ClosePath => {
                                    content.extend_from_slice(b"h\n");
                                }
                            }
                        }
                        
                        let op = match (obj.fill.as_str() != "transparent" && !obj.fill.is_empty(), 
                                        obj.stroke.as_str() != "transparent" && obj.stroke_width > 0.0) {
                            (true, true) => "B",
                            (true, false) => "f",
                            (false, true) => "S",
                            (false, false) => "n",
                        };
                        content.extend_from_slice(format!("{}\n", op).as_bytes());
                    }
                }
                content.extend_from_slice(b"Q\n");
            }
        }
        write_objects(objects, &mut content, height, &opacities, &image_map);

        let _content_id = writer.start_obj();
        writer.write_raw(format!("<< /Length {} >>\nstream\n", content.len()).as_bytes());
        writer.write_raw(&content);
        writer.write_raw(b"\nendstream\n");
        writer.end_obj();
        
        writer.finish()
    }
}

fn parse_hex_color(hex: &str) -> (u8, u8, u8) {
    if hex == "transparent" || hex.is_empty() { return (0, 0, 0); }
    let hex = hex.trim_start_matches('#');
    if hex.len() == 6 {
        if let Ok(c) = u32::from_str_radix(hex, 16) {
            return (((c >> 16) & 0xff) as u8, ((c >> 8) & 0xff) as u8, (c & 0xff) as u8);
        }
    } else if hex.len() == 3 {
        if let Ok(c) = u16::from_str_radix(hex, 16) {
            let r = ((c >> 8) & 0xf) as u8;
            let g = ((c >> 4) & 0xf) as u8;
            let b = (c & 0xf) as u8;
            return (r | (r << 4), g | (g << 4), b | (b << 4));
        }
    }
    (0, 0, 0)
}

#[derive(Debug)]
pub enum AiError {
    InvalidSignature,
}

pub struct AiParser<'a> {
    data: &'a [u8],
    mediabox: [f64; 4],
    ext_g_states: HashMap<String, ExtGState>,
    x_objects: HashMap<String, Vec<u8>>,
    x_object_info: HashMap<String, (u32, u32, String)>,
}

#[derive(Clone, Debug)]
struct ExtGState {
    opacity: Option<f64>,
    blend_mode: Option<String>,
}

impl<'a> AiParser<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        AiParser {
            data,
            mediabox: [0.0, 0.0, 800.0, 600.0],
            ext_g_states: HashMap::new(),
            x_objects: HashMap::new(),
            x_object_info: HashMap::new(),
        }
    }

    fn find_object(&self, id: u32) -> Option<(usize, usize)> {
        let pattern = format!("{} 0 obj", id);
        let content = String::from_utf8_lossy(self.data);
        if let Some(pos) = content.find(&pattern) {
            let start = pos;
            if let Some(end) = content[start..].find("endobj") {
                return Some((start, start + end + 6));
            }
        }
        None
    }

    fn get_dict_value(&self, dict: &str, key: &str) -> Option<String> {
        if let Some(pos) = dict.find(key) {
            let val_area = &dict[pos + key.len()..].trim_start();
            let mut end = 0;
            let bytes = val_area.as_bytes();
            while end < bytes.len() && !bytes[end].is_ascii_whitespace() && bytes[end] != b'/' && bytes[end] != b'<' && bytes[end] != b'>' && bytes[end] != b'[' {
                end += 1;
            }
            if end > 0 {
                return Some(val_area[..end].to_string());
            }
        }
        None
    }

    fn parse_resources(&mut self, res_dict: &str) {
        if let Some(egs_pos) = res_dict.find("/ExtGState") {
            let sub = &res_dict[egs_pos..];
            let dict_content = self.extract_bracketed(sub, "<<", ">>");
            if !dict_content.is_empty() {
                self.parse_ext_g_state_resource(&dict_content);
            }
        }

        if let Some(xo_pos) = res_dict.find("/XObject") {
            let sub = &res_dict[xo_pos..];
            let dict_content = self.extract_bracketed(sub, "<<", ">>");
            if !dict_content.is_empty() {
                self.parse_xobject_resource(&dict_content);
            }
        }
    }

    fn extract_bracketed(&self, text: &str, open: &str, close: &str) -> String {
        if let Some(start_pos) = text.find(open) {
            let mut depth = 0;
            let mut i = start_pos;
            let bytes = text.as_bytes();
            let open_bytes = open.as_bytes();
            let close_bytes = close.as_bytes();
            
            while i + close_bytes.len() <= bytes.len() {
                if i + open_bytes.len() <= bytes.len() && &bytes[i..i+open_bytes.len()] == open_bytes {
                    depth += 1;
                    i += open_bytes.len();
                } else if i + close_bytes.len() <= bytes.len() && &bytes[i..i+close_bytes.len()] == close_bytes {
                    depth -= 1;
                    i += close_bytes.len();
                    if depth == 0 {
                        return text[start_pos..i].to_string();
                    }
                } else {
                    i += 1;
                }
            }
        }
        String::new()
    }

    fn parse_xobject_resource(&mut self, dict: &str) {
        let mut i = 0;
        while let Some(key_pos) = dict[i..].find('/') {
            let key_start = i + key_pos + 1;
            let mut key_end = key_start;
            while key_end < dict.len() && !dict.as_bytes()[key_end].is_ascii_whitespace() && dict.as_bytes()[key_end] != b'/' {
                key_end += 1;
            }
            let key = &dict[key_start..key_end];
            
            let val_area = &dict[key_end..].trim_start();
            let parts: Vec<&str> = val_area.split_whitespace().collect();
            if parts.len() >= 3 && parts[1] == "0" && parts[2] == "R" {
                if let Ok(obj_id) = parts[0].parse::<u32>() {
                    self.load_xobject(key, obj_id);
                }
            }
            i = key_end;
        }
    }

    fn load_xobject(&mut self, name: &str, id: u32) {
        if let Some((start, end)) = self.find_object(id) {
            let obj_data = &self.data[start..end];
            let obj_str = String::from_utf8_lossy(obj_data);
            
            if let Some(stream_pos) = obj_str.find("stream") {
                let header = &obj_str[..stream_pos];
                let width = self.get_dict_value(header, "/Width").and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
                let height = self.get_dict_value(header, "/Height").and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
                let filter = self.get_dict_value(header, "/Filter").unwrap_or_default();
                
                self.x_object_info.insert(name.to_string(), (width, height, filter.clone()));

                let stream_start = start + stream_pos + 6;
                let mut real_start = stream_start;
                while real_start < end && (self.data[real_start] == b'\r' || self.data[real_start] == b'\n') {
                    real_start += 1;
                }
                
                if let Some(stream_end_pos) = obj_str[stream_pos..].find("endstream") {
                    let stream_data = &self.data[real_start..start + stream_pos + stream_end_pos];
                    if filter.contains("FlateDecode") {
                        if let Ok(decompressed) = self.decompress_flate(stream_data) {
                            self.x_objects.insert(name.to_string(), decompressed);
                        }
                    } else {
                        self.x_objects.insert(name.to_string(), stream_data.to_vec());
                    }
                }
            }
        }
    }

    fn parse_ext_g_state_resource(&mut self, dict: &str) {
        let mut i = 0;
        while let Some(key_pos) = dict[i..].find('/') {
            let key_start = i + key_pos + 1;
            let mut key_end = key_start;
            while key_end < dict.len() && !dict.as_bytes()[key_end].is_ascii_whitespace() && dict.as_bytes()[key_end] != b'/' {
                key_end += 1;
            }
            let key = &dict[key_start..key_end];
            
            let val_area = &dict[key_end..].trim_start();
            if val_area.starts_with("<<") {
                let gs_dict = self.extract_bracketed(val_area, "<<", ">>");
                self.ext_g_states.insert(key.to_string(), self.parse_ext_g_state_dict(&gs_dict));
            } else {
                let parts: Vec<&str> = val_area.split_whitespace().collect();
                if parts.len() >= 3 && parts[1] == "0" && parts[2] == "R" {
                    if let Ok(obj_id) = parts[0].parse::<u32>() {
                        if let Some((s, e)) = self.find_object(obj_id) {
                            let gs_dict = String::from_utf8_lossy(&self.data[s..e]);
                            self.ext_g_states.insert(key.to_string(), self.parse_ext_g_state_dict(&gs_dict));
                        }
                    }
                }
            }
            i = key_end;
        }
    }

    pub fn parse(&mut self) -> Result<Ai, AiError> {
        if !self.data.starts_with(b"%PDF-") && !self.data.starts_with(b"%AI") {
            return Err(AiError::InvalidSignature);
        }

        let content = String::from_utf8_lossy(self.data);
        let mut root_id = 0;
        if let Some(pos) = content.find("/Root") {
            let parts: Vec<&str> = content[pos+5..].split_whitespace().collect();
            if parts.len() >= 3 && parts[1] == "0" && parts[2] == "R" {
                root_id = parts[0].parse::<u32>().unwrap_or(0);
            }
        }

        if root_id == 0 {
            if let Some(pos) = content.find("/Type /Catalog") {
                let obj_start = content[..pos].rfind("obj").unwrap_or(0);
                let line = &content[..obj_start].trim();
                let parts: Vec<&str> = line.split_whitespace().collect();
                if let Some(last) = parts.last() {
                    if let Ok(id) = last.parse::<u32>() { root_id = id; }
                }
            }
        }

        let mut first_page_id = 0;
        if root_id > 0 {
            if let Some((s, e)) = self.find_object(root_id) {
                let catalog_dict = String::from_utf8_lossy(&self.data[s..e]);
                if let Some(pages_val) = self.get_dict_value(&catalog_dict, "/Pages") {
                    if let Ok(pages_id) = pages_val.parse::<u32>() {
                        if let Some((ps, pe)) = self.find_object(pages_id) {
                            let pages_dict = String::from_utf8_lossy(&self.data[ps..pe]);
                            if let Some(kids_start) = pages_dict.find("/Kids") {
                                let kids_area = self.extract_bracketed(&pages_dict[kids_start..], "[", "]");
                                let parts: Vec<&str> = kids_area.trim_matches(|c| c == '[' || c == ']').split_whitespace().collect();
                                if parts.len() >= 3 && parts[1] == "0" && parts[2] == "R" {
                                    first_page_id = parts[0].parse::<u32>().unwrap_or(0);
                                }
                            }
                        }
                    }
                }
            }
        }

        let mut content_streams = Vec::new();
        if first_page_id > 0 {
            if let Some((s, e)) = self.find_object(first_page_id) {
                let page_dict = String::from_utf8_lossy(&self.data[s..e]);
                
                if let Some(mb_pos) = page_dict.find("/MediaBox") {
                    let mb_str = self.extract_bracketed(&page_dict[mb_pos..], "[", "]");
                    let parts: Vec<f64> = mb_str.trim_matches(|c| c == '[' || c == ']').split_whitespace().filter_map(|p| p.parse::<f64>().ok()).collect();
                    if parts.len() == 4 { self.mediabox = [parts[0], parts[1], parts[2], parts[3]]; }
                }

                if let Some(res_pos) = page_dict.find("/Resources") {
                    let res_val = self.get_dict_value(&page_dict, "/Resources");
                    if let Some(val) = res_val {
                        if val == "<<" {
                             let res_dict = self.extract_bracketed(&page_dict[res_pos..], "<<", ">>");
                             self.parse_resources(&res_dict);
                        } else if let Ok(res_id) = val.parse::<u32>() {
                             if let Some((rs, re)) = self.find_object(res_id) {
                                 let res_dict = String::from_utf8_lossy(&self.data[rs..re]);
                                 self.parse_resources(&res_dict);
                             }
                        }
                    }
                }

                if let Some(cont_pos) = page_dict.find("/Contents") {
                    let cont_val = self.get_dict_value(&page_dict, "/Contents");
                    if let Some(val) = cont_val {
                        if val == "[" {
                            let kids_area = self.extract_bracketed(&page_dict[cont_pos..], "[", "]");
                            let mut j = 0;
                            let parts: Vec<&str> = kids_area.trim_matches(|c| c == '[' || c == ']').split_whitespace().collect();
                            while j + 2 < parts.len() {
                                if parts[j+1] == "0" && parts[j+2] == "R" {
                                    if let Ok(id) = parts[j].parse::<u32>() {
                                        if let Some(stream) = self.load_stream(id) { content_streams.push(stream); }
                                    }
                                }
                                j += 3;
                            }
                        } else if let Ok(cont_id) = val.parse::<u32>() {
                            if let Some(stream) = self.load_stream(cont_id) { content_streams.push(stream); }
                        }
                    }
                }
            }
        }

        if content_streams.is_empty() {
            content_streams = self.extract_content_streams();
        }

        let width = self.mediabox[2] - self.mediabox[0];
        let height = self.mediabox[3] - self.mediabox[1];
        
        let mut next_id = 1;
        let mut all_objects = Vec::new();
        for stream in content_streams {
            let mut stream_objects = self.parse_content_stream(&stream, &mut next_id);
            all_objects.append(&mut stream_objects);
        }

        Ok(Ai { width, height, objects: all_objects })
    }

    fn load_stream(&self, id: u32) -> Option<Vec<u8>> {
        if let Some((start, end)) = self.find_object(id) {
            let obj_data = &self.data[start..end];
            let obj_str = String::from_utf8_lossy(obj_data);
            if let Some(stream_pos) = obj_str.find("stream") {
                let header = &obj_str[..stream_pos];
                let stream_start = start + stream_pos + 6;
                let mut real_start = stream_start;
                while real_start < end && (self.data[real_start] == b'\r' || self.data[real_start] == b'\n') {
                    real_start += 1;
                }
                if let Some(stream_end_pos) = obj_str[stream_pos..].find("endstream") {
                    let stream_data = &self.data[real_start..start + stream_pos + stream_end_pos];
                    if header.contains("/FlateDecode") {
                        return self.decompress_flate(stream_data).ok();
                    } else {
                        return Some(stream_data.to_vec());
                    }
                }
            }
        }
        None
    }

    fn parse_ext_g_state_dict(&self, dict: &str) -> ExtGState {
        let mut gs = ExtGState { opacity: None, blend_mode: None };
        if let Some(ca_pos) = dict.find("/ca") {
            let val = &dict[ca_pos+3..].split_whitespace().next().unwrap_or("");
            if let Ok(v) = val.parse::<f64>() { gs.opacity = Some(v); }
        } else if let Some(ca_cap_pos) = dict.find("/CA") {
            let val = &dict[ca_cap_pos+3..].split_whitespace().next().unwrap_or("");
            if let Ok(v) = val.parse::<f64>() { gs.opacity = Some(v); }
        }
        if let Some(bm_pos) = dict.find("/BM") {
            let mut val = dict[bm_pos+3..].split_whitespace().next().unwrap_or("").trim_matches('/');
            if val == "Normal" { val = "source-over"; }
            gs.blend_mode = Some(val.to_lowercase());
        }
        gs
    }

    fn extract_content_streams(&self) -> Vec<Vec<u8>> {
        let mut streams = Vec::new();
        let mut i = 0;
        while i < self.data.len() {
            if i + 6 < self.data.len() && &self.data[i..i+6] == b"stream" {
                let start = i + 6;
                let mut stream_start = start;
                while stream_start < self.data.len() && (self.data[stream_start] == b'\r' || self.data[stream_start] == b'\n') {
                    stream_start += 1;
                }
                let mut end = stream_start;
                let mut found_end = false;
                while end + 9 < self.data.len() {
                    if &self.data[end..end+9] == b"endstream" {
                        found_end = true;
                        break;
                    }
                    end += 1;
                }
                if found_end {
                    let stream_data = &self.data[stream_start..end];
                    let header_search_start = if i > 300 { i - 300 } else { 0 };
                    let header_area = String::from_utf8_lossy(&self.data[header_search_start..i]);
                    if header_area.contains("/FlateDecode") {
                        if let Ok(decompressed) = self.decompress_flate(stream_data) {
                            streams.push(decompressed);
                        }
                    } else if !header_area.contains("/Filter") {
                        streams.push(stream_data.to_vec());
                    }
                }
                i = end + 9;
            } else {
                i += 1;
            }
        }
        streams
    }

    fn decompress_flate(&self, data: &[u8]) -> Result<Vec<u8>, std::io::Error> {
        use std::io::Read;
        let mut decoder = flate2::read::ZlibDecoder::new(data);
        let mut result = Vec::new();
        if decoder.read_to_end(&mut result).is_ok() {
            return Ok(result);
        }
        let mut decoder = flate2::read::DeflateDecoder::new(data);
        let mut result = Vec::new();
        decoder.read_to_end(&mut result)?;
        Ok(result)
    }

    fn parse_content_stream(&self, stream: &[u8], next_id: &mut u32) -> Vec<VectorObject> {
        let mut objects = Vec::new();
        let mut current_path = String::new();
        let mut state_stack = vec![GraphicsState::default()];
        let canvas_height = self.mediabox[3] - self.mediabox[1];
        let origin_x = self.mediabox[0];
        let origin_y = self.mediabox[1];
        let tokens = self.tokenize(stream);
        let mut i = 0;
        while i < tokens.len() {
            let token = tokens[i].as_str();
            match token {
                "q" => {
                    let new_state = state_stack.last().unwrap().clone();
                    state_stack.push(new_state);
                }
                "Q" => {
                    if state_stack.len() > 1 { state_stack.pop(); }
                }
                "cm" => {
                    let nums = self.get_nums_backwards(&tokens, i);
                    if nums.len() == 6 {
                        let mat = Affine::new([nums[0], nums[1], nums[2], nums[3], nums[4], nums[5]]);
                        let current_state = state_stack.last_mut().unwrap();
                        current_state.transform = current_state.transform * mat;
                    }
                }
                "m" | "l" => {
                    let nums = self.get_nums_backwards(&tokens, i);
                    if nums.len() == 2 {
                        let op = if token == "m" || current_path.is_empty() { "M" } else { "L" };
                        current_path.push_str(&format!("{} {} {} ", op, nums[0], nums[1]));
                    }
                }
                "c" => {
                    let nums = self.get_nums_backwards(&tokens, i);
                    if nums.len() == 6 {
                        if current_path.is_empty() { current_path.push_str(&format!("M {} {} ", nums[0], nums[1])); }
                        current_path.push_str(&format!("C {} {}, {} {}, {} {} ", nums[0], nums[1], nums[2], nums[3], nums[4], nums[5]));
                    }
                }
                "v" => {
                    let nums = self.get_nums_backwards(&tokens, i);
                    if nums.len() == 4 { current_path.push_str(&format!("S {} {}, {} {} ", nums[0], nums[1], nums[2], nums[3])); }
                }
                "y" => {
                    let nums = self.get_nums_backwards(&tokens, i);
                    if nums.len() == 4 { current_path.push_str(&format!("C {} {}, {} {}, {} {} ", nums[0], nums[1], nums[2], nums[3], nums[2], nums[3])); }
                }
                "h" => {
                    if !current_path.is_empty() { current_path.push_str("Z "); }
                }
                "re" => {
                    let nums = self.get_nums_backwards(&tokens, i);
                    if nums.len() == 4 {
                        let (x, y, w, h) = (nums[0], nums[1], nums[2], nums[3]);
                        current_path.push_str(&format!("M {} {} L {} {} L {} {} L {} {} Z ", x, y, x + w, y, x + w, y + h, x, y + h));
                    }
                }
                "w" => {
                    let nums = self.get_nums_backwards(&tokens, i);
                    if !nums.is_empty() { 
                        let current_state = state_stack.last_mut().unwrap();
                        current_state.stroke_width = *nums.last().unwrap(); 
                    }
                }
                "J" => {
                    let nums = self.get_nums_backwards(&tokens, i);
                    if let Some(&cap) = nums.last() {
                        let current_state = state_stack.last_mut().unwrap();
                        current_state.stroke_cap = match cap as u8 { 0 => "butt", 1 => "round", 2 => "square", _ => "butt" }.to_string();
                    }
                }
                "j" => {
                    let nums = self.get_nums_backwards(&tokens, i);
                    if let Some(&join) = nums.last() {
                        let current_state = state_stack.last_mut().unwrap();
                        current_state.stroke_join = match join as u8 { 0 => "miter", 1 => "round", 2 => "bevel", _ => "miter" }.to_string();
                    }
                }
                "d" => {
                    if i > 0 && tokens[i-1] == "]" {
                         let mut j = i as i32 - 2;
                         let mut dash_array = Vec::new();
                         while j >= 0 && tokens[j as usize] != "[" {
                             if let Ok(v) = tokens[j as usize].parse::<f64>() { dash_array.push(v); }
                             j -= 1;
                         }
                         dash_array.reverse();
                         state_stack.last_mut().unwrap().stroke_dash = dash_array;
                    }
                }
                "gs" => {
                    if i > 0 {
                        let gs_name = tokens[i-1].trim_start_matches('/');
                        if let Some(gs) = self.ext_g_states.get(gs_name) {
                            let current_state = state_stack.last_mut().unwrap();
                            if let Some(op) = gs.opacity { current_state.opacity = op; }
                            if let Some(bm) = &gs.blend_mode { current_state.blend_mode = bm.clone(); }
                        }
                    }
                }
                "Do" => {
                    if i > 0 {
                        let name = tokens[i-1].trim_start_matches('/');
                        if let Some(rgba) = self.x_objects.get(name) {
                            let (w, h, _) = self.x_object_info.get(name).cloned().unwrap_or((1, 1, String::new()));
                            let current_state = state_stack.last().unwrap();
                            let p0 = current_state.transform * kurbo::Point::new(0.0, 0.0);
                            let p1 = current_state.transform * kurbo::Point::new(1.0, 1.0);
                            let wx = p0.x - origin_x;
                            let wy = canvas_height - (p1.y - origin_y);
                            let ww = (p1.x - p0.x).abs();
                            let wh = (p1.y - p0.y).abs();
                            let mut png_bytes = Vec::new();
                            if let Some(img_buffer) = image::RgbaImage::from_raw(w, h, rgba.clone()) {
                                let dyn_img = image::DynamicImage::ImageRgba8(img_buffer);
                                let mut cursor = std::io::Cursor::new(&mut png_bytes);
                                let _ = dyn_img.write_to(&mut cursor, image::ImageOutputFormat::Png);
                            }
                            objects.push(VectorObject {
                                id: *next_id, shape_type: ShapeType::Image, name: format!("Ai Image {}", name), x: wx, y: wy, width: ww, height: wh, rotation: 0.0, fill: "transparent".to_string(), stroke: "transparent".to_string(), stroke_width: 0.0, opacity: current_state.opacity, visible: true, locked: false, blend_mode: current_state.blend_mode.clone(), stroke_cap: "butt".to_string(), stroke_join: "miter".to_string(), stroke_dash: Vec::new(), layer_style: crate::LayerStyle::default(), mask_id: None, is_mask: false, sides: 0, inner_radius: 0.0, corner_radius: 0.0, path_data: String::new(), 
                                intelligent_type: String::new(),
                                intelligent_params: Vec::new(),
                                brush_id: 0, stroke_points: Vec::new(), text_content: String::new(), font_family: String::new(), font_size: 0.0, font_weight: String::new(), text_align: String::new(), kerning: 0.0, leading: 1.2, tracking: 0.0, shadow_color: "transparent".to_string(), shadow_blur: 0.0, shadow_offset_x: 0.0, shadow_offset_y: 0.0, sx: 0.0, sy: 0.0, sw: ww, sh: wh, brightness: 1.0, contrast: 1.0, saturate: 1.0, hue_rotate: 0.0, blur: 0.0, grayscale: 0.0, sepia: 0.0, invert: 0.0, raw_image: Some(png_bytes), raw_rgba: Some(rgba.clone()), raw_rgba_width: w, raw_rgba_height: h, image: None, fill_gradient: None, stroke_gradient: None, children: None,
                            });
                            *next_id += 1;
                        }
                    }
                }
                "BT" => {
                    let mut text_state = state_stack.last().unwrap().clone();
                    let mut text_matrix = Affine::IDENTITY;
                    let mut j = i + 1;
                    let mut text_content = String::new();
                    while j < tokens.len() && tokens[j] != "ET" {
                        match tokens[j].as_str() {
                            "Tf" => {
                                if j >= 2 {
                                    text_state.font_family = tokens[j-2].trim_start_matches('/').to_string();
                                    if let Ok(sz) = tokens[j-1].parse::<f64>() { text_state.font_size = sz; }
                                }
                            }
                            "Tm" => {
                                let nums = self.get_nums_backwards(&tokens, j);
                                if nums.len() == 6 { text_matrix = Affine::new([nums[0], nums[1], nums[2], nums[3], nums[4], nums[5]]); }
                            }
                            "Td" | "TD" => {
                                let nums = self.get_nums_backwards(&tokens, j);
                                if nums.len() == 2 { text_matrix = text_matrix * Affine::translate((nums[0], nums[1])); }
                            }
                            "Tj" => {
                                if j > 0 {
                                    let s = tokens[j-1].trim_matches(|c| c == '(' || c == ')').to_string();
                                    text_content.push_str(&s);
                                }
                            }
                            "TJ" => {
                                let mut k = j - 1;
                                if tokens[k] == "]" {
                                    k -= 1;
                                    while k > 0 && tokens[k] != "[" {
                                        if tokens[k].starts_with('(') { text_content.push_str(tokens[k].trim_matches(|c| c == '(' || c == ')')); }
                                        k -= 1;
                                    }
                                }
                            }
                            _ => {} 
                        }
                        j += 1;
                    }
                    if !text_content.is_empty() {
                        let combined_transform = text_state.transform * text_matrix;
                        let pos = combined_transform * kurbo::Point::new(0.0, 0.0);
                        let wx = pos.x - origin_x;
                        let wy = canvas_height - (pos.y - origin_y);
                        objects.push(VectorObject {
                            id: *next_id, shape_type: ShapeType::Text, name: format!("Ai Text {}", *next_id), x: wx, y: wy - text_state.font_size, width: text_content.len() as f64 * (text_state.font_size * 0.6), height: text_state.font_size, rotation: 0.0, fill: text_state.fill.clone(), stroke: text_state.stroke.clone(), stroke_width: text_state.stroke_width, visible: true, locked: false, opacity: text_state.opacity, blend_mode: text_state.blend_mode.clone(), stroke_cap: text_state.stroke_cap.clone(), stroke_join: text_state.stroke_join.clone(), stroke_dash: text_state.stroke_dash.clone(), layer_style: crate::LayerStyle::default(), mask_id: None, is_mask: false, sides: 0, inner_radius: 0.0, corner_radius: 0.0, path_data: String::new(), 
                            intelligent_type: String::new(),
                            intelligent_params: Vec::new(),
                            brush_id: 0, stroke_points: Vec::new(), text_content, font_family: text_state.font_family.clone(), font_size: text_state.font_size, font_weight: "normal".to_string(), text_align: "left".to_string(), kerning: 0.0, leading: 1.2, tracking: 0.0, shadow_color: "transparent".to_string(), shadow_blur: 0.0, shadow_offset_x: 0.0, shadow_offset_y: 0.0, sx: 0.0, sy: 0.0, sw: 0.0, sh: 0.0, brightness: 1.0, contrast: 1.0, saturate: 1.0, hue_rotate: 0.0, blur: 0.0, grayscale: 0.0, sepia: 0.0, invert: 0.0, raw_image: None, raw_rgba: None, raw_rgba_width: 0, raw_rgba_height: 0, image: None, fill_gradient: None, stroke_gradient: None, children: None,
                        });
                        *next_id += 1;
                    }
                    i = j;
                }
                "rg" | "k" | "g" => {
                    let nums = self.get_nums_backwards(&tokens, i);
                    let color = self.to_color(&nums);
                    let current_state = state_stack.last_mut().unwrap();
                    current_state.fill = color;
                }
                "RG" | "K" | "G" => {
                    let nums = self.get_nums_backwards(&tokens, i);
                    let color = self.to_color(&nums);
                    let current_state = state_stack.last_mut().unwrap();
                    current_state.stroke = color;
                }
                "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" => {
                    if !current_path.is_empty() {
                        let is_fill = token.to_lowercase().contains('f') || token.to_lowercase().contains('b');
                        let is_stroke = token.to_lowercase().contains('s') || token.to_lowercase().contains('b');
                        if let Ok(mut bez) = BezPath::from_svg(&current_path) {
                            if bez.segments().next().is_some() {
                                let current_state = state_stack.last().unwrap();
                                bez.apply_affine(current_state.transform);
                                let mut transformed_path = BezPath::new();
                                for el in bez.iter() {
                                    transformed_path.push(match el {
                                        kurbo::PathEl::MoveTo(p) => kurbo::PathEl::MoveTo(kurbo::Point::new(p.x - origin_x, canvas_height - (p.y - origin_y))),
                                        kurbo::PathEl::LineTo(p) => kurbo::PathEl::LineTo(kurbo::Point::new(p.x - origin_x, canvas_height - (p.y - origin_y))),
                                        kurbo::PathEl::CurveTo(p1, p2, p3) => kurbo::PathEl::CurveTo(kurbo::Point::new(p1.x - origin_x, canvas_height - (p1.y - origin_y)), kurbo::Point::new(p2.x - origin_x, canvas_height - (p2.y - origin_y)), kurbo::Point::new(p3.x - origin_x, canvas_height - (p3.y - origin_y))),
                                        kurbo::PathEl::QuadTo(p1, p2) => kurbo::PathEl::QuadTo(kurbo::Point::new(p1.x - origin_x, canvas_height - (p1.y - origin_y)), kurbo::Point::new(p2.x - origin_x, canvas_height - (p2.y - origin_y))),
                                        kurbo::PathEl::ClosePath => kurbo::PathEl::ClosePath,
                                    });
                                }
                                if transformed_path.segments().next().is_some() {
                                    let rect = transformed_path.bounding_box();
                                    let x = rect.x0; let y = rect.y0; let w = rect.width().max(1.0); let h = rect.height().max(1.0);
                                    let is_full_artboard = x.abs() < 2.0 && y.abs() < 2.0 && (w - (self.mediabox[2] - self.mediabox[0])).abs() < 2.0;
                                    if !(is_full_artboard && current_state.fill == "#000000" && !is_stroke) {
                                        transformed_path.apply_affine(Affine::translate((-x, -y)));
                                        objects.push(VectorObject {
                                            id: *next_id, shape_type: ShapeType::Path, name: format!("Ai Path {}", *next_id), x, y, width: w, height: h, rotation: 0.0, fill: if is_fill { current_state.fill.clone() } else { "transparent".to_string() }, stroke: if is_stroke { current_state.stroke.clone() } else { "transparent".to_string() }, stroke_width: if is_stroke { current_state.stroke_width } else { 0.0 }, visible: true, locked: false, opacity: current_state.opacity, blend_mode: current_state.blend_mode.clone(), stroke_cap: current_state.stroke_cap.clone(), stroke_join: current_state.stroke_join.clone(), stroke_dash: current_state.stroke_dash.clone(), layer_style: crate::LayerStyle::default(), mask_id: None, is_mask: false, sides: 0, inner_radius: 0.0, corner_radius: 0.0, path_data: transformed_path.to_svg(), 
                                            intelligent_type: String::new(),
                                            intelligent_params: Vec::new(),
                                            brush_id: 0, stroke_points: Vec::new(), text_content: String::new(), font_family: String::new(), font_size: 0.0, font_weight: String::new(), text_align: String::new(), kerning: 0.0, leading: 1.2, tracking: 0.0, shadow_color: "transparent".to_string(), shadow_blur: 0.0, shadow_offset_x: 0.0, shadow_offset_y: 0.0, sx: 0.0, sy: 0.0, sw: 0.0, sh: 0.0, brightness: 1.0, contrast: 1.0, saturate: 1.0, hue_rotate: 0.0, blur: 0.0, grayscale: 0.0, sepia: 0.0, invert: 0.0, raw_image: None, raw_rgba: None, raw_rgba_width: 0, raw_rgba_height: 0, image: None, fill_gradient: None, stroke_gradient: None, children: None,
                                        });
                                        *next_id += 1;
                                    }
                                }
                            }
                        }
                        current_path.clear();
                    }
                }
                _ => {} 
            }
            i += 1;
        }
        objects
    }

    fn tokenize(&self, stream: &[u8]) -> Vec<String> {
        let mut tokens = Vec::new();
        let mut current = String::new();
        let mut in_string = false;
        let mut string_depth = 0;
        let mut j = 0;
        while j < stream.len() {
            let b = stream[j];
            if in_string {
                current.push(b as char);
                if b == b'(' { string_depth += 1; }
                else if b == b')' {
                    if string_depth > 0 { string_depth -= 1; }
                    else { in_string = false; tokens.push(current.clone()); current.clear(); }
                }
            } else if b <= 32 { 
                if !current.is_empty() { tokens.push(current.clone()); current.clear(); }
            } else if b == b'(' {
                if !current.is_empty() { tokens.push(current.clone()); current.clear(); }
                in_string = true; string_depth = 0; current.push('(');
            } else if b == b'[' || b == b']' || b == b'<' || b == b'>' || b == b'/' {
                if !current.is_empty() { tokens.push(current.clone()); current.clear(); }
                tokens.push((b as char).to_string());
            } else {
                current.push(b as char);
            }
            j += 1;
        }
        if !current.is_empty() { tokens.push(current); }
        tokens
    }

    fn get_nums_backwards(&self, tokens: &[String], current_idx: usize) -> Vec<f64> {
        let mut nums = Vec::new();
        let mut j = current_idx as i32 - 1;
        while j >= 0 && nums.len() < 6 {
            let t = &tokens[j as usize];
            if let Ok(n) = t.parse::<f64>() { nums.push(n); }
            else { break; }
            j -= 1;
        }
        nums.reverse();
        nums
    }

    fn to_color(&self, nums: &[f64]) -> String {
        match nums.len() {
            1 => {
                let v = (nums[0] * 255.0).clamp(0.0, 255.0) as u8;
                format!("#{:02x}{:02x}{:02x}", v, v, v)
            }
            3 => {
                let r = (nums[0] * 255.0).clamp(0.0, 255.0) as u8;
                let g = (nums[1] * 255.0).clamp(0.0, 255.0) as u8;
                let b = (nums[2] * 255.0).clamp(0.0, 255.0) as u8;
                format!("#{:02x}{:02x}{:02x}", r, g, b)
            }
            4 => {
                let c = nums[0]; let m = nums[1]; let y = nums[2]; let k = nums[3];
                let r = ((1.0 - c) * (1.0 - k) * 255.0).clamp(0.0, 255.0) as u8;
                let g = ((1.0 - m) * (1.0 - k) * 255.0).clamp(0.0, 255.0) as u8;
                let b = ((1.0 - y) * (1.0 - k) * 255.0).clamp(0.0, 255.0) as u8;
                format!("#{:02x}{:02x}{:02x}", r, g, b)
            }
            _ => "#000000".to_string(), 
        }
    }
}

#[derive(Clone)]
struct GraphicsState {
    transform: Affine,
    stroke_width: f64,
    stroke: String,
    fill: String,
    opacity: f64,
    blend_mode: String,
    stroke_cap: String,
    stroke_join: String,
    stroke_dash: Vec<f64>,
    font_family: String,
    font_size: f64,
}

impl Default for GraphicsState {
    fn default() -> Self {
        GraphicsState {
            transform: Affine::IDENTITY,
            stroke_width: 1.0,
            stroke: "transparent".to_string(),
            fill: "#000000".to_string(),
            opacity: 1.0,
            blend_mode: "source-over".to_string(),
            stroke_cap: "butt".to_string(),
            stroke_join: "miter".to_string(),
            stroke_dash: Vec::new(),
            font_family: "Inter, sans-serif".to_string(),
            font_size: 12.0,
        }
    }
}
