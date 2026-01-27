use super::*;
use std::f64::consts::PI;

pub struct CircleShape;
impl IntelligentShape for CircleShape {
    fn get_metadata(&self) -> IntelligentShapeMetadata {
        IntelligentShapeMetadata {
            id: "circle".to_string(),
            name: "Circle".to_string(),
            parameters: vec![
                ShapeParameter { name: "Start Angle".to_string(), key: "start".to_string(), min: 0.0, max: 360.0, default: 0.0, step: 1.0 },
                ShapeParameter { name: "End Angle".to_string(), key: "end".to_string(), min: 0.0, max: 360.0, default: 360.0, step: 1.0 },
                ShapeParameter { name: "Inner Radius".to_string(), key: "inner".to_string(), min: 0.0, max: 1.0, default: 0.0, step: 0.01 },
            ],
            icon: "M 12,2 A 10,10 0 1 1 12,22 A 10,10 0 1 1 12,2 Z".to_string(),
        }
    }

    fn generate_path(&self, w: f64, h: f64, params: &[f64]) -> String {
        let start_angle = params.get(0).cloned().unwrap_or(0.0).to_radians();
        let end_angle = params.get(1).cloned().unwrap_or(360.0).to_radians();
        let inner_ratio = params.get(2).cloned().unwrap_or(0.0);

        let cx = w / 2.0;
        let cy = h / 2.0;
        let rx = w / 2.0;
        let ry = h / 2.0;

        let sweep = end_angle - start_angle;
        if sweep.abs() >= 2.0 * PI {
            if inner_ratio <= 0.0 {
                // Full ellipse
                format!(
                    "M {cx},{start_y} A {rx},{ry} 0 1 1 {cx},{end_y} A {rx},{ry} 0 1 1 {cx},{start_y} Z",
                    cx=cx, start_y=cy-ry, end_y=cy+ry, rx=rx, ry=ry
                )
            } else {
                // Donut
                let irx = rx * inner_ratio;
                let iry = ry * inner_ratio;
                format!(
                    "M {cx},{start_y} A {rx},{ry} 0 1 1 {cx},{end_y} A {rx},{ry} 0 1 1 {cx},{start_y} M {cx},{i_start_y} A {irx},{iry} 0 1 0 {cx},{i_end_y} A {irx},{iry} 0 1 0 {cx},{i_start_y} Z",
                    cx=cx, start_y=cy-ry, end_y=cy+ry, rx=rx, ry=ry,
                    i_start_y=cy-iry, i_end_y=cy+iry, irx=irx, iry=iry
                )
            }
        } else {
            // Pie or Arc
            let x1 = cx + rx * start_angle.cos();
            let y1 = cy + ry * start_angle.sin();
            let x2 = cx + rx * end_angle.cos();
            let y2 = cy + ry * end_angle.sin();
            
            let large_arc = if sweep > PI { 1 } else { 0 };

            if inner_ratio <= 0.0 {
                format!(
                    "M {cx},{cy} L {x1},{y1} A {rx},{ry} 0 {large_arc} 1 {x2},{y2} Z",
                    cx=cx, cy=cy, x1=x1, y1=y1, x2=x2, y2=y2, rx=rx, ry=ry, large_arc=large_arc
                )
            } else {
                let irx = rx * inner_ratio;
                let iry = ry * inner_ratio;
                let ix1 = cx + irx * start_angle.cos();
                let iy1 = cy + iry * start_angle.sin();
                let ix2 = cx + irx * end_angle.cos();
                let iy2 = cy + iry * end_angle.sin();
                
                format!(
                    "M {ix1},{iy1} L {x1},{y1} A {rx},{ry} 0 {large_arc} 1 {x2},{y2} L {ix2},{iy2} A {irx},{iry} 0 {large_arc} 0 {ix1},{iy1} Z",
                    x1=x1, y1=y1, x2=x2, y2=y2, rx=rx, ry=ry,
                    ix1=ix1, iy1=iy1, ix2=ix2, iy2=iy2, irx=irx, iry=iry, large_arc=large_arc
                )
            }
        }
    }
}
