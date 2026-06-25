/// A `<wpt>` element parsed from a GPX file.
#[derive(Clone)]
pub struct Waypoint {
    pub latitude: f64,
    pub longitude: f64,
    pub elevation: Option<f64>,
    pub name: String,
    pub description: Option<String>,
    pub comment: Option<String>,
    pub symbol: Option<String>,
    /// Race-specific type: `"Start"`, `"TimeBarrier"`, `"LifeBase"`, `"Arrival"`, or `None`.
    pub wpt_type: Option<String>,
    /// Unix epoch (seconds), parsed from `<time>` using ISO 8601.
    pub time: Option<i64>,
    /// Planned stop duration in seconds, from `<stopDuration>`. LifeBase-only.
    pub stop_duration: Option<u32>,
}

impl Waypoint {
    /// Any typed waypoint (non-null wpt_type) is a section boundary.
    pub fn is_section_boundary(&self) -> bool {
        self.wpt_type.is_some()
    }

    /// Stage boundaries are Start, LifeBase, and Arrival (not TimeBarrier).
    pub fn is_stage_boundary(&self) -> bool {
        match self.wpt_type.as_deref() {
            Some("Start") | Some("LifeBase") | Some("Arrival") => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_waypoint(wpt_type: Option<&str>) -> Waypoint {
        Waypoint {
            latitude: 0.0,
            longitude: 0.0,
            elevation: None,
            name: String::new(),
            description: None,
            comment: None,
            symbol: None,
            wpt_type: wpt_type.map(String::from),
            time: None,
            stop_duration: None,
        }
    }

    #[test]
    fn stage_boundary_types() {
        assert!(make_waypoint(Some("Start")).is_stage_boundary());
        assert!(make_waypoint(Some("LifeBase")).is_stage_boundary());
        assert!(make_waypoint(Some("Arrival")).is_stage_boundary());
    }

    #[test]
    fn time_barrier_is_section_not_stage() {
        let wpt = make_waypoint(Some("TimeBarrier"));
        assert!(wpt.is_section_boundary());
        assert!(!wpt.is_stage_boundary());
    }

    #[test]
    fn untyped_waypoint_is_not_a_boundary() {
        let wpt = make_waypoint(None);
        assert!(!wpt.is_section_boundary());
        assert!(!wpt.is_stage_boundary());
    }

    #[test]
    fn arrival_is_both_section_and_stage_boundary() {
        let wpt = make_waypoint(Some("Arrival"));
        assert!(wpt.is_section_boundary());
        assert!(wpt.is_stage_boundary());
    }

    #[test]
    fn start_is_both_section_and_stage_boundary() {
        let wpt = make_waypoint(Some("Start"));
        assert!(wpt.is_section_boundary());
        assert!(wpt.is_stage_boundary());
    }
}
