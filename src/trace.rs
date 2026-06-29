use crate::area::Area;
use crate::climbs::{detect_climbs, ClimbStats};
use crate::elevation::{
    compute_gain_loss, compute_slopes, cumulative_horizontal_distances, ELEV_MEDIAN_RADIUS_KM,
    ELEV_NOISE_THRESHOLD_M,
};
use crate::extrema::{find_peaks, find_valleys};
use crate::simplify::douglas_peucker_indices;
use crate::{Elevation, Location, TraceError};

const DP_EPSILON_KM: f64 = 0.015;
const DP_THRESHOLD: usize = 1000;
/// Early-stop lock-in for `find_closest_point_from` (km).
const LOCK_IN_KM: f64 = 1.0;
/// Early-stop margin past the current best before giving up (km).
const EARLY_STOP_KM: f64 = 2.0;

/// A non-empty, precomputed GPS trace.
///
/// `Trace` is never empty — [`Trace::new`] rejects empty input — so every
/// method below can assume `locations` has at least one element instead of
/// special-casing it.
#[derive(Debug, PartialEq)]
pub struct Trace {
    /// Possibly D-P simplified locations (owned). Never empty.
    pub(crate) locations: Vec<Location>,
    /// Cumulative haversine distance from start, km.  `[0] == 0.0`.
    pub(crate) cumulative_distances: Vec<f64>,
    /// Cumulative denoised elevation gain at each location (m).
    pub(crate) cumulative_elevation_gains: Vec<f64>,
    /// Cumulative denoised elevation loss at each location (m).
    pub(crate) cumulative_elevation_losses: Vec<f64>,
    /// Smoothed slope (% grade) at each location.
    pub(crate) slopes: Vec<f64>,
    /// Indices of detected peaks.
    pub(crate) peaks: Vec<usize>,
    /// Indices of detected valleys.
    pub(crate) valleys: Vec<usize>,
    /// Qualifying Garmin-style climb segments.
    pub(crate) climbs: Vec<ClimbStats>,
    pub(crate) total_distance: f64,
    pub(crate) total_elevation_gain: f64,
    pub(crate) total_elevation_loss: f64,
    pub(crate) area: Area,
    pub(crate) elevation: Elevation,
}

impl Trace {
    /// Builds a trace from raw locations. Fails with [`TraceError::EmptyTrace`]
    /// if `raw` is empty — a `Trace` is never empty once constructed.
    pub fn new(raw: &[Location]) -> Result<Self, TraceError> {
        if raw.is_empty() {
            return Err(TraceError::EmptyTrace);
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
        let total_distance = *cumulative_distances
            .last()
            .expect("locations is non-empty, so cumulative_distances has at least one element");

        let area = locations.iter().fold(
            Area {
                min_longitude: locations[0].longitude,
                max_longitude: locations[0].longitude,
                min_latitude: locations[0].latitude,
                max_latitude: locations[0].latitude,
            },
            |acc, loc| Area {
                min_latitude: acc.min_latitude.min(loc.latitude),
                max_latitude: acc.max_latitude.max(loc.latitude),
                min_longitude: acc.min_longitude.min(loc.longitude),
                max_longitude: acc.max_longitude.max(loc.longitude),
            },
        );

        let elevation = locations
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
            );

        // Denoised elevation computed on the full-resolution raw signal.
        let gain_loss = compute_gain_loss(raw, ELEV_MEDIAN_RADIUS_KM, ELEV_NOISE_THRESHOLD_M);

        // Map raw cumulative values back to the (possibly simplified) working set.
        let n_pts = src_indices.len();
        let mut cumulative_elevation_gains = Vec::with_capacity(n_pts);
        let mut cumulative_elevation_losses = Vec::with_capacity(n_pts);
        for &src in &src_indices {
            cumulative_elevation_gains.push(gain_loss.cum_gain[src]);
            cumulative_elevation_losses.push(gain_loss.cum_loss[src]);
        }

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

        Ok(Trace {
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
            area,
            elevation,
        })
    }

    // ── Public accessors ──────────────────────────────────────────────────────

    /// Simplified locations slice. Never empty.
    pub fn locations(&self) -> &[Location] {
        &self.locations
    }

