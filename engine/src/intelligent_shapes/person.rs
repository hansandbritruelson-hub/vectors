use super::*;

pub struct PersonShape;
impl IntelligentShape for PersonShape {
    fn get_metadata(&self) -> IntelligentShapeMetadata {
        IntelligentShapeMetadata {
            id: "person".to_string(),
            name: "Person".to_string(),
            parameters: vec![
                ShapeParameter { name: "Head Size".to_string(), key: "head".to_string(), min: 0.1, max: 0.4, default: 0.2, step: 0.01 },
                ShapeParameter { name: "Body Fat".to_string(), key: "fat".to_string(), min: 0.2, max: 0.8, default: 0.4, step: 0.01 },
                ShapeParameter { name: "Arm Length".to_string(), key: "arms".to_string(), min: 0.1, max: 0.5, default: 0.3, step: 0.01 },
                ShapeParameter { name: "Leg Length".to_string(), key: "legs".to_string(), min: 0.2, max: 0.6, default: 0.4, step: 0.01 },
            ],
            icon: "M 12,2 A 3,3 0 1 0 12,8 A 3,3 0 1 0 12,2 M 6,22 V 18 Q 6,10 12,10 Q 18,10 18,18 V 22".to_string(),
        }
    }

    fn generate_path(&self, w: f64, h: f64, params: &[f64]) -> String {
        let head_r = params.get(0).cloned().unwrap_or(0.2) * (h.min(w) / 2.0);
        let body_fat = params.get(1).cloned().unwrap_or(0.4) * w;
        let arm_l = params.get(2).cloned().unwrap_or(0.3) * w;
        let leg_l = params.get(3).cloned().unwrap_or(0.4) * h;

        let cx = w / 2.0;
        let head_y = head_r;
        let neck_y = head_r * 2.0;
        let body_h = h - neck_y - leg_l;
        let torso_bottom = neck_y + body_h;
        
        format!(
            "M {cx},{head_y} m -{head_r},0 a {head_r},{head_r} 0 1,0 {head_r_2},0 a {head_r},{head_r} 0 1,0 -{head_r_2},0 \
             M {cx},{neck_y} L {arm_start},{neck_y} L {arm_start},{arm_end} M {arm_end_r},{arm_end} L {arm_end_r},{neck_y} L {cx},{neck_y} \
             V {torso_bottom} L {leg_l_x},{h} M {leg_r_x},{h} L {cx},{torso_bottom} \
             M {torso_left},{neck_y} H {torso_right} V {torso_bottom} H {torso_left} Z",
            cx=cx, head_y=head_y, head_r=head_r, head_r_2=head_r*2.0,
            neck_y=neck_y, 
            arm_start=cx - body_fat/2.0 - arm_l,
            arm_end=neck_y + body_h * 0.4,
            arm_end_r=cx + body_fat/2.0 + arm_l,
            torso_bottom=torso_bottom,
            leg_l_x=cx - body_fat/3.0,
            leg_r_x=cx + body_fat/3.0,
            h=h,
            torso_left=cx - body_fat/2.0,
            torso_right=cx + body_fat/2.0
        )
    }
}
