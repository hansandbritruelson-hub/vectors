use super::*;
use std::f64::consts::PI;

use wasm_bindgen::JsValue;

pub struct StarsBackground;

impl SmartBackground for StarsBackground {
    fn get_metadata(&self) -> SmartBackgroundMetadata {
        SmartBackgroundMetadata {
            id: "stars".to_string(),
            name: "Starry Night".to_string(),
            parameters: vec![
                BackgroundParameter { name: "Star Count".to_string(), key: "count".to_string(), min: 10.0, max: 1000.0, default: 200.0, step: 1.0, kind: ParameterKind::Range },
                BackgroundParameter { name: "Brightness".to_string(), key: "brightness".to_string(), min: 0.0, max: 1.0, default: 0.8, step: 0.01, kind: ParameterKind::Range },
                BackgroundParameter { name: "Twinkle".to_string(), key: "twinkle".to_string(), min: 0.0, max: 1.0, default: 0.5, step: 0.01, kind: ParameterKind::Range },
            ],
            icon: "M 12,2 L 14.5,9 L 22,9 L 16,13.5 L 18.5,21 L 12,16.5 L 5.5,21 L 8,13.5 L 2,9 L 9.5,9 Z".to_string(),
        }
    }

    fn render(&self, ctx: &CanvasRenderingContext2d, w: f64, h: f64, params: &[f64]) {
        let count = params.get(0).cloned().unwrap_or(200.0) as i32;
        let brightness = params.get(1).cloned().unwrap_or(0.8);
        
        // Background
        ctx.set_fill_style(&JsValue::from_str("#050510"));
        ctx.fill_rect(0.0, 0.0, w, h);

        // Simple pseudo-random stars based on fixed seed
        let mut seed = 12345.0;
        let mut rand = || {
            seed = (seed * 1103515245.0 + 12345.0) % 2147483648.0;
            seed / 2147483648.0
        };

        for _ in 0..count {
            let x = rand() * w;
            let y = rand() * h;
            let size = rand() * 1.5;
            let op = rand() * brightness;

            ctx.set_fill_style(&JsValue::from_str(&format!("rgba(255, 255, 255, {})", op)));
            ctx.begin_path();
            let _ = ctx.arc(x, y, size, 0.0, PI * 2.0);
            ctx.fill();
        }
    }
}
