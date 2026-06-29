//! Shared computation template for section and stage analysis.
//!
//! Both `section::compute_from_waypoints` and `stage::compute_from_waypoints`
//! follow the same pattern: filter waypoints to boundaries, iterate pairwise,
//! resolve trace indices, accumulate physiological metrics, compute derived stats.
//! This module captures the invariant core.

use crate::location::Location;
use crate::pace_model::{AnalysisOptions, RECOVERY_LIFE_BASE};
use crate::segment::{self, SegmentParams, SegmentState};
use crate::trace::Trace;
use crate::waypoint::Waypoint;

/// Raw metrics computed for one interval (section or stage).
/// The caller maps these into its own typed struct (SectionStats / StageStats).
pub(crate) struct IntervalMetrics {
    pub id: usize,
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
    pub difficulty: u8,
    pub estimated_duration_s: f64,
    pub pace_factor: f64,
    pub max_completion_time: Option<i64>,
    pub cutoff_ratio: Option<f64>,
    pub stop_duration: Option<u32>,
    /// Index of the stage this interval belongs to (only meaningful for sections).
    pub stage_idx: usize,
}

/// Compute interval metrics between consecutive boundary waypoints.
///
/// `boundary_filter` selects which waypoints count as interval boundaries.
/// `track_stage_idx` enables stage-index tracking (used by sections to know
/// which stage each section belongs to).
pub(crate) fn compute_intervals(
    trace: &Trace,
    waypoints: &[Waypoint],
    options: &AnalysisOptions,
    boundary_filter: fn(&Waypoint) -> bool,
    track_stage_idx: bool,
) -> Option<Vec<IntervalMetrics>> {
    let base_pace_s_per_km = options.base_pace_s_per_km;
    let k_fatigue = options.k_fatigue;
    let life_base_stop_s = options.life_base_stop_s;
    let weather = &options.weather;

    let boundary_wpts: Vec<&Waypoint> = waypoints.iter().filter(|w| boundary_filter(w)).collect();
    if boundary_wpts.len() < 2 {
        return None;
    }

    let mut results = Vec::with_capacity(boundary_wpts.len() - 1);
    let mut search_start = 0usize;
    let clock_start = boundary_wpts[0].time;
    let mut state = SegmentState {
        d_eff_m: 0.0,
        elapsed_s: 0.0,
    };
    let mut current_stage_idx = 0usize;
    let mut stage_active = false;

    for i in 0..boundary_wpts.len() - 1 {
        // Track stage index for section-level analysis.
        if track_stage_idx && boundary_wpts[i].is_stage_boundary() {
            if stage_active {
                current_stage_idx += 1;
            } else {
                stage_active = true;
            }
        }

        let start_wpt = boundary_wpts[i];
        let end_wpt = boundary_wpts[i + 1];

        let start_target = Location {
            longitude: start_wpt.longitude,
            latitude: start_wpt.latitude,
            altitude: 0.0,
        };
        let end_target = Location {
            longitude: end_wpt.longitude,
            latitude: end_wpt.latitude,
            altitude: 0.0,
        };

        let start_index = match trace.find_closest_point_from(&start_target, search_start) {
            Some((_, idx, _)) => idx,
            None => continue,
        };
        let end_index = match trace.find_closest_point_from(&end_target, start_index + 1) {
            Some((_, idx, _)) => idx,
            None => continue,
        };
        search_start = end_index;

        if start_index >= end_index {
            continue;
        }

        let bearing = start_target.calculate_bearing_to(&end_target);
        let dist_km =
            trace.cumulative_distances[end_index] - trace.cumulative_distances[start_index];
        let elevation_gain_m = trace.cumulative_elevation_gains[end_index]
            - trace.cumulative_elevation_gains[start_index];
        let elevation_loss_m = trace.cumulative_elevation_losses[end_index]
            - trace.cumulative_elevation_losses[start_index];

        let interval_weather = weather.find(&end_wpt.name);
        let params = SegmentParams {
            base_pace_s_per_km,
            k_fatigue,
            clock_start,
            weather: interval_weather,
        };
        let metrics = segment::compute(trace, start_index, end_index, &params, &mut state);

        if end_wpt.wpt_type.as_deref() == Some("LifeBase") {
            state.d_eff_m *= 1.0 - RECOVERY_LIFE_BASE;
        }

        let avg_slope = if dist_km > 0.0 {
            ((elevation_gain_m - elevation_loss_m) / (dist_km * 1000.0)) * 100.0
        } else {
            0.0
        };
        let avg_pf = if dist_km > 0.0 {
            metrics.total_weighted_dist_km / dist_km
        } else {
            1.0
        };
        let stop_secs: f64 = if let Some(sd) = end_wpt.stop_duration {
            sd as f64
        } else if end_wpt.wpt_type.as_deref() == Some("LifeBase") {
            life_base_stop_s as f64
        } else {
            0.0
        };
        let estimated_duration_s = metrics.total_time + stop_secs;
        let difficulty: u8 = if avg_pf < 1.1 {
            1
        } else if avg_pf < 1.4 {
            2
        } else if avg_pf < 1.8 {
            3
        } else if avg_pf < 2.5 {
            4
        } else {
            5
        };
        let max_completion_time = match (start_wpt.time, end_wpt.time) {
            (Some(t0), Some(t1)) => Some(t1 - t0),
            _ => None,
        };
        let cutoff_ratio = max_completion_time.and_then(|mct| {
            if mct <= 0 {
                None
            } else {
                Some(estimated_duration_s / mct as f64)
            }
        });

        results.push(IntervalMetrics {
            id: i,
            start_index,
            end_index,
            point_count: end_index - start_index + 1,
            start_location: start_wpt.name.clone(),
            end_location: end_wpt.name.clone(),
            total_distance_km: dist_km,
            total_elevation_gain_m: elevation_gain_m,
            total_elevation_loss_m: elevation_loss_m,
            avg_slope,
            max_slope: metrics.max_slope,
            min_elevation: metrics.min_elevation,
            max_elevation: metrics.max_elevation,
            start_time: start_wpt.time,
            end_time: end_wpt.time,
            bearing,
            difficulty,
            estimated_duration_s,
            pace_factor: avg_pf,
            max_completion_time,
            cutoff_ratio,
            stop_duration: end_wpt.stop_duration,
            stage_idx: current_stage_idx,
        });
    }

    Some(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::location::Location;
    use crate::pace_model::AnalysisOptions;

    fn make_trace(locs: &[Location]) -> Trace {
        Trace::new(locs).unwrap()
    }

    fn flat_locs(n: usize) -> Vec<Location> {
        (0..n)
            .map(|i| Location {
                longitude: 0.0,
                latitude: i as f64 * 0.001,
                altitude: 100.0,
            })
            .collect()
    }

    fn make_waypoint(
        lat: f64,
        name: &str,
        wpt_type: Option<&str>,
        time: Option<i64>,
        stop_duration: Option<u32>,
    ) -> Waypoint {
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
            stop_duration,
        }
    }

    #[test]
    fn returns_none_with_fewer_than_two_boundaries() {
        let trace = make_trace(&flat_locs(5));
        let wpts = vec![make_waypoint(0.0, "Start", Some("Start"), None, None)];
        assert!(compute_intervals(
            &trace,
            &wpts,
            &AnalysisOptions::default(),
            Waypoint::is_section_boundary,
            false
        )
        .is_none());
    }

    #[test]
    fn returns_none_with_empty_waypoints() {
        let trace = make_trace(&flat_locs(5));
        let wpts: Vec<Waypoint> = vec![];
        assert!(compute_intervals(
            &trace,
            &wpts,
            &AnalysisOptions::default(),
            Waypoint::is_section_boundary,
            false
        )
        .is_none());
    }

    #[test]
    fn basic_interval_has_positive_metrics() {
        let trace = make_trace(&flat_locs(10));
        let wpts = vec![
            make_waypoint(0.0, "Start", Some("Start"), None, None),
            make_waypoint(0.009, "End", Some("Arrival"), None, None),
        ];
        let intervals = compute_intervals(
            &trace,
            &wpts,
            &AnalysisOptions::default(),
            Waypoint::is_section_boundary,
            false,
        )
        .unwrap();
        assert_eq!(intervals.len(), 1);
        assert!(intervals[0].total_distance_km > 0.0);
        assert!(intervals[0].estimated_duration_s > 0.0);
        assert_eq!(intervals[0].id, 0);
        assert_eq!(intervals[0].start_location, "Start");
        assert_eq!(intervals[0].end_location, "End");
    }

    #[test]
    fn stop_duration_from_waypoint() {
        let trace = make_trace(&flat_locs(10));
        let wpts = vec![
            make_waypoint(0.0, "Start", Some("Start"), None, None),
            make_waypoint(0.009, "End", Some("TimeBarrier"), None, Some(600)),
        ];
        let intervals = compute_intervals(
            &trace,
            &wpts,
            &AnalysisOptions::default(),
            Waypoint::is_section_boundary,
            false,
        )
        .unwrap();
        assert_eq!(intervals[0].stop_duration, Some(600));
        // Duration includes the 600s stop
        assert!(intervals[0].estimated_duration_s > 600.0);
    }

    #[test]
    fn life_base_stop_from_options() {
        let trace = make_trace(&flat_locs(10));
        let wpts = vec![
            make_waypoint(0.0, "Start", Some("Start"), None, None),
            make_waypoint(0.009, "End", Some("LifeBase"), None, None),
        ];
        let opts = AnalysisOptions::default().life_base_stop(1800);
        let intervals =
            compute_intervals(&trace, &wpts, &opts, Waypoint::is_section_boundary, false).unwrap();
        // Duration includes the 1800s default life base stop
        assert!(intervals[0].estimated_duration_s > 1800.0);
    }

    #[test]
    fn max_completion_time_computed_from_timestamps() {
        let trace = make_trace(&flat_locs(10));
        let wpts = vec![
            make_waypoint(0.0, "Start", Some("Start"), Some(1000), None),
            make_waypoint(0.009, "End", Some("Arrival"), Some(5000), None),
        ];
        let intervals = compute_intervals(
            &trace,
            &wpts,
            &AnalysisOptions::default(),
            Waypoint::is_section_boundary,
            false,
        )
        .unwrap();
        assert_eq!(intervals[0].max_completion_time, Some(4000));
        assert!(intervals[0].cutoff_ratio.is_some());
    }

    #[test]
    fn zero_max_completion_time_yields_none_cutoff() {
        let trace = make_trace(&flat_locs(10));
        let wpts = vec![
            make_waypoint(0.0, "Start", Some("Start"), Some(1000), None),
            make_waypoint(0.009, "End", Some("Arrival"), Some(1000), None),
        ];
        let intervals = compute_intervals(
            &trace,
            &wpts,
            &AnalysisOptions::default(),
            Waypoint::is_section_boundary,
            false,
        )
        .unwrap();
        assert_eq!(intervals[0].max_completion_time, Some(0));
        assert!(intervals[0].cutoff_ratio.is_none());
    }

    #[test]
    fn stage_idx_tracking_enabled() {
        let trace = make_trace(&flat_locs(15));
        let wpts = vec![
            make_waypoint(0.0, "Start", Some("Start"), None, None),
            make_waypoint(0.004, "TB", Some("TimeBarrier"), None, None),
            make_waypoint(0.008, "LB", Some("LifeBase"), None, None),
            make_waypoint(0.012, "End", Some("Arrival"), None, None),
        ];
        let intervals = compute_intervals(
            &trace,
            &wpts,
            &AnalysisOptions::default().life_base_stop(0),
            Waypoint::is_section_boundary,
            true,
        )
        .unwrap();
        assert_eq!(intervals.len(), 3);
        assert_eq!(intervals[0].stage_idx, 0);
        assert_eq!(intervals[1].stage_idx, 0);
        assert_eq!(intervals[2].stage_idx, 1);
    }

    #[test]
    fn stage_idx_tracking_disabled() {
        let trace = make_trace(&flat_locs(15));
        let wpts = vec![
            make_waypoint(0.0, "Start", Some("Start"), None, None),
            make_waypoint(0.008, "LB", Some("LifeBase"), None, None),
            make_waypoint(0.012, "End", Some("Arrival"), None, None),
        ];
        let intervals = compute_intervals(
            &trace,
            &wpts,
            &AnalysisOptions::default().life_base_stop(0),
            Waypoint::is_stage_boundary,
            false,
        )
        .unwrap();
        assert_eq!(intervals.len(), 2);
        // Without tracking, all stage_idx remain 0
        assert_eq!(intervals[0].stage_idx, 0);
        assert_eq!(intervals[1].stage_idx, 0);
    }

    #[test]
    fn difficulty_level_one_on_flat() {
        let trace = make_trace(&flat_locs(10));
        let wpts = vec![
            make_waypoint(0.0, "Start", Some("Start"), None, None),
            make_waypoint(0.009, "End", Some("Arrival"), None, None),
        ];
        let intervals = compute_intervals(
            &trace,
            &wpts,
            &AnalysisOptions::default(),
            Waypoint::is_section_boundary,
            false,
        )
        .unwrap();
        assert_eq!(intervals[0].difficulty, 1);
    }

    #[test]
    fn difficulty_increases_with_slope() {
        // Steep uphill: should produce higher difficulty
        let locs: Vec<Location> = (0..10)
            .map(|i| Location {
                longitude: 0.0,
                latitude: i as f64 * 0.001,
                altitude: i as f64 * 80.0, // Very steep
            })
            .collect();
        let trace = make_trace(&locs);
        let wpts = vec![
            make_waypoint(0.0, "Start", Some("Start"), None, None),
            make_waypoint(0.009, "End", Some("Arrival"), None, None),
        ];
        let intervals = compute_intervals(
            &trace,
            &wpts,
            &AnalysisOptions::default(),
            Waypoint::is_section_boundary,
            false,
        )
        .unwrap();
        assert!(intervals[0].difficulty > 1);
        assert!(intervals[0].pace_factor > 1.1);
    }

    #[test]
    fn bearing_is_computed() {
        let trace = make_trace(&flat_locs(10));
        let wpts = vec![
            make_waypoint(0.0, "Start", Some("Start"), None, None),
            make_waypoint(0.009, "End", Some("Arrival"), None, None),
        ];
        let intervals = compute_intervals(
            &trace,
            &wpts,
            &AnalysisOptions::default(),
            Waypoint::is_section_boundary,
            false,
        )
        .unwrap();
        // Northbound trace → bearing should be ~0 degrees (north)
        assert!(intervals[0].bearing.abs() < 1.0 || (intervals[0].bearing - 360.0).abs() < 1.0);
    }
}
