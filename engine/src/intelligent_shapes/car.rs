use super::*;

pub struct CarShape;
impl IntelligentShape for CarShape {
    fn get_metadata(&self) -> IntelligentShapeMetadata {
        IntelligentShapeMetadata {
            id: "car".to_string(),
            name: "Car".to_string(),
            parameters: vec![
                ShapeParameter { name: "Body Type (Sedan-SUV)".to_string(), key: "type".to_string(), min: 0.0, max: 1.0, default: 0.2, step: 0.01 },
                ShapeParameter { name: "Roof Length".to_string(), key: "roof".to_string(), min: 0.3, max: 0.9, default: 0.6, step: 0.01 },
                ShapeParameter { name: "Wheel Size".to_string(), key: "wheel_s".to_string(), min: 0.1, max: 0.3, default: 0.18, step: 0.01 },
                ShapeParameter { name: "Wheel Base".to_string(), key: "wheel_b".to_string(), min: 0.5, max: 0.9, default: 0.7, step: 0.01 },
            ],
            icon: "M 3,11 L 5,6 H 13 L 15,11 H 21 V 17 H 3 V 11 M 6,17 A 2,2 0 1 0 6,21 A 2,2 0 1 0 6,17 M 18,17 A 2,2 0 1 0 18,21 A 2,2 0 1 0 18,17".to_string(),
        }
    }

    fn generate_path(&self, w: f64, h: f64, params: &[f64]) -> String {
        let body_type = params.get(0).cloned().unwrap_or(0.2); // 0.0 = sedan, 1.0 = SUV
        let roof_len = params.get(1).cloned().unwrap_or(0.6) * w;
        let wheel_r = params.get(2).cloned().unwrap_or(0.18) * h;
        let wheel_base = params.get(3).cloned().unwrap_or(0.7) * w;

        let ground_y = h - wheel_r;
        let body_h = h * 0.4;
        let body_y = ground_y - body_h;
        
        let cabin_h = h * (0.3 + body_type * 0.2);
        let cabin_y = body_y - cabin_h;
        
        let wheel_x1 = (w - wheel_base) / 2.0 + wheel_r;
        let wheel_x2 = w - (w - wheel_base) / 2.0 - wheel_r;
        
        let mut d = format!("M 0,{body_y} L 0,{ground_y} L {w},{ground_y} L {w},{body_y} ", w=w, body_y=body_y, ground_y=ground_y);
        
        let cabin_x1 = w * 0.2;
        let cabin_x2 = cabin_x1 + roof_len;
        let cabin_top_x1 = cabin_x1 + (w * 0.1 * (1.0 - body_type));
        
        d.push_str(&format!("L {cabin_x2},{body_y} L {cabin_x2},{cabin_y} L {cabin_top_x1},{cabin_y} L {cabin_x1},{body_y} Z ", cabin_x1=cabin_x1, cabin_x2=cabin_x2, cabin_y=cabin_y, cabin_top_x1=cabin_top_x1, body_y=body_y));
        
        for &wx in &[wheel_x1, wheel_x2] {
            d.push_str(&format!("M {wx},{ground_y} m -{r},0 a {r},{r} 0 1,0 {r2},0 a {r},{r} 0 1,0 -{r2},0 ", wx=wx, ground_y=ground_y, r=wheel_r, r2=wheel_r*2.0));
        }
        d
    }
}
