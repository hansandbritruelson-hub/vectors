use kurbo::{ BezPath, Affine, Shape };
use crate::{VectorObject, ShapeType};

pub struct Ai {
    pub width: f64,
    pub height: f64,
    pub objects: Vec<VectorObject>,
}

impl Ai {
    pub fn export(width: f64, height: f64, objects: &[VectorObject]) -> Vec<u8> {
        let mut pdf = Vec::new();
        
        // 1. Header
        pdf.extend_from_slice(b"%PDF-1.4\n");
        pdf.extend_from_slice(b"%\xE2\xE3\xCF\xD3\n"); // Binary marker
        
        // AI specific header comment
        pdf.extend_from_slice(b"%AI12_FileFormatLevel: 3\n");

        let mut offsets = Vec::new();
        
        // 2. Catalog (Object 1)
        offsets.push(pdf.len());
        pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");
        
        // 3. Pages (Object 2)
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n").as_bytes());
        
        // 4. Page (Object 3)
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!(
            "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Contents 4 0 R /Resources << /Font << /F1 << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> >> >> >>\nendobj\n",
            width, height
        ).as_bytes());
        
        // 5. Content Stream (Object 4)
        let mut content = Vec::new();
        
        for obj in objects {
            if !obj.visible { continue; }
            content.extend_from_slice(format!("q\n").as_bytes());
            
            // Set opacity if less than 1.0 (requires ExtGState which is complex for minimal export, 
            // so we'll just do colors for now, but we could add basic support if needed)
            
            if obj.shape_type != ShapeType::Image && obj.shape_type != ShapeType::Group {
                let fill_rgb = self::parse_hex_color(&obj.fill);
                let stroke_rgb = self::parse_hex_color(&obj.stroke);
                
                content.extend_from_slice(format!("{} {} {} rg\n", fill_rgb.0 as f32 / 255.0, fill_rgb.1 as f32 / 255.0, fill_rgb.2 as f32 / 255.0).as_bytes());
                content.extend_from_slice(format!("{} {} {} RG\n", stroke_rgb.0 as f32 / 255.0, stroke_rgb.1 as f32 / 255.0, stroke_rgb.2 as f32 / 255.0).as_bytes());
                content.extend_from_slice(format!("{} w\n", obj.stroke_width).as_bytes());
                
                let mut path_data = match obj.shape_type {
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
            } else if obj.shape_type == ShapeType::Text {
                content.extend_from_slice(b"BT\n");
                content.extend_from_slice(format!("/F1 {} Tf\n", obj.font_size).as_bytes());
                
                // Handle text rotation
                let cos_r = obj.rotation.cos();
                let sin_r = obj.rotation.sin();
                // PDF Tm is [a b c d e f]
                // Our rotation is clockwise, PDF rotation is counter-clockwise.
                // a=cos, b=-sin, c=sin, d=cos
                content.extend_from_slice(format!("{} {} {} {} {} {} Tm\n", 
                    cos_r, -sin_r, sin_r, cos_r, obj.x, height - obj.y).as_bytes());
                
                content.extend_from_slice(format!("({})", obj.text_content.replace("(", "\\(").replace(")", "\\)")).as_bytes());
                content.extend_from_slice(b"\nET\n");
            }
            
            content.extend_from_slice(format!("Q\n").as_bytes());
        }

        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("4 0 obj\n<< /Length {} >>\nstream\n", content.len()).as_bytes());
        pdf.extend_from_slice(&content);
        pdf.extend_from_slice(b"\nendstream\nendobj\n");
        
