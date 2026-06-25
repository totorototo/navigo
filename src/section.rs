use crate::location::Location;
use crate::pace_model::{WeatherLookup, RECOVERY_LIFE_BASE};
use crate::segment;
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

/// Compute section statistics between consecutive section-boundary waypoints
/// (Start / TimeBarrier / LifeBase / Arrival).
///
/// Returns `None` when fewer than 2 section boundaries exist.
pub fn compute_from_waypoints(
    trace: &Trace,
    waypoints: &[Waypoint],
    base_pace_s_per_km: f64,
    k_fatigue: f64,
    life_base_stop_s: u32,
    weather: &WeatherLookup,
) -> Option<Vec<SectionStats>> {
    let section_wpts: Vec<&Waypoint> =
        waypoints.iter().filter(|w| w.is_section_boundary()).collect();
    if section_wpts.len() < 2 {
        return None;
    }

    let mut sections = Vec::new();
    let mut search_start = 0usize;
    let mut current_stage_idx = 0usize;
    let mut stage_active = false;
    let mut d_eff_m = 0.0_f64;
    let clock_start = section_wpts[0].time;
    let mut elapsed_s = 0.0_f64;

    for i in 0..section_wpts.len() - 1 {
        if section_wpts[i].is_stage_boundary() {
            if stage_active {
                current_stage_idx += 1;
            } else {
                stage_active = true;
            }
        }

        let start_wpt = section_wpts[i];
        let end_wpt = section_wpts[i + 1];

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
        let dist_km = trace.cumulative_distances[end_index]
            - trace.cumulative_distances[start_index];
        let elevation_gain_m = trace.cumulative_elevation_gains[end_index]
            - trace.cumulative_elevation_gains[start_index];
        let elevation_loss_m = trace.cumulative_elevation_losses[end_index]
            - trace.cumulative_elevation_losses[start_index];

        let section_weather = weather.find(&end_wpt.name);
        let metrics = segment::compute(
            trace,
            start_index,
            end_index,
            base_pace_s_per_km,
            k_fatigue,
            clock_start,
            section_weather,
            &mut d_eff_m,
            &mut elapsed_s,
        );

        if end_wpt.wpt_type.as_deref() == Some("LifeBase") {
            d_eff_m *= 1.0 - RECOVERY_LIFE_BASE;
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

        sections.push(SectionStats {
            section_id: i,
            stage_idx: current_stage_idx,
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
        });
    }

    Some(sections)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pace_model::{DEFAULT_BASE_PACE_S_PER_KM, K_FATIGUE, DEFAULT_LIFE_BASE_STOP_S};

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

    fn empty_weather() -> WeatherLookup {
        WeatherLookup::empty()
    }

    #[test]
    fn returns_none_with_no_section_boundaries() {
        let trace = make_trace(4);
        let waypoints = vec![
            make_waypoint(0.0, "A", None, None),
            make_waypoint(0.001, "B", None, None),
        ];
        assert!(compute_from_waypoints(
            &trace,
            &waypoints,
            DEFAULT_BASE_PACE_S_PER_KM,
            K_FATIGUE,
            DEFAULT_LIFE_BASE_STOP_S,
            &empty_weather()
        )
        .is_none());
    }

    #[test]
    fn returns_none_with_only_one_boundary() {
        let trace = make_trace(4);
        let waypoints = vec![
            make_waypoint(0.0, "Start", Some("Start"), None),
            make_waypoint(0.001, "Plain", None, None),
        ];
        assert!(compute_from_waypoints(
            &trace,
            &waypoints,
            DEFAULT_BASE_PACE_S_PER_KM,
            K_FATIGUE,
            DEFAULT_LIFE_BASE_STOP_S,
            &empty_weather()
        )
        .is_none());
    }

    #[test]
    fn basic_two_boundary_section() {
        let trace = make_trace(6);
        let waypoints = vec![
            make_waypoint(0.0, "Start", Some("Start"), None),
            make_waypoint(0.005, "TB1", Some("TimeBarrier"), None),
        ];
        let sections = compute_from_waypoints(
            &trace,
            &waypoints,
            DEFAULT_BASE_PACE_S_PER_KM,
            K_FATIGUE,
            DEFAULT_LIFE_BASE_STOP_S,
            &empty_weather(),
        )
        .unwrap();
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
        let sections = compute_from_waypoints(
            &trace,
            &waypoints,
            DEFAULT_BASE_PACE_S_PER_KM,
            K_FATIGUE,
            DEFAULT_LIFE_BASE_STOP_S,
            &empty_weather(),
        )
        .unwrap();
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
        let sections = compute_from_waypoints(
            &trace,
            &waypoints,
            DEFAULT_BASE_PACE_S_PER_KM,
            K_FATIGUE,
            DEFAULT_LIFE_BASE_STOP_S,
            &empty_weather(),
        )
        .unwrap();
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
        let sections = compute_from_waypoints(
            &trace,
            &waypoints,
            DEFAULT_BASE_PACE_S_PER_KM,
            K_FATIGUE,
            DEFAULT_LIFE_BASE_STOP_S,
            &empty_weather(),
        )
        .unwrap();
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
        let sections = compute_from_waypoints(
            &trace,
            &waypoints,
            DEFAULT_BASE_PACE_S_PER_KM,
            K_FATIGUE,
            DEFAULT_LIFE_BASE_STOP_S,
            &empty_weather(),
        )
        .unwrap();
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
            &trace, &wpts_lb, DEFAULT_BASE_PACE_S_PER_KM, K_FATIGUE, 0, &empty_weather(),
        )
        .unwrap();
        let sections_tb = compute_from_waypoints(
            &trace, &wpts_tb, DEFAULT_BASE_PACE_S_PER_KM, K_FATIGUE, 0, &empty_weather(),
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
            &trace, &wpts_day, DEFAULT_BASE_PACE_S_PER_KM, K_FATIGUE, 0, &empty_weather(),
        )
        .unwrap();
        let night_s = compute_from_waypoints(
            &trace, &wpts_night, DEFAULT_BASE_PACE_S_PER_KM, K_FATIGUE, 0, &empty_weather(),
        )
        .unwrap();
        assert!(night_s[0].estimated_duration_s > day[0].estimated_duration_s);
    }
}


