use crate::interval::{compute_intervals, IntervalMetrics};
use crate::pace_model::AnalysisOptions;
use crate::trace::Trace;
use crate::waypoint::Waypoint;

#[cfg_attr(feature = "wasm", derive(serde::Serialize))]
pub struct SectionStats {
    pub section_id: usize,
    /// Index of the stage this section belongs to.
    pub stage_idx: usize,
    pub start_index: usize,
    pub end_index: usize,
    pub point_count: usize,
    pub start_location: String,
    pub end_location: String,
    pub total_distance_km: f64,
    pub total_elevation_gain_m: f64,
    pub total_elevation_loss_m: f64,
    pub avg_slope: f64,
    pub max_slope: f64,
    pub min_elevation: f64,
    pub max_elevation: f64,
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
    pub bearing: f64,
    /// 1–5 based on average Minetti pace factor (1→<1.1, 2→<1.4, 3→<1.8, 4→<2.5, 5→≥2.5).
    pub difficulty: u8,
    /// Moving time + planned stop at end checkpoint (seconds).
    pub estimated_duration_s: f64,
    pub pace_factor: f64,
    /// Allowed completion time in seconds (end_time − start_time), or `None` if absent.
    pub max_completion_time: Option<i64>,
    /// estimated_duration / max_completion_time; `None` if no cutoff; >1.0 means cutoff missed.
    pub cutoff_ratio: Option<f64>,
    /// Planned stop at the end checkpoint (seconds), from `<stopDuration>` or `None`.
    pub stop_duration: Option<u32>,
}

impl From<IntervalMetrics> for SectionStats {
    fn from(m: IntervalMetrics) -> Self {
        Self {
            section_id: m.id,
            stage_idx: m.stage_idx,
            start_index: m.start_index,
            end_index: m.end_index,
            point_count: m.point_count,
            start_location: m.start_location,
            end_location: m.end_location,
            total_distance_km: m.total_distance_km,
            total_elevation_gain_m: m.total_elevation_gain_m,
            total_elevation_loss_m: m.total_elevation_loss_m,
            avg_slope: m.avg_slope,
            max_slope: m.max_slope,
            min_elevation: m.min_elevation,
            max_elevation: m.max_elevation,
            start_time: m.start_time,
            end_time: m.end_time,
            bearing: m.bearing,
            difficulty: m.difficulty,
            estimated_duration_s: m.estimated_duration_s,
            pace_factor: m.pace_factor,
            max_completion_time: m.max_completion_time,
            cutoff_ratio: m.cutoff_ratio,
            stop_duration: m.stop_duration,
        }
    }
}

/// Compute section statistics between consecutive section-boundary waypoints
/// (Start / TimeBarrier / LifeBase / Arrival).
///
/// Returns `None` when fewer than 2 section boundaries exist.
pub fn compute_from_waypoints(
    trace: &Trace,
    waypoints: &[Waypoint],
    options: &AnalysisOptions,
) -> Option<Vec<SectionStats>> {
    let intervals = compute_intervals(
        trace,
        waypoints,
        options,
        Waypoint::is_section_boundary,
        true,
    )?;
    Some(intervals.into_iter().map(SectionStats::from).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::location::Location;
    use crate::pace_model::AnalysisOptions;

    fn make_trace(n: usize) -> Trace {
        let locs: Vec<Location> = (0..n)
            .map(|i| Location {
                longitude: 0.0,
                latitude: i as f64 * 0.001,
                altitude: 100.0 + i as f64 * 5.0,
            })
            .collect();
        Trace::new(&locs).unwrap()
    }

    fn make_waypoint(lat: f64, name: &str, wpt_type: Option<&str>, time: Option<i64>) -> Waypoint {
        Waypoint {
            latitude: lat,
            longitude: 0.0,
            elevation: None,
            name: name.to_string(),
            description: None,
            comment: None,
            symbol: None,
            wpt_type: wpt_type.map(String::from),
            time,
            stop_duration: None,
        }
    }

    #[test]
    fn returns_none_with_no_section_boundaries() {
        let trace = make_trace(4);
        let waypoints = vec![
            make_waypoint(0.0, "A", None, None),
            make_waypoint(0.001, "B", None, None),
        ];
        assert!(compute_from_waypoints(&trace, &waypoints, &AnalysisOptions::default()).is_none());
    }

    #[test]
    fn basic_two_boundary_section() {
        let trace = make_trace(6);
        let waypoints = vec![
            make_waypoint(0.0, "Start", Some("Start"), None),
            make_waypoint(0.005, "TB1", Some("TimeBarrier"), None),
        ];
        let sections =
            compute_from_waypoints(&trace, &waypoints, &AnalysisOptions::default()).unwrap();
        assert_eq!(sections.len(), 1);
        assert!(sections[0].total_distance_km > 0.0);
        assert!(sections[0].point_count > 0);
    }

    #[test]
    fn plain_waypoints_are_ignored() {
        let trace = make_trace(8);
        let waypoints = vec![
            make_waypoint(0.0, "Start", Some("Start"), None),
            make_waypoint(0.001, "Plain1", None, None),
            make_waypoint(0.003, "TB1", Some("TimeBarrier"), None),
            make_waypoint(0.004, "Plain2", None, None),
            make_waypoint(0.007, "End", Some("Arrival"), None),
        ];
        let sections =
            compute_from_waypoints(&trace, &waypoints, &AnalysisOptions::default()).unwrap();
        assert_eq!(sections.len(), 2);
    }

    #[test]
    fn stage_idx_increments_at_life_base() {
        let locs: Vec<Location> = (0..13)
            .map(|i| Location {
                longitude: 0.0,
                latitude: i as f64 * 0.001,
                altitude: 100.0,
            })
            .collect();
        let trace = Trace::new(&locs).unwrap();
        let waypoints = vec![
            make_waypoint(0.0, "Start", Some("Start"), None),
            make_waypoint(0.003, "TB1", Some("TimeBarrier"), None),
            make_waypoint(0.006, "LB1", Some("LifeBase"), None),
            make_waypoint(0.009, "TB2", Some("TimeBarrier"), None),
            make_waypoint(0.011, "Arrival", Some("Arrival"), None),
        ];
        let sections =
            compute_from_waypoints(&trace, &waypoints, &AnalysisOptions::default()).unwrap();
        assert_eq!(sections.len(), 4);
        assert_eq!(sections[0].stage_idx, 0);
        assert_eq!(sections[1].stage_idx, 0);
        assert_eq!(sections[2].stage_idx, 1);
        assert_eq!(sections[3].stage_idx, 1);
    }

    #[test]
    fn max_completion_time_from_timestamps() {
        let trace = make_trace(6);
        let waypoints = vec![
            make_waypoint(0.0, "Start", Some("Start"), Some(1_000_000)),
            make_waypoint(0.005, "End", Some("TimeBarrier"), Some(1_007_200)),
        ];
        let sections =
            compute_from_waypoints(&trace, &waypoints, &AnalysisOptions::default()).unwrap();
        assert_eq!(sections[0].max_completion_time, Some(7200));
        assert_eq!(sections[0].start_time, Some(1_000_000));
        assert_eq!(sections[0].end_time, Some(1_007_200));
    }

    #[test]
    fn max_completion_time_null_without_timestamps() {
        let trace = make_trace(4);
        let waypoints = vec![
            make_waypoint(0.0, "Start", Some("Start"), None),
            make_waypoint(0.002, "End", Some("Arrival"), None),
        ];
        let sections =
            compute_from_waypoints(&trace, &waypoints, &AnalysisOptions::default()).unwrap();
        assert_eq!(sections[0].max_completion_time, None);
        assert_eq!(sections[0].cutoff_ratio, None);
    }

    #[test]
    fn life_base_recovery_reduces_fatigue() {
        let locs: Vec<Location> = (0..10)
            .map(|i| Location {
                longitude: 0.0,
                latitude: i as f64 * 0.001,
                altitude: 100.0,
            })
            .collect();
        let trace = Trace::new(&locs).unwrap();
        let wpts_lb = vec![
            make_waypoint(0.0, "Start", Some("Start"), None),
            make_waypoint(0.004, "LB", Some("LifeBase"), None),
            make_waypoint(0.009, "Arrival", Some("Arrival"), None),
        ];
        let wpts_tb = vec![
            make_waypoint(0.0, "Start", Some("Start"), None),
            make_waypoint(0.004, "TB", Some("TimeBarrier"), None),
            make_waypoint(0.009, "Arrival", Some("Arrival"), None),
        ];
        let sections_lb = compute_from_waypoints(
            &trace,
            &wpts_lb,
            &AnalysisOptions::default().life_base_stop(0),
        )
        .unwrap();
        let sections_tb = compute_from_waypoints(
            &trace,
            &wpts_tb,
            &AnalysisOptions::default().life_base_stop(0),
        )
        .unwrap();
        assert!(
            (sections_lb[0].estimated_duration_s - sections_tb[0].estimated_duration_s).abs()
                < 1e-6
        );
        assert!(sections_lb[1].estimated_duration_s < sections_tb[1].estimated_duration_s);
    }

    #[test]
    fn night_section_slower_than_day() {
        let locs: Vec<Location> = (0..5)
            .map(|i| Location {
                longitude: 0.0,
                latitude: i as f64 * 0.001,
                altitude: 100.0,
            })
            .collect();
        let trace = Trace::new(&locs).unwrap();
        let noon: i64 = 12 * 3600;
        let night: i64 = 3 * 3600 + 30 * 60;
        let wpts_day = vec![
            make_waypoint(0.0, "Start", Some("Start"), Some(noon)),
            make_waypoint(0.004, "Arrival", Some("Arrival"), None),
        ];
        let wpts_night = vec![
            make_waypoint(0.0, "Start", Some("Start"), Some(night)),
            make_waypoint(0.004, "Arrival", Some("Arrival"), None),
        ];
        let day = compute_from_waypoints(
            &trace,
            &wpts_day,
            &AnalysisOptions::default().life_base_stop(0),
        )
        .unwrap();
        let night_s = compute_from_waypoints(
            &trace,
            &wpts_night,
            &AnalysisOptions::default().life_base_stop(0),
        )
        .unwrap();
        assert!(night_s[0].estimated_duration_s > day[0].estimated_duration_s);
    }
}
