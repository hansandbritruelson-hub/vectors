use serde::{Serialize, Deserialize};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
pub enum ShapeType {
    Rectangle,
    Circle,
    Ellipse,
    Star,
    Polygon,
    Image,
    Path,
    Text,
    Group,
    Adjustment,
    Guide,
    Intelligent,
    SmartBackground,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
pub enum HandleType {
    TopLeft, TopRight, BottomLeft, BottomRight,
    Top, Bottom, Left, Right,
    Rotate,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GradientStop {
    pub offset: f64, // 0.0 to 1.0
    pub color: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Gradient {
    pub is_radial: bool,
    pub x1: f64, pub y1: f64, // Start point (or center for radial)
    pub x2: f64, pub y2: f64, // End point (or radius point for radial)
    pub r1: f64, // Inner radius (radial only)
    pub r2: f64, // Outer radius (radial only)
    pub stops: Vec<GradientStop>,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
pub enum EffectType {
    DropShadow,
    InnerShadow,
    OuterGlow,
    InnerGlow,
    BevelEmboss,
    ColorOverlay,
    GradientOverlay,
    PatternOverlay,
    Stroke,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LayerEffect {
    pub effect_type: EffectType,
    pub enabled: bool,
    pub color: String,
    pub opacity: f64,
    pub blur: f64,
    pub x: f64,
    pub y: f64,
    pub size: f64,
    pub spread: f64,
    pub blend_mode: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct LayerStyle {
    pub effects: Vec<LayerEffect>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Guide {
    pub orientation: String, // "horizontal" or "vertical"
    pub position: f64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Artboard {
    pub width: f64,
    pub height: f64,
    pub background: String,
    pub guides: Vec<Guide>,
}
