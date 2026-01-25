use wasm_bindgen::prelude::*;
use crate::engine::VectorEngine;
use crate::types::ShapeType;
use web_sys::HtmlImageElement;

#[wasm_bindgen]
impl VectorEngine {
    pub fn erase_image(&mut self, id: u32, x: f64, y: f64, radius: f64) -> bool {
        if let Some(obj) = self.objects.iter_mut().find(|o| o.id == id) {
            if obj.shape_type != ShapeType::Image { return false; }
            let pixels = match &mut obj.raw_rgba { Some(p) => p, None => return false, };
            let width = obj.raw_rgba_width as f64;
            let height = obj.raw_rgba_height as f64;
            let cx = obj.x + obj.width / 2.0; let cy = obj.y + obj.height / 2.0;
            let dx = x - cx; let dy = y - cy;
            let cos_r = (-obj.rotation).cos(); let sin_r = (-obj.rotation).sin();
            let lx = dx * cos_r - dy * sin_r; let ly = dx * sin_r + dy * cos_r;
            let px = (lx / obj.width + 0.5) * width; let py = (ly / obj.height + 0.5) * height;
            let scale_x = width / obj.width; let scale_y = height / obj.height;
            let p_radius = radius * (scale_x + scale_y) / 2.0;
            let r2 = p_radius * p_radius;
            let i_width = obj.raw_rgba_width as i32; let i_height = obj.raw_rgba_height as i32;
            let min_px = (px - p_radius).floor() as i32; let max_px = (px + p_radius).ceil() as i32;
            let min_py = (py - p_radius).floor() as i32; let max_py = (py + p_radius).ceil() as i32;
            let mut modified = false;
            for iy in min_py..max_py {
                if iy < 0 || iy >= i_height { continue; }
                for ix in min_px..max_px {
                    if ix < 0 || ix >= i_width { continue; }
                    let dx_p = ix as f64 - px; let dy_p = iy as f64 - py;
                    if dx_p*dx_p + dy_p*dy_p <= r2 {
                        let idx = (iy * i_width + ix) as usize * 4;
                        if pixels[idx + 3] != 0 { pixels[idx + 3] = 0; modified = true; }
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
            let (width, height, o_x, o_y, o_w, o_h, o_rot) = (obj.raw_rgba_width, obj.raw_rgba_height, obj.x, obj.y, obj.width, obj.height, obj.rotation);
            let to_local = |wx: f64, wy: f64| {
                let cx = o_x + o_w / 2.0; let cy = o_y + o_h / 2.0;
                let dx = wx - cx; let dy = wy - cy;
                let cos_r = (-o_rot).cos(); let sin_r = (-o_rot).sin();
                let lx = dx * cos_r - dy * sin_r; let ly = dx * sin_r + dy * cos_r;
                (((lx / o_w + 0.5) * width as f64) as i32, ((ly / o_h + 0.5) * height as f64) as i32)
            };
            let (lsx, lsy) = to_local(src_x, src_y); let (ldx, ldy) = to_local(dst_x, dst_y);
            let p_radius = (radius * (width as f64 / o_w)) as i32; let r2 = p_radius * p_radius;
            let pixels = match &mut obj.raw_rgba { Some(p) => p, None => return false, };
            let (i_width, i_height) = (width as i32, height as i32);
            let mut modified = false; let mut new_pixels = pixels.clone();
            for dy in -p_radius..p_radius {
                for dx in -p_radius..p_radius {
                    if dx*dx + dy*dy <= r2 {
                        let (sx, sy, tx, ty) = (lsx + dx, lsy + dy, ldx + dx, ldy + dy);
                        if sx >= 0 && sx < i_width && sy >= 0 && sy < i_height && tx >= 0 && tx < i_width && ty >= 0 && ty < i_height {
                            let src_idx = (sy * i_width + sx) as usize * 4; let dst_idx = (ty * i_width + tx) as usize * 4;
                            new_pixels[dst_idx] = pixels[src_idx]; new_pixels[dst_idx+1] = pixels[src_idx+1];
                            new_pixels[dst_idx+2] = pixels[src_idx+2]; new_pixels[dst_idx+3] = pixels[src_idx+3];
                            modified = true;
                        }
                    }
                }
            }
            if modified { *pixels = new_pixels; }
            return modified;
        }
        false
    }

    pub fn get_image_rgba(&self, id: u32) -> Option<Vec<u8>> { self.objects.iter().find(|o| o.id == id).and_then(|o| o.raw_rgba.clone()) }
    pub fn get_image_width(&self, id: u32) -> u32 { self.objects.iter().find(|o| o.id == id).map(|o| o.raw_rgba_width).unwrap_or(0) }
    pub fn get_image_height(&self, id: u32) -> u32 { self.objects.iter().find(|o| o.id == id).map(|o| o.raw_rgba_height).unwrap_or(0) }
    pub fn set_image_raw(&mut self, id: u32, data: Vec<u8>) -> bool {
        if let Some(obj) = self.objects.iter_mut().find(|o| o.id == id) { obj.raw_image = Some(data); true } else { false }
    }
    pub fn set_image_object(&mut self, id: u32, image_val: JsValue) -> bool {
        if let Some(obj) = self.objects.iter_mut().find(|o| o.id == id) {
            if let Some(image) = image_val.dyn_ref::<HtmlImageElement>() {
                if obj.sw == 0.0 { obj.sw = image.width() as f64; } if obj.sh == 0.0 { obj.sh = image.height() as f64; }
            } else if let Some(canvas) = image_val.dyn_ref::<web_sys::HtmlCanvasElement>() {
                if obj.sw == 0.0 { obj.sw = canvas.width() as f64; } if obj.sh == 0.0 { obj.sh = canvas.height() as f64; }
            }
            obj.image = Some(image_val); true
        } else { false }
    }
}
