mod area;
pub mod calibration;
mod climbs;
mod elevation;
mod error;
mod extrema;
pub mod gpx;
#[cfg(test)]
mod helper;
pub mod leg;
mod location;
pub mod minetti;
pub mod pace_model;
pub mod section;
mod simplify;
pub mod segment;
pub mod stage;
mod trace;
pub mod time;
pub mod waypoint;
#[cfg(feature = "wasm")]
mod wasm;

pub use area::Area;
pub use calibration::{BoundaryKind, Recalibration, RecalibratedEta};
pub use climbs::ClimbStats;
pub use elevation::{Elevation, GainLoss};
pub use error::TraceError;
pub use gpx::{GpxMetadata, parse_trace_points, parse_waypoints, parse_metadata};
pub use leg::LegStats;
pub use location::Location;
pub use pace_model::{WeatherConditions, WeatherLookup};
pub use section::SectionStats;
pub use stage::StageStats;
pub use time::parse_iso8601_to_epoch;
pub use trace::Trace;
pub use waypoint::Waypoint;
#[cfg(feature = "wasm")]
pub use wasm::build_trace as build_wasm_trace;

pub fn build_trace(locations: &[Location]) -> Result<Trace, TraceError> {
    Trace::new(locations)
}