        let xref_pos = pdf.len();
        pdf.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len() + 1).as_bytes());
        for offset in &offsets {
            pdf.extend_from_slice(format!("{:010} 00000 n \n", offset).as_bytes());
        }
        
        pdf.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF", offsets.len() + 1, xref_pos).as_bytes());
        
        pdf
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
    ext_g_states: std::collections::HashMap<String, ExtGState>,
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
            ext_g_states: std::collections::HashMap::new(),
        }
    }

    pub fn parse(&mut self) -> Result<Ai, AiError> {
        if !self.data.starts_with(b"%PDF-") && !self.data.starts_with(b"%AI") {
            return Err(AiError::InvalidSignature);
        }

        if let Some(mediabox) = self.find_mediabox() {
            self.mediabox = [mediabox[0], mediabox[1], mediabox[2], mediabox[3]];
        }

        self.parse_ext_g_states();

        let width = self.mediabox[2] - self.mediabox[0];
        let height = self.mediabox[3] - self.mediabox[1];
        
        let mut next_id = 1;
        let content_streams = self.extract_content_streams();
        
        let mut all_objects = Vec::new();
        for stream in content_streams {
            let mut stream_objects = self.parse_content_stream(&stream, &mut next_id);
            all_objects.append(&mut stream_objects);
        }

        Ok(Ai {
            width,
            height,
            objects: all_objects,
        })
    }

    fn parse_ext_g_states(&mut self) {
        let content = String::from_utf8_lossy(self.data);
        if let Some(pos) = content.find("/ExtGState") {
            let start = pos;
            let sub = &content[start..];
            if let Some(dict_start) = sub.find("<<") {
                let mut depth = 0;
                let mut dict_end = 0;
                let mut j = dict_start;
                let bytes = sub.as_bytes();
                while j + 1 < bytes.len() {
                    if bytes[j] == b'<' && bytes[j+1] == b'<' { depth += 1; j += 2; }
                    else if bytes[j] == b'>' && bytes[j+1] == b'>' { 
                        depth -= 1; j += 2; 
                        if depth == 0 { dict_end = j; break; }
                    }
                    else { j += 1; }
                }
                
                if dict_end > dict_start {
                    let dict_content = &sub[dict_start..dict_end];
                    let mut i = 0;
                    while let Some(key_pos) = dict_content[i..].find('/') {
                        let key_start = i + key_pos + 1;
                        let mut key_end = key_start;
                        while key_end < dict_content.len() && !dict_content.as_bytes()[key_end].is_ascii_whitespace() && dict_content.as_bytes()[key_end] != b'/' && dict_content.as_bytes()[key_end] != b'<' {
                            key_end += 1;
                        }
                        let key = &dict_content[key_start..key_end];
                        
                        let val_area = &dict_content[key_end..];
                        if let Some(obj_start) = val_area.find("<<") {
                            let mut d = 0;
                            let mut obj_end = 0;
                            let v_bytes = val_area.as_bytes();
                            let mut k = 0;
                            while k + 1 < v_bytes.len() {
                                if v_bytes[k] == b'<' && v_bytes[k+1] == b'<' { d += 1; k += 2; }
                                else if v_bytes[k] == b'>' && v_bytes[k+1] == b'>' { 
                                    d -= 1; k += 2; 
                                    if d == 0 { obj_end = k; break; }
                                }
                                else { k += 1; }
                            }
                            if obj_end > 0 {
                                let gs_dict = &val_area[obj_start..obj_end];
                                self.ext_g_states.insert(key.to_string(), self.parse_ext_g_state_dict(gs_dict));
                            }
                        }
                        i = key_end;
                    }
                }
            }
        }
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

    fn find_mediabox(&self) -> Option<Vec<f64>> {
        let content = String::from_utf8_lossy(self.data);
        let keys = ["/MediaBox", "/CropBox", "/ArtBox"];
        for key in keys {
            if let Some(pos) = content.find(key) {
                let sub_start = pos;
                let sub_end = (pos + 100).min(content.len());
                let sub = &content[sub_start..sub_end];
                if let Some(start) = sub.find('[') {
                    if let Some(end) = sub[start..].find(']') {
                        let actual_end = start + end;
                        let parts: Vec<f64> = sub[start + 1..actual_end]
                            .split_whitespace()
                            .filter_map(|p| p.parse::<f64>().ok())
                            .collect();
                        if parts.len() == 4 {
                            return Some(parts);
                        }
                    }
                }
            }
        }
        None
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
                    if state_stack.len() > 1 {
                        state_stack.pop();
                    }
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
                        if current_path.is_empty() {
                            current_path.push_str(&format!("M {} {} ", nums[0], nums[1]));
                        }
                        current_path.push_str(&format!("C {} {}, {} {}, {} {} ", 
                            nums[0], nums[1], nums[2], nums[3], nums[4], nums[5]));
                    }
                }
                "v" => {
                    let nums = self.get_nums_backwards(&tokens, i);
                    if nums.len() == 4 {
                        current_path.push_str(&format!("S {} {}, {} {} ", nums[0], nums[1], nums[2], nums[3]));
                    }
                }
                "y" => {
                    let nums = self.get_nums_backwards(&tokens, i);
                    if nums.len() == 4 {
                        current_path.push_str(&format!("C {} {}, {} {}, {} {} ", nums[0], nums[1], nums[2], nums[3], nums[2], nums[3]));
                    }
                }
                "h" => {
                    if !current_path.is_empty() {
                        current_path.push_str("Z ");
                    }
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
                        current_state.stroke_cap = match cap as u8 {
                            0 => "butt", 1 => "round", 2 => "square", _ => "butt",
                        }.to_string();
                    }
                }
                "j" => {
                    let nums = self.get_nums_backwards(&tokens, i);
                    if let Some(&join) = nums.last() {
                        let current_state = state_stack.last_mut().unwrap();
                        current_state.stroke_join = match join as u8 {
                            0 => "miter", 1 => "round", 2 => "bevel", _ => "miter",
                        }.to_string();
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
                                if nums.len() == 6 {
                                    text_matrix = Affine::new([nums[0], nums[1], nums[2], nums[3], nums[4], nums[5]]);
                                }
                            }
                            "Td" | "TD" => {
                                let nums = self.get_nums_backwards(&tokens, j);
                                if nums.len() == 2 {
                                    text_matrix = text_matrix * Affine::translate((nums[0], nums[1]));
                                }
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
                                        if tokens[k].starts_with('(') {
                                            text_content.push_str(tokens[k].trim_matches(|c| c == '(' || c == ')'));
                                        }
                                        k -= 1;
                                    }
                                }
                            }
                            _ => {} // Ignore unknown tokens
                        }
                        j += 1;
                    }
                    
                    if !text_content.is_empty() {
                        let combined_transform = text_state.transform * text_matrix;
                        let pos = combined_transform * kurbo::Point::new(0.0, 0.0);
                        let wx = pos.x - origin_x;
                        let wy = canvas_height - (pos.y - origin_y);

                        objects.push(VectorObject {
                            id: *next_id,
                            shape_type: ShapeType::Text,
                            name: format!("Ai Text {}", *next_id),
                            x: wx, y: wy - text_state.font_size,
                            width: text_content.len() as f64 * (text_state.font_size * 0.6),
                            height: text_state.font_size,
                            rotation: 0.0,
                            fill: text_state.fill.clone(),
                            stroke: text_state.stroke.clone(),
                            stroke_width: text_state.stroke_width,
                            visible: true, locked: false, opacity: text_state.opacity,
                            blend_mode: text_state.blend_mode.clone(),
                            stroke_cap: text_state.stroke_cap.clone(),
                            stroke_join: text_state.stroke_join.clone(),
                            stroke_dash: text_state.stroke_dash.clone(),
                            layer_style: crate::LayerStyle::default(),
                            mask_id: None,
                            is_mask: false,
                            sides: 0, inner_radius: 0.0, corner_radius: 0.0,
                            path_data: String::new(),
                            brush_id: 0, stroke_points: Vec::new(),
                            text_content, font_family: text_state.font_family.clone(), font_size: text_state.font_size, 
                            font_weight: "normal".to_string(), text_align: "left".to_string(),
                            kerning: 0.0, leading: 1.2, tracking: 0.0,
                            shadow_color: "transparent".to_string(), shadow_blur: 0.0, shadow_offset_x: 0.0, shadow_offset_y: 0.0,
                            sx: 0.0, sy: 0.0, sw: 0.0, sh: 0.0,
                            brightness: 1.0, contrast: 1.0, saturate: 1.0, hue_rotate: 0.0, blur: 0.0, grayscale: 0.0, sepia: 0.0, invert: 0.0,
                            raw_image: None, raw_rgba: None, raw_rgba_width: 0, raw_rgba_height: 0, image: None,
                            fill_gradient: None, stroke_gradient: None, children: None,
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
                                        kurbo::PathEl::CurveTo(p1, p2, p3) => kurbo::PathEl::CurveTo(
                                            kurbo::Point::new(p1.x - origin_x, canvas_height - (p1.y - origin_y)),
                                            kurbo::Point::new(p2.x - origin_x, canvas_height - (p2.y - origin_y)),
                                            kurbo::Point::new(p3.x - origin_x, canvas_height - (p3.y - origin_y)),
                                        ),
                                        kurbo::PathEl::QuadTo(p1, p2) => kurbo::PathEl::QuadTo(
                                            kurbo::Point::new(p1.x - origin_x, canvas_height - (p1.y - origin_y)),
                                            kurbo::Point::new(p2.x - origin_x, canvas_height - (p2.y - origin_y)),
                                        ),
                                        kurbo::PathEl::ClosePath => kurbo::PathEl::ClosePath,
                                    });
                                }

                                if transformed_path.segments().next().is_some() {
                                    let rect = transformed_path.bounding_box();
                                    let x = rect.x0;
                                    let y = rect.y0;
                                    let w = rect.width().max(1.0);
                                    let h = rect.height().max(1.0);
                                    
                                    let is_full_artboard = x.abs() < 2.0 && y.abs() < 2.0 && 
                                                         (w - (self.mediabox[2] - self.mediabox[0])).abs() < 2.0;
                                    
                                    if is_full_artboard && current_state.fill == "#000000" && !is_stroke {
                                        // Skip
                                    } else {
                                        transformed_path.apply_affine(Affine::translate((-x, -y)));
                                        
                                        objects.push(VectorObject {
                                            id: *next_id,
                                            shape_type: ShapeType::Path,
                                            name: format!("Ai Path {}", *next_id),
                                            x, y, width: w, height: h,
                                            rotation: 0.0,
                                            fill: if is_fill { current_state.fill.clone() } else { "transparent".to_string() },
                                            stroke: if is_stroke { current_state.stroke.clone() } else { "transparent".to_string() },
                                            stroke_width: if is_stroke { current_state.stroke_width } else { 0.0 },
                                            visible: true, locked: false, opacity: current_state.opacity,
                                            blend_mode: current_state.blend_mode.clone(),
                                            stroke_cap: current_state.stroke_cap.clone(),
                                            stroke_join: current_state.stroke_join.clone(),
                                            stroke_dash: current_state.stroke_dash.clone(),
                                            layer_style: crate::LayerStyle::default(),
                                            mask_id: None,
                                            is_mask: false,
                                            sides: 0, inner_radius: 0.0, corner_radius: 0.0,
                                            path_data: transformed_path.to_svg(),
                                            brush_id: 0, stroke_points: Vec::new(),
                                            text_content: String::new(), font_family: String::new(), font_size: 0.0, font_weight: String::new(), text_align: String::new(),
                                            kerning: 0.0, leading: 1.2, tracking: 0.0,
                                            shadow_color: "transparent".to_string(), shadow_blur: 0.0, shadow_offset_x: 0.0, shadow_offset_y: 0.0,
                                            sx: 0.0, sy: 0.0, sw: 0.0, sh: 0.0,
                                            brightness: 1.0, contrast: 1.0, saturate: 1.0, hue_rotate: 0.0, blur: 0.0, grayscale: 0.0, sepia: 0.0, invert: 0.0,
                                            raw_image: None, raw_rgba: None, raw_rgba_width: 0, raw_rgba_height: 0, image: None,
                                            fill_gradient: None, stroke_gradient: None, children: None,
                                        });
                                        *next_id += 1;
                                    }
                                }
                            }
                        }
                        current_path.clear();
                    }
                }
                _ => {} // Ignore unknown tokens
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
                    else {
                        in_string = false;
                        tokens.push(current.clone());
                        current.clear();
                    }
                }
            } else if b <= 32 { // Whitespace
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
            } else if b == b'(' {
                if !current.is_empty() { tokens.push(current.clone()); current.clear(); }
                in_string = true;
                string_depth = 0;
                current.push('(');
            } else if b == b'[' || b == b']' || b == b'<' || b == b'>' || b == b'/' {
                if !current.is_empty() { tokens.push(current.clone()); current.clear(); }
                tokens.push((b as char).to_string());
            } else {
                current.push(b as char);
            }
            j += 1;
        }
        if !current.is_empty() {
            tokens.push(current);
        }
        tokens
    }

    fn get_nums_backwards(&self, tokens: &[String], current_idx: usize) -> Vec<f64> {
        let mut nums = Vec::new();
        let mut j = current_idx as i32 - 1;
        while j >= 0 && nums.len() < 6 {
            let t = &tokens[j as usize];
            if let Ok(n) = t.parse::<f64>() {
                nums.push(n);
            } else {
                // PDF operands are strictly before. Stop at first non-number.
                break;
            }
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