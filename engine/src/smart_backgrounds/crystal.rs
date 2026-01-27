use super::*;
use wasm_bindgen::JsValue;

pub struct CrystalBackground;

impl SmartBackground for CrystalBackground {
    fn get_metadata(&self) -> SmartBackgroundMetadata {
        SmartBackgroundMetadata {
            id: "crystal".to_string(),
            name: "Geometric Crystal".to_string(),
            parameters: vec![
                BackgroundParameter { name: "Cell Size".to_string(), key: "size".to_string(), min: 30.0, max: 200.0, default: 80.0, step: 1.0, kind: ParameterKind::Range },
                BackgroundParameter { name: "Hue Variance".to_string(), key: "hue".to_string(), min: 0.0, max: 100.0, default: 20.0, step: 1.0, kind: ParameterKind::Range },
                BackgroundParameter { name: "Main Color".to_string(), key: "color".to_string(), min: 0.0, max: 16777215.0, default: 0x4488FF as f64, step: 1.0, kind: ParameterKind::Color },
                BackgroundParameter { name: "Randomness".to_string(), key: "jitter".to_string(), min: 0.0, max: 1.0, default: 0.4, step: 0.05, kind: ParameterKind::Range },
                BackgroundParameter { name: "Show Wireframe".to_string(), key: "wire".to_string(), min: 0.0, max: 1.0, default: 0.0, step: 1.0, kind: ParameterKind::Bool },
            ],
            icon: "M 12,2 L 22,8 L 22,16 L 12,22 L 2,16 L 2,8 Z".to_string(),
        }
    }

    fn render(&self, ctx: &CanvasRenderingContext2d, w: f64, h: f64, params: &[f64]) {
        let size = params.get(0).cloned().unwrap_or(80.0);
        let hue_var = params.get(1).cloned().unwrap_or(20.0);
        let color_val = params.get(2).cloned().unwrap_or(0x4488FF as f64) as u32;
        let jitter = params.get(3).cloned().unwrap_or(0.4);
        let show_wire = params.get(4).cloned().unwrap_or(0.0) > 0.5;

        let r_base = ((color_val >> 16) & 0xFF) as f64;
        let g_base = ((color_val >> 8) & 0xFF) as f64;
        let b_base = (color_val & 0xFF) as f64;

        ctx.set_fill_style(&JsValue::from_str("#111"));
        ctx.fill_rect(0.0, 0.0, w, h);

        let mut seed = 55555.0;
        let mut rand = || {
            seed = (seed * 1103515245.0 + 12345.0) % 2147483648.0;
            seed / 2147483648.0
        };

        let cols = (w / size) as i32 + 2;
        let rows = (h / size) as i32 + 2;

        let mut points = Vec::new();
        for y_idx in 0..rows {
            let mut row = Vec::new();
            for x_idx in 0..cols {
                let px = x_idx as f64 * size + (rand() - 0.5) * size * jitter;
                let py = y_idx as f64 * size + (rand() - 0.5) * size * jitter;
                row.push((px, py));
            }
            points.push(row);
        }

        for y in 0..(rows - 1) {
            for x in 0..(cols - 1) {
                let p1 = points[y as usize][x as usize];
                let p2 = points[y as usize][(x + 1) as usize];
                let p3 = points[(y + 1) as usize][x as usize];
                let p4 = points[(y + 1) as usize][(x + 1) as usize];

                // Triangle 1
                self.draw_tri(ctx, p1, p2, p3, r_base, g_base, b_base, hue_var, &mut rand, show_wire);
                // Triangle 2
                self.draw_tri(ctx, p2, p4, p3, r_base, g_base, b_base, hue_var, &mut rand, show_wire);
            }
        }
    }
}

impl CrystalBackground {
    fn draw_tri(&self, ctx: &CanvasRenderingContext2d, p1: (f64, f64), p2: (f64, f64), p3: (f64, f64), r: f64, g: f64, b: f64, var: f64, rand: &mut dyn FnMut() -> f64, wire: bool) {
        let v = (rand() - 0.5) * var;
        let dr = (r + v).clamp(0.0, 255.0);
        let dg = (g + v).clamp(0.0, 255.0);
        let db = (b + v).clamp(0.0, 255.0);

        ctx.set_fill_style(&JsValue::from_str(&format!("rgb({}, {}, {})", dr, dg, db)));
        ctx.begin_path();
        ctx.move_to(p1.0, p1.1);
        ctx.line_to(p2.0, p2.1);
        ctx.line_to(p3.0, p3.1);
        ctx.close_path();
        ctx.fill();

        if wire {
            ctx.set_stroke_style(&JsValue::from_str("rgba(255, 255, 255, 0.1)"));
            ctx.set_line_width(0.5);
            ctx.stroke();
        }
    }
}
