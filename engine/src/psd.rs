use std::convert::TryInto;

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

pub struct PsdHeader {
    pub version: u16,
    pub channels: u16,
    pub height: u32,
    pub width: u32,
    pub depth: u16,
    pub color_mode: ColorMode,
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
}

pub struct Psd {
    pub width: u32,
    pub height: u32,
    pub layers: Vec<PsdLayer>,
    pub composite_rgba: Vec<u8>,
    pub color_mode: ColorMode,
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

    pub fn color_mode(&self) -> ColorMode {
        self.color_mode
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn layers(&self) -> &[PsdLayer] {
        &self.layers
    }

    pub fn rgba(&self) -> Vec<u8> {
        self.composite_rgba.clone()
    }
}

impl PsdLayer {
    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn layer_top(&self) -> i32 {
        self.top
    }

    pub fn layer_left(&self) -> i32 {
        self.left
    }

    pub fn visible(&self) -> bool {
        self.visible
    }

    pub fn opacity(&self) -> u8 {
        self.opacity
    }

    pub fn blend_mode(&self) -> &str {
        &self.blend_mode
    }

    pub fn rgba(&self) -> Vec<u8> {
        self.rgba.clone()
    }
    
    pub fn layer_type(&self) -> PsdLayerType {
        self.layer_type
    }
}

pub struct PsdParser<'a> {
    data: &'a [u8],
    cursor: usize,
}

fn cmyk_to_rgb(c: u8, m: u8, y: u8, k: u8) -> (u8, u8, u8) {
    let c = 255 - c;
    let m = 255 - m;
    let y = 255 - y;
    let k = 255 - k;

    let r = (c as f32 * k as f32) / 255.0;
    let g = (m as f32 * k as f32) / 255.0;
    let b = (y as f32 * k as f32) / 255.0;

    (r as u8, g as u8, b as u8)
}

impl<'a> PsdParser<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        PsdParser { data, cursor: 0 }
    }

    fn read_u8(&mut self) -> Result<u8, PsdError> {
        if self.cursor + 1 > self.data.len() {
            return Err(PsdError::UnexpectedEndOfFile);
        }
        let val = self.data[self.cursor];
        self.cursor += 1;
        Ok(val)
    }

    fn read_u16(&mut self) -> Result<u16, PsdError> {
        if self.cursor + 2 > self.data.len() {
            return Err(PsdError::UnexpectedEndOfFile);
        }
        let val = u16::from_be_bytes(self.data[self.cursor..self.cursor + 2].try_into().unwrap());
        self.cursor += 2;
        Ok(val)
    }

    fn read_u32(&mut self) -> Result<u32, PsdError> {
        if self.cursor + 4 > self.data.len() {
            return Err(PsdError::UnexpectedEndOfFile);
        }
        let val = u32::from_be_bytes(self.data[self.cursor..self.cursor + 4].try_into().unwrap());
        self.cursor += 4;
        Ok(val)
    }

    fn read_i32(&mut self) -> Result<i32, PsdError> {
        if self.cursor + 4 > self.data.len() {
            return Err(PsdError::UnexpectedEndOfFile);
        }
        let val = i32::from_be_bytes(self.data[self.cursor..self.cursor + 4].try_into().unwrap());
        self.cursor += 4;
        Ok(val)
    }

    fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], PsdError> {
        if self.cursor + len > self.data.len() {
            return Err(PsdError::UnexpectedEndOfFile);
        }
        let val = &self.data[self.cursor..self.cursor + len];
        self.cursor += len;
        Ok(val)
    }

    fn skip(&mut self, len: usize) -> Result<(), PsdError> {
        if self.cursor + len > self.data.len() {
            return Err(PsdError::UnexpectedEndOfFile);
        }
        self.cursor += len;
        Ok(())
    }

    pub fn parse(&mut self) -> Result<Psd, PsdError> {
        let header = self.parse_header()?;
        
        // Color Mode Data Section
        let color_mode_data_len = self.read_u32()? as usize;
        self.skip(color_mode_data_len)?;

        // Image Resources Section
        let image_resources_len = self.read_u32()? as usize;
        self.skip(image_resources_len)?;

        // Layer and Mask Information Section
        let layers = self.parse_layers(&header)?;

        // Image Data Section (Composite Image)
        let composite_rgba = self.parse_composite(&header)?;

        Ok(Psd {
            width: header.width,
            height: header.height,
            layers,
            composite_rgba,
            color_mode: header.color_mode,
        })
    }

    fn parse_header(&mut self) -> Result<PsdHeader, PsdError> {
        let signature = self.read_bytes(4)?;
        if signature != b"8BPS" {
            return Err(PsdError::InvalidSignature);
        }

        let version = self.read_u16()?;
        if version != 1 && version != 2 {
            return Err(PsdError::UnsupportedVersion);
        }

        self.skip(6)?; // Reserved

        let channels = self.read_u16()?;
        let height = self.read_u32()?;
        let width = self.read_u32()?;
        let depth = self.read_u16()?;
        let color_mode_raw = self.read_u16()?;
        let color_mode = ColorMode::from_u16(color_mode_raw).ok_or(PsdError::UnsupportedColorMode)?;

        if depth != 8 {
            return Err(PsdError::UnsupportedDepth);
        }

        Ok(PsdHeader {
            version,
            channels,
            height,
            width,
            depth,
            color_mode,
        })
    }

    fn parse_layers(&mut self, header: &PsdHeader) -> Result<Vec<PsdLayer>, PsdError> {
        let section_len = if header.version == 1 {
            self.read_u32()? as u64
        } else {
            if self.cursor + 8 > self.data.len() { return Err(PsdError::UnexpectedEndOfFile); }
            let len = u64::from_be_bytes(self.data[self.cursor..self.cursor + 8].try_into().unwrap());
            self.cursor += 8;
            len
        };

        if section_len == 0 {
            return Ok(Vec::new());
        }

        let section_end = self.cursor + section_len as usize;

        // Layer Info
        let layer_info_len = if header.version == 1 {
            self.read_u32()? as u64
        } else {
            if self.cursor + 8 > self.data.len() { return Err(PsdError::UnexpectedEndOfFile); }
            let len = u64::from_be_bytes(self.data[self.cursor..self.cursor + 8].try_into().unwrap());
            self.cursor += 8;
            len
        };

        if layer_info_len == 0 {
            self.cursor = section_end;
            return Ok(Vec::new());
        }

        let layer_count_raw = self.read_u16()?;
        let layer_count = (layer_count_raw as i16).abs() as usize;
        
        let mut layers = Vec::with_capacity(layer_count);
        let mut layer_records = Vec::with_capacity(layer_count);

        for _ in 0..layer_count {
            let top = self.read_i32()?;
            let left = self.read_i32()?;
            let bottom = self.read_i32()?;
            let right = self.read_i32()?;
            let channels_count = self.read_u16()?;
            
            let mut channel_infos = Vec::new();
            for _ in 0..channels_count {
                let id = self.read_u16()?;
                let len = if header.version == 1 {
                    self.read_u32()? as u64
                } else {
                    if self.cursor + 8 > self.data.len() { return Err(PsdError::UnexpectedEndOfFile); }
                    let len = u64::from_be_bytes(self.data[self.cursor..self.cursor + 8].try_into().unwrap());
                    self.cursor += 8;
                    len
                };
                channel_infos.push((id, len));
            }

            let blend_signature = self.read_bytes(4)?;
            if blend_signature != b"8BIM" {
                return Err(PsdError::InvalidLayerData);
            }

            let blend_mode_key = self.read_bytes(4)?;
            let opacity = self.read_u8()?;
            let _clipping = self.read_u8()?;
            let flags = self.read_u8()?;
            self.skip(1)?; // filler

            let extra_data_len = self.read_u32()?;
            let extra_data_end = self.cursor + extra_data_len as usize;

            // Layer Mask Data
            let mask_data_len = self.read_u32()?;
            self.skip(mask_data_len as usize)?;

            // Layer Blending Ranges
            let blending_ranges_len = self.read_u32()?;
            self.skip(blending_ranges_len as usize)?;

            // Layer Name (Pascal string padded to 4 bytes)
            let name_len = self.read_u8()?;
            let name_bytes = self.read_bytes(name_len as usize)?;
            let mut name = String::from_utf8_lossy(name_bytes).to_string();
            
            // Padding
            let padded_name_len = (name_len + 1 + 3) & !3;
            let padding_to_skip = padded_name_len as usize - (name_len as usize + 1);
            self.skip(padding_to_skip)?;

            // Additional Layer Info
            let mut layer_type = PsdLayerType::Normal;
            while self.cursor < extra_data_end {
                let signature = self.read_bytes(4)?;
                if signature != b"8BIM" && signature != b"8B64" {
                    break;
                }
                let key = self.read_bytes(4)?;
                let len = self.read_u32()? as usize;
                let next_info = self.cursor + ((len + 3) & !3);

                match key {
                    b"luni" => {
                        let unicode_name_len = self.read_u32()? as usize;
                        let unicode_bytes = self.read_bytes(unicode_name_len)?;
                        let utf16_data: Vec<u16> = unicode_bytes.chunks_exact(2)
                            .map(|c| u16::from_be_bytes([c[0], c[1]]))
                            .collect();
                        if let Ok(s) = String::from_utf16(&utf16_data) {
                            name = s.trim_matches(char::from(0)).to_string();
                        }
                    }
                    b"lsct" => {
                        let divider_type = self.read_u32()?;
                        layer_type = match divider_type {
                            1 => PsdLayerType::FolderOpen,
                            2 => PsdLayerType::FolderClosed,
                            3 => PsdLayerType::SectionDivider,
                            _ => PsdLayerType::Normal,
                        };
                    }
                    _ => {}
                }
                self.cursor = next_info;
            }
            self.cursor = extra_data_end;

            let width = (right - left).max(0) as u32;
            let height = (bottom - top).max(0) as u32;
            let visible = (flags & (1 << 1)) == 0;

            let blend_mode = match blend_mode_key {
                b"norm" => "Normal",
                b"mul " => "Multiply",
                b"scrn" => "Screen",
                b"over" => "Overlay",
                b"dark" => "Darken",
                b"lite" => "Lighten",
                b"idiv" => "ColorDodge",
                b"ibrn" => "ColorBurn",
                b"hLit" => "HardLight",
                b"sLit" => "SoftLight",
                b"diff" => "Difference",
                b"smud" => "Exclusion",
                b"hue " => "Hue",
                b"sat " => "Saturation",
                b"colr" => "Color",
                b"lum " => "Luminosity",
                _ => "Normal",
            }.to_string();

            layer_records.push(LayerRecord {
                name, top, left, bottom, right, width, height, opacity, visible, blend_mode, channel_infos, layer_type
            });
        }

        // Channel Image Data
        for record in layer_records.into_iter() {
            let pixel_count = (record.width * record.height) as usize;
            let mut rgba = vec![0u8; pixel_count * 4];
            if pixel_count > 0 {
                for i in 0..pixel_count {
                    rgba[i * 4 + 3] = 255;
                }
            }

            let mut channels = std::collections::HashMap::new();
            for (channel_id, channel_len) in record.channel_infos {
                let channel_data = self.read_channel_data(record.width, record.height, channel_len)?;
                channels.insert(channel_id, channel_data);
            }

            if header.color_mode == ColorMode::Rgb {
                for (&id, data) in &channels {
                    let offset = match id {
                        0 => Some(0), // R
                        1 => Some(1), // G
                        2 => Some(2), // B
                        65535 => Some(3), // Alpha
                        _ => None,
                    };
                    if let Some(off) = offset {
                        for (i, &pixel) in data.iter().enumerate() {
                            if i * 4 + off < rgba.len() {
                                rgba[i * 4 + off] = pixel;
                            }
                        }
                    }
                }
            } else if header.color_mode == ColorMode::Cmyk {
                let c = channels.get(&0);
                let m = channels.get(&1);
                let y = channels.get(&2);
                let k = channels.get(&3);
                let a = channels.get(&65535);

                for i in 0..pixel_count {
                    let cv = c.map(|v| v[i]).unwrap_or(0);
                    let mv = m.map(|v| v[i]).unwrap_or(0);
                    let yv = y.map(|v| v[i]).unwrap_or(0);
                    let kv = k.map(|v| v[i]).unwrap_or(0);
                    let av = a.map(|v| v[i]).unwrap_or(255);

                    let (r, g, b) = cmyk_to_rgb(cv, mv, yv, kv);
                    rgba[i * 4] = r;
                    rgba[i * 4 + 1] = g;
                    rgba[i * 4 + 2] = b;
                    rgba[i * 4 + 3] = av;
                }
            } else if header.color_mode == ColorMode::Grayscale {
                let g = channels.get(&0);
                let a = channels.get(&65535);
                for i in 0..pixel_count {
                    let gv = g.map(|v| v[i]).unwrap_or(0);
                    let av = a.map(|v| v[i]).unwrap_or(255);
                    rgba[i * 4] = gv;
                    rgba[i * 4 + 1] = gv;
                    rgba[i * 4 + 2] = gv;
                    rgba[i * 4 + 3] = av;
                }
            }

            layers.push(PsdLayer {
                name: record.name,
                top: record.top,
                left: record.left,
                bottom: record.bottom,
                right: record.right,
                width: record.width,
                height: record.height,
                opacity: record.opacity,
                visible: record.visible,
                blend_mode: record.blend_mode,
                rgba,
                layer_type: record.layer_type,
            });
        }

        self.cursor = section_end;
        Ok(layers)
    }

    fn read_channel_data(&mut self, width: u32, height: u32, len: u64) -> Result<Vec<u8>, PsdError> {
        if len < 2 || width == 0 || height == 0 {
            if len > 0 { self.skip(len as usize)?; }
            return Ok(vec![0u8; (width * height) as usize]);
        }

        let compression = self.read_u16()?;
        let data_len = len as usize - 2;

        match compression {
            0 => { // Raw
                let bytes = self.read_bytes(data_len)?;
                if bytes.len() < (width * height) as usize {
                    let mut out = bytes.to_vec();
                    out.resize((width * height) as usize, 0);
                    Ok(out)
                } else {
                    Ok(bytes[0..(width * height) as usize].to_vec())
                }
            }
            1 => { // RLE
                let mut scanline_lengths = Vec::with_capacity(height as usize);
                for _ in 0..height {
                    scanline_lengths.push(self.read_u16()?);
                }

                let mut channel_data = Vec::with_capacity((width * height) as usize);
                for y in 0..height {
                    let line_len = scanline_lengths[y as usize] as usize;
                    let compressed = self.read_bytes(line_len)?;
                    let decompressed = self.decode_rle(compressed, width as usize)?;
                    channel_data.extend_from_slice(&decompressed);
                }
                Ok(channel_data)
            }
            _ => {
                if data_len > 0 { self.skip(data_len)?; }
                Ok(vec![0u8; (width * height) as usize])
            }
        }
    }

    fn decode_rle(&self, data: &[u8], expected_len: usize) -> Result<Vec<u8>, PsdError> {
        let mut output = Vec::with_capacity(expected_len);
        let mut i = 0;
        while i < data.len() && output.len() < expected_len {
            let n = data[i] as i8;
            i += 1;
            if n >= 0 {
                let count = n as usize + 1;
                for _ in 0..count {
                    if i < data.len() && output.len() < expected_len {
                        output.push(data[i]);
                        i += 1;
                    }
                }
            } else if n > -128 {
                let count = (-n) as usize + 1;
                if i < data.len() {
                    let val = data[i];
                    i += 1;
                    for _ in 0..count {
                        if output.len() < expected_len {
                            output.push(val);
                        }
                    }
                }
            }
        }
        if output.len() < expected_len {
            output.resize(expected_len, 0);
        }
        Ok(output)
    }

    fn parse_composite(&mut self, header: &PsdHeader) -> Result<Vec<u8>, PsdError> {
        if self.cursor + 2 > self.data.len() {
            return Ok(vec![255u8; (header.width * header.height * 4) as usize]);
        }
        let compression = self.read_u16()?;
        
        let pixel_count = (header.width * header.height) as usize;
        let mut rgba = vec![255u8; pixel_count * 4];

        let mut channel_data_list = Vec::with_capacity(header.channels as usize);

        if compression == 0 {
            for _ in 0..header.channels {
                channel_data_list.push(self.read_bytes(pixel_count)?.to_vec());
            }
        } else if compression == 1 {
            let mut scanline_lengths = Vec::with_capacity(header.height as usize * header.channels as usize);
            for _ in 0..(header.height as u32 * header.channels as u32) {
                scanline_lengths.push(self.read_u16()?);
            }

            for channel_idx in 0..header.channels {
                let mut channel_data = Vec::with_capacity(pixel_count);
                for y in 0..header.height {
                    let len = scanline_lengths[(channel_idx as u32 * header.height + y) as usize] as usize;
                    let compressed = self.read_bytes(len)?;
                    let decompressed = self.decode_rle(compressed, header.width as usize)?;
                    channel_data.extend_from_slice(&decompressed);
                }
                channel_data_list.push(channel_data);
            }
        }

        if header.color_mode == ColorMode::Rgb {
            if header.channels < 4 {
                for i in 0..pixel_count {
                    rgba[i * 4 + 3] = 255;
                }
            }
            for channel_idx in 0..header.channels {
                let offset = match channel_idx {
                    0 => Some(0), 1 => Some(1), 2 => Some(2), 3 => Some(3),
                    _ => None,
                };
                if let Some(off) = offset {
                    if let Some(data) = channel_data_list.get(channel_idx as usize) {
                        for (i, &pixel) in data.iter().enumerate() {
                            if i * 4 + off < rgba.len() {
                                rgba[i * 4 + off] = pixel;
                            }
                        }
                    }
                }
            }
        } else if header.color_mode == ColorMode::Cmyk {
            for i in 0..pixel_count {
                let cv = channel_data_list.get(0).map(|v| v[i]).unwrap_or(0);
                let mv = channel_data_list.get(1).map(|v| v[i]).unwrap_or(0);
                let yv = channel_data_list.get(2).map(|v| v[i]).unwrap_or(0);
                let kv = channel_data_list.get(3).map(|v| v[i]).unwrap_or(0);
                let av = channel_data_list.get(4).map(|v| v[i]).unwrap_or(255);
                let (r, g, b) = cmyk_to_rgb(cv, mv, yv, kv);
                rgba[i * 4] = r;
                rgba[i * 4 + 1] = g;
                rgba[i * 4 + 2] = b;
                rgba[i * 4 + 3] = av;
            }
        } else if header.color_mode == ColorMode::Grayscale {
            if let Some(data) = channel_data_list.get(0) {
                let alpha = channel_data_list.get(1);
                for i in 0..pixel_count {
                    let gv = data[i];
                    let av = alpha.map(|v| v[i]).unwrap_or(255);
                    rgba[i * 4] = gv;
                    rgba[i * 4 + 1] = gv;
                    rgba[i * 4 + 2] = gv;
                    rgba[i * 4 + 3] = av;
                }
            }
        }
        Ok(rgba)
    }
}

