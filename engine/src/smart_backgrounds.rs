use serde::{Serialize, Deserialize};
use web_sys::CanvasRenderingContext2d;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "lowercase")]
pub enum ParameterKind {
    Range,
    Color,
    Bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BackgroundParameter {
    pub name: String,
    pub key: String,
    pub min: f64,
    pub max: f64,
    pub default: f64,
    pub step: f64,
    pub kind: ParameterKind,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SmartBackgroundMetadata {
    pub id: String,
    pub name: String,
    pub parameters: Vec<BackgroundParameter>,
    pub icon: String,
}

pub trait SmartBackground {
    fn get_metadata(&self) -> SmartBackgroundMetadata;
    fn render(&self, ctx: &CanvasRenderingContext2d, width: f64, height: f64, params: &[f64]);
}

pub mod stars;
pub mod grass;
pub mod ocean;
pub mod nebula;
pub mod circuit;
pub mod crystal;
pub mod mesh;

pub fn get_all_backgrounds() -> Vec<Box<dyn SmartBackground>> {
    vec![
        Box::new(stars::StarsBackground),
        Box::new(grass::GrassBackground),
        Box::new(ocean::OceanBackground),
        Box::new(nebula::NebulaBackground),
        Box::new(circuit::CircuitBackground),
        Box::new(crystal::CrystalBackground),
        Box::new(mesh::MeshBackground),
    ]
}

pub fn get_background_by_id(id: &str) -> Option<Box<dyn SmartBackground>> {
    match id {
        "stars" => Some(Box::new(stars::StarsBackground)),
        "grass" => Some(Box::new(grass::GrassBackground)),
        "ocean" => Some(Box::new(ocean::OceanBackground)),
        "nebula" => Some(Box::new(nebula::NebulaBackground)),
        "circuit" => Some(Box::new(circuit::CircuitBackground)),
        "crystal" => Some(Box::new(crystal::CrystalBackground)),
        "mesh" => Some(Box::new(mesh::MeshBackground)),
        _ => None
    }
}
