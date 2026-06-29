use crate::Location;

// ── Garmin Climb Pro qualification thresholds ─────────────────────────────────
// A climb is counted only when ALL three conditions hold:
//   score       = climb_dist_km × avg_gradient  > 3.5   (= 3 500 m×%)
//   climb_dist  >= 0.5 km  (= 500 m)
//   avg_gradient >= 3 %

const MIN_CLIMB_SCORE: f64 = 3.5; // km × %
const MIN_CLIMB_DIST_KM: f64 = 0.5;
const MIN_AVG_GRADIENT: f64 = 3.0; // %

// ── Data types ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "wasm", derive(serde::Serialize))]
#[cfg_attr(feature = "wasm", serde(rename_all = "camelCase"))]
pub struct ClimbStats {
    /// Index of the climb-start (valley) in the working-locations array.
    pub start_index: usize,
    /// Index of the summit (peak) in the working-locations array.
    pub end_index: usize,
    /// Cumulative distance at the climb start (km from trace start).
    pub start_dist_km: f64,
    /// Horizontal length of the climb (km).
    pub climb_dist_km: f64,
    /// Elevation gain from valley to summit (metres).
    pub elevation_gain: f64,
    /// Altitude of the summit (metres).
    pub summit_elev: f64,
    /// Average gradient of the climb (%).
    pub avg_gradient: f64,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn qualifies_as_climb(dist_km: f64, avg_gradient: f64) -> bool {
    dist_km >= MIN_CLIMB_DIST_KM
        && avg_gradient >= MIN_AVG_GRADIENT
        && dist_km * avg_gradient > MIN_CLIMB_SCORE
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Derive climb segments from detected peaks and valleys.
///
/// For each peak, the nearest preceding valley is found.  Falls back to index 0
/// when no valley precedes the peak (the trail start acts as the implicit origin).
/// Each valley can only be claimed by one climb — once used, it is consumed.
///
/// `peaks` and `valleys` must be sorted ascending.
/// `cum_distances` must be aligned with `locations` (same length, km).
pub fn detect_climbs(
    peaks: &[usize],
    valleys: &[usize],
    locations: &[Location],
    cum_distances: &[f64],
) -> Vec<ClimbStats> {
    if peaks.is_empty() || locations.is_empty() || cum_distances.is_empty() {
        return vec![];
    }
    debug_assert_eq!(locations.len(), cum_distances.len());

    let mut climbs = Vec::new();
    let mut valley_cursor = 0usize;
    let mut min_valley_start = 0usize; // no two climbs may share the same start

    for &peak_idx in peaks {
        debug_assert!(peak_idx < locations.len());

        // Advance valley_cursor to the last valley strictly before this peak
        while valley_cursor + 1 < valleys.len() && valleys[valley_cursor + 1] < peak_idx {
            valley_cursor += 1;
        }

        let ampd_valley_idx = if !valleys.is_empty() && valleys[valley_cursor] < peak_idx {
            valleys[valley_cursor]
        } else {
            0
        };

        // If the nearest AMPD valley was already claimed by a prior climb, scan the
        // unclaimed range for the actual lowest point to use as the valley base.
        let valley_idx = if ampd_valley_idx < min_valley_start {
            (min_valley_start..peak_idx)
                .min_by(|&a, &b| {
                    locations[a]
                        .altitude
                        .partial_cmp(&locations[b].altitude)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .unwrap_or(min_valley_start)
        } else {
            ampd_valley_idx
        };

        if valley_idx >= peak_idx {
            continue;
        }

        let valley_elev = locations[valley_idx].altitude;
        let start_dist = cum_distances[valley_idx];
        let end_dist = cum_distances[peak_idx];
        let climb_dist = if end_dist > start_dist {
            end_dist - start_dist
        } else {
            0.0
        };

        let summit_elev = locations[peak_idx].altitude;
        let elev_gain = if summit_elev > valley_elev {
            summit_elev - valley_elev
        } else {
            0.0
        };

        // grade % = (rise_m / run_m) × 100 — convert km → m in the denominator
        let avg_gradient = if climb_dist > 0.0 {
            (elev_gain / (climb_dist * 1000.0)) * 100.0
        } else {
            0.0
        };

        if !qualifies_as_climb(climb_dist, avg_gradient) {
            continue;
        }

        climbs.push(ClimbStats {
            start_index: valley_idx,
            end_index: peak_idx,
            start_dist_km: start_dist,
            climb_dist_km: climb_dist,
            elevation_gain: elev_gain,
            summit_elev,
            avg_gradient,
        });

        min_valley_start = peak_idx;
    }

    climbs
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Location;

    fn loc(lat: f64, alt: f64) -> Location {
        Location {
            latitude: lat,
            longitude: 0.0,
            altitude: alt,
        }
    }

    #[test]
    fn empty_inputs_return_no_climbs() {
        let climbs = detect_climbs(&[], &[], &[], &[]);
        assert!(climbs.is_empty());

        let pts = vec![loc(0.0, 100.0), loc(0.1, 200.0)];
        let dists = vec![0.0, 1.0];
        let climbs = detect_climbs(&[], &[], &pts, &dists);
        assert!(climbs.is_empty());
    }

    #[test]
    fn single_qualifying_climb() {
        // Valley at idx 0 (100 m), peak at idx 4 (500 m), 4 km horizontal
        let pts = vec![
            loc(0.0, 100.0),
            loc(0.1, 200.0),
            loc(0.2, 350.0),
            loc(0.3, 450.0),
            loc(0.4, 500.0),
        ];
        let dists = vec![0.0, 1.0, 2.0, 3.0, 4.0]; // km
        let peaks = vec![4usize];
        let valleys = vec![0usize];

        let climbs = detect_climbs(&peaks, &valleys, &pts, &dists);
        assert_eq!(climbs.len(), 1);
        let c = &climbs[0];
        assert_eq!(c.start_index, 0);
        assert_eq!(c.end_index, 4);
        assert!((c.climb_dist_km - 4.0).abs() < 1e-9);
        assert!((c.elevation_gain - 400.0).abs() < 1e-9);
        assert!((c.summit_elev - 500.0).abs() < 1e-9);
        assert!((c.avg_gradient - 10.0).abs() < 1e-6); // 400/(4000)*100 = 10%
    }

    #[test]
    fn two_peaks_no_ampd_valley_between_them_uses_fallback_minimum() {
        // AMPD detected no valley between the two peaks.  The fallback path
        // should find the lowest point in the unclaimed range and use it as
        // the valley base for the second climb.
        //
        // Climb A: valley=0 (100 m) → peak=2 (600 m), 2 km, 25 %, score 50 ✓
        // Climb B: fallback valley=3 (580 m) → peak=4 (700 m), 1.5 km, 8 %, score 12 ✓
        let pts = vec![
            loc(0.0, 100.0), // 0: valley
            loc(0.1, 200.0), // 1
            loc(0.2, 600.0), // 2: peak A
            loc(0.3, 580.0), // 3: dip — not detected by AMPD, used as fallback valley
            loc(0.4, 700.0), // 4: peak B
        ];
        let dists = vec![0.0, 0.5, 2.0, 2.5, 4.0];
        let peaks = vec![2usize, 4];
        let valleys = vec![0usize];

        let climbs = detect_climbs(&peaks, &valleys, &pts, &dists);
        assert_eq!(climbs.len(), 2);
        assert_eq!(climbs[0].start_index, 0);
        assert_eq!(climbs[0].end_index, 2);
        assert_eq!(climbs[1].start_index, 3);
        assert_eq!(climbs[1].end_index, 4);
    }

    #[test]
    fn garmin_qualification_filter() {
        // Six valley→peak pairs; only the last qualifies (see inline comments).
        //   A: 0.4 km  dist < 0.5  → fails dist
        //   B: 0.6 km, 1.67% grade → fails grade
        //   C: 0.5 km, 3%,  score=1.5 ≤ 3.5 → fails score
        //   D: 0.6 km, 5%,  score=3.0 ≤ 3.5 → fails score
        //   E: 0.7 km, 5%,  score=3.5 not > 3.5 → fails score
        //   F: 0.8 km, 6%,  score=4.8 > 3.5  ✓
        let pts = vec![
            loc(0.0, 0.0),  //  0 valley
            loc(0.1, 20.0), //  1 peak A: 0.4 km, 20 m → 5%,  score=2.0
            loc(0.2, 0.0),  //  2 valley
            loc(0.3, 10.0), //  3 peak B: 0.6 km, 10 m → 1.67%
            loc(0.4, 0.0),  //  4 valley
            loc(0.5, 15.0), //  5 peak C: 0.5 km, 15 m → 3%
            loc(0.6, 0.0),  //  6 valley
            loc(0.7, 30.0), //  7 peak D: 0.6 km, 30 m → 5%
            loc(0.8, 0.0),  //  8 valley
            loc(0.9, 35.0), //  9 peak E: 0.7 km, 35 m → 5%
            loc(1.0, 0.0),  // 10 valley
            loc(1.1, 48.0), // 11 peak F: 0.8 km, 48 m → 6%
        ];
        let dists = vec![0.0, 0.4, 0.8, 1.4, 1.8, 2.3, 2.7, 3.3, 3.7, 4.4, 4.8, 5.6];
        let peaks = vec![1, 3, 5, 7, 9, 11];
        let valleys = vec![0, 2, 4, 6, 8, 10];

        let climbs = detect_climbs(&peaks, &valleys, &pts, &dists);
        assert_eq!(climbs.len(), 1, "expected only peak F to qualify");
        assert_eq!(climbs[0].start_index, 10);
        assert_eq!(climbs[0].end_index, 11);
        assert!((climbs[0].climb_dist_km - 0.8).abs() < 1e-9);
        assert!((climbs[0].avg_gradient - 6.0).abs() < 1e-3);
    }
}