    /// Cumulative haversine distances (km). Same length as `locations()`.
    pub fn cumulative_distances(&self) -> &[f64] {
        &self.cumulative_distances
    }

    /// Cumulative denoised elevation gain at each point (m).
    pub fn cumulative_elevation_gains(&self) -> &[f64] {
        &self.cumulative_elevation_gains
    }

    /// Cumulative denoised elevation loss at each point (m).
    pub fn cumulative_elevation_losses(&self) -> &[f64] {
        &self.cumulative_elevation_losses
    }

    /// Smoothed slope (% grade) at each point.
    pub fn slopes(&self) -> &[f64] {
        &self.slopes
    }

    /// Indices of detected elevation peaks.
    pub fn peaks(&self) -> &[usize] {
        &self.peaks
    }

    /// Indices of detected elevation valleys.
    pub fn valleys(&self) -> &[usize] {
        &self.valleys
    }

    /// Qualifying Garmin-style climb segments.
    pub fn climbs(&self) -> &[ClimbStats] {
        &self.climbs
    }

    /// Total trace distance in km.
    pub fn total_distance(&self) -> f64 {
        self.total_distance
    }

    /// Denoised total elevation gain (m).
    pub fn total_elevation_gain(&self) -> f64 {
        self.total_elevation_gain
    }

    /// Denoised total elevation loss (m).
    pub fn total_elevation_loss(&self) -> f64 {
        self.total_elevation_loss
    }

    // ── Distance helpers ──────────────────────────────────────────────────────

    /// Index of the first location at or beyond `dist_km` (binary search).
    pub fn index_at_distance(&self, dist_km: f64) -> usize {
        let pos = self.cumulative_distances.partition_point(|&d| d < dist_km);
        pos.min(self.locations.len() - 1)
    }

    /// Location at cumulative distance `dist_km`, or `None` if negative.
    pub fn point_at_distance(&self, dist_km: f64) -> Option<&Location> {
        if dist_km < 0.0 {
            return None;
        }
        self.locations.get(self.index_at_distance(dist_km))
    }

