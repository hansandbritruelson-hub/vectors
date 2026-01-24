use serde::{Serialize, Deserialize};
use kurbo::{BezPath, Point, Vec2, Shape, ParamCurve, ParamCurveArclen};
use web_sys::{CanvasRenderingContext2d, HtmlImageElement};
use wasm_bindgen::JsValue;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum BrushTip {
    Calligraphic {
        angle: f64,     // in radians
        roundness: f64, // 0.0 to 1.0
    },
    Image {
        // We'll store the image data URL or some reference
        // For now, let's assume we have an image element or raw data
        image_id: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Brush {
    pub id: u32,
    pub name: String,
    pub tip: BrushTip,
    pub size: f64,
    pub spacing: f64,           // percentage of size, e.g., 0.25
    pub pressure_enabled: bool,
    pub min_size_fraction: f64, // 0.0 to 1.0
    pub smoothing: f64,         // 0.0 to 1.0
    pub scatter: f64,           // 0.0 to 1.0
    pub rotation_jitter: f64,    // 0.0 to 1.0
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct StrokePoint {
    pub x: f64,
    pub y: f64,
    pub pressure: f64,
}

pub struct BrushEngine {
    pub brushes: Vec<Brush>,
}

impl BrushEngine {
    pub fn new() -> Self {
        let mut engine = BrushEngine { brushes: Vec::new() };
        engine.add_default_brushes();
        engine
    }

    fn add_default_brushes(&mut self) {
        self.brushes.push(Brush {
            id: 1,
            name: "Basic Round".to_string(),
            tip: BrushTip::Calligraphic { angle: 0.0, roundness: 1.0 },
            size: 10.0,
            spacing: 0.1,
            pressure_enabled: true,
            min_size_fraction: 0.2,
            smoothing: 0.5,
            scatter: 0.0,
            rotation_jitter: 0.0,
        });

        self.brushes.push(Brush {
            id: 2,
            name: "Calligraphic Flat".to_string(),
            tip: BrushTip::Calligraphic { angle: std::f64::consts::PI / 4.0, roundness: 0.1 },
            size: 20.0,
            spacing: 0.05,
            pressure_enabled: true,
            min_size_fraction: 0.5,
            smoothing: 0.3,
            scatter: 0.0,
            rotation_jitter: 0.0,
        });
    }

    pub fn render_stroke(
        &self,
        ctx: &CanvasRenderingContext2d,
        brush: &Brush,
        path: &BezPath,
        color: &str,
        image_map: &std::collections::HashMap<String, HtmlImageElement>,
    ) {
        let step = (brush.size * brush.spacing).max(1.0);
        let mut dist_remaining = 0.0;
        
        ctx.save();
        
        let segments: Vec<_> = path.segments().collect();

        for seg in segments.into_iter() {
            let seg_len = seg.arclen(0.1);
            let mut t_dist = dist_remaining;
            
            while t_dist <= seg_len {
                let t = seg.inv_arclen(t_dist, 0.1);
                let pos = seg.eval(t);
                
                // For now, use constant pressure since BezPath doesn't store it.
                // In a more advanced version, we could interpolate from a separate pressure profile.
                let pressure = 1.0; 

                let size = if brush.pressure_enabled {
                    brush.size * (brush.min_size_fraction + (1.0 - brush.min_size_fraction) * pressure)
                } else {
                    brush.size
                };

                self.render_dab(ctx, brush, pos, size, color, image_map);
                
                t_dist += step;
            }
            dist_remaining = t_dist - seg_len;
        }
        
        ctx.restore();
    }

    fn render_dab(
        &self,
        ctx: &CanvasRenderingContext2d,
        brush: &Brush,
        pos: Point,
        size: f64,
        color: &str,
        image_map: &std::collections::HashMap<String, HtmlImageElement>,
    ) {
        ctx.save();
        ctx.translate(pos.x, pos.y).unwrap();
        
        // Apply scatter
        if brush.scatter > 0.0 {
            let offset_x = (js_sys::Math::random() - 0.5) * brush.scatter * brush.size * 5.0;
            let offset_y = (js_sys::Math::random() - 0.5) * brush.scatter * brush.size * 5.0;
            ctx.translate(offset_x, offset_y).unwrap();
        }

        // Apply rotation jitter
        if brush.rotation_jitter > 0.0 {
            let angle = js_sys::Math::random() * std::f64::consts::PI * 2.0 * brush.rotation_jitter;
            ctx.rotate(angle).unwrap();
        }

        match &brush.tip {
            BrushTip::Calligraphic { angle, roundness } => {
                ctx.rotate(*angle).unwrap();
                ctx.scale(1.0, *roundness).unwrap();
                
                ctx.begin_path();
                let _ = ctx.arc(0.0, 0.0, size / 2.0, 0.0, std::f64::consts::PI * 2.0);
                ctx.set_fill_style(&JsValue::from_str(color));
                ctx.fill();
            }
            BrushTip::Image { image_id } => {
                if let Some(img) = image_map.get(image_id) {
                    // For image brushes, we often want to "tint" the image.
                    // This is hard with pure canvas drawImage.
                    // A common trick is to use globalCompositeOperation = 'source-in'
                    // or just draw the image if it's already tinted.
                    // For now, let's just draw it.
                    let _ = ctx.draw_image_with_html_image_element_and_dw_and_dh(
                        img,
                        -size / 2.0,
                        -size / 2.0,
                        size,
                        size,
                    );
                }
            }
        }
        
        ctx.restore();
    }

    // Helper to convert points to an outline (legacy, might still be useful)
    pub fn points_to_outline(&self, brush_id: u32, points: &[StrokePoint]) -> BezPath {
        // ... (existing logic or simplified)
        BezPath::new()
    }
}

