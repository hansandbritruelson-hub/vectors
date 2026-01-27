use super::*;

use wasm_bindgen::JsValue;

pub struct GrassBackground;

impl SmartBackground for GrassBackground {
    fn get_metadata(&self) -> SmartBackgroundMetadata {
        SmartBackgroundMetadata {
            id: "grass".to_string(),
            name: "Grassy Hill".to_string(),
            parameters: vec![
                BackgroundParameter { name: "Hill Height".to_string(), key: "height".to_string(), min: 0.0, max: 1.0, default: 0.3, step: 0.01, kind: ParameterKind::Range },
                BackgroundParameter { name: "Grass Density".to_string(), key: "density".to_string(), min: 10.0, max: 500.0, default: 100.0, step: 1.0, kind: ParameterKind::Range },
                BackgroundParameter { name: "Sky Color".to_string(), key: "sky".to_string(), min: 0.0, max: 1.0, default: 0.5, step: 0.01, kind: ParameterKind::Range },
            ],
            icon: "M 3,21 L 5,12 M 12,21 L 12,8 M 21,21 L 19,12".to_string(),
        }
    }

    fn render(&self, ctx: &CanvasRenderingContext2d, w: f64, h: f64, params: &[f64]) {
        let hill_h_ratio = params.get(0).cloned().unwrap_or(0.3);
        let density = params.get(1).cloned().unwrap_or(100.0) as i32;

        // Sky
        let sky_grad = ctx.create_linear_gradient(0.0, 0.0, 0.0, h);
        let _ = sky_grad.add_color_stop(0.0, "#87CEEB");
        let _ = sky_grad.add_color_stop(1.0, "#E0F6FF");
        ctx.set_fill_style(&sky_grad.into());
        ctx.fill_rect(0.0, 0.0, w, h);

        // Hill
        let hill_y = h * (1.0 - hill_h_ratio);
        ctx.set_fill_style(&JsValue::from_str("#2d5a27"));
        ctx.begin_path();
        ctx.move_to(0.0, h);
        ctx.line_to(0.0, hill_y);
        ctx.bezier_curve_to(w * 0.3, hill_y - 50.0, w * 0.7, hill_y + 50.0, w, hill_y);
        ctx.line_to(w, h);
        ctx.close_path();
        ctx.fill();

        // Grass blades
        let mut seed = 54321.0;
        let mut rand = || {
            seed = (seed * 1103515245.0 + 12345.0) % 2147483648.0;
            seed / 2147483648.0
        };

        ctx.set_stroke_style(&JsValue::from_str("#3e7b36"));
        ctx.set_line_width(2.0);
        for _ in 0..density {
            let x = rand() * w;
            // Place grass on the hill
            // Approximate y based on the bezier curve used for hill
            // curve: P0=(0, hy), CP1=(w*0.3, hy-50), CP2=(w*0.7, hy+50), P3=(w, hy)
            let t = x / w;
            let hy = (1.0-t).powi(3)*hill_y + 
                     3.0*(1.0-t).powi(2)*t*(hill_y - 50.0) + 
                     3.0*(1.0-t)*t.powi(2)*(hill_y + 50.0) + 
                     t.powi(3)*hill_y;
            
            let y = hy + rand() * (h - hy);
            let bh = 5.0 + rand() * 15.0;
            let angle = (rand() - 0.5) * 0.5;

            ctx.begin_path();
            ctx.move_to(x, y);
            ctx.line_to(x + angle * bh, y - bh);
            ctx.stroke();
        }
    }
}