struct LayerRecord {
    name: String,
    top: i32,
    left: i32,
    bottom: i32,
    right: i32,
    width: u32,
    height: u32,
    opacity: u8,
    visible: bool,
    blend_mode: String,
    channel_infos: Vec<(u16, u64)>,
    layer_type: PsdLayerType,
}

pub struct PsdWriter<'a> {
    psd: &'a Psd,
}

impl<'a> PsdWriter<'a> {
    pub fn new(psd: &'a Psd) -> Self {
        PsdWriter { psd }
    }

    pub fn write(&mut self) -> Result<Vec<u8>, PsdError> {
        let mut data = Vec::new();

        // 1. Header
        data.extend_from_slice(b"8BPS");
        data.extend_from_slice(&1u16.to_be_bytes()); // Version
        data.extend_from_slice(&[0u8; 6]); // Reserved
        data.extend_from_slice(&3u16.to_be_bytes()); // Channels (RGB)
        data.extend_from_slice(&self.psd.height.to_be_bytes());
        data.extend_from_slice(&self.psd.width.to_be_bytes());
        data.extend_from_slice(&8u16.to_be_bytes()); // Depth
        data.extend_from_slice(&3u16.to_be_bytes()); // ColorMode RGB

        // 2. Color Mode Data
        data.extend_from_slice(&0u32.to_be_bytes());

        // 3. Image Resources
        let mut resources = Vec::new();
        // Resolution info (72 dpi)
        resources.extend_from_slice(b"8BIM");
        resources.extend_from_slice(&1005u16.to_be_bytes()); // ResolutionInfo
        resources.extend_from_slice(&0u16.to_be_bytes()); // Name
        resources.extend_from_slice(&16u32.to_be_bytes()); // Length
        resources.extend_from_slice(&72u16.to_be_bytes()); resources.extend_from_slice(&0u16.to_be_bytes()); // HRes
        resources.extend_from_slice(&1u16.to_be_bytes()); resources.extend_from_slice(&1u16.to_be_bytes()); // HRes unit
        resources.extend_from_slice(&72u16.to_be_bytes()); resources.extend_from_slice(&0u16.to_be_bytes()); // VRes
        resources.extend_from_slice(&1u16.to_be_bytes()); resources.extend_from_slice(&1u16.to_be_bytes()); // VRes unit

        data.extend_from_slice(&(resources.len() as u32).to_be_bytes());
        data.extend_from_slice(&resources);

        // 4. Layer and Mask Information
        let mut layer_info = Vec::new();
        let layer_count = self.psd.layers.len() as i16;
        layer_info.extend_from_slice(&layer_count.to_be_bytes());

        let mut channel_data = Vec::new();

        for layer in &self.psd.layers {
            layer_info.extend_from_slice(&layer.top.to_be_bytes());
            layer_info.extend_from_slice(&layer.left.to_be_bytes());
            layer_info.extend_from_slice(&layer.bottom.to_be_bytes());
            layer_info.extend_from_slice(&layer.right.to_be_bytes());
            
            let channels = if layer.layer_type == PsdLayerType::Normal { 4 } else { 0 };
            layer_info.extend_from_slice(&(channels as u16).to_be_bytes());

            if layer.layer_type == PsdLayerType::Normal {
                let pixel_count = (layer.width * layer.height) as u64;
                for id in [0, 1, 2, 65535] { // R, G, B, A
                    layer_info.extend_from_slice(&(id as u16).to_be_bytes());
                    layer_info.extend_from_slice(&(pixel_count + 2).to_be_bytes()); // Raw + 2 bytes for compression type
                }
            }

            layer_info.extend_from_slice(b"8BIM");
            let blend_mode = match layer.blend_mode.as_str() {
                "Multiply" => b"mul ", "Screen" => b"scrn", "Overlay" => b"over",
                "Darken" => b"dark", "Lighten" => b"lite", "ColorDodge" => b"idiv",
                "ColorBurn" => b"ibrn", "HardLight" => b"hLit", "SoftLight" => b"sLit",
                "Difference" => b"diff", "Exclusion" => b"smud", "Hue" => b"hue ",
                "Saturation" => b"sat ", "Color" => b"colr", "Luminosity" => b"lum ",
                _ => b"norm",
            };
            layer_info.extend_from_slice(blend_mode);
            layer_info.extend_from_slice(&layer.opacity.to_be_bytes());
            layer_info.extend_from_slice(&0u8.to_be_bytes()); // Clipping
            let flags = if layer.visible { 0u8 } else { 1 << 1 };
            layer_info.extend_from_slice(&flags.to_be_bytes());
            layer_info.extend_from_slice(&0u8.to_be_bytes()); // Filler

            // Extra data
            let mut extra = Vec::new();
            extra.extend_from_slice(&0u32.to_be_bytes()); // Mask data len
            extra.extend_from_slice(&0u32.to_be_bytes()); // Blending ranges len
            
            let name_bytes = layer.name.as_bytes();
            let name_len = name_bytes.len().min(255) as u8;
            extra.push(name_len);
            extra.extend_from_slice(&name_bytes[..name_len as usize]);
            let padding = (4 - ((name_len as usize + 1) % 4)) % 4;
            extra.extend_from_slice(&vec![0u8; padding]);

            // luni and lsct
            if !layer.name.is_empty() {
                extra.extend_from_slice(b"8BIM");
                extra.extend_from_slice(b"luni");
                let utf16: Vec<u8> = layer.name.encode_utf16().flat_map(|u| u.to_be_bytes()).collect();
                extra.extend_from_slice(&((utf16.len() + 4) as u32).to_be_bytes());
                extra.extend_from_slice(&(layer.name.chars().count() as u32).to_be_bytes());
                extra.extend_from_slice(&utf16);
            }

            if layer.layer_type != PsdLayerType::Normal {
                extra.extend_from_slice(b"8BIM");
                extra.extend_from_slice(b"lsct");
                extra.extend_from_slice(&4u32.to_be_bytes());
                let divider_type = match layer.layer_type {
                    PsdLayerType::FolderOpen => 1u32,
                    PsdLayerType::FolderClosed => 2u32,
                    PsdLayerType::SectionDivider => 3u32,
                    _ => 0u32,
                };
                extra.extend_from_slice(&divider_type.to_be_bytes());
            }

            layer_info.extend_from_slice(&(extra.len() as u32).to_be_bytes());
            layer_info.extend_from_slice(&extra);

            if layer.layer_type == PsdLayerType::Normal {
                // Write raw data for each channel
                for channel_idx in 0..4 {
                    channel_data.extend_from_slice(&0u16.to_be_bytes()); // Raw compression
                    let pixel_count = (layer.width * layer.height) as usize;
                    let channel_offset = match channel_idx { 0 => 0, 1 => 1, 2 => 2, 3 => 3, _ => 0 };
                    for i in 0..pixel_count {
                        channel_data.push(layer.rgba[i * 4 + channel_offset]);
                    }
                }
            }
        }

        let mut layer_and_mask = Vec::new();
        layer_and_mask.extend_from_slice(&((layer_info.len() + 4) as u32).to_be_bytes());
        layer_and_mask.extend_from_slice(&layer_info);
        layer_and_mask.extend_from_slice(&channel_data);

        data.extend_from_slice(&(layer_and_mask.len() as u32).to_be_bytes());
        data.extend_from_slice(&layer_and_mask);

        // 5. Image Data (Composite)
        data.extend_from_slice(&0u16.to_be_bytes()); // Raw
        let total_pixels = (self.psd.width * self.psd.height) as usize;
        for c in 0..3 { // R, G, B
            for i in 0..total_pixels {
                data.push(self.psd.composite_rgba[i * 4 + c]);
            }
        }

        Ok(data)
    }
}
