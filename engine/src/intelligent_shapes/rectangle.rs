use super::*;

pub struct RectangleShape;
impl IntelligentShape for RectangleShape {
    fn get_metadata(&self) -> IntelligentShapeMetadata {
        IntelligentShapeMetadata {
            id: "rectangle".to_string(),
            name: "Rectangle".to_string(),
            parameters: vec![
                ShapeParameter { name: "Corner Radius".to_string(), key: "radius".to_string(), min: 0.0, max: 1.0, default: 0.0, step: 0.01 },
            ],
            icon: "M 2,2 H 22 V 22 H 2 Z".to_string(),
        }
    }

    fn generate_path(&self, w: f64, h: f64, params: &[f64]) -> String {
        let radius_ratio = params.get(0).cloned().unwrap_or(0.0);
        let max_r = (w.min(h) / 2.0);
        let r = radius_ratio * max_r;

        if r <= 0.0 {
            format!("M 0,0 H {} V {} H 0 Z", w, h)
        } else {
            format!(
                "M {r},0 H {w_r} A {r},{r} 0 0 1 {w}, {r} V {h_r} A {r},{r} 0 0 1 {w_r},{h} H {r} A {r},{r} 0 0 1 0,{h_r} V {r} A {r},{r} 0 0 1 {r},0 Z",
                r=r, w=w, h=h, w_r=w-r, h_r=h-r
            )
        }
    }
}
