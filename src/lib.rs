mod area;
mod climbs;
#[cfg(test)]
mod data;
mod elevation;
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
pub use location::Location;
pub use trace::Trace;
#[cfg(feature = "wasm")]
pub use wasm::build_trace as build_wasm_trace;

pub fn build_trace(locations: &[Location]) -> Trace {
    Trace::new(locations)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_build_a_trace() {
        let paris = Location {
            longitude: 2.350987,
            latitude: 48.856667,
            altitude: 0.0,
        };
        let moscow = Location {
            longitude: 37.617634,
            latitude: 55.755787,
            altitude: 0.0,
        };
        let locations = vec![paris, moscow];
        let trace = build_trace(&locations);
        assert!(!trace.locations.is_empty());
        assert!((trace.length() - 2486.340992526076).abs() < 1e-6);
    }
}
