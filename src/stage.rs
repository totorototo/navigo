use crate::interval::{compute_intervals, IntervalMetrics};
use crate::pace_model::AnalysisOptions;
use crate::trace::Trace;
use crate::waypoint::Waypoint;

#[cfg_attr(feature = "wasm", derive(serde::Serialize))]
pub struct StageStats {
    pub stage_id: usize,
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
    /// 1–5 based on average Minetti pace factor.
    pub difficulty: u8,
    /// Moving time + planned stop at end checkpoint (seconds).
    pub estimated_duration_s: f64,
    pub pace_factor: f64,
    pub max_completion_time: Option<i64>,
    pub cutoff_ratio: Option<f64>,
    pub stop_duration: Option<u32>,
}

impl From<IntervalMetrics> for StageStats {
    fn from(m: IntervalMetrics) -> Self {
        Self {
            stage_id: m.id,
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

/// Compute stage statistics between consecutive stage-boundary waypoints
/// (Start / LifeBase / Arrival). TimeBarrier waypoints are skipped — they only
/// appear in sections, not stages.
///
/// Returns `None` when fewer than 2 stage boundaries exist.
pub fn compute_from_waypoints(
    trace: &Trace,
    waypoints: &[Waypoint],
    options: &AnalysisOptions,
) -> Option<Vec<StageStats>> {
    let intervals = compute_intervals(
        trace,
        waypoints,
        options,
        Waypoint::is_stage_boundary,
        false,
    )?;
    Some(intervals.into_iter().map(StageStats::from).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::location::Location;

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
    fn returns_none_with_fewer_than_two_stage_boundaries() {
        let trace = make_trace(4);
        let no_boundaries = vec![
            make_waypoint(0.0, "A", None, None),
            make_waypoint(0.001, "B", None, None),
        ];
        assert!(
            compute_from_waypoints(&trace, &no_boundaries, &AnalysisOptions::default()).is_none()
        );

        let one_boundary = vec![
            make_waypoint(0.0, "Start", Some("Start"), None),
            make_waypoint(0.001, "Plain", None, None),
        ];
        assert!(
            compute_from_waypoints(&trace, &one_boundary, &AnalysisOptions::default()).is_none()
        );
    }

    #[test]
    fn time_barriers_excluded_from_stages() {
        let trace = make_trace(8);
        let waypoints = vec![
            make_waypoint(0.0, "Start", Some("Start"), None),
            make_waypoint(0.003, "TB1", Some("TimeBarrier"), None),
            make_waypoint(0.007, "Arrival", Some("Arrival"), None),
        ];
        let stages =
            compute_from_waypoints(&trace, &waypoints, &AnalysisOptions::default()).unwrap();
        assert_eq!(stages.len(), 1);
        assert!(stages[0].total_distance_km > 0.0);
    }

    #[test]
    fn start_lifebase_arrival_two_stages() {
        let locs: Vec<Location> = (0..10)
            .map(|i| Location {
                longitude: 0.0,
                latitude: i as f64 * 0.001,
                altitude: 100.0 + i as f64 * 50.0,
            })
            .collect();
        let trace = Trace::new(&locs).unwrap();
        let waypoints = vec![
            make_waypoint(0.0, "Start", Some("Start"), None),
            make_waypoint(0.005, "LifeBase", Some("LifeBase"), None),
            make_waypoint(0.009, "Arrival", Some("Arrival"), None),
        ];
        let stages =
            compute_from_waypoints(&trace, &waypoints, &AnalysisOptions::default()).unwrap();
        assert_eq!(stages.len(), 2);
        assert_eq!(stages[0].stage_id, 0);
        assert_eq!(stages[1].stage_id, 1);
        assert!(stages[0].total_distance_km > 0.0);
        assert!(stages[1].total_distance_km > 0.0);
        assert!(stages[0].end_index <= stages[1].start_index);
    }

    #[test]
    fn max_completion_time_from_timestamps() {
        let trace = make_trace(6);
        let waypoints = vec![
            make_waypoint(0.0, "Start", Some("Start"), Some(1_000_000)),
            make_waypoint(0.005, "End", Some("Arrival"), Some(1_003_600)),
        ];
        let stages =
            compute_from_waypoints(&trace, &waypoints, &AnalysisOptions::default()).unwrap();
        assert_eq!(stages[0].max_completion_time, Some(3600));
        assert!(stages[0].cutoff_ratio.unwrap() < 1.0);
    }

    #[test]
    fn stop_duration_adds_to_estimated_time() {
        let trace = make_trace(10);
        let stop_s: u32 = 3600;

        let without_stop = {
            let waypoints = vec![
                make_waypoint(0.0, "Start", Some("Start"), None),
                make_waypoint(0.005, "LB", Some("LifeBase"), None),
                make_waypoint(0.009, "Arrival", Some("Arrival"), None),
            ];
            compute_from_waypoints(
                &trace,
                &waypoints,
                &AnalysisOptions::default().life_base_stop(0),
            )
            .unwrap()
        };

        let with_stop = {
            let mut lb = make_waypoint(0.005, "LB", Some("LifeBase"), None);
            lb.stop_duration = Some(stop_s);
            let waypoints = vec![
                make_waypoint(0.0, "Start", Some("Start"), None),
                lb,
                make_waypoint(0.009, "Arrival", Some("Arrival"), None),
            ];
            compute_from_waypoints(
                &trace,
                &waypoints,
                &AnalysisOptions::default().life_base_stop(0),
            )
            .unwrap()
        };

        // Stage 0 ends at LB — should include the stop.
        let diff = with_stop[0].estimated_duration_s - without_stop[0].estimated_duration_s;
        assert!((diff - stop_s as f64).abs() < 1.0);
        // Stage 1 is unaffected by LB stop.
        assert!(
            (with_stop[1].estimated_duration_s - without_stop[1].estimated_duration_s).abs() < 1.0
        );
    }
}
