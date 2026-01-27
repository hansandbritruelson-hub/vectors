use wasm_bindgen::prelude::*;

pub mod types;
pub mod objects;
pub mod engine;
pub mod selection;
pub mod render;
pub mod commands;
pub mod io;
pub mod svg;
pub mod image_ops;
pub mod psd;
pub mod ai;
pub mod tracer;
pub mod brush;
pub mod boolean;
pub mod warp;
pub mod intelligent_shapes;
pub mod smart_backgrounds;

#[wasm_bindgen]
pub fn get_intelligent_shapes_metadata() -> JsValue {
    let shapes = intelligent_shapes::get_all_shapes();
    let metadata: Vec<_> = shapes.iter().map(|s| s.get_metadata()).collect();
    serde_wasm_bindgen::to_value(&metadata).unwrap()
}

#[wasm_bindgen]
pub fn get_smart_backgrounds_metadata() -> JsValue {
    let backgrounds = smart_backgrounds::get_all_backgrounds();
    let metadata: Vec<_> = backgrounds.iter().map(|s| s.get_metadata()).collect();
    serde_wasm_bindgen::to_value(&metadata).unwrap()
}

pub use engine::VectorEngine;
pub use types::*;
pub use objects::*;