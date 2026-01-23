use image::GrayImage;
use std::collections::{HashMap, HashSet};

pub struct Tracer {
    width: u32,
    height: u32,
}

#[derive(Hash, Eq, PartialEq, Clone, Copy, Debug)]
struct Point {
    x: i32, // Fixed point math to avoid float precision issues during joining
    y: i32,
}

impl Tracer {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    pub fn trace(&self, image: &GrayImage, threshold: u8) -> String {
        let mut segments = Vec::new();

        // 1. Generate all segments using Marching Squares
        for y in 0..self.height - 1 {
            for x in 0..self.width - 1 {
                let v00 = image.get_pixel(x, y)[0] < threshold;
                let v10 = image.get_pixel(x + 1, y)[0] < threshold;
                let v11 = image.get_pixel(x + 1, y + 1)[0] < threshold;
                let v01 = image.get_pixel(x, y + 1)[0] < threshold;

                let mut case_index = 0;
                if v00 { case_index |= 1; }
                if v10 { case_index |= 2; }
                if v11 { case_index |= 4; }
                if v01 { case_index |= 8; }

                if case_index == 0 || case_index == 15 { continue; }

                let cell_segments = match case_index {
                    1 | 14 => vec![(0, 1, 1, 0)],
                    2 | 13 => vec![(1, 0, 2, 1)],
                    4 | 11 => vec![(2, 1, 1, 2)],
                    8 | 7  => vec![(1, 2, 0, 1)],
                    3 | 12 => vec![(0, 1, 2, 1)],
                    6 | 9  => vec![(1, 0, 1, 2)],
                    5 => vec![(0, 1, 1, 0), (2, 1, 1, 2)],
                    10 => vec![(1, 0, 2, 1), (0, 1, 1, 2)],
                    _ => vec![],
                };

                for (sx, sy, ex, ey) in cell_segments {
                    let p1 = Point { x: (x as i32 * 2) + sx, y: (y as i32 * 2) + sy };
                    let p2 = Point { x: (x as i32 * 2) + ex, y: (y as i32 * 2) + ey };
                    segments.push((p1, p2));
                }
            }
        }

        // 2. Join segments into paths
        let mut adj = HashMap::new();
        for (p1, p2) in segments {
            adj.entry(p1).or_insert_with(Vec::new).push(p2);
            adj.entry(p2).or_insert_with(Vec::new).push(p1);
        }

        let mut path_data = String::new();
        let mut visited = HashSet::new();

        for start_node in adj.keys() {
            if visited.contains(start_node) { continue; }

            let mut path = Vec::new();
            let mut current = *start_node;
            
            // Find an endpoint for open paths, or just start if closed
            let mut start = *start_node;
            if adj.get(&start).unwrap().len() == 1 {
                 // Already at an endpoint
            } else {
                // Try to find an endpoint nearby
                let mut temp_visited = HashSet::new();
                let mut stack = vec![start];
                while let Some(node) = stack.pop() {
                    if temp_visited.contains(&node) { continue; }
                    temp_visited.insert(node);
                    if adj.get(&node).unwrap().len() == 1 {
                        start = node;
                        break;
                    }
                    for next in adj.get(&node).unwrap() {
                        stack.push(*next);
                    }
                }
            }

            current = start;
            loop {
                visited.insert(current);
                path.push(current);
                
                let next_options: Vec<_> = adj.get(&current).unwrap()
                    .iter()
                    .filter(|p| !visited.contains(*p))
                    .collect();

                if next_options.is_empty() {
                    // Check if we can close the loop
                    let can_close = adj.get(&current).unwrap().iter().any(|p| *p == start && path.len() > 2);
                    if can_close {
                        path_data.push_str(&self.format_path(&path, true));
                    } else {
                        path_data.push_str(&self.format_path(&path, false));
                    }
                    break;
                }
                current = *next_options[0];
            }
        }

        path_data
    }

    fn format_path(&self, points: &[Point], closed: bool) -> String {
        if points.is_empty() { return String::new(); }
        
        let mut d = format!("M {} {} ", points[0].x as f64 / 2.0, points[0].y as f64 / 2.0);
        for i in 1..points.len() {
            d.push_str(&format!("L {} {} ", points[i].x as f64 / 2.0, points[i].y as f64 / 2.0));
        }
        if closed {
            d.push_str("Z ");
        }
        d
    }
}