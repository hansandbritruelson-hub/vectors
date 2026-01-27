use kurbo::{Point, BezPath, PathEl, Shape, Affine};
use serde::{Serialize, Deserialize};
use wasm_bindgen::prelude::*;
use crate::objects::VectorObject;
use crate::types::ShapeType;
use crate::engine::VectorEngine;

#[derive(Serialize, Deserialize, Clone)]
pub struct WarpMesh {
    pub rows: usize,
    pub cols: usize,
    pub control_points: Vec<Point>, // Flattened 2D array of points (rows+1) * (cols+1)
    pub original_bounds: (f64, f64, f64, f64), // x, y, width, height
}

impl WarpMesh {
    pub fn new(rows: usize, cols: usize, x: f64, y: f64, width: f64, height: f64) -> Self {
        let mut control_points = Vec::new();
        for r in 0..=rows {
            for c in 0..=cols {
                let px = x + (c as f64 / cols as f64) * width;
                let py = y + (r as f64 / rows as f64) * height;
                control_points.push(Point::new(px, py));
            }
        }
        Self {
            rows,
            cols,
            control_points,
            original_bounds: (x, y, width, height),
        }
    }

    pub fn warp_point(&self, p: Point) -> Point {
        let (ox, oy, ow, oh) = self.original_bounds;
        
        // Normalize coordinates to [0, 1] relative to original mesh
        let nx = (p.x - ox) / ow;
        let ny = (p.y - oy) / oh;

        let c_float = nx * (self.cols as f64);
        let r_float = ny * (self.rows as f64);

        let c0 = (c_float.floor() as usize).clamp(0, self.cols - 1);
        let c1 = (c0 + 1).clamp(0, self.cols);
        let r0 = (r_float.floor() as usize).clamp(0, self.rows - 1);
        let r1 = (r0 + 1).clamp(0, self.rows);

        let u = c_float - c0 as f64;
        let v = r_float - r0 as f64;

        let p00 = self.control_points[r0 * (self.cols + 1) + c0];
        let p10 = self.control_points[r0 * (self.cols + 1) + c1];
        let p01 = self.control_points[r1 * (self.cols + 1) + c0];
        let p11 = self.control_points[r1 * (self.cols + 1) + c1];

        // Bilinear interpolation
        let top = p00.to_vec2() * (1.0 - u) + p10.to_vec2() * u;
        let bottom = p01.to_vec2() * (1.0 - u) + p11.to_vec2() * u;
        let final_p = top * (1.0 - v) + bottom * v;

        Point::new(final_p.x, final_p.y)
    }

    pub fn warp_path(&self, path: &BezPath) -> BezPath {
        let mut warped = BezPath::new();
        for el in path.elements() {
            match el {
                PathEl::MoveTo(p) => warped.move_to(self.warp_point(*p)),
                PathEl::LineTo(p) => warped.line_to(self.warp_point(*p)),
                PathEl::QuadTo(p1, p2) => warped.quad_to(self.warp_point(*p1), self.warp_point(*p2)),
                PathEl::CurveTo(p1, p2, p3) => warped.curve_to(self.warp_point(*p1), self.warp_point(*p2), self.warp_point(*p3)),
                PathEl::ClosePath => warped.close_path(),
            }
        }
        warped
    }
}

#[wasm_bindgen]
impl VectorEngine {
    pub fn create_warp_mesh(&self, rows: usize, cols: usize) -> JsValue {
        if self.selected_ids.is_empty() {
            return JsValue::NULL;
        }

        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;

        for id in &self.selected_ids {
            if let Some(obj) = self.objects.iter().find(|o| o.id == *id) {
                let (ox1, oy1, ox2, oy2) = obj.get_world_bounds();
                min_x = min_x.min(ox1);
                min_y = min_y.min(oy1);
                max_x = max_x.max(ox2);
                max_y = max_y.max(oy2);
            }
        }

        let width = max_x - min_x;
        let height = max_y - min_y;

        // Add a small padding
        let padding = 20.0;
        let mesh = WarpMesh::new(rows, cols, min_x - padding, min_y - padding, width + padding * 2.0, height + padding * 2.0);
        
        serde_wasm_bindgen::to_value(&mesh).unwrap()
    }

    pub fn apply_warp_mesh(&mut self, mesh_js: JsValue, save_undo: bool) {
        let mesh: WarpMesh = match serde_wasm_bindgen::from_value(mesh_js) {
            Ok(m) => m,
            Err(_) => return,
        };

        if save_undo {
            self.save_state("Apply Warp Mesh");
        }

        let selected_ids = self.selected_ids.clone();
        for id in selected_ids {
            if let Some(obj_idx) = self.objects.iter().position(|o| o.id == id) {
                let mut obj = self.objects[obj_idx].clone();
                
                // Convert object to path if it's not one
                let mut path = match obj.shape_type {
                    ShapeType::Path => BezPath::from_svg(&obj.path_data).unwrap_or_default(),
                    ShapeType::Rectangle => {
                        let rect = kurbo::Rect::new(0.0, 0.0, obj.width, obj.height);
                        rect.to_path(0.1)
                    },
                    ShapeType::Circle | ShapeType::Ellipse => {
                        let ellipse = kurbo::Ellipse::new((obj.width / 2.0, obj.height / 2.0), (obj.width / 2.0, obj.height / 2.0), 0.0);
                        ellipse.to_path(0.1)
                    },
                    _ => continue, // Skip for now
                };

                // Transform path to world coordinates
                let transform = Affine::translate((obj.x + obj.width / 2.0, obj.y + obj.height / 2.0))
                    * Affine::rotate(obj.rotation)
                    * Affine::translate((-obj.width / 2.0, -obj.height / 2.0));
                
                path.apply_affine(transform);

                // Warp the path
                let warped_path = mesh.warp_path(&path);

                // Warp stroke points if it's a brush stroke
                if obj.brush_id > 0 && !obj.stroke_points.is_empty() {
                    for sp in obj.stroke_points.iter_mut() {
                        let p_world = transform * Point::new(sp.x, sp.y);
                        let p_warped = mesh.warp_point(p_world);
                        let p_local = Affine::translate((-warped_path.bounding_box().x0, -warped_path.bounding_box().y0)) * p_warped;
                        sp.x = p_local.x;
                        sp.y = p_local.y;
                    }
                }

                // Update object
                let bbox = warped_path.bounding_box();
                obj.x = bbox.x0;
                obj.y = bbox.y0;
                obj.width = bbox.width();
                obj.height = bbox.height();
                obj.rotation = 0.0;
                obj.shape_type = ShapeType::Path;

                let mut normalized = warped_path.clone();
                normalized.apply_affine(Affine::translate((-bbox.x0, -bbox.y0)));
                obj.path_data = normalized.to_svg();

                self.objects[obj_idx] = obj;
            }
        }
    }
}
