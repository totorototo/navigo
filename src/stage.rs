use crate::location::Location;
use crate::pace_model::{WeatherLookup, RECOVERY_LIFE_BASE};
use crate::segment;
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

/// Compute stage statistics between consecutive stage-boundary waypoints
/// (Start / LifeBase / Arrival). TimeBarrier waypoints are skipped — they only
/// appear in sections, not stages.
///
/// Returns `None` when fewer than 2 stage boundaries exist.
pub fn compute_from_waypoints(
    trace: &Trace,
    waypoints: &[Waypoint],
    base_pace_s_per_km: f64,
    k_fatigue: f64,
    life_base_stop_s: u32,
    weather: &WeatherLookup,
) -> Option<Vec<StageStats>> {
    let stage_wpts: Vec<&Waypoint> =
        waypoints.iter().filter(|w| w.is_stage_boundary()).collect();
    if stage_wpts.len() < 2 {
        return None;
    }

    let mut stages = Vec::new();
    let mut search_start = 0usize;
    let mut d_eff_m = 0.0_f64;
    let clock_start = stage_wpts[0].time;
    let mut elapsed_s = 0.0_f64;

    for i in 0..stage_wpts.len() - 1 {
        let start_wpt = stage_wpts[i];
        let end_wpt = stage_wpts[i + 1];

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

        let stage_weather = weather.find(&end_wpt.name);
        let metrics = segment::compute(
            trace,
            start_index,
            end_index,
            base_pace_s_per_km,
            k_fatigue,
            clock_start,
            stage_weather,
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

        stages.push(StageStats {
            stage_id: i,
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

    Some(stages)
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

    #[test]
    fn returns_none_with_fewer_than_two_stage_boundaries() {
        let trace = make_trace(4);
        let no_boundaries = vec![
            make_waypoint(0.0, "A", None, None),
            make_waypoint(0.001, "B", None, None),
        ];
        assert!(compute_from_waypoints(
            &trace,
            &no_boundaries,
            DEFAULT_BASE_PACE_S_PER_KM,
            K_FATIGUE,
            DEFAULT_LIFE_BASE_STOP_S,
            &WeatherLookup::empty()
        )
        .is_none());

        let one_boundary = vec![
            make_waypoint(0.0, "Start", Some("Start"), None),
            make_waypoint(0.001, "Plain", None, None),
        ];
        assert!(compute_from_waypoints(
            &trace,
            &one_boundary,
            DEFAULT_BASE_PACE_S_PER_KM,
            K_FATIGUE,
            DEFAULT_LIFE_BASE_STOP_S,
            &WeatherLookup::empty()
        )
        .is_none());
    }

    #[test]
    fn time_barriers_excluded_from_stages() {
        let trace = make_trace(8);
        let waypoints = vec![
            make_waypoint(0.0, "Start", Some("Start"), None),
            make_waypoint(0.003, "TB1", Some("TimeBarrier"), None),
            make_waypoint(0.007, "Arrival", Some("Arrival"), None),
        ];
        let stages = compute_from_waypoints(
            &trace,
            &waypoints,
            DEFAULT_BASE_PACE_S_PER_KM,
            K_FATIGUE,
            DEFAULT_LIFE_BASE_STOP_S,
            &WeatherLookup::empty(),
        )
        .unwrap();
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
        let stages = compute_from_waypoints(
            &trace,
            &waypoints,
            DEFAULT_BASE_PACE_S_PER_KM,
            K_FATIGUE,
            DEFAULT_LIFE_BASE_STOP_S,
            &WeatherLookup::empty(),
        )
        .unwrap();
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
        let stages = compute_from_waypoints(
            &trace,
            &waypoints,
            DEFAULT_BASE_PACE_S_PER_KM,
            K_FATIGUE,
            DEFAULT_LIFE_BASE_STOP_S,
            &WeatherLookup::empty(),
        )
        .unwrap();
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
                DEFAULT_BASE_PACE_S_PER_KM,
                K_FATIGUE,
                0,
                &WeatherLookup::empty(),
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
                DEFAULT_BASE_PACE_S_PER_KM,
                K_FATIGUE,
                0,
                &WeatherLookup::empty(),
            )
            .unwrap()
        };

        // Stage 0 ends at LB — should include the stop.
        let diff = with_stop[0].estimated_duration_s - without_stop[0].estimated_duration_s;
        assert!((diff - stop_s as f64).abs() < 1.0);
        // Stage 1 is unaffected by LB stop.
        assert!((with_stop[1].estimated_duration_s - without_stop[1].estimated_duration_s).abs() < 1.0);
    }
}
