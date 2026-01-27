use super::*;

pub struct CloudShape;
impl IntelligentShape for CloudShape {
    fn get_metadata(&self) -> IntelligentShapeMetadata {
        IntelligentShapeMetadata {
            id: "cloud".to_string(),
            name: "Cloud".to_string(),
            parameters: vec![
                ShapeParameter { name: "Puffiness".to_string(), key: "puff".to_string(), min: 0.5, max: 1.5, default: 1.0, step: 0.01 },
                ShapeParameter { name: "Flat Bottom".to_string(), key: "flat".to_string(), min: 0.0, max: 1.0, default: 0.2, step: 0.01 },
            ],
            icon: "M 17.5,19 A 5.5,5.5 0 0 0 17.5,8 A 7.5,7.5 0 0 0 6,9 A 5,5 0 0 0 6,19 Z".to_string(),
        }
    }

    fn generate_path(&self, w: f64, h: f64, params: &[f64]) -> String {
        let puff = params.get(0).cloned().unwrap_or(1.0);
        let flat = params.get(1).cloned().unwrap_or(0.2);
        
        let bh = h * (1.0 - flat * 0.5); // base height
        
        format!(
            "M {},{} C {},{} {},{} {},{} C {},{} {},{} {},{} C {},{} {},{} {},{} C {},{} {},{} {},{} Z",
            w * 0.2, bh,
            w * 0.05, bh * 0.8 * puff, w * 0.05, bh * 0.2 * puff, w * 0.25, bh * 0.3,
            w * 0.3, 0.0, w * 0.6, 0.0, w * 0.7, bh * 0.2,
            w * 0.95, bh * 0.2 * puff, w * 0.95, bh * 0.8 * puff, w * 0.8, bh,
            w * 0.6, bh * (1.0 + flat), w * 0.4, bh * (1.0 + flat), w * 0.2, bh
        )
    }
}