    /// Slice of locations between two cumulative distances (both ends inclusive).
    pub fn slice_between_distances(&self, start_km: f64, end_km: f64) -> Option<&[Location]> {
        if start_km < 0.0 || end_km < 0.0 || start_km > end_km {
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
        if start_from >= self.locations.len() {
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
    pub fn area(&self) -> &Area {
        &self.area
    }

    /// Raw (non-denoised) elevation gain/loss over the working locations.
    pub fn elevation(&self) -> &Elevation {
        &self.elevation
    }

    /// Sub-slice by index range (both ends inclusive).
    pub fn get_section(
        &self,
        start_index: usize,
        end_index: usize,
    ) -> Result<&[Location], TraceError> {
        if start_index >= end_index {
            return Err(TraceError::InvalidRange {
                start_index,
                end_index,
            });
        }
        if end_index >= self.locations.len() {
            return Err(TraceError::IndexOutOfBounds {
                index: end_index,
                len: self.locations.len(),
            });
        }
        Ok(&self.locations[start_index..=end_index])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::helper;

    // ── Construction ──────────────────────────────────────────────────────────

    #[test]
    fn empty_input_is_rejected() {
        assert_eq!(Trace::new(&[]), Err(TraceError::EmptyTrace));
    }

    #[test]
    fn single_location_trace() {
        let loc = Location {
            longitude: 0.0,
            latitude: 0.0,
            altitude: 100.0,
        };
        let trace = Trace::new(&[loc]).unwrap();
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
        let trace = Trace::new(&[paris, moscow]).unwrap();
        let len = trace.length();
        assert!((len - 2486.340992526076).abs() < 1e-6);
    }

    #[test]
    fn full_dataset_builds_successfully() {
        let locations = helper::get_locations();
        let trace = Trace::new(&locations).unwrap();
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
        let trace = Trace::new(&locations).unwrap();
        assert_eq!(trace.cumulative_distances[0], 0.0);
        for i in 1..trace.cumulative_distances.len() {
            assert!(trace.cumulative_distances[i] >= trace.cumulative_distances[i - 1]);
        }
    }

    #[test]
    fn total_elevation_gain_and_loss_non_negative() {
        let locations = helper::get_locations();
        let trace = Trace::new(&locations).unwrap();
        assert!(trace.total_elevation_gain >= 0.0);
        assert!(trace.total_elevation_loss >= 0.0);
    }

    // ── point_at_distance ─────────────────────────────────────────────────────

    #[test]
    fn point_at_distance_negative_returns_none() {
        let locations = helper::get_locations();
        let trace = Trace::new(&locations).unwrap();
        assert!(trace.point_at_distance(-1.0).is_none());
    }

    #[test]
    fn point_at_distance_zero_returns_first() {
        let locations = helper::get_locations();
        let trace = Trace::new(&locations).unwrap();
        let pt = trace.point_at_distance(0.0).unwrap();
        assert_eq!(pt, &trace.locations[0]);
    }

    #[test]
    fn point_at_distance_beyond_end_returns_last() {
        let locations = helper::get_locations();
        let trace = Trace::new(&locations).unwrap();
        let pt = trace.point_at_distance(1_000_000.0).unwrap();
        assert_eq!(pt, trace.locations.last().unwrap());
    }

    // ── slice_between_distances ───────────────────────────────────────────────

    #[test]
    fn slice_full_range_covers_all_locations() {
        let locations = helper::get_locations();
        let trace = Trace::new(&locations).unwrap();
        let slice = trace
            .slice_between_distances(0.0, trace.total_distance)
            .unwrap();
        assert_eq!(slice.len(), trace.locations.len());
    }

    #[test]
    fn slice_invalid_range_returns_none() {
        let locations = helper::get_locations();
        let trace = Trace::new(&locations).unwrap();
        assert!(trace.slice_between_distances(10.0, 5.0).is_none());
        assert!(trace.slice_between_distances(-1.0, 5.0).is_none());
    }

    // ── index_at_distance ─────────────────────────────────────────────────────

    #[test]
    fn index_at_distance_zero_is_zero() {
        let locations = helper::get_locations();
        let trace = Trace::new(&locations).unwrap();
        assert_eq!(trace.index_at_distance(0.0), 0);
    }

    #[test]
    fn index_at_distance_end_is_last() {
        let locations = helper::get_locations();
        let trace = Trace::new(&locations).unwrap();
        let last = trace.locations.len() - 1;
        assert_eq!(trace.index_at_distance(trace.total_distance), last);
    }

    // ── find_closest_point ────────────────────────────────────────────────────

    #[test]
    fn find_closest_point_exact_match() {
        let locations = helper::get_locations();
        let trace = Trace::new(&locations).unwrap();
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
        let trace = Trace::new(&[paris, moscow]).unwrap();
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
        let trace = Trace::new(&[paris, moscow]).unwrap();
        let area = trace.area();
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
        let trace = Trace::new(&locations).unwrap();
        let elevation = trace.elevation();
        assert_eq!(elevation.positive, 50.0);
        assert_eq!(elevation.negative, 30.0);
    }

    // ── get_section ───────────────────────────────────────────────────────────

    #[test]
    fn get_section_returns_sub_slice() {
        let locations = helper::get_locations();
        let trace = Trace::new(&locations).unwrap();
        let section = trace.get_section(1, 3).unwrap();
        assert_eq!(section, &trace.locations[1..=3]);
    }

    #[test]
    fn get_section_start_not_smaller_than_end_errs() {
        let locations = helper::get_locations();
        let trace = Trace::new(&locations).unwrap();
        assert_eq!(
            trace.get_section(2, 2),
            Err(TraceError::InvalidRange {
                start_index: 2,
                end_index: 2
            })
        );
        assert_eq!(
            trace.get_section(3, 1),
            Err(TraceError::InvalidRange {
                start_index: 3,
                end_index: 1
            })
        );
    }

    #[test]
    fn get_section_out_of_bounds_errs() {
        let locations = helper::get_locations();
        let trace = Trace::new(&locations).unwrap();
        let len = trace.locations.len();
        assert_eq!(
            trace.get_section(0, len),
            Err(TraceError::IndexOutOfBounds { index: len, len })
        );
    }
}
