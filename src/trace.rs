use crate::area::Area;
use crate::climbs::{detect_climbs, ClimbStats};
use crate::elevation::{
    compute_gain_loss, compute_slopes, cumulative_horizontal_distances, ELEV_MEDIAN_RADIUS_KM,
    ELEV_NOISE_THRESHOLD_M,
};
use crate::extrema::{find_peaks, find_valleys};
use crate::simplify::douglas_peucker_indices;
use crate::{Elevation, Location};

const DP_EPSILON_KM: f64 = 0.015;
const DP_THRESHOLD: usize = 1000;
/// Early-stop lock-in for `find_closest_point_from` (km).
const LOCK_IN_KM: f64 = 1.0;
/// Early-stop margin past the current best before giving up (km).
const EARLY_STOP_KM: f64 = 2.0;

#[derive(Debug)]
pub struct Trace {
    /// Possibly D-P simplified locations (owned).
    pub locations: Vec<Location>,
    /// Cumulative haversine distance from start, km.  `[0] == 0.0`.
    pub cumulative_distances: Vec<f64>,
    /// Cumulative denoised elevation gain at each location (m).
    pub cumulative_elevation_gains: Vec<f64>,
    /// Cumulative denoised elevation loss at each location (m).
    pub cumulative_elevation_losses: Vec<f64>,
    /// Smoothed slope (% grade) at each location.
    pub slopes: Vec<f64>,
    /// Indices of detected peaks.
    pub peaks: Vec<usize>,
    /// Indices of detected valleys.
    pub valleys: Vec<usize>,
    /// Qualifying Garmin-style climb segments.
    pub climbs: Vec<ClimbStats>,
    pub total_distance: f64,
    pub total_elevation_gain: f64,
    pub total_elevation_loss: f64,
}

impl Trace {
    pub fn new(raw: &[Location]) -> Self {
        if raw.is_empty() {
            return Trace {
                locations: vec![],
                cumulative_distances: vec![],
                cumulative_elevation_gains: vec![],
                cumulative_elevation_losses: vec![],
                slopes: vec![],
                peaks: vec![],
                valleys: vec![],
                climbs: vec![],
                total_distance: 0.0,
                total_elevation_gain: 0.0,
                total_elevation_loss: 0.0,
            };
        }

        let n = raw.len();

        // Simplify large datasets to keep per-point math cheap.
        let (locations, src_indices): (Vec<Location>, Vec<usize>) = if n > DP_THRESHOLD {
            let indices = douglas_peucker_indices(raw, DP_EPSILON_KM);
            let simplified = indices.iter().map(|&i| raw[i]).collect();
            (simplified, indices)
        } else {
            (raw.to_vec(), (0..n).collect())
        };

        let cumulative_distances = cumulative_horizontal_distances(&locations);
        let total_distance = *cumulative_distances.last().unwrap_or(&0.0);

        // Denoised elevation computed on the full-resolution raw signal.
        let gain_loss = compute_gain_loss(raw, ELEV_MEDIAN_RADIUS_KM, ELEV_NOISE_THRESHOLD_M);

        // Map raw cumulative values back to the (possibly simplified) working set.
        let cumulative_elevation_gains: Vec<f64> = src_indices
            .iter()
            .map(|&src| gain_loss.cum_gain[src])
            .collect();
        let cumulative_elevation_losses: Vec<f64> = src_indices
            .iter()
            .map(|&src| gain_loss.cum_loss[src])
            .collect();

        let slopes = compute_slopes(&locations, &cumulative_distances);

        let elevations: Vec<f32> = locations.iter().map(|l| l.altitude as f32).collect();
        let peaks = if locations.len() >= 3 {
            find_peaks(&elevations)
        } else {
            vec![]
        };
        let valleys = if locations.len() >= 3 {
            find_valleys(&elevations)
        } else {
            vec![]
        };

        let climbs = detect_climbs(&peaks, &valleys, &locations, &cumulative_distances);

        Trace {
            locations,
            cumulative_distances,
            cumulative_elevation_gains,
            cumulative_elevation_losses,
            slopes,
            peaks,
            valleys,
            climbs,
            total_distance,
            total_elevation_gain: gain_loss.total_gain,
            total_elevation_loss: gain_loss.total_loss,
        }
    }

