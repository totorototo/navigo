use crate::location::Location;
use crate::trace::Trace;
use crate::waypoint::Waypoint;

#[cfg_attr(feature = "wasm", derive(serde::Serialize))]
pub struct LegStats {
    pub leg_id: usize,
    /// Index of the section this leg belongs to.
    pub section_idx: usize,
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
    /// Bearing from start to end waypoint (degrees from north).
    pub bearing: f64,
    /// 1–5 based on Naismith effort ratio vs flat terrain.
    pub difficulty: u8,
    /// Estimated travel time using Naismith's rule (seconds).
    pub estimated_duration_s: f64,
}

/// Compute leg statistics between all consecutive waypoints.
///
/// Returns an empty slice when fewer than 2 waypoints are provided.
/// Uses Naismith's rule for duration (simple hiking estimate, no fatigue model).
pub fn compute_from_waypoints(trace: &Trace, waypoints: &[Waypoint]) -> Vec<LegStats> {
    if waypoints.len() < 2 {
        return Vec::new();
    }

    let mut legs = Vec::new();
    let mut search_start = 0usize;
    let mut current_section_idx = 0usize;
    let mut section_active = false;

    for i in 0..waypoints.len() - 1 {
        if waypoints[i].is_section_boundary() {
            if section_active {
                current_section_idx += 1;
            } else {
                section_active = true;
            }
        }

        let start_wpt = &waypoints[i];
        let end_wpt = &waypoints[i + 1];

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

        let mut min_elevation = trace.locations[start_index].altitude;
        let mut max_elevation = trace.locations[start_index].altitude;
        let mut max_slope = 0.0_f64;
        for j in start_index..end_index {
            let ele = trace.locations[j].altitude;
            min_elevation = min_elevation.min(ele);
            max_elevation = max_elevation.max(ele);
            max_slope = max_slope.max(trace.slopes[j].abs());
        }

        let avg_slope = if dist_km > 0.0 {
            ((elevation_gain_m - elevation_loss_m) / (dist_km * 1000.0)) * 100.0
        } else {
            0.0
        };

        // Naismith's rule: 5 km/h flat + 600 m/h climbing.
        let flat_time_h = dist_km / 5.0;
        let naismith_time_h = flat_time_h + elevation_gain_m / 600.0;
        let estimated_duration_s = naismith_time_h * 3600.0;
        let effort_ratio = if flat_time_h > 0.0 {
            naismith_time_h / flat_time_h
        } else {
            1.0
        };
        let difficulty: u8 = if effort_ratio < 1.2 {
            1
        } else if effort_ratio < 1.5 {
            2
        } else if effort_ratio < 2.0 {
            3
        } else if effort_ratio < 2.7 {
            4
        } else {
            5
        };

        legs.push(LegStats {
            leg_id: i,
            section_idx: current_section_idx,
            start_index,
            end_index,
            point_count: end_index - start_index + 1,
            start_location: start_wpt.name.clone(),
            end_location: end_wpt.name.clone(),
            total_distance_km: dist_km,
            total_elevation_gain_m: elevation_gain_m,
            total_elevation_loss_m: elevation_loss_m,
            avg_slope,
            max_slope,
            min_elevation,
            max_elevation,
            bearing,
            difficulty,
            estimated_duration_s,
        });
    }

    legs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_trace(points: &[(f64, f64, f64)]) -> Trace {
        let locs: Vec<Location> = points
            .iter()
            .map(|&(lat, lon, alt)| Location {
                longitude: lon,
                latitude: lat,
                altitude: alt,
            })
            .collect();
        Trace::new(&locs).unwrap()
    }

    fn make_waypoint(lat: f64, lon: f64, name: &str, wpt_type: Option<&str>) -> Waypoint {
        Waypoint {
            latitude: lat,
            longitude: lon,
            elevation: None,
            name: name.to_string(),
            description: None,
            comment: None,
            symbol: None,
            wpt_type: wpt_type.map(String::from),
            time: None,
            stop_duration: None,
        }
    }

    #[test]
    fn basic_two_legs() {
        let points: Vec<(f64, f64, f64)> = (0..11)
            .map(|i| {
                (
                    i as f64 * 0.01,
                    -122.0 + i as f64 * 0.01,
                    100.0 + i as f64 * 20.0,
                )
            })
            .collect();
        let trace = make_trace(&points);
        let waypoints = vec![
            make_waypoint(0.0, -122.0, "Start", None),
            make_waypoint(0.05, -121.95, "Middle", None),
            make_waypoint(0.10, -121.90, "End", None),
        ];
        let legs = compute_from_waypoints(&trace, &waypoints);
        assert_eq!(legs.len(), 2);
        for leg in &legs {
            assert!(leg.total_distance_km > 0.0);
            assert!(leg.total_elevation_gain_m >= 0.0);
            assert!(leg.point_count > 0);
            assert!(leg.start_index < leg.end_index);
        }
    }

    #[test]
    fn single_waypoint_returns_empty() {
        let trace = make_trace(&[(37.0, -122.0, 100.0), (37.1, -122.1, 200.0)]);
        let waypoints = vec![make_waypoint(37.0, -122.0, "Only", None)];
        assert!(compute_from_waypoints(&trace, &waypoints).is_empty());
    }

    #[test]
    fn difficulty_flat_is_one() {
        let points: Vec<(f64, f64, f64)> = (0..4).map(|i| (0.0, i as f64 * 0.001, 100.0)).collect();
        let trace = make_trace(&points);
        let waypoints = vec![
            make_waypoint(0.0, 0.0, "Start", None),
            make_waypoint(0.0, 0.003, "End", None),
        ];
        let legs = compute_from_waypoints(&trace, &waypoints);
        assert_eq!(legs.len(), 1);
        assert_eq!(legs[0].difficulty, 1);
        assert!(legs[0].estimated_duration_s > 0.0);
    }

    #[test]
    fn difficulty_steep_is_high() {
        let points = vec![
            (0.000, 0.0, 0.0),
            (0.001, 0.0, 50.0),
            (0.002, 0.0, 100.0),
            (0.003, 0.0, 150.0),
        ];
        let trace = make_trace(&points);
        let waypoints = vec![
            make_waypoint(0.0, 0.0, "Bottom", None),
            make_waypoint(0.003, 0.0, "Top", None),
        ];
        let legs = compute_from_waypoints(&trace, &waypoints);
        assert_eq!(legs.len(), 1);
        assert!(legs[0].difficulty >= 3);
    }

    #[test]
    fn section_idx_tracks_section_boundaries() {
        let points: Vec<(f64, f64, f64)> =
            (0..11).map(|i| (i as f64 * 0.001, 0.0, 100.0)).collect();
        let trace = make_trace(&points);
        let waypoints = vec![
            make_waypoint(0.0, 0.0, "Start", Some("Start")),
            make_waypoint(0.002, 0.0, "Plain1", None),
            make_waypoint(0.004, 0.0, "BH1", Some("TimeBarrier")),
            make_waypoint(0.006, 0.0, "Plain2", None),
            make_waypoint(0.008, 0.0, "Arrival", Some("Arrival")),
        ];
        let legs = compute_from_waypoints(&trace, &waypoints);
        assert_eq!(legs.len(), 4);
        assert_eq!(legs[0].section_idx, 0);
        assert_eq!(legs[1].section_idx, 0);
        assert_eq!(legs[2].section_idx, 1);
        assert_eq!(legs[3].section_idx, 1);
    }

    #[test]
    fn elevation_loss_tracked() {
        // Descending trace — gain should be zero, loss positive.
        let points = vec![
            (0.000, 0.0, 500.0),
            (0.001, 0.0, 400.0),
            (0.002, 0.0, 300.0),
            (0.003, 0.0, 200.0),
        ];
        let trace = make_trace(&points);
        let waypoints = vec![
            make_waypoint(0.000, 0.0, "Top", None),
            make_waypoint(0.003, 0.0, "Bottom", None),
        ];
        let legs = compute_from_waypoints(&trace, &waypoints);
        assert_eq!(legs.len(), 1);
        assert_eq!(legs[0].total_elevation_gain_m, 0.0);
        assert!(legs[0].total_elevation_loss_m > 0.0);
    }

    #[test]
    fn bearing_non_zero_for_eastward_leg() {
        // Points moving east (longitude increasing, latitude fixed).
        let points = vec![
            (0.0, 0.000, 100.0),
            (0.0, 0.001, 100.0),
            (0.0, 0.002, 100.0),
            (0.0, 0.003, 100.0),
        ];
        let trace = make_trace(&points);
        let waypoints = vec![
            make_waypoint(0.0, 0.0, "West", None),
            make_waypoint(0.0, 0.003, "East", None),
        ];
        let legs = compute_from_waypoints(&trace, &waypoints);
        assert_eq!(legs.len(), 1);
        // East-bound bearing is around 90°.
        assert!((legs[0].bearing - 90.0).abs() < 5.0);
    }
}
