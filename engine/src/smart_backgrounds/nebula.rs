use super::*;
use wasm_bindgen::JsValue;
use std::f64::consts::PI;

pub struct NebulaBackground;

impl SmartBackground for NebulaBackground {
    fn get_metadata(&self) -> SmartBackgroundMetadata {
        SmartBackgroundMetadata {
            id: "nebula".to_string(),
            name: "Cosmic Nebula".to_string(),
            parameters: vec![
                BackgroundParameter { name: "Cloud Count".to_string(), key: "count".to_string(), min: 5.0, max: 50.0, default: 20.0, step: 1.0, kind: ParameterKind::Range },
                BackgroundParameter { name: "Nebula Color 1".to_string(), key: "color1".to_string(), min: 0.0, max: 16777215.0, default: 0xFF00FF as f64, step: 1.0, kind: ParameterKind::Color },
                BackgroundParameter { name: "Nebula Color 2".to_string(), key: "color2".to_string(), min: 0.0, max: 16777215.0, default: 0x00FFFF as f64, step: 1.0, kind: ParameterKind::Color },
                BackgroundParameter { name: "Glow Intensity".to_string(), key: "glow".to_string(), min: 0.0, max: 1.0, default: 0.4, step: 0.01, kind: ParameterKind::Range },
                BackgroundParameter { name: "Star Dust".to_string(), key: "stars".to_string(), min: 0.0, max: 1000.0, default: 300.0, step: 10.0, kind: ParameterKind::Range },
            ],
            icon: "M 12,2 C 15,5 20,8 22,12 C 20,16 15,19 12,22 C 9,19 4,16 2,12 C 4,8 9,5 12,2".to_string(),
        }
    }

    fn render(&self, ctx: &CanvasRenderingContext2d, w: f64, h: f64, params: &[f64]) {
        let count = params.get(0).cloned().unwrap_or(20.0) as i32;
        let c1 = params.get(1).cloned().unwrap_or(0xFF00FF as f64) as u32;
        let c2 = params.get(2).cloned().unwrap_or(0x00FFFF as f64) as u32;
        let glow = params.get(3).cloned().unwrap_or(0.4);
        let stars = params.get(4).cloned().unwrap_or(300.0) as i32;

        let r1 = ((c1 >> 16) & 0xFF) as f64;
        let g1 = ((c1 >> 8) & 0xFF) as f64;
        let b1 = (c1 & 0xFF) as f64;

        let r2 = ((c2 >> 16) & 0xFF) as f64;
        let g2 = ((c2 >> 8) & 0xFF) as f64;
        let b2 = (c2 & 0xFF) as f64;

        // Background
        ctx.set_fill_style(&JsValue::from_str("#020005"));
        ctx.fill_rect(0.0, 0.0, w, h);

        let mut seed = 98765.0;
        let mut rand = || {
            seed = (seed * 1103515245.0 + 12345.0) % 2147483648.0;
            seed / 2147483648.0
        };

        // Render nebula clouds
        for _ in 0..count {
            let x = rand() * w;
            let y = rand() * h;
            let radius = 100.0 + rand() * 300.0;
            let mix = rand();
            
            let r = r1 * mix + r2 * (1.0 - mix);
            let g = g1 * mix + g2 * (1.0 - mix);
            let b = b1 * mix + b2 * (1.0 - mix);

            let grad = ctx.create_radial_gradient(x, y, 0.0, x, y, radius).unwrap();
            let _ = grad.add_color_stop(0.0, &format!("rgba({}, {}, {}, {})", r, g, b, glow * 0.3));
            let _ = grad.add_color_stop(0.5, &format!("rgba({}, {}, {}, {})", r, g, b, glow * 0.1));
            let _ = grad.add_color_stop(1.0, "rgba(0, 0, 0, 0)");

            ctx.set_fill_style(&grad.into());
            ctx.fill_rect(x - radius, y - radius, radius * 2.0, radius * 2.0);
        }

        // Star dust
        for _ in 0..stars {
            let x = rand() * w;
            let y = rand() * h;
            let sz = rand() * 1.2;
            let op = rand();
            ctx.set_fill_style(&JsValue::from_str(&format!("rgba(255, 255, 255, {})", op)));
            ctx.begin_path();
            let _ = ctx.arc(x, y, sz, 0.0, PI * 2.0);
            ctx.fill();
        }
    }
}
