use super::*;
use wasm_bindgen::JsValue;

pub struct MeshBackground;

impl SmartBackground for MeshBackground {
    fn get_metadata(&self) -> SmartBackgroundMetadata {
        SmartBackgroundMetadata {
            id: "mesh".to_string(),
            name: "Soft Mesh".to_string(),
            parameters: vec![
                BackgroundParameter { name: "Point Count".to_string(), key: "count".to_string(), min: 3.0, max: 15.0, default: 6.0, step: 1.0, kind: ParameterKind::Range },
                BackgroundParameter { name: "Color 1".to_string(), key: "color1".to_string(), min: 0.0, max: 16777215.0, default: 0xFF88CC as f64, step: 1.0, kind: ParameterKind::Color },
                BackgroundParameter { name: "Color 2".to_string(), key: "color2".to_string(), min: 0.0, max: 16777215.0, default: 0x88CCFF as f64, step: 1.0, kind: ParameterKind::Color },
                BackgroundParameter { name: "Color 3".to_string(), key: "color3".to_string(), min: 0.0, max: 16777215.0, default: 0xCCFF88 as f64, step: 1.0, kind: ParameterKind::Color },
                BackgroundParameter { name: "Blur Size".to_string(), key: "blur".to_string(), min: 0.5, max: 2.0, default: 1.0, step: 0.1, kind: ParameterKind::Range },
            ],
            icon: "M 12,21 C 12,21 2,16 2,12 C 2,8 12,3 12,3 C 12,3 22,8 22,12 C 22,16 12,21 12,21".to_string(),
        }
    }

    fn render(&self, ctx: &CanvasRenderingContext2d, w: f64, h: f64, params: &[f64]) {
        let count = params.get(0).cloned().unwrap_or(6.0) as i32;
        let c1 = params.get(1).cloned().unwrap_or(0xFF88CC as f64) as u32;
        let c2 = params.get(2).cloned().unwrap_or(0x88CCFF as f64) as u32;
        let c3 = params.get(3).cloned().unwrap_or(0xCCFF88 as f64) as u32;
        let blur = params.get(4).cloned().unwrap_or(1.0);

        let colors = [
            ((c1 >> 16) & 0xFF, (c1 >> 8) & 0xFF, c1 & 0xFF),
            ((c2 >> 16) & 0xFF, (c2 >> 8) & 0xFF, c2 & 0xFF),
            ((c3 >> 16) & 0xFF, (c3 >> 8) & 0xFF, c3 & 0xFF),
        ];

        ctx.set_fill_style(&JsValue::from_str("#fff"));
        ctx.fill_rect(0.0, 0.0, w, h);

        let mut seed = 77777.0;
        let mut rand = || {
            seed = (seed * 1103515245.0 + 12345.0) % 2147483648.0;
            seed / 2147483648.0
        };

        for i in 0..count {
            let x = rand() * w;
            let y = rand() * h;
            let radius = (w.max(h)) * 0.8 * blur;
            let color = colors[i as usize % 3];

            let grad = ctx.create_radial_gradient(x, y, 0.0, x, y, radius).unwrap();
            let _ = grad.add_color_stop(0.0, &format!("rgba({}, {}, {}, 0.8)", color.0, color.1, color.2));
            let _ = grad.add_color_stop(1.0, "rgba(255, 255, 255, 0)");

            ctx.set_fill_style(&grad.into());
            ctx.set_global_composite_operation("multiply").unwrap_or(());
            ctx.fill_rect(0.0, 0.0, w, h);
        }
        ctx.set_global_composite_operation("source-over").unwrap_or(());
    }
}
