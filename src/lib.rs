mod area;
mod climbs;
mod elevation;
mod error;
mod extrema;
#[cfg(test)]
mod helper;
mod location;
mod simplify;
mod trace;
#[cfg(feature = "wasm")]
mod wasm;

pub use area::Area;
pub use climbs::ClimbStats;
pub use elevation::{Elevation, GainLoss};
pub use error::TraceError;
pub use location::Location;
pub use trace::Trace;
#[cfg(feature = "wasm")]
pub use wasm::build_trace as build_wasm_trace;

pub fn build_trace(locations: &[Location]) -> Result<Trace, TraceError> {
    Trace::new(locations)
}
