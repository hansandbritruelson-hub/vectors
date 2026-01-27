use std::convert::TryInto;
use std::io::Read;
use flate2::read::ZlibDecoder;

#[derive(Debug)]
pub enum PsdError {
    InvalidSignature,
    UnsupportedVersion,
    UnsupportedColorMode,
    UnsupportedDepth,
    UnexpectedEndOfFile,
    InvalidLayerData,
    DecompressionError,
    IoError,
    ZipError,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColorMode {
    Bitmap = 0,
    Grayscale = 1,
    Indexed = 2,
    Rgb = 3,
    Cmyk = 4,
    Multichannel = 7,
    Duotone = 8,
    Lab = 9,
}

impl ColorMode {
    fn from_u16(value: u16) -> Option<Self> {
        match value {
            0 => Some(ColorMode::Bitmap),
            1 => Some(ColorMode::Grayscale),
            2 => Some(ColorMode::Indexed),
            3 => Some(ColorMode::Rgb),
            4 => Some(ColorMode::Cmyk),
            7 => Some(ColorMode::Multichannel),
            8 => Some(ColorMode::Duotone),
            9 => Some(ColorMode::Lab),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PsdLayerType {
    Normal = 0,
    FolderOpen = 1,
    FolderClosed = 2,
    SectionDivider = 3,
}

#[derive(Debug, Clone)]
pub struct PsdMaskInfo {
    pub top: i32,
    pub left: i32,
    pub bottom: i32,
    pub right: i32,
    pub default_color: u8,
    pub flags: u8,
}

pub struct PsdLayer {
    pub name: String,
    pub top: i32,
    pub left: i32,
    pub bottom: i32,
    pub right: i32,
    pub width: u32,
    pub height: u32,
    pub opacity: u8,
    pub visible: bool,
    pub blend_mode: String,
    pub rgba: Vec<u8>,
    pub layer_type: PsdLayerType,
    pub clipping: bool,
    pub mask_info: Option<PsdMaskInfo>,
    pub text_data: Option<String>,
    pub vector_mask: Option<String>,
}

pub struct Psd {
    pub width: u32,
    pub height: u32,
    pub layers: Vec<PsdLayer>,
    pub composite_rgba: Vec<u8>,
    pub color_mode: ColorMode,
    pub palette: Vec<u8>,
}

impl Psd {
    pub fn from_bytes(data: &[u8]) -> Result<Self, PsdError> {
        let mut parser = PsdParser::new(data);
        parser.parse()
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, PsdError> {
        let mut writer = PsdWriter::new(self);
        writer.write()
    }

    pub fn color_mode(&self) -> ColorMode { self.color_mode }
    pub fn width(&self) -> u32 { self.width }
    pub fn height(&self) -> u32 { self.height }
    pub fn layers(&self) -> &[PsdLayer] { &self.layers }
    pub fn rgba(&self) -> Vec<u8> { self.composite_rgba.clone() }
}

impl PsdLayer {
    pub fn width(&self) -> u32 { self.width }
    pub fn height(&self) -> u32 { self.height }
    pub fn name(&self) -> &str { &self.name }
    pub fn layer_top(&self) -> i32 { self.top }
    pub fn layer_left(&self) -> i32 { self.left }
    pub fn visible(&self) -> bool { self.visible }
    pub fn opacity(&self) -> u8 { self.opacity }
    pub fn blend_mode(&self) -> &str { &self.blend_mode }
    pub fn rgba(&self) -> Vec<u8> { self.rgba.clone() }
    pub fn layer_type(&self) -> PsdLayerType { self.layer_type }
}

pub struct PsdHeader {
    pub version: u16,
    pub channels: u16,
    pub height: u32,
    pub width: u32,
    pub depth: u16,
    pub color_mode: ColorMode,
}

pub struct PsdParser<'a> {
    data: &'a [u8],
    cursor: usize,
}

fn cmyk_to_rgb(c: u8, m: u8, y: u8, k: u8) -> (u8, u8, u8) {
    let c = c as f32 / 255.0;
    let m = m as f32 / 255.0;
    let y = y as f32 / 255.0;
    let k = k as f32 / 255.0;
    let r = 255.0 * (1.0 - c) * (1.0 - k);
    let g = 255.0 * (1.0 - m) * (1.0 - k);
    let b = 255.0 * (1.0 - y) * (1.0 - k);
    (r.clamp(0.0, 255.0) as u8, g.clamp(0.0, 255.0) as u8, b.clamp(0.0, 255.0) as u8)
}

fn lab_to_rgb(l: u8, a: u8, b: u8) -> (u8, u8, u8) {
    let l_f = (l as f32 / 255.0) * 100.0;
    let a_f = a as f32 - 128.0;
    let b_f = b as f32 - 128.0;
    let var_y = (l_f + 16.0) / 116.0;
    let var_x = a_f / 500.0 + var_y;
    let var_z = var_y - b_f / 200.0;
    fn pivot(n: f32) -> f32 { if n.powi(3) > 0.008856 { n.powi(3) } else { (n - 16.0 / 116.0) / 7.787 } }
    let x = pivot(var_x) * 95.047;
    let y = pivot(var_y) * 100.000;
    let z = pivot(var_z) * 108.883;
    let x = x / 100.0; let y = y / 100.0; let z = z / 100.0;
    let r_l = x * 3.2406 + y * -1.5372 + z * -0.4986;
    let g_l = x * -0.9689 + y * 1.8758 + z * 0.0415;
    let b_l = x * 0.0557 + y * -0.2040 + z * 1.0570;
    fn gamma(n: f32) -> f32 { if n > 0.0031308 { 1.055 * n.powf(1.0 / 2.4) - 0.055 } else { 12.92 * n } }
    let r = (gamma(r_l) * 255.0).clamp(0.0, 255.0);
    let g = (gamma(g_l) * 255.0).clamp(0.0, 255.0);
    let b = (gamma(b_l) * 255.0).clamp(0.0, 255.0);
    (r as u8, g as u8, b as u8)
}

fn decode_rle(data: &[u8], expected: usize) -> Result<Vec<u8>, PsdError> {
    let mut out = Vec::with_capacity(expected); let mut i = 0;
    while i < data.len() && out.len() < expected {
        let n = data[i] as i8; i += 1;
        if n >= 0 { let c = n as usize + 1; for _ in 0..c { if i < data.len() && out.len() < expected { out.push(data[i]); i += 1; } } }
        else if n > -128 { let c = (-n) as usize + 1; if i < data.len() { let v = data[i]; i += 1; for _ in 0..c { if out.len() < expected { out.push(v); } } } }
    }
    if out.len() < expected { out.resize(expected, 0); }
    Ok(out)
}

impl<'a> PsdParser<'a> {
    pub fn new(data: &'a [u8]) -> Self { PsdParser { data, cursor: 0 } }
    fn read_u8(&mut self) -> Result<u8, PsdError> { if self.cursor + 1 > self.data.len() { return Err(PsdError::UnexpectedEndOfFile); } let val = self.data[self.cursor]; self.cursor += 1; Ok(val) }
    fn read_u16(&mut self) -> Result<u16, PsdError> { if self.cursor + 2 > self.data.len() { return Err(PsdError::UnexpectedEndOfFile); } let val = u16::from_be_bytes(self.data[self.cursor..self.cursor + 2].try_into().unwrap()); self.cursor += 2; Ok(val) }
    fn read_i16(&mut self) -> Result<i16, PsdError> { if self.cursor + 2 > self.data.len() { return Err(PsdError::UnexpectedEndOfFile); } let val = i16::from_be_bytes(self.data[self.cursor..self.cursor + 2].try_into().unwrap()); self.cursor += 2; Ok(val) }
    fn read_u32(&mut self) -> Result<u32, PsdError> { if self.cursor + 4 > self.data.len() { return Err(PsdError::UnexpectedEndOfFile); } let val = u32::from_be_bytes(self.data[self.cursor..self.cursor + 4].try_into().unwrap()); self.cursor += 4; Ok(val) }
    fn read_i32(&mut self) -> Result<i32, PsdError> { if self.cursor + 4 > self.data.len() { return Err(PsdError::UnexpectedEndOfFile); } let val = i32::from_be_bytes(self.data[self.cursor..self.cursor + 4].try_into().unwrap()); self.cursor += 4; Ok(val) }
    fn read_u64(&mut self) -> Result<u64, PsdError> { if self.cursor + 8 > self.data.len() { return Err(PsdError::UnexpectedEndOfFile); } let val = u64::from_be_bytes(self.data[self.cursor..self.cursor + 8].try_into().unwrap()); self.cursor += 8; Ok(val) }
    fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], PsdError> { if self.cursor + len > self.data.len() { return Err(PsdError::UnexpectedEndOfFile); } let val = &self.data[self.cursor..self.cursor + len]; self.cursor += len; Ok(val) }
    fn skip(&mut self, len: usize) -> Result<(), PsdError> { if self.cursor + len > self.data.len() { return Err(PsdError::UnexpectedEndOfFile); } self.cursor += len; Ok(()) }

    pub fn parse(&mut self) -> Result<Psd, PsdError> {
        let header = self.parse_header()?;
        let color_mode_data_len = self.read_u32()? as usize;
        let palette = if header.color_mode == ColorMode::Indexed && color_mode_data_len > 0 { self.read_bytes(color_mode_data_len)?.to_vec() } else { self.skip(color_mode_data_len)?; Vec::new() };
        let image_resources_len = self.read_u32()? as usize;
        self.skip(image_resources_len)?;
        let layers = self.parse_layers(&header, &palette)?;
        let composite_rgba = self.parse_composite(&header, &palette)?;
        Ok(Psd { width: header.width, height: header.height, layers, composite_rgba, color_mode: header.color_mode, palette })
    }

    fn parse_header(&mut self) -> Result<PsdHeader, PsdError> {
        let signature = self.read_bytes(4)?;
        if signature != b"8BPS" { return Err(PsdError::InvalidSignature); }
        let version = self.read_u16()?;
        if version != 1 && version != 2 { return Err(PsdError::UnsupportedVersion); }
        self.skip(6)?;
        let channels = self.read_u16()?;
        let height = self.read_u32()?;
        let width = self.read_u32()?;
        let depth = self.read_u16()?;
        let color_mode_raw = self.read_u16()?;
        let color_mode = ColorMode::from_u16(color_mode_raw).ok_or(PsdError::UnsupportedColorMode)?;
        if depth != 8 && depth != 16 { return Err(PsdError::UnsupportedDepth); }
        Ok(PsdHeader { version, channels, height, width, depth, color_mode })
    }

    fn parse_layers(&mut self, header: &PsdHeader, palette: &[u8]) -> Result<Vec<PsdLayer>, PsdError> {
        let section_len = if header.version == 1 { self.read_u32()? as u64 } else { self.read_u64()? };
        if section_len == 0 { return Ok(Vec::new()); }
        let section_end = self.cursor + section_len as usize;
        let layer_info_len = if header.version == 1 { self.read_u32()? as u64 } else { self.read_u64()? };
        if layer_info_len == 0 { self.cursor = section_end; return Ok(Vec::new()); }
        let layer_count_raw = self.read_i16()?;
        let layer_count = layer_count_raw.abs() as usize;
        let mut layers = Vec::with_capacity(layer_count);
        let mut layer_records = Vec::with_capacity(layer_count);
        for _ in 0..layer_count {
            let top = self.read_i32()?; let left = self.read_i32()?; let bottom = self.read_i32()?; let right = self.read_i32()?;
            let channels_count = self.read_u16()?;
            let mut channel_infos = Vec::new();
            for _ in 0..channels_count {
                let id = self.read_i16()?;
                let len = if header.version == 1 { self.read_u32()? as u64 } else { self.read_u64()? };
                channel_infos.push((id, len));
            }
            if self.read_bytes(4)? != b"8BIM" { return Err(PsdError::InvalidLayerData); }
            let blend_mode_key = self.read_bytes(4)?;
            let opacity = self.read_u8()?;
            let clipping = self.read_u8()?;
            let flags = self.read_u8()?;
            self.skip(1)?;
            let extra_data_len = self.read_u32()?;
            let extra_data_end = self.cursor + extra_data_len as usize;
            let mut mask_info = None;
            let mask_data_len = self.read_u32()?;
            if mask_data_len > 0 {
                let m_top = self.read_i32()?; let m_left = self.read_i32()?; let m_bottom = self.read_i32()?; let m_right = self.read_i32()?;
                let m_default_color = self.read_u8()?; let m_flags = self.read_u8()?;
                mask_info = Some(PsdMaskInfo { top: m_top, left: m_left, bottom: m_bottom, right: m_right, default_color: m_default_color, flags: m_flags });
                self.skip(mask_data_len as usize - 20)?;
            }
            let blending_ranges_len = self.read_u32()?; self.skip(blending_ranges_len as usize)?;
            let name_len = self.read_u8()?;
            let name_bytes = self.read_bytes(name_len as usize)?;
            let mut name = String::from_utf8_lossy(name_bytes).to_string();
            self.skip(((name_len + 1 + 3) & !3) as usize - (name_len as usize + 1))?;
            let mut layer_type = PsdLayerType::Normal;
            let mut text_data = None;
            let mut vector_mask_path = None;
            while self.cursor < extra_data_end {
                let sig = self.read_bytes(4)?;
                if sig != b"8BIM" && sig != b"8B64" { break; }
                let key = self.read_bytes(4)?;
                let len = if header.version == 1 { self.read_u32()? as u64 } else { self.read_u64()? };
                let next = self.cursor + ((len as usize + 3) & !3);
                match key {
                    b"luni" => {
                        let u_len = self.read_u32()? as usize;
                        let u_bytes = self.read_bytes(u_len)?;
                        let utf16: Vec<u16> = u_bytes.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
                        if let Ok(s) = String::from_utf16(&utf16) { name = s.trim_matches(char::from(0)).to_string(); }
                    }
                    b"lsct" => {
                        let divider = self.read_u32()?;
                        layer_type = match divider { 1 => PsdLayerType::FolderOpen, 2 => PsdLayerType::FolderClosed, 3 => PsdLayerType::SectionDivider, _ => PsdLayerType::Normal };
                    }
                    b"TySh" => {
                        if len > 50 {
                            self.skip(48)?;
                            let t_len = self.read_u16()? as usize;
                            if t_len > 0 && self.cursor + t_len <= next { text_data = Some(String::from_utf8_lossy(self.read_bytes(t_len)?).to_string()); }
                        }
                    }
                    b"vmsk" | b"vsms" => {
                        // Vector Mask parsing
                        if len >= 26 {
                            self.skip(4)?; // Version and flags
                            let path_record_count = (len - 4) / 26;
                            let mut path_data = String::new();
                            for _ in 0..path_record_count {
                                let selector = self.read_u16()?;
                                match selector {
                                    1 | 2 | 4 | 5 => { // Closed/Open subpath length record
                                        self.skip(24)?;
                                    }
                                    6 | 7 | 8 => { // Path points
                                        // Points are 8.24 fixed point [0.0, 1.0] relative to doc size
                                        let mut coords = [0f64; 6];
                                        for i in 0..6 {
                                            let val = self.read_i32()?;
                                            coords[i] = val as f64 / 16777216.0;
                                        }
                                        let y = coords[0] * header.height as f64;
                                        let x = coords[1] * header.width as f64;
                                        if path_data.is_empty() { path_data.push_str(&format!("M {} {}", x, y)); }
                                        else { path_data.push_str(&format!(" L {} {}", x, y)); }
                                    }
                                    _ => { self.skip(24)?; }
                                }
                            }
                            if !path_data.is_empty() { vector_mask_path = Some(path_data); }
                        }
                    }
                    _ => {}
                }
                self.cursor = next;
            }
            self.cursor = extra_data_end;
            let blend_mode = match blend_mode_key {
                b"norm" => "Normal", b"mul " => "Multiply", b"scrn" => "Screen", b"over" => "Overlay", b"dark" => "Darken", b"lite" => "Lighten",
                b"idiv" => "ColorDodge", b"ibrn" => "ColorBurn", b"hLit" => "HardLight", b"sLit" => "SoftLight", b"diff" => "Difference",
                b"smud" => "Exclusion", b"hue " => "Hue", b"sat " => "Saturation", b"colr" => "Color", b"lum " => "Luminosity", _ => "Normal",
            }.to_string();
            layer_records.push(LayerRecord { name, top, left, bottom, right, width: (right - left).max(0) as u32, height: (bottom - top).max(0) as u32, opacity, visible: (flags & (1 << 1)) == 0, blend_mode, channel_infos, layer_type, clipping: clipping > 0, mask_info, text_data, vector_mask: vector_mask_path });
        }
        for record in layer_records {
            let pixel_count = (record.width * record.height) as usize;
            let mut rgba = vec![0u8; pixel_count * 4];
            if pixel_count > 0 { for i in 0..pixel_count { rgba[i * 4 + 3] = 255; } }
            let mut channels = std::collections::HashMap::new();
            for (id, len) in record.channel_infos {
                let (c_w, c_h) = if id < 0 { if let Some(m) = &record.mask_info { ((m.right - m.left).max(0) as u32, (m.bottom - m.top).max(0) as u32) } else { (record.width, record.height) } } else { (record.width, record.height) };
                channels.insert(id, self.read_channel_data(c_w, c_h, len, header.depth)?);
            }
            match header.color_mode {
                ColorMode::Rgb => {
                    for (&id, data) in &channels {
                        let off = match id { 0 => Some(0), 1 => Some(1), 2 => Some(2), -1 | -2 => Some(3), _ => None };
                        if let Some(o) = off { 
                            if id >= 0 { 
                                for (i, &p) in data.iter().enumerate() { if i * 4 + o < rgba.len() { rgba[i * 4 + o] = p; } } 
                            } else if id == -2 {
                                // Apply user mask to alpha with alignment
                                if let Some(m) = &record.mask_info {
                                    let m_w = (m.right - m.left).max(0) as u32;
                                    let m_h = (m.bottom - m.top).max(0) as u32;
                                    
                                    for y in 0..record.height {
                                        let global_y = record.top + y as i32;
                                        if global_y >= m.top && global_y < m.bottom {
                                            let mask_y = (global_y - m.top) as u32;
                                            for x in 0..record.width {
                                                let global_x = record.left + x as i32;
                                                if global_x >= m.left && global_x < m.right {
                                                    let mask_x = (global_x - m.left) as u32;
                                                    let mask_idx = (mask_y * m_w + mask_x) as usize;
                                                    let layer_idx = (y * record.width + x) as usize;
                                                    if mask_idx < data.len() && layer_idx * 4 + 3 < rgba.len() {
                                                        let mask_val = data[mask_idx];
                                                        rgba[layer_idx * 4 + 3] = ((rgba[layer_idx * 4 + 3] as u16 * mask_val as u16) / 255) as u8;
                                                    }
                                                } else {
                                                    // Outside mask bounds - use default color
                                                    let layer_idx = (y * record.width + x) as usize;
                                                    rgba[layer_idx * 4 + 3] = ((rgba[layer_idx * 4 + 3] as u16 * m.default_color as u16) / 255) as u8;
                                                }
                                            }
                                        } else {
                                            // Outside mask row bounds
                                            for x in 0..record.width {
                                                let layer_idx = (y * record.width + x) as usize;
                                                rgba[layer_idx * 4 + 3] = ((rgba[layer_idx * 4 + 3] as u16 * m.default_color as u16) / 255) as u8;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                ColorMode::Cmyk => {
                    let c = channels.get(&0); let m = channels.get(&1); let y = channels.get(&2); let k = channels.get(&3); 
                    let a = channels.get(&-1); let mask = channels.get(&-2);
                    for i in 0..pixel_count {
                        let (r, g, b) = cmyk_to_rgb(c.map(|v| v[i]).unwrap_or(0), m.map(|v| v[i]).unwrap_or(0), y.map(|v| v[i]).unwrap_or(0), k.map(|v| v[i]).unwrap_or(0));
                        rgba[i * 4] = r; rgba[i * 4 + 1] = g; rgba[i * 4 + 2] = b; 
                        let mut alpha = a.map(|v| v[i]).unwrap_or(255);
                        if let Some(mv) = mask { alpha = ((alpha as u16 * mv[i] as u16) / 255) as u8; }
                        rgba[i * 4 + 3] = alpha;
                    }
                }
                ColorMode::Lab => {
                    let l = channels.get(&0); let a_l = channels.get(&1); let b_l = channels.get(&2); 
                    let alpha_ch = channels.get(&-1); let mask = channels.get(&-2);
                    for i in 0..pixel_count {
                        let (r, g, b) = lab_to_rgb(l.map(|v| v[i]).unwrap_or(0), a_l.map(|v| v[i]).unwrap_or(128), b_l.map(|v| v[i]).unwrap_or(128));
                        rgba[i * 4] = r; rgba[i * 4 + 1] = g; rgba[i * 4 + 2] = b; 
                        let mut alpha = alpha_ch.map(|v| v[i]).unwrap_or(255);
                        if let Some(mv) = mask { alpha = ((alpha as u16 * mv[i] as u16) / 255) as u8; }
                        rgba[i * 4 + 3] = alpha;
                    }
                }
                ColorMode::Indexed => {
                    if let Some(idx) = channels.get(&0) {
                        let mask = channels.get(&-2);
                        for (i, &v) in idx.iter().enumerate() { 
                            if i * 4 + 3 < rgba.len() { 
                                rgba[i * 4] = palette[v as usize]; rgba[i * 4 + 1] = palette[v as usize + 256]; rgba[i * 4 + 2] = palette[v as usize + 512]; 
                                let mut alpha = 255u8; if let Some(mv) = mask { alpha = mv[i]; } rgba[i * 4 + 3] = alpha; 
                            } 
                        }
                    }
                }
                ColorMode::Grayscale | ColorMode::Bitmap => {
                    let g = channels.get(&0); let a = channels.get(&-1); let mask = channels.get(&-2);
                    for i in 0..pixel_count { 
                        let v = g.map(|v| v[i]).unwrap_or(0); 
                        rgba[i * 4] = v; rgba[i * 4 + 1] = v; rgba[i * 4 + 2] = v; 
                        let mut alpha = a.map(|v| v[i]).unwrap_or(255);
                        if let Some(mv) = mask { alpha = ((alpha as u16 * mv[i] as u16) / 255) as u8; }
                        rgba[i * 4 + 3] = alpha; 
                    }
                }
                _ => {}
            }
            layers.push(PsdLayer { name: record.name, top: record.top, left: record.left, bottom: record.bottom, right: record.right, width: record.width, height: record.height, opacity: record.opacity, visible: record.visible, blend_mode: record.blend_mode, rgba, layer_type: record.layer_type, clipping: record.clipping, mask_info: record.mask_info, text_data: record.text_data, vector_mask: record.vector_mask });
        }
        self.cursor = section_end;
        Ok(layers)
    }

    fn read_channel_data(&mut self, width: u32, height: u32, len: u64, depth: u16) -> Result<Vec<u8>, PsdError> {
        if len < 2 || width == 0 || height == 0 { if len > 0 { self.skip(len as usize)?; } return Ok(vec![0u8; (width * height) as usize]); }
        let compression = self.read_u16()?;
        let mut data = match compression {
            0 => self.read_bytes(len as usize - 2)?.to_vec(),
            1 => {
                let mut lens = Vec::with_capacity(height as usize);
                for _ in 0..height { lens.push(self.read_u16()?); }
                let mut out = Vec::with_capacity((width * height) as usize);
                for &l in &lens { out.extend_from_slice(&decode_rle(self.read_bytes(l as usize)?, width as usize)?); }
                out
            }
            2 | 3 => {
                let mut dec = ZlibDecoder::new(self.read_bytes(len as usize - 2)?);
                let mut out = Vec::new(); dec.read_to_end(&mut out).map_err(|_| PsdError::ZipError)?;
                if compression == 3 { for y in 0..height { let start = (y * width) as usize; for x in 1..width as usize { out[start + x] = out[start + x].wrapping_add(out[start + x - 1]); } } }
                out
            }
            _ => return Err(PsdError::DecompressionError),
        };
        if depth == 16 {
            let mut out8 = Vec::with_capacity(data.len() / 2);
            for i in 0..data.len() / 2 { out8.push(data[i * 2]); }
            data = out8;
        }
        if data.len() < (width * height) as usize { data.resize((width * height) as usize, 0); }
        Ok(data)
    }

    fn parse_composite(&mut self, header: &PsdHeader, palette: &[u8]) -> Result<Vec<u8>, PsdError> {
        if self.cursor + 2 > self.data.len() { return Ok(vec![255u8; (header.width * header.height * 4) as usize]); }
        let comp = self.read_u16()?;
        let pixel_count = (header.width * header.height) as usize;
        let mut channels = Vec::new();
        if comp == 0 { for _ in 0..header.channels { channels.push(self.read_bytes(pixel_count)?.to_vec()); } }
        else if comp == 1 {
            let mut lens = Vec::with_capacity(header.height as usize * header.channels as usize);
            for _ in 0..(header.height as u32 * header.channels as u32) { lens.push(self.read_u16()?); }
            for c in 0..header.channels {
                let mut out = Vec::with_capacity(pixel_count);
                for y in 0..header.height { out.extend_from_slice(&decode_rle(self.read_bytes(lens[(c as u32 * header.height + y) as usize] as usize)?, header.width as usize)?); }
                channels.push(out);
            }
        }
        let mut rgba = vec![255u8; pixel_count * 4];
        match header.color_mode {
            ColorMode::Rgb => {
                for c in 0..header.channels.min(4) {
                    let off = match c { 0 => 0, 1 => 1, 2 => 2, 3 => 3, _ => continue };
                    if let Some(d) = channels.get(c as usize) { for (i, &p) in d.iter().enumerate() { rgba[i * 4 + off] = p; } }
                }
            }
            ColorMode::Cmyk => { for i in 0..pixel_count { let (r, g, b) = cmyk_to_rgb(channels[0][i], channels[1][i], channels[2][i], channels[3][i]); rgba[i * 4] = r; rgba[i * 4 + 1] = g; rgba[i * 4 + 2] = b; rgba[i * 4 + 3] = channels.get(4).map(|v| v[i]).unwrap_or(255); } }
            ColorMode::Lab => { for i in 0..pixel_count { let (r, g, b) = lab_to_rgb(channels[0][i], channels[1][i], channels[2][i]); rgba[i * 4] = r; rgba[i * 4 + 1] = g; rgba[i * 4 + 2] = b; rgba[i * 4 + 3] = channels.get(3).map(|v| v[i]).unwrap_or(255); } }
            ColorMode::Indexed => { if let Some(idx) = channels.get(0) { for (i, &v) in idx.iter().enumerate() { rgba[i * 4] = palette[v as usize]; rgba[i * 4 + 1] = palette[v as usize + 256]; rgba[i * 4 + 2] = palette[v as usize + 512]; rgba[i * 4 + 3] = 255; } } }
            ColorMode::Grayscale | ColorMode::Bitmap => { if let Some(d) = channels.get(0) { let a = channels.get(1); for i in 0..pixel_count { let v = d[i]; rgba[i * 4] = v; rgba[i * 4 + 1] = v; rgba[i * 4 + 2] = v; rgba[i * 4 + 3] = a.map(|v| v[i]).unwrap_or(255); } } }
            _ => {}
        }
        Ok(rgba)
    }
}

struct LayerRecord {
    pub name: String,
    pub top: i32,
    pub left: i32,
    pub bottom: i32,
    pub right: i32,
    pub width: u32,
    pub height: u32,
    pub opacity: u8,
    pub visible: bool,
    pub blend_mode: String,
    pub channel_infos: Vec<(i16, u64)>,
    pub layer_type: PsdLayerType,
    pub clipping: bool,
    pub mask_info: Option<PsdMaskInfo>,
    pub text_data: Option<String>,
    pub vector_mask: Option<String>,
}

pub struct PsdWriter<'a> { psd: &'a Psd }
impl<'a> PsdWriter<'a> {
    pub fn new(psd: &'a Psd) -> Self { PsdWriter { psd } }
    pub fn write(&mut self) -> Result<Vec<u8>, PsdError> {
        let mut data = Vec::new();
        data.extend_from_slice(b"8BPS"); data.extend_from_slice(&1u16.to_be_bytes()); data.extend_from_slice(&[0u8; 6]); data.extend_from_slice(&3u16.to_be_bytes());
        data.extend_from_slice(&self.psd.height.to_be_bytes()); data.extend_from_slice(&self.psd.width.to_be_bytes()); data.extend_from_slice(&8u16.to_be_bytes()); data.extend_from_slice(&3u16.to_be_bytes());
        data.extend_from_slice(&0u32.to_be_bytes());
        let mut res = Vec::new(); res.extend_from_slice(b"8BIM"); res.extend_from_slice(&1005u16.to_be_bytes()); res.extend_from_slice(&0u16.to_be_bytes()); res.extend_from_slice(&16u32.to_be_bytes());
        res.extend_from_slice(&72u16.to_be_bytes()); res.extend_from_slice(&0u16.to_be_bytes()); res.extend_from_slice(&1u16.to_be_bytes()); res.extend_from_slice(&1u16.to_be_bytes());
        res.extend_from_slice(&72u16.to_be_bytes()); res.extend_from_slice(&0u16.to_be_bytes()); res.extend_from_slice(&1u16.to_be_bytes()); res.extend_from_slice(&1u16.to_be_bytes());
        data.extend_from_slice(&(res.len() as u32).to_be_bytes()); data.extend_from_slice(&res);
        let mut l_info = Vec::new(); l_info.extend_from_slice(&(self.psd.layers.len() as i16).to_be_bytes());
        let mut c_data = Vec::new();
        for l in &self.psd.layers {
            l_info.extend_from_slice(&l.top.to_be_bytes()); l_info.extend_from_slice(&l.left.to_be_bytes()); l_info.extend_from_slice(&l.bottom.to_be_bytes()); l_info.extend_from_slice(&l.right.to_be_bytes());
            let ch = if l.layer_type == PsdLayerType::Normal { 4 } else { 0 };
            l_info.extend_from_slice(&(ch as u16).to_be_bytes());
            if l.layer_type == PsdLayerType::Normal { let p_c = (l.width * l.height) as u64; for id in [0, 1, 2, 65535] { l_info.extend_from_slice(&(id as u16).to_be_bytes()); l_info.extend_from_slice(&(p_c + 2).to_be_bytes()); } }
            l_info.extend_from_slice(b"8BIM");
            l_info.extend_from_slice(match l.blend_mode.as_str() { "Multiply" => b"mul ", "Screen" => b"scrn", "Overlay" => b"over", "Darken" => b"dark", "Lighten" => b"lite", "ColorDodge" => b"idiv", "ColorBurn" => b"ibrn", "HardLight" => b"hLit", "SoftLight" => b"sLit", "Difference" => b"diff", "Exclusion" => b"smud", "Hue" => b"hue ", "Saturation" => b"sat ", "Color" => b"colr", "Luminosity" => b"lum ", _ => b"norm" });
            l_info.push(l.opacity); l_info.push(0); l_info.push(if l.visible { 0 } else { 2 }); l_info.push(0);
            let mut ex = Vec::new(); ex.extend_from_slice(&0u32.to_be_bytes()); ex.extend_from_slice(&0u32.to_be_bytes());
            let n_b = l.name.as_bytes(); let n_l = n_b.len().min(255) as u8; ex.push(n_l); ex.extend_from_slice(&n_b[..n_l as usize]); ex.extend_from_slice(&vec![0u8; (4 - ((n_l as usize + 1) % 4)) % 4]);
            if !l.name.is_empty() {
                ex.extend_from_slice(b"8BIM"); ex.extend_from_slice(b"luni");
                let u16_n: Vec<u8> = l.name.encode_utf16().flat_map(|u| u.to_be_bytes()).collect();
                ex.extend_from_slice(&((u16_n.len() + 4) as u32).to_be_bytes()); ex.extend_from_slice(&(l.name.chars().count() as u32).to_be_bytes()); ex.extend_from_slice(&u16_n);
            }
            if l.layer_type != PsdLayerType::Normal {
                ex.extend_from_slice(b"8BIM"); ex.extend_from_slice(b"lsct"); ex.extend_from_slice(&4u32.to_be_bytes());
                ex.extend_from_slice(&match l.layer_type { PsdLayerType::FolderOpen => 1u32, PsdLayerType::FolderClosed => 2u32, PsdLayerType::SectionDivider => 3u32, _ => 0u32 }.to_be_bytes());
            }
            l_info.extend_from_slice(&(ex.len() as u32).to_be_bytes()); l_info.extend_from_slice(&ex);
            if l.layer_type == PsdLayerType::Normal { for c in 0..4 { c_data.extend_from_slice(&0u16.to_be_bytes()); let off = match c { 0 => 0, 1 => 1, 2 => 2, 3 => 3, _ => 0 }; for i in 0..(l.width * l.height) as usize { c_data.push(l.rgba[i * 4 + off]); } } }
        }
        let mut lm = Vec::new(); lm.extend_from_slice(&((l_info.len() + 4) as u32).to_be_bytes()); lm.extend_from_slice(&l_info); lm.extend_from_slice(&c_data);
        data.extend_from_slice(&(lm.len() as u32).to_be_bytes()); data.extend_from_slice(&lm);
        data.extend_from_slice(&0u16.to_be_bytes());
        for c in 0..3 { for i in 0..(self.psd.width * self.psd.height) as usize { data.push(self.psd.composite_rgba[i * 4 + c]); } }
        Ok(data)
    }
}