    // ── Distance helpers ──────────────────────────────────────────────────────

    /// Index of the first location at or beyond `dist_km` (binary search).
    pub fn index_at_distance(&self, dist_km: f64) -> usize {
        if self.cumulative_distances.is_empty() {
            return 0;
        }
        let pos = self.cumulative_distances.partition_point(|&d| d < dist_km);
        pos.min(self.locations.len() - 1)
    }

    /// Location at cumulative distance `dist_km`, or `None` if negative / empty.
    pub fn point_at_distance(&self, dist_km: f64) -> Option<&Location> {
        if dist_km < 0.0 || self.locations.is_empty() {
            return None;
        }
        self.locations.get(self.index_at_distance(dist_km))
    }

    /// Slice of locations between two cumulative distances (both ends inclusive).
    pub fn slice_between_distances(&self, start_km: f64, end_km: f64) -> Option<&[Location]> {
        if self.locations.is_empty() || start_km < 0.0 || end_km < 0.0 || start_km > end_km {
            return None;
        }
        let start_idx = self.index_at_distance(start_km);
        let end_idx = self.index_at_distance(end_km);
        if start_idx <= end_idx && end_idx < self.locations.len() {
            Some(&self.locations[start_idx..=end_idx])
        } else {
            None
        }
    }

    // ── Closest-point search ──────────────────────────────────────────────────

    /// Closest location on the whole trace to `target`.
    pub fn find_closest_point(&self, target: &Location) -> Option<(&Location, usize, f64)> {
        self.find_closest_point_from(target, 0)
    }

    /// Closest location at or after `start_from`.
    ///
    /// Early-stop heuristic: once a candidate within `LOCK_IN_KM` is found the
    /// scan aborts when distance grows beyond `best + EARLY_STOP_KM`, so loop
    /// courses snap to the first nearby occurrence rather than a later return leg.
    pub fn find_closest_point_from(
        &self,
        target: &Location,
        start_from: usize,
    ) -> Option<(&Location, usize, f64)> {
        if self.locations.is_empty() || start_from >= self.locations.len() {
            return None;
        }
        let mut best_dist = f64::INFINITY;
        let mut best_idx = start_from;

        for (offset, loc) in self.locations[start_from..].iter().enumerate() {
            let d = target.calculate_distance_to(loc);
            if d < best_dist {
                best_dist = d;
                best_idx = start_from + offset;
            } else if best_dist < LOCK_IN_KM && d > best_dist + EARLY_STOP_KM {
                break;
            }
        }
        Some((&self.locations[best_idx], best_idx, best_dist))
    }

    // ── Convenience accessors ─────────────────────────────────────────────────

    /// Total trace distance (km) — alias for `total_distance`.
    pub fn length(&self) -> f64 {
        self.total_distance
    }

    /// Bounding box of all locations.
    pub fn area(&self) -> Result<Area, &str> {
        let first = self.locations.first().ok_or("could not compute area")?;
        let area = self.locations.iter().fold(
            Area {
                min_longitude: first.longitude,
                max_longitude: first.longitude,
                min_latitude: first.latitude,
                max_latitude: first.latitude,
            },
            |acc, loc| Area {
                min_latitude: acc.min_latitude.min(loc.latitude),
                max_latitude: acc.max_latitude.max(loc.latitude),
                min_longitude: acc.min_longitude.min(loc.longitude),
                max_longitude: acc.max_longitude.max(loc.longitude),
            },
        );
        Ok(area)
    }

    /// Raw (non-denoised) elevation gain/loss over the working locations.
    /// Provided for backward compatibility; prefer `total_elevation_gain/loss`
    /// for accurate denoised values.
    pub fn elevation(&self) -> Elevation {
        self.locations
            .windows(2)
            .map(|w| w[1].altitude - w[0].altitude)
            .fold(
                Elevation {
                    positive: 0.0,
                    negative: 0.0,
                },
                |mut acc, delta| {
                    if delta > 0.0 {
                        acc.positive += delta;
                    } else {
                        acc.negative += delta.abs();
                    }
                    acc
                },
            )
    }

