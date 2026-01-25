use wasm_bindgen::prelude::*;
use crate::engine::VectorEngine;
use crate::objects::VectorObject;
use crate::types::{ShapeType, EffectType};
use kurbo::BezPath;
use web_sys::{CanvasRenderingContext2d, Path2d};

#[wasm_bindgen]
impl VectorEngine {
    pub fn render(&self, ctx: &CanvasRenderingContext2d) {
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
        
        ctx.set_fill_style_str(&self.artboard.background);
        ctx.set_shadow_color("rgba(0,0,0,0.5)");
        ctx.set_shadow_blur(20.0);
        ctx.fill_rect(0.0, 0.0, self.artboard.width, self.artboard.height);
        ctx.set_shadow_color("transparent");

        if let Some(first) = self.objects.first() {
            if first.shape_type == ShapeType::Image && first.locked {
                self.render_checkerboard(ctx, self.artboard.width, self.artboard.height);
            }
        }

        for obj in &self.objects {
            self.render_object(ctx, obj);
        }
        
        self.render_guides(ctx);
        ctx.restore();
        
        if !self.hide_selection {
            self.render_selection_overlay(ctx);
        }

        ctx.restore();
    }

    fn render_checkerboard(&self, ctx: &CanvasRenderingContext2d, width: f64, height: f64) {
        let size = 16.0;
        ctx.save();
        ctx.set_fill_style_str("#ffffff");
        ctx.fill_rect(0.0, 0.0, width, height);
        ctx.set_fill_style_str("#e5e5e5");
        let cols = (width / size).ceil() as i32;
        let rows = (height / size).ceil() as i32;
        for r in 0..rows {
            for c in 0..cols {
                if (r + c) % 2 != 0 {
                    ctx.fill_rect(c as f64 * size, r as f64 * size, size, size);
                }
            }
        }
        ctx.restore();
    }

