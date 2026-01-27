use super::*;

pub struct GearShape;
impl IntelligentShape for GearShape {
    fn get_metadata(&self) -> IntelligentShapeMetadata {
        IntelligentShapeMetadata {
            id: "gear".to_string(),
            name: "Gear".to_string(),
            parameters: vec![
                ShapeParameter { name: "Teeth".to_string(), key: "teeth".to_string(), min: 4.0, max: 40.0, default: 12.0, step: 1.0 },
                ShapeParameter { name: "Hole Size".to_string(), key: "hole".to_string(), min: 0.0, max: 0.8, default: 0.3, step: 0.01 },
                ShapeParameter { name: "Tooth Depth".to_string(), key: "depth".to_string(), min: 0.05, max: 0.4, default: 0.15, step: 0.01 },
            ],
            icon: "M 12,8 A 4,4 0 1 0 12,16 A 4,4 0 1 0 12,8 M 12,2 L 14,4 H 16 L 18,6 V 8 L 20,10 V 14 L 18,16 V 18 L 16,20 H 14 L 12,22 L 10,20 H 8 L 6,18 V 16 L 4,14 V 10 L 6,8 V 6 L 8,4 H 10 Z".to_string(),
        }
    }

    fn generate_path(&self, w: f64, h: f64, params: &[f64]) -> String {
        let teeth = params.get(0).cloned().unwrap_or(12.0) as usize;
        let hole_ratio = params.get(1).cloned().unwrap_or(0.3);
        let depth_ratio = params.get(2).cloned().unwrap_or(0.15);
        
        let cx = w / 2.0;
        let cy = h / 2.0;
        let r_outer = w.min(h) / 2.0;
        let r_inner = r_outer * (1.0 - depth_ratio);
        let r_hole = r_outer * hole_ratio;
        
        let mut d = String::new();
        for i in 0..teeth {
            let angle_step = 2.0 * PI / teeth as f64;
            let a1 = i as f64 * angle_step;
            let a2 = a1 + angle_step * 0.25;
            let a3 = a1 + angle_step * 0.5;
            let a4 = a1 + angle_step * 0.75;
            
            let x1 = cx + r_inner * a1.cos();
            let y1 = cy + r_inner * a1.sin();
            let x2 = cx + r_outer * a2.cos();
            let y2 = cy + r_outer * a2.sin();
            let x3 = cx + r_outer * a3.cos();
            let y3 = cy + r_outer * a3.sin();
            let x4 = cx + r_inner * a4.cos();
            let y4 = cy + r_inner * a4.sin();
            
            if i == 0 { d.push_str(&format!("M {},{}", x1, y1)); } else { d.push_str(&format!(" L {},{}", x1, y1)); }
            d.push_str(&format!(" L {},{} L {},{} L {},{}", x2, y2, x3, y3, x4, y4));
        }
        d.push_str(" Z");
        if r_hole > 0.0 {
            d.push_str(&format!(" M {},{} A {},{} 0 1 0 {},{} A {},{} 0 1 0 {},{} Z",
                cx + r_hole, cy, r_hole, r_hole, cx - r_hole, cy, r_hole, r_hole, cx + r_hole, cy
            ));
        }
        d
    }
}