    /// Sub-slice by index range (both ends inclusive).
    pub fn get_section(&self, start_index: usize, end_index: usize) -> Result<Vec<Location>, &str> {
        if self.locations.is_empty() {
            return Err("could not compute section");
        }
        if start_index >= end_index {
            return Err("start_index must be smaller than end_index");
        }
        if end_index >= self.locations.len() {
            return Err("index out of bounds");
        }
        Ok(self.locations[start_index..=end_index].to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::helper;

    // ── Construction ──────────────────────────────────────────────────────────

    #[test]
    fn empty_trace() {
        let trace = Trace::new(&[]);
        assert!(trace.locations.is_empty());
        assert_eq!(trace.total_distance, 0.0);
        assert_eq!(trace.total_elevation_gain, 0.0);
        assert_eq!(trace.total_elevation_loss, 0.0);
        assert!(trace.point_at_distance(0.0).is_none());
        assert!(trace.slice_between_distances(0.0, 1.0).is_none());
    }

    #[test]
    fn single_location_trace() {
        let loc = Location {
            longitude: 0.0,
            latitude: 0.0,
            altitude: 100.0,
        };
        let trace = Trace::new(&[loc]);
        assert_eq!(trace.locations.len(), 1);
        assert_eq!(trace.total_distance, 0.0);
    }

    #[test]
    fn two_location_trace_has_positive_distance() {
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
        let trace = Trace::new(&[paris, moscow]);
        let len = trace.length();
        assert!((len - 2486.340992526076).abs() < 1e-6);
    }

    #[test]
    fn full_dataset_builds_successfully() {
        let locations = helper::get_locations();
        let trace = Trace::new(&locations);
        assert!(!trace.locations.is_empty());
        assert!(trace.total_distance > 0.0);
        assert_eq!(trace.cumulative_distances.len(), trace.locations.len());
        assert_eq!(
            trace.cumulative_elevation_gains.len(),
            trace.locations.len()
        );
        assert_eq!(
            trace.cumulative_elevation_losses.len(),
            trace.locations.len()
        );
        assert_eq!(trace.slopes.len(), trace.locations.len());
    }

    // ── Precomputed arrays ────────────────────────────────────────────────────

    #[test]
    fn cumulative_distances_start_zero_and_non_decreasing() {
        let locations = helper::get_locations();
        let trace = Trace::new(&locations);
        assert_eq!(trace.cumulative_distances[0], 0.0);
        for i in 1..trace.cumulative_distances.len() {
            assert!(trace.cumulative_distances[i] >= trace.cumulative_distances[i - 1]);
        }
    }

    #[test]
    fn total_elevation_gain_and_loss_non_negative() {
        let locations = helper::get_locations();
        let trace = Trace::new(&locations);
        assert!(trace.total_elevation_gain >= 0.0);
        assert!(trace.total_elevation_loss >= 0.0);
    }

    // ── point_at_distance ─────────────────────────────────────────────────────

    #[test]
    fn point_at_distance_negative_returns_none() {
        let locations = helper::get_locations();
        let trace = Trace::new(&locations);
        assert!(trace.point_at_distance(-1.0).is_none());
    }

    #[test]
    fn point_at_distance_zero_returns_first() {
        let locations = helper::get_locations();
        let trace = Trace::new(&locations);
        let pt = trace.point_at_distance(0.0).unwrap();
        assert_eq!(pt, &trace.locations[0]);
    }

    #[test]
    fn point_at_distance_beyond_end_returns_last() {
        let locations = helper::get_locations();
        let trace = Trace::new(&locations);
        let pt = trace.point_at_distance(1_000_000.0).unwrap();
        assert_eq!(pt, trace.locations.last().unwrap());
    }

    // ── slice_between_distances ───────────────────────────────────────────────

    #[test]
    fn slice_full_range_covers_all_locations() {
        let locations = helper::get_locations();
        let trace = Trace::new(&locations);
        let slice = trace
            .slice_between_distances(0.0, trace.total_distance)
            .unwrap();
        assert_eq!(slice.len(), trace.locations.len());
    }

    #[test]
    fn slice_invalid_range_returns_none() {
        let locations = helper::get_locations();
        let trace = Trace::new(&locations);
        assert!(trace.slice_between_distances(10.0, 5.0).is_none());
        assert!(trace.slice_between_distances(-1.0, 5.0).is_none());
    }

    // ── index_at_distance ─────────────────────────────────────────────────────

    #[test]
    fn index_at_distance_zero_is_zero() {
        let locations = helper::get_locations();
        let trace = Trace::new(&locations);
        assert_eq!(trace.index_at_distance(0.0), 0);
    }

    #[test]
    fn index_at_distance_end_is_last() {
        let locations = helper::get_locations();
        let trace = Trace::new(&locations);
        let last = trace.locations.len() - 1;
        assert_eq!(trace.index_at_distance(trace.total_distance), last);
    }

    // ── find_closest_point ────────────────────────────────────────────────────

    #[test]
    fn find_closest_point_on_empty_returns_none() {
        let trace = Trace::new(&[]);
        let target = Location {
            longitude: 0.0,
            latitude: 0.0,
            altitude: 0.0,
        };
        assert!(trace.find_closest_point(&target).is_none());
    }

    #[test]
    fn find_closest_point_exact_match() {
        let locations = helper::get_locations();
        let trace = Trace::new(&locations);
        let target = &trace.locations[0];
        let (_, idx, dist) = trace.find_closest_point(target).unwrap();
        assert_eq!(idx, 0);
        assert!(dist < 1e-9);
    }

    #[test]
    fn find_closest_point_between_paris_and_moscow() {
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
        let trace = Trace::new(&[paris, moscow]);
        let near_paris = Location {
            longitude: 1.350987,
            latitude: 49.856667,
            altitude: 0.0,
        };
        let (loc, _, _) = trace.find_closest_point(&near_paris).unwrap();
        assert_eq!(loc, &paris);
    }

    // ── area ──────────────────────────────────────────────────────────────────

    #[test]
    fn area_empty_trace_returns_error() {
        let trace = Trace::new(&[]);
        assert!(trace.area().is_err());
    }

    #[test]
    fn area_paris_to_moscow() {
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
        let trace = Trace::new(&[paris, moscow]);
        let area = trace.area().unwrap();
        assert_eq!(area.min_longitude, 2.350987);
        assert_eq!(area.max_longitude, 37.617634);
        assert_eq!(area.min_latitude, 48.856667);
        assert_eq!(area.max_latitude, 55.755787);
    }

    // ── elevation ─────────────────────────────────────────────────────────────

    #[test]
    fn elevation_sums_gains_and_losses() {
        let locations = [
            Location {
                longitude: 0.0,
                latitude: 0.0,
                altitude: 100.0,
            },
            Location {
                longitude: 0.0,
                latitude: 0.001,
                altitude: 150.0,
            },
            Location {
                longitude: 0.0,
                latitude: 0.002,
                altitude: 120.0,
            },
        ];
        let trace = Trace::new(&locations);
        let elevation = trace.elevation();
        assert_eq!(elevation.positive, 50.0);
        assert_eq!(elevation.negative, 30.0);
    }

    #[test]
    fn elevation_on_empty_trace_is_zero() {
        let trace = Trace::new(&[]);
        let elevation = trace.elevation();
        assert_eq!(elevation.positive, 0.0);
        assert_eq!(elevation.negative, 0.0);
    }

    // ── get_section ───────────────────────────────────────────────────────────

    #[test]
    fn get_section_returns_sub_slice() {
        let locations = helper::get_locations();
        let trace = Trace::new(&locations);
        let section = trace.get_section(1, 3).unwrap();
        assert_eq!(section, trace.locations[1..=3].to_vec());
    }

    #[test]
    fn get_section_on_empty_trace_errs() {
        let trace = Trace::new(&[]);
        assert!(trace.get_section(0, 1).is_err());
    }

    #[test]
    fn get_section_start_not_smaller_than_end_errs() {
        let locations = helper::get_locations();
        let trace = Trace::new(&locations);
        assert!(trace.get_section(2, 2).is_err());
        assert!(trace.get_section(3, 1).is_err());
    }

    #[test]
    fn get_section_out_of_bounds_errs() {
        let locations = helper::get_locations();
        let trace = Trace::new(&locations);
        let len = trace.locations.len();
        assert!(trace.get_section(0, len).is_err());
    }

    // ── index_at_distance / slice_between_distances on empty trace ───────────

    #[test]
    fn index_at_distance_on_empty_trace_is_zero() {
        let trace = Trace::new(&[]);
        assert_eq!(trace.index_at_distance(5.0), 0);
    }
}
