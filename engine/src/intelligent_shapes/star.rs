use super::*;
use std::f64::consts::PI;

pub struct StarShape;
impl IntelligentShape for StarShape {
    fn get_metadata(&self) -> IntelligentShapeMetadata {
        IntelligentShapeMetadata {
            id: "star".to_string(),
            name: "Star".to_string(),
            parameters: vec![
                ShapeParameter { name: "Points".to_string(), key: "points".to_string(), min: 3.0, max: 50.0, default: 5.0, step: 1.0 },
                ShapeParameter { name: "Inner Radius".to_string(), key: "inner".to_string(), min: 0.0, max: 1.0, default: 0.5, step: 0.01 },
                ShapeParameter { name: "Roundness".to_string(), key: "roundness".to_string(), min: 0.0, max: 1.0, default: 0.0, step: 0.01 },
            ],
            icon: "M 12,2 L 15,9 H 22 L 16,14 L 18,21 L 12,17 L 6,21 L 8,14 L 2,9 H 9 Z".to_string(),
        }
    }

    fn generate_path(&self, w: f64, h: f64, params: &[f64]) -> String {
        let points = params.get(0).cloned().unwrap_or(5.0) as u32;
        let inner_ratio = params.get(1).cloned().unwrap_or(0.5);
        let roundness = params.get(2).cloned().unwrap_or(0.0);

        let cx = w / 2.0;
        let cy = h / 2.0;
        let r_outer = w.min(h) / 2.0;
        let r_inner = r_outer * inner_ratio;

        let mut path = String::new();
        
        for i in 0..(points * 2) {
            let angle = (i as f64 * PI / points as f64) - (PI / 2.0);
            let r = if i % 2 == 0 { r_outer } else { r_inner };
            let x = cx + r * angle.cos();
            let y = cy + r * angle.sin();

            if i == 0 {
                path.push_str(&format!("M {},{}", x, y));
            } else {
                // For now, simple lines. Roundness would require Bezier curves.
                // Keeping it simple for the first iteration.
                path.push_str(&format!(" L {},{}", x, y));
            }
        }
        path.push_str(" Z");
        path
    }
}
