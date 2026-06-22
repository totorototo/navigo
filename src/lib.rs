mod analyzer;
mod area;
mod data;
mod elevation;
#[cfg(test)]
mod helper;
mod location;
mod trace;
mod utils;

pub use analyzer::Analyzer;
pub use area::Area;
pub use elevation::Elevation;
pub use location::Location;
pub use trace::Trace;

pub fn build_trace(locations: &[Location]) -> Trace<'_> {
    Trace { locations }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::approx_equal;

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

        let length = trace.length();

        assert!(trace.locations.len() > 0);
        assert!(approx_equal(length, 2486.340992526076, 1e-10));
    }
}
