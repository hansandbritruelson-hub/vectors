use super::*;
use wasm_bindgen::JsValue;

pub struct CircuitBackground;

impl SmartBackground for CircuitBackground {
    fn get_metadata(&self) -> SmartBackgroundMetadata {
        SmartBackgroundMetadata {
            id: "circuit".to_string(),
            name: "Tech Circuit".to_string(),
            parameters: vec![
                BackgroundParameter { name: "Grid Size".to_string(), key: "grid".to_string(), min: 20.0, max: 100.0, default: 40.0, step: 1.0, kind: ParameterKind::Range },
                BackgroundParameter { name: "Density".to_string(), key: "density".to_string(), min: 0.1, max: 1.0, default: 0.5, step: 0.05, kind: ParameterKind::Range },
                BackgroundParameter { name: "Trace Color".to_string(), key: "color".to_string(), min: 0.0, max: 16777215.0, default: 0x00FF88 as f64, step: 1.0, kind: ParameterKind::Color },
                BackgroundParameter { name: "Glow Nodes".to_string(), key: "nodes".to_string(), min: 0.0, max: 1.0, default: 1.0, step: 1.0, kind: ParameterKind::Bool },
                BackgroundParameter { name: "Line Width".to_string(), key: "width".to_string(), min: 0.5, max: 5.0, default: 1.5, step: 0.1, kind: ParameterKind::Range },
            ],
            icon: "M 2,2 L 10,2 L 10,10 L 22,10 L 22,22".to_string(),
        }
    }

    fn render(&self, ctx: &CanvasRenderingContext2d, w: f64, h: f64, params: &[f64]) {
        let grid = params.get(0).cloned().unwrap_or(40.0);
        let density = params.get(1).cloned().unwrap_or(0.5);
        let color_val = params.get(2).cloned().unwrap_or(0x00FF88 as f64) as u32;
        let show_nodes = params.get(3).cloned().unwrap_or(1.0) > 0.5;
        let line_width = params.get(4).cloned().unwrap_or(1.5);

        let r = ((color_val >> 16) & 0xFF) as f64;
        let g = ((color_val >> 8) & 0xFF) as f64;
        let b = (color_val & 0xFF) as f64;

        ctx.set_fill_style(&JsValue::from_str("#0a0a0f"));
        ctx.fill_rect(0.0, 0.0, w, h);

        let mut seed = 44444.0;
        let mut rand = || {
            seed = (seed * 1103515245.0 + 12345.0) % 2147483648.0;
            seed / 2147483648.0
        };

        ctx.set_stroke_style(&JsValue::from_str(&format!("rgba({}, {}, {}, 0.4)", r, g, b)));
        ctx.set_line_width(line_width);

        let cols = (w / grid) as i32 + 1;
        let rows = (h / grid) as i32 + 1;

        for x_idx in 0..cols {
            for y_idx in 0..rows {
                if rand() > density { continue; }

                let x = x_idx as f64 * grid;
                let y = y_idx as f64 * grid;

                ctx.begin_path();
                ctx.move_to(x, y);

                // Choose direction: right, down, or 45 deg
                let dir = rand();
                let (nx, ny) = if dir < 0.33 {
                    (x + grid, y)
                } else if dir < 0.66 {
                    (x, y + grid)
                } else {
                    (x + grid, y + grid)
                };

                ctx.line_to(nx, ny);
                ctx.stroke();

                if show_nodes && rand() > 0.7 {
                    ctx.set_fill_style(&JsValue::from_str(&format!("rgb({}, {}, {})", r, g, b)));
                    ctx.begin_path();
                    let _ = ctx.arc(x, y, line_width * 1.5, 0.0, std::f64::consts::PI * 2.0);
                    ctx.fill();
                }
            }
        }
    }
}
