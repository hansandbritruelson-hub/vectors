use super::*;

pub struct ArrowShape;
impl IntelligentShape for ArrowShape {
    fn get_metadata(&self) -> IntelligentShapeMetadata {
        IntelligentShapeMetadata {
            id: "arrow".to_string(),
            name: "Arrow".to_string(),
            parameters: vec![
                ShapeParameter { name: "Head Width".to_string(), key: "head_w".to_string(), min: 0.1, max: 1.0, default: 0.6, step: 0.01 },
                ShapeParameter { name: "Head Length".to_string(), key: "head_l".to_string(), min: 0.1, max: 0.9, default: 0.4, step: 0.01 },
                ShapeParameter { name: "Shaft Thickness".to_string(), key: "shaft".to_string(), min: 0.05, max: 0.8, default: 0.3, step: 0.01 },
            ],
            icon: "M 2,12 H 14 V 6 L 22,12 L 14,18 V 12 Z".to_string(),
        }
    }

    fn generate_path(&self, w: f64, h: f64, params: &[f64]) -> String {
        let head_w_ratio = params.get(0).cloned().unwrap_or(0.6);
        let head_l = params.get(1).cloned().unwrap_or(0.4) * w;
        let shaft_h_ratio = params.get(2).cloned().unwrap_or(0.3);

        let shaft_y1 = h * (0.5 - shaft_h_ratio / 2.0);
        let shaft_y2 = h * (0.5 + shaft_h_ratio / 2.0);
        let shaft_end_x = w - head_l;
        let head_y1 = h * (0.5 - head_w_ratio / 2.0);
        let head_y2 = h * (0.5 + head_w_ratio / 2.0);

        format!(
            "M 0,{sy1} H {sx} V {hy1} L {w},{cy} L {sx},{hy2} V {sy2} H 0 Z",
            sy1=shaft_y1, sy2=shaft_y2, sx=shaft_end_x, hy1=head_y1, hy2=head_y2, w=w, cy=h/2.0
        )
    }
}
