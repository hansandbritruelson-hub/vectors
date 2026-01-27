use super::*;

pub struct SpeechBubbleShape;
impl IntelligentShape for SpeechBubbleShape {
    fn get_metadata(&self) -> IntelligentShapeMetadata {
        IntelligentShapeMetadata {
            id: "speech_bubble".to_string(),
            name: "Speech Bubble".to_string(),
            parameters: vec![
                ShapeParameter { name: "Tail Position".to_string(), key: "tail_pos".to_string(), min: 0.1, max: 0.9, default: 0.3, step: 0.01 },
                ShapeParameter { name: "Tail Width".to_string(), key: "tail_w".to_string(), min: 0.05, max: 0.3, default: 0.15, step: 0.01 },
                ShapeParameter { name: "Tail Height".to_string(), key: "tail_h".to_string(), min: 0.1, max: 0.5, default: 0.2, step: 0.01 },
                ShapeParameter { name: "Roundness".to_string(), key: "round".to_string(), min: 0.0, max: 50.0, default: 10.0, step: 1.0 },
            ],
            icon: "M 21,15 A 8,8 0 1 0 5,15 L 3,21 L 9,19 A 8,8 0 0 0 21,15 Z".to_string(),
        }
    }

    fn generate_path(&self, w: f64, h: f64, params: &[f64]) -> String {
        let tail_pos = params.get(0).cloned().unwrap_or(0.3) * w;
        let tail_w = params.get(1).cloned().unwrap_or(0.15) * w;
        let tail_h = params.get(2).cloned().unwrap_or(0.2) * h;
        let r = params.get(3).cloned().unwrap_or(10.0).min(w/2.0).min((h-tail_h)/2.0);

        let bh = h - tail_h; // bubble height
        
        format!(
            "M {r},0 H {w_r} A {r},{r} 0 0 1 {w}, {r} V {bh_r} A {r},{r} 0 0 1 {w_r},{bh} H {t_end} L {t_pos},{h} L {t_start},{bh} H {r} A {r},{r} 0 0 1 0,{bh_r} V {r} A {r},{r} 0 0 1 {r},0 Z",
            r=r, w=w, bh=bh, h=h,
            w_r=w-r, bh_r=bh-r,
            t_start=tail_pos - tail_w/2.0,
            t_end=tail_pos + tail_w/2.0,
            t_pos=tail_pos
        )
    }
}
