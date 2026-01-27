use super::*;
use std::f64::consts::PI;

pub struct PolygonShape;
impl IntelligentShape for PolygonShape {
    fn get_metadata(&self) -> IntelligentShapeMetadata {
        IntelligentShapeMetadata {
            id: "polygon".to_string(),
            name: "Polygon".to_string(),
            parameters: vec![
                ShapeParameter { name: "Sides".to_string(), key: "sides".to_string(), min: 3.0, max: 100.0, default: 6.0, step: 1.0 },
                ShapeParameter { name: "Roundness".to_string(), key: "roundness".to_string(), min: 0.0, max: 1.0, default: 0.0, step: 0.01 },
            ],
            icon: "M 12,2 L 21,7 V 17 L 12,22 L 3,17 V 7 Z".to_string(),
        }
    }

    fn generate_path(&self, w: f64, h: f64, params: &[f64]) -> String {
        let sides = params.get(0).cloned().unwrap_or(6.0) as u32;
        // let roundness = params.get(1).cloned().unwrap_or(0.0);

        let cx = w / 2.0;
        let cy = h / 2.0;
        let r = w.min(h) / 2.0;

        let mut path = String::new();
        
        for i in 0..sides {
            let angle = (i as f64 * 2.0 * PI / sides as f64) - (PI / 2.0);
            let x = cx + r * angle.cos();
            let y = cy + r * angle.sin();

            if i == 0 {
                path.push_str(&format!("M {},{}", x, y));
            } else {
                path.push_str(&format!(" L {},{}", x, y));
            }
        }
        path.push_str(" Z");
        path
    }
}