    fn render_selection_overlay(&self, ctx: &CanvasRenderingContext2d) {
        if self.selected_ids.is_empty() { return; }
        if self.selected_ids.len() == 1 {
             let id = self.selected_ids[0];
             if let Some(obj) = self.objects.iter().find(|o| o.id == id) {
                ctx.save();
                ctx.translate(obj.x + obj.width / 2.0, obj.y + obj.height / 2.0).unwrap();
                ctx.rotate(obj.rotation).unwrap();
                ctx.translate(-obj.width / 2.0, -obj.height / 2.0).unwrap();
                ctx.set_stroke_style_str("#4facfe");
                ctx.set_line_width(1.5 / self.viewport_zoom);
                ctx.set_line_dash(&js_sys::Array::new()).unwrap(); 
                ctx.stroke_rect(0.0, 0.0, obj.width, obj.height);
                let handle_size = 8.0 / self.viewport_zoom;
                let rotate_offset = -30.0 / self.viewport_zoom;
                ctx.set_fill_style_str("#ffffff");
                ctx.set_stroke_style_str("#4facfe");
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
                ctx.set_stroke_style_str("#4facfe");
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

    fn render_object(&self, ctx: &CanvasRenderingContext2d, obj: &VectorObject) {
        if !obj.visible || obj.is_mask { return; }
        ctx.save();
        if let Some(mask_id) = obj.mask_id {
            if let Some(mask_obj) = self.objects.iter().find(|o| o.id == mask_id) {
                ctx.save();
                ctx.translate(mask_obj.x + mask_obj.width / 2.0, mask_obj.y + mask_obj.height / 2.0).unwrap();
                ctx.rotate(mask_obj.rotation).unwrap();
                ctx.translate(-mask_obj.width / 2.0, -mask_obj.height / 2.0).unwrap();
                self.define_object_path(ctx, mask_obj);
                ctx.restore();
                ctx.clip();
            }
        }
        ctx.set_global_alpha(obj.opacity);
        ctx.set_global_composite_operation(&obj.blend_mode).unwrap_or(());
        if obj.shape_type == ShapeType::Adjustment {
            let filter = format!(
                "brightness({}%) contrast({}%) saturate({}%) hue-rotate({}deg) blur({}px) grayscale({}%) sepia({}%) invert({}%)",
                obj.brightness * 100.0, obj.contrast * 100.0, obj.saturate * 100.0, obj.hue_rotate, obj.blur, obj.grayscale * 100.0, obj.sepia * 100.0, obj.invert * 100.0
            );
            ctx.set_filter(&filter);
            return;
        }
        for effect in &obj.layer_style.effects {
            if !effect.enabled { continue; }
            if effect.effect_type == EffectType::DropShadow {
                ctx.set_shadow_color(&effect.color);
                ctx.set_shadow_blur(effect.blur);
                ctx.set_shadow_offset_x(effect.x);
                ctx.set_shadow_offset_y(effect.y);
            }
        }
        ctx.translate(obj.x + obj.width / 2.0, obj.y + obj.height / 2.0).unwrap();
        ctx.rotate(obj.rotation).unwrap();
        ctx.translate(-obj.width / 2.0, -obj.height / 2.0).unwrap();
        if obj.shape_type == ShapeType::Group {
            if let Some(children) = &obj.children {
                for child in children { self.render_object(ctx, child); }
            }
        } else {
            if let Some(grad) = &obj.fill_gradient {
                let canvas_grad_opt = if grad.is_radial {
                    ctx.create_radial_gradient(grad.x1, grad.y1, grad.r1, grad.x2, grad.y2, grad.r2).ok()
                } else {
                    Some(ctx.create_linear_gradient(grad.x1, grad.y1, grad.x2, grad.y2))
                };
                if let Some(canvas_grad) = canvas_grad_opt {
                    for stop in &grad.stops { let _ = canvas_grad.add_color_stop(stop.offset as f32, &stop.color); }
                    ctx.set_fill_style(&canvas_grad);
                }
            } else { ctx.set_fill_style_str(&obj.fill); }

            if let Some(grad) = &obj.stroke_gradient {
                let canvas_grad_opt = if grad.is_radial {
                    ctx.create_radial_gradient(grad.x1, grad.y1, grad.r1, grad.x2, grad.y2, grad.r2).ok()
                } else {
                    Some(ctx.create_linear_gradient(grad.x1, grad.y1, grad.x2, grad.y2))
                };
                if let Some(canvas_grad) = canvas_grad_opt {
                    for stop in &grad.stops { let _ = canvas_grad.add_color_stop(stop.offset as f32, &stop.color); }
                    ctx.set_stroke_style(&canvas_grad);
                }
            } else { ctx.set_stroke_style_str(&obj.stroke); }
            
            ctx.set_line_width(obj.stroke_width);
            ctx.set_line_cap(&obj.stroke_cap);
            ctx.set_line_join(&obj.stroke_join);
            ctx.set_shadow_color(&obj.shadow_color);
            ctx.set_shadow_blur(obj.shadow_blur);
            ctx.set_shadow_offset_x(obj.shadow_offset_x);
            ctx.set_shadow_offset_y(obj.shadow_offset_y);

            if !obj.stroke_dash.is_empty() {
                 let dash_array = js_sys::Array::new();
                 for &d in &obj.stroke_dash { dash_array.push(&JsValue::from_f64(d)); }
                 let _ = ctx.set_line_dash(&dash_array);
            } else { let _ = ctx.set_line_dash(&js_sys::Array::new()); }

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
                    if let Some(img_val) = &obj.image {
                        if let Some(img) = img_val.dyn_ref::<web_sys::HtmlImageElement>() {
                            let _ = ctx.draw_image_with_html_image_element_and_sw_and_sh_and_dx_and_dy_and_dw_and_dh(
                                img, obj.sx, obj.sy, obj.sw, obj.sh, 0.0, 0.0, obj.width, obj.height
                            );
                        } else if let Some(canvas) = img_val.dyn_ref::<web_sys::HtmlCanvasElement>() {
                            let _ = ctx.draw_image_with_html_canvas_element_and_sw_and_sh_and_dx_and_dy_and_dw_and_dh(
                                canvas, obj.sx, obj.sy, obj.sw, obj.sh, 0.0, 0.0, obj.width, obj.height
                            );
                        }
                    }
                }
                ShapeType::Path => {
                    if !obj.path_data.is_empty() {
                         if let Ok(path) = BezPath::from_svg(&obj.path_data) {
                             if obj.brush_id > 0 {
                                 if let Some(brush) = self.brush_engine.brushes.iter().find(|b| b.id == obj.brush_id) {
                                     self.brush_engine.render_stroke(ctx, brush, &path, &obj.fill, &self.brush_image_map);
                                 }
                             } else {
                                 if let Ok(p) = Path2d::new_with_path_string(&obj.path_data) {
                                     ctx.fill_with_path_2d(&p);
                                     if obj.stroke_width > 0.0 { ctx.stroke_with_path(&p); }
                                 }
                             }
                         }
                    }
                }
                ShapeType::Text => {
                    ctx.set_font(&format!("{} {}px {}", obj.font_weight, obj.font_size, obj.font_family));
                    ctx.set_text_align(&obj.text_align);
                    let _ = ctx.fill_text(&obj.text_content, 0.0, obj.font_size);
                    if obj.stroke_width > 0.0 { let _ = ctx.stroke_text(&obj.text_content, 0.0, obj.font_size); }
                }
                _ => {}
            }
        }
        ctx.restore();
    }

    fn define_object_path(&self, ctx: &CanvasRenderingContext2d, obj: &VectorObject) {
        match obj.shape_type {
            ShapeType::Rectangle => {
                if obj.corner_radius > 0.0 {
                    let r = obj.corner_radius.min(obj.width / 2.0).min(obj.height / 2.0);
                    ctx.begin_path(); ctx.move_to(r, 0.0); ctx.line_to(obj.width - r, 0.0);
                    ctx.arc_to(obj.width, 0.0, obj.width, r, r).unwrap(); ctx.line_to(obj.width, obj.height - r);
                    ctx.arc_to(obj.width, obj.height, obj.width - r, obj.height, r).unwrap(); ctx.line_to(r, obj.height);
                    ctx.arc_to(0.0, obj.height, 0.0, obj.height - r, r).unwrap(); ctx.line_to(0.0, r);
                    ctx.arc_to(0.0, 0.0, r, 0.0, r).unwrap(); ctx.close_path();
                } else { ctx.begin_path(); ctx.rect(0.0, 0.0, obj.width, obj.height); }
            }
            ShapeType::Circle | ShapeType::Ellipse => {
                ctx.begin_path(); let _ = ctx.ellipse(obj.width / 2.0, obj.height / 2.0, obj.width / 2.0, obj.height / 2.0, 0.0, 0.0, std::f64::consts::PI * 2.0);
            }
            ShapeType::Polygon => { self.draw_poly(ctx, obj.width / 2.0, obj.height / 2.0, obj.width / 2.0, obj.sides, 0.0); }
            ShapeType::Star => { self.draw_star(ctx, obj.width / 2.0, obj.height / 2.0, obj.width / 2.0, obj.inner_radius * (obj.width / 2.0), obj.sides); }
            _ => { ctx.begin_path(); ctx.rect(0.0, 0.0, obj.width, obj.height); }
        }
    }

    fn render_guides(&self, ctx: &CanvasRenderingContext2d) {
        ctx.save();
        ctx.set_stroke_style_str("cyan");
        ctx.set_line_width(1.0 / self.viewport_zoom);
        for guide in &self.artboard.guides {
            ctx.begin_path();
            if guide.orientation == "horizontal" { ctx.move_to(-10000.0, guide.position); ctx.line_to(10000.0, guide.position); }
            else { ctx.move_to(guide.position, -10000.0); ctx.line_to(guide.position, 10000.0); }
            ctx.stroke();
        }
        ctx.restore();
    }

    fn draw_poly(&self, ctx: &CanvasRenderingContext2d, cx: f64, cy: f64, r: f64, sides: u32, rot: f64) {
        ctx.begin_path();
        for i in 0..sides {
            let angle = rot + (i as f64 * 2.0 * std::f64::consts::PI / sides as f64);
            let x = cx + r * angle.cos(); let y = cy + r * angle.sin();
            if i == 0 { ctx.move_to(x, y); } else { ctx.line_to(x, y); }
        }
        ctx.close_path();
    }

    fn draw_star(&self, ctx: &CanvasRenderingContext2d, cx: f64, cy: f64, r_outer: f64, r_inner: f64, points: u32) {
        ctx.begin_path();
        for i in 0..(points * 2) {
            let r = if i % 2 == 0 { r_outer } else { r_inner };
            let angle = (i as f64 * std::f64::consts::PI / points as f64) - (std::f64::consts::PI / 2.0);
            let x = cx + r * angle.cos(); let y = cy + r * angle.sin();
            if i == 0 { ctx.move_to(x, y); } else { ctx.line_to(x, y); }
        }
        ctx.close_path();
    }
}
