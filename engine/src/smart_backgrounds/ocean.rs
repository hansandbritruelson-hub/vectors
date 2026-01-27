use super::*;
use wasm_bindgen::JsValue;

pub struct OceanBackground;

impl SmartBackground for OceanBackground {
    fn get_metadata(&self) -> SmartBackgroundMetadata {
        SmartBackgroundMetadata {
            id: "ocean".to_string(),
            name: "Deep Ocean".to_string(),
            parameters: vec![
                BackgroundParameter { name: "Wave Layers".to_string(), key: "layers".to_string(), min: 1.0, max: 10.0, default: 5.0, step: 1.0, kind: ParameterKind::Range },
                BackgroundParameter { name: "Wave Amplitude".to_string(), key: "amplitude".to_string(), min: 10.0, max: 200.0, default: 50.0, step: 1.0, kind: ParameterKind::Range },
                BackgroundParameter { name: "Wave Frequency".to_string(), key: "frequency".to_string(), min: 0.001, max: 0.05, default: 0.01, step: 0.001, kind: ParameterKind::Range },
                BackgroundParameter { name: "Ocean Color".to_string(), key: "color".to_string(), min: 0.0, max: 16777215.0, default: 0x004466 as f64, step: 1.0, kind: ParameterKind::Color },
                BackgroundParameter { name: "Show Foam".to_string(), key: "foam".to_string(), min: 0.0, max: 1.0, default: 1.0, step: 1.0, kind: ParameterKind::Bool },
            ],
            icon: "M 2,12 C 4,10 6,10 8,12 C 10,14 12,14 14,12 C 16,10 18,10 20,12 C 22,14 24,14 26,12".to_string(),
        }
    }

    fn render(&self, ctx: &CanvasRenderingContext2d, w: f64, h: f64, params: &[f64]) {
        let layers = params.get(0).cloned().unwrap_or(5.0) as i32;
        let amp = params.get(1).cloned().unwrap_or(50.0);
        let freq = params.get(2).cloned().unwrap_or(0.01);
        let base_color = params.get(3).cloned().unwrap_or(0x004466 as f64) as u32;
        let show_foam = params.get(4).cloned().unwrap_or(1.0) > 0.5;

        let r = ((base_color >> 16) & 0xFF) as f64;
        let g = ((base_color >> 8) & 0xFF) as f64;
        let b = (base_color & 0xFF) as f64;

        // Background sky/upper ocean
        ctx.set_fill_style(&JsValue::from_str(&format!("rgb({}, {}, {})", r * 0.5, g * 0.5, b * 0.5)));
        ctx.fill_rect(0.0, 0.0, w, h);

        for i in 0..layers {
            let ratio = i as f64 / layers as f64;
            let layer_y = h * (0.3 + ratio * 0.7);
            
            let layer_r = r * (0.6 + ratio * 0.4);
            let layer_g = g * (0.6 + ratio * 0.4);
            let layer_b = b * (0.6 + ratio * 0.4);

            ctx.set_fill_style(&JsValue::from_str(&format!("rgb({}, {}, {})", layer_r, layer_g, layer_b)));
            
            ctx.begin_path();
            ctx.move_to(0.0, h);
            ctx.line_to(0.0, layer_y);

            let steps = 50;
            for s in 0..=steps {
                let x = (s as f64 / steps as f64) * w;
                let offset = (i as f64 * 1.5) + (x * freq);
                let y = layer_y + (offset.sin() * amp * (1.0 - ratio * 0.5));
                ctx.line_to(x, y);
            }

            ctx.line_to(w, h);
            ctx.close_path();
            ctx.fill();

            if show_foam && i > 0 {
                ctx.set_stroke_style(&JsValue::from_str(&format!("rgba(255, 255, 255, {})", 0.1 + ratio * 0.2)));
                ctx.set_line_width(2.0);
                ctx.begin_path();
                for s in 0..=steps {
                    let x = (s as f64 / steps as f64) * w;
                    let offset = (i as f64 * 1.5) + (x * freq);
                    let y = layer_y + (offset.sin() * amp * (1.0 - ratio * 0.5)) - 2.0;
                    if s == 0 { ctx.move_to(x, y); } else { ctx.line_to(x, y); }
                }
                ctx.stroke();
            }
        }
    }
}
