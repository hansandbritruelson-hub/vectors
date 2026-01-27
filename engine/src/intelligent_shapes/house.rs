use super::*;

pub struct HouseShape;
impl IntelligentShape for HouseShape {
    fn get_metadata(&self) -> IntelligentShapeMetadata {
        IntelligentShapeMetadata {
            id: "house".to_string(),
            name: "House".to_string(),
            parameters: vec![
                ShapeParameter { name: "Roof Height".to_string(), key: "roof_h".to_string(), min: 0.0, max: 1.0, default: 0.4, step: 0.01 },
                ShapeParameter { name: "Door Position".to_string(), key: "door_pos".to_string(), min: 0.1, max: 0.9, default: 0.5, step: 0.01 },
                ShapeParameter { name: "Door Size".to_string(), key: "door_size".to_string(), min: 0.1, max: 0.4, default: 0.25, step: 0.01 },
            ],
            icon: "M 12,2 L 2,12 H 5 V 22 H 10 V 16 H 14 V 22 H 19 V 12 H 22 Z".to_string(),
        }
    }

    fn generate_path(&self, w: f64, h: f64, params: &[f64]) -> String {
        let roof_h = params.get(0).cloned().unwrap_or(0.4) * h;
        let door_pos = params.get(1).cloned().unwrap_or(0.5);
        let door_size = params.get(2).cloned().unwrap_or(0.25);
        
        let door_w = w * door_size;
        let door_h = (h - roof_h) * 0.6;
        let door_x = (w * door_pos) - (door_w / 2.0);
        let door_y = h - door_h;

        let mut path = format!("M 0,{} L {},0 L {},{} L {},{} L {},{}", 
            roof_h, w / 2.0, w, roof_h, w, h, door_x + door_w, h);
        path.push_str(&format!(" L {},{} L {},{} L {},{} L 0,{} Z",
            door_x + door_w, door_y, door_x, door_y, door_x, h, h));
        path
    }
}
