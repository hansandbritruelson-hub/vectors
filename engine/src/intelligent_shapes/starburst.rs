use super::*;

pub struct StarBurstShape;
impl IntelligentShape for StarBurstShape {
    fn get_metadata(&self) -> IntelligentShapeMetadata {
        IntelligentShapeMetadata {
            id: "starburst".to_string(),
            name: "Star Burst".to_string(),
            parameters: vec![
                ShapeParameter { name: "Points".to_string(), key: "points".to_string(), min: 3.0, max: 100.0, default: 24.0, step: 1.0 },
                ShapeParameter { name: "Inner Radius".to_string(), key: "inner".to_string(), min: 0.0, max: 1.0, default: 0.7, step: 0.01 },
                ShapeParameter { name: "Irregularity".to_string(), key: "irreg".to_string(), min: 0.0, max: 1.0, default: 0.0, step: 0.01 },
            ],
            icon: "M 12,2 L 14,7 L 19,5 L 17,10 L 22,12 L 17,14 L 19,19 L 14,17 L 12,22 L 10,17 L 5,19 L 7,14 L 2,12 L 7,10 L 5,5 L 10,7 Z".to_string(),
        }
    }

    fn generate_path(&self, w: f64, h: f64, params: &[f64]) -> String {
        let points = params.get(0).cloned().unwrap_or(24.0) as usize;
        let inner_ratio = params.get(1).cloned().unwrap_or(0.7);
        let irregularity = params.get(2).cloned().unwrap_or(0.0);
        
        let cx = w / 2.0;
        let cy = h / 2.0;
        let r_outer = w.min(h) / 2.0;
        let r_inner = r_outer * inner_ratio;
        
        let mut d = String::new();
        for i in 0..points {
            let angle_step = 2.0 * PI / points as f64;
            let a1 = i as f64 * angle_step;
            let a2 = a1 + angle_step * 0.5;
            let offset1 = if irregularity > 0.0 { (i as f64 * 1.23).sin() * irregularity * 0.2 } else { 0.0 };
            let offset2 = if irregularity > 0.0 { (i as f64 * 4.56).cos() * irregularity * 0.2 } else { 0.0 };
            let r1 = r_outer * (1.0 + offset1);
            let r2 = r_inner * (1.0 + offset2);
            let x1 = cx + r1 * a1.cos();
            let y1 = cy + r1 * a1.sin();
            let x2 = cx + r2 * a2.cos();
            let y2 = cy + r2 * a2.sin();
            if i == 0 { d.push_str(&format!("M {},{}", x1, y1)); } else { d.push_str(&format!(" L {},{}", x1, y1)); }
            d.push_str(&format!(" L {},{}", x2, y2));
        }
        d.push_str(" Z");
        d
    }
}
