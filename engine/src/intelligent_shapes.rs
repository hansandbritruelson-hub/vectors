use serde::{Serialize, Deserialize};
use std::f64::consts::PI;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ShapeParameter {
    pub name: String,
    pub key: String,
    pub min: f64,
    pub max: f64,
    pub default: f64,
    pub step: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct IntelligentShapeMetadata {
    pub id: String,
    pub name: String,
    pub parameters: Vec<ShapeParameter>,
    pub icon: String,
}

pub trait IntelligentShape {
    fn get_metadata(&self) -> IntelligentShapeMetadata;
    fn generate_path(&self, width: f64, height: f64, params: &[f64]) -> String;
}

pub mod house;
pub mod speech_bubble;
pub mod cloud;
pub mod gear;
pub mod person;
pub mod car;
pub mod arrow;
pub mod starburst;
pub mod rectangle;
pub mod circle;
pub mod star;
pub mod polygon;

pub fn get_all_shapes() -> Vec<Box<dyn IntelligentShape>> {
    vec![
        Box::new(rectangle::RectangleShape),
        Box::new(circle::CircleShape),
        Box::new(star::StarShape),
        Box::new(polygon::PolygonShape),
        Box::new(house::HouseShape),
        Box::new(speech_bubble::SpeechBubbleShape),
        Box::new(cloud::CloudShape),
        Box::new(gear::GearShape),
        Box::new(person::PersonShape),
        Box::new(car::CarShape),
        Box::new(arrow::ArrowShape),
        Box::new(starburst::StarBurstShape),
    ]
}

pub fn get_shape_by_id(id: &str) -> Option<Box<dyn IntelligentShape>> {
    match id {
        "rectangle" => Some(Box::new(rectangle::RectangleShape)),
        "circle" => Some(Box::new(circle::CircleShape)),
        "star" => Some(Box::new(star::StarShape)),
        "polygon" => Some(Box::new(polygon::PolygonShape)),
        "house" => Some(Box::new(house::HouseShape)),
        "speech_bubble" => Some(Box::new(speech_bubble::SpeechBubbleShape)),
        "cloud" => Some(Box::new(cloud::CloudShape)),
        "gear" => Some(Box::new(gear::GearShape)),
        "person" => Some(Box::new(person::PersonShape)),
        "car" => Some(Box::new(car::CarShape)),
        "arrow" => Some(Box::new(arrow::ArrowShape)),
        "starburst" => Some(Box::new(starburst::StarBurstShape)),
        _ => None
    }
}
