use wasm_bindgen::prelude::*;
use crate::types::{Artboard, Guide};
use crate::objects::VectorObject;
use crate::brush::BrushEngine;
use web_sys::HtmlImageElement;
use std::collections::HashMap;

#[derive(Clone)]
pub struct EngineState {
    pub objects: Vec<VectorObject>,
    pub next_id: u32,
    pub selected_ids: Vec<u32>,
    pub artboard: Artboard,
    pub clip_to_artboard: bool,
    pub action_name: String,
}

#[wasm_bindgen]
pub struct VectorEngine {
    pub(crate) objects: Vec<VectorObject>,
    pub(crate) next_id: u32,
    pub(crate) selected_ids: Vec<u32>,
    pub viewport_x: f64,
    pub viewport_y: f64,
    pub viewport_zoom: f64,
    pub(crate) artboard: Artboard,
    pub clip_to_artboard: bool,
    pub hide_selection: bool,
    pub(crate) undo_stack: Vec<EngineState>,
    pub(crate) redo_stack: Vec<EngineState>,
    pub(crate) brush_engine: BrushEngine,
    pub(crate) brush_image_map: HashMap<String, HtmlImageElement>,
}

#[wasm_bindgen]
impl VectorEngine {
    #[wasm_bindgen(constructor)]
    pub fn new() -> VectorEngine {
        console_error_panic_hook::set_once();
        
        VectorEngine {
            objects: Vec::new(),
            next_id: 1,
            selected_ids: Vec::new(),
            viewport_x: 0.0,
            viewport_y: 0.0,
            viewport_zoom: 1.0,
            artboard: Artboard {
                width: 800.0,
                height: 600.0,
                background: "#ffffff".to_string(),
                guides: Vec::new(),
            },
            clip_to_artboard: false,
            hide_selection: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            brush_engine: BrushEngine::new(),
            brush_image_map: HashMap::new(),
        }
    }

    pub(crate) fn save_state(&mut self, action_name: &str) {
        let state = EngineState {
            objects: self.objects.clone(),
            next_id: self.next_id,
            selected_ids: self.selected_ids.clone(),
            artboard: self.artboard.clone(),
            clip_to_artboard: self.clip_to_artboard,
            action_name: action_name.to_string(),
        };
        self.undo_stack.push(state);
        if self.undo_stack.len() > 100 {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    pub fn undo(&mut self) -> bool {
        if let Some(prev_state) = self.undo_stack.pop() {
            let current_state = EngineState {
                objects: self.objects.clone(),
                next_id: self.next_id,
                selected_ids: self.selected_ids.clone(),
                artboard: self.artboard.clone(),
                clip_to_artboard: self.clip_to_artboard,
                action_name: "Redo State".to_string(),
            };
            self.redo_stack.push(current_state);

            self.objects = prev_state.objects;
            self.next_id = prev_state.next_id;
            self.selected_ids = prev_state.selected_ids;
            self.artboard = prev_state.artboard;
            self.clip_to_artboard = prev_state.clip_to_artboard;
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        if let Some(next_state) = self.redo_stack.pop() {
            let current_state = EngineState {
                objects: self.objects.clone(),
                next_id: self.next_id,
                selected_ids: self.selected_ids.clone(),
                artboard: self.artboard.clone(),
                clip_to_artboard: self.clip_to_artboard,
                action_name: "Undo State".to_string(),
            };
            self.undo_stack.push(current_state);

            self.objects = next_state.objects;
            self.next_id = next_state.next_id;
            self.selected_ids = next_state.selected_ids;
            self.artboard = next_state.artboard;
            self.clip_to_artboard = next_state.clip_to_artboard;
            true
        } else {
            false
        }
    }

    pub fn set_viewport(&mut self, x: f64, y: f64, zoom: f64) {
        self.viewport_x = x;
        self.viewport_y = y;
        self.viewport_zoom = zoom;
    }

    pub fn get_history(&self) -> String {
        let history: Vec<String> = self.undo_stack.iter().map(|s| s.action_name.clone()).collect();
        serde_json::to_string(&history).unwrap_or("[]".to_string())
    }

    pub fn get_artboard(&self) -> String {
        serde_json::to_string(&self.artboard).unwrap_or("{}".to_string())
    }

    pub fn get_objects_json(&self) -> String {
        serde_json::to_string(&self.objects).unwrap_or_else(|_| "[]".to_string())
    }

    pub fn get_selected_ids(&self) -> String {
        serde_json::to_string(&self.selected_ids).unwrap_or("[]".to_string())
    }
}
