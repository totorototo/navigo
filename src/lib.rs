mod area;
pub mod calibration;
mod climbs;
mod elevation;
mod error;
mod extrema;
pub mod gpx;
#[cfg(test)]
mod helper;
mod interval;
pub mod leg;
mod location;
pub mod minetti;
pub mod pace_model;
pub mod section;
pub mod segment;
mod simplify;
pub mod stage;
pub mod time;
mod trace;
#[cfg(feature = "wasm")]
mod wasm;
pub mod waypoint;

pub use area::Area;
pub use calibration::{BoundaryKind, RecalibratedEta, Recalibration};
pub use climbs::ClimbStats;
pub use elevation::{Elevation, GainLoss};
pub use error::TraceError;
pub use gpx::{parse_metadata, parse_trace_points, parse_waypoints, GpxMetadata};
pub use leg::LegStats;
pub use location::Location;
pub use pace_model::{AnalysisOptions, WeatherConditions, WeatherLookup};
pub use section::SectionStats;
pub use stage::StageStats;
pub use time::parse_iso8601_to_epoch;
pub use trace::Trace;
#[cfg(feature = "wasm")]
pub use wasm::build_trace as build_wasm_trace;
pub use waypoint::Waypoint;

pub fn build_trace(locations: &[Location]) -> Result<Trace, TraceError> {
    Trace::new(locations)
}
