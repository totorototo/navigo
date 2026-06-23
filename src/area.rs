#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "wasm", derive(serde::Serialize))]
pub struct Area {
    pub min_longitude: f64,
    pub max_longitude: f64,
    pub min_latitude: f64,
    pub max_latitude: f64,
}
