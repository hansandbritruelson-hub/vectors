use wasm_bindgen::prelude::*;
use crate::engine::VectorEngine;
use crate::types::HandleType;

#[wasm_bindgen]
impl VectorEngine {
    pub fn select_point(&mut self, tx: f64, ty: f64, shift: bool, ignore_locked: bool) -> String {
        let mut hit_id = None;
        for obj in self.objects.iter().rev() {
            if obj.locked && !ignore_locked { continue; }
            
            let cx = obj.x + obj.width / 2.0;
            let cy = obj.y + obj.height / 2.0;
            let px = tx - cx;
            let py = ty - cy;
            let cos_r = (-obj.rotation).cos();
            let sin_r = (-obj.rotation).sin();
            let rx = px * cos_r - py * sin_r;
            let ry = px * sin_r + py * cos_r;
            
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

    pub fn select_rect(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, shift: bool, ignore_locked: bool) -> String {
        let mut sx = x1;
        let mut sy = y1;
        let mut ex = x2;
        let mut ey = y2;
        if sx > ex { std::mem::swap(&mut sx, &mut ex); }
        if sy > ey { std::mem::swap(&mut sy, &mut ey); }

        if !shift {
            self.selected_ids.clear();
        }
        for obj in &self.objects {
            if obj.locked && !ignore_locked { continue; }
            let obj_x2 = obj.x + obj.width;
            let obj_y2 = obj.y + obj.height;
            if obj.x < ex && obj_x2 > sx && obj.y < ey && obj_y2 > sy {
                 if !self.selected_ids.contains(&obj.id) {
                     self.selected_ids.push(obj.id);
                 }
            }
        }
        self.get_selected_ids()
    }

    pub fn hit_test_handles(&self, tx: f64, ty: f64) -> String {
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
                    (0.0, 0.0, HandleType::TopLeft), (obj.width, 0.0, HandleType::TopRight),
                    (0.0, obj.height, HandleType::BottomLeft), (obj.width, obj.height, HandleType::BottomRight),
                    (obj.width / 2.0, 0.0, HandleType::Top), (obj.width / 2.0, obj.height, HandleType::Bottom),
                    (0.0, obj.height / 2.0, HandleType::Left), (obj.width, obj.height / 2.0, HandleType::Right),
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
}