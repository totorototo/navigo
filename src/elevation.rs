use crate::Location;

/// Denoising parameters mirroring the Zig terminus defaults.
pub const ELEV_NOISE_THRESHOLD_M: f64 = 3.0; // hysteresis deadband in altitude-meters
pub const ELEV_MEDIAN_RADIUS_KM: f64 = 0.015; // ±15 m window for median filter
pub const SLOPE_HALF_WINDOW_KM: f64 = 0.005; // ±5 m half-window for slope estimation
const MAX_WINDOW_SAMPLES: usize = 256;

// ── Public types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "wasm", derive(serde::Serialize))]
pub struct Elevation {
    pub positive: f64,
    pub negative: f64,
}

/// Cumulative elevation gain/loss computed on the denoised signal.
#[derive(Debug)]
pub struct GainLoss {
    /// Per-point cumulative elevation gain (monotonically non-decreasing).
    pub cum_gain: Vec<f64>,
    /// Per-point cumulative elevation loss (monotonically non-decreasing).
    pub cum_loss: Vec<f64>,
    pub total_gain: f64,
    pub total_loss: f64,
}

// ── Core functions ────────────────────────────────────────────────────────────

/// Cumulative horizontal (haversine) distances in km, length == locations.len().
/// First element is always 0.0.
pub fn cumulative_horizontal_distances(locations: &[Location]) -> Vec<f64> {
    let mut cum = vec![0.0f64; locations.len()];
    let mut acc = 0.0f64;
    for i in 1..locations.len() {
        acc += locations[i - 1].calculate_distance_to(&locations[i]);
        cum[i] = acc;
    }
    cum
}

/// Returns the median of a mutable slice (sorts in place).
fn median(scratch: &mut [f64]) -> f64 {
    scratch.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = scratch.len();
    match n {
        0 => 0.0,
        n if n % 2 == 1 => scratch[n / 2],
        n => (scratch[n / 2 - 1] + scratch[n / 2]) / 2.0,
    }
}

/// Distance-windowed median smoother.
/// For each point, gathers all altitude values whose cumulative distance lies
/// within ±`radius_km`, then returns the median.  The window is capped at
/// `MAX_WINDOW_SAMPLES` to bound memory usage.
pub fn median_smooth(locations: &[Location], cum_dist: &[f64], radius_km: f64) -> Vec<f64> {
    debug_assert_eq!(locations.len(), cum_dist.len());
    let n = locations.len();
    let mut out = vec![0.0f64; n];
    if n == 0 {
        return out;
    }

    let mut scratch = vec![0.0f64; MAX_WINDOW_SAMPLES];
    let alt_cache: Vec<f64> = locations.iter().map(|l| l.altitude).collect();
    let mut lo = 0usize;
    let mut hi = 0usize;

    for i in 0..n {
        // Advance lo: drop points that have fallen outside the left edge
        while lo < i && cum_dist[i] - cum_dist[lo] > radius_km {
            lo += 1;
        }
        // Advance hi: include points that have entered the right edge
        while hi + 1 < n && cum_dist[hi + 1] - cum_dist[i] <= radius_km {
            hi += 1;
        }

        let (mut win_lo, mut win_hi) = (lo, hi);
        if win_hi - win_lo + 1 > MAX_WINDOW_SAMPLES {
            let half = MAX_WINDOW_SAMPLES / 2;
            win_lo = i.saturating_sub(half);
            win_hi = (win_lo + MAX_WINDOW_SAMPLES - 1).min(n - 1);
        }

        let count = win_hi - win_lo + 1;
        scratch[..count].copy_from_slice(&alt_cache[win_lo..win_lo + count]);
        out[i] = median(&mut scratch[..count]);
    }
    out
}

/// Hysteresis accumulator: only register gain/loss when the elevation change
/// exceeds `threshold_m`, eliminating GPS jitter from the D+/D− totals.
pub fn accumulate(elevations: &[f64], threshold_m: f64) -> GainLoss {
    let n = elevations.len();
    let mut cum_gain = vec![0.0f64; n];
    let mut cum_loss = vec![0.0f64; n];

    if n == 0 {
        return GainLoss {
            cum_gain,
            cum_loss,
            total_gain: 0.0,
            total_loss: 0.0,
        };
    }

    let mut gain = 0.0f64;
    let mut loss = 0.0f64;
    let mut ref_elev = elevations[0];

    for i in 1..n {
        let e = elevations[i];
        if e > ref_elev + threshold_m {
            gain += e - ref_elev;
            ref_elev = e;
        } else if e < ref_elev - threshold_m {
            loss += ref_elev - e;
            ref_elev = e;
        }
        cum_gain[i] = gain;
        cum_loss[i] = loss;
    }

    GainLoss {
        cum_gain,
        cum_loss,
        total_gain: gain,
        total_loss: loss,
    }
}

/// Full pipeline: median-smooth the altitude signal then accumulate with hysteresis.
/// Always uses the raw (full-resolution) locations for maximum accuracy.
pub fn compute_gain_loss(locations: &[Location], radius_km: f64, threshold_m: f64) -> GainLoss {
    if locations.is_empty() {
        return GainLoss {
            cum_gain: vec![],
            cum_loss: vec![],
            total_gain: 0.0,
            total_loss: 0.0,
        };
    }
    let cum_dist = cumulative_horizontal_distances(locations);
    let smoothed = median_smooth(locations, &cum_dist, radius_km);
    accumulate(&smoothed, threshold_m)
}

/// Smoothed slope (percent grade) at each point, estimated over a centered
/// ±`SLOPE_HALF_WINDOW_KM` distance window.  Uses binary search to find the
/// window endpoints for O(n log n) overall complexity.
pub fn compute_slopes(locations: &[Location], cum_dist: &[f64]) -> Vec<f64> {
    debug_assert_eq!(locations.len(), cum_dist.len());
    let n = locations.len();
    let mut slopes = vec![0.0f64; n];
    if n == 0 {
        return slopes;
    }

    for i in 0..n {
        let current = cum_dist[i];

        // Furthest point behind that is still within the window
        let behind_idx = if i == 0 {
            0
        } else {
            let target = current - SLOPE_HALF_WINDOW_KM;
            let pos = cum_dist[..i].partition_point(|&d| d <= target);
            if pos > 0 {
                pos - 1
            } else {
                0
            }
        };

        // Furthest point ahead that is still within the window
        let ahead_idx = if i + 1 < n {
            let target = current + SLOPE_HALF_WINDOW_KM;
            let rel = cum_dist[i + 1..].partition_point(|&d| d < target);
            (i + 1 + rel).min(n - 1)
        } else {
            n - 1
        };

        let segment_dist_km = cum_dist[ahead_idx] - cum_dist[behind_idx];
        let segment_elev_m = locations[ahead_idx].altitude - locations[behind_idx].altitude;

        slopes[i] = if segment_dist_km > 0.0 {
            (segment_elev_m / (segment_dist_km * 1000.0)) * 100.0
        } else {
            0.0
        };
    }
    slopes
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Location;

    fn loc(lat: f64, lon: f64, alt: f64) -> Location {
        Location {
            latitude: lat,
            longitude: lon,
            altitude: alt,
        }
    }

    // — accumulate ————————————————————————————————————————————————————————————

    #[test]
    fn flat_noisy_signal_yields_near_zero_gain() {
        let elevs = vec![100.0, 101.5, 99.0, 100.5, 98.5, 101.0, 99.5, 100.0];
        let gl = accumulate(&elevs, 3.0);
        assert!(gl.total_gain.abs() < 1e-9, "gain={}", gl.total_gain);
        assert!(gl.total_loss.abs() < 1e-9, "loss={}", gl.total_loss);
    }

    #[test]
    fn clean_monotonic_climb_counts_full_gain() {
        let elevs = vec![0.0, 10.0, 20.0, 30.0, 40.0, 50.0];
        let gl = accumulate(&elevs, 3.0);
        assert!((gl.total_gain - 50.0).abs() < 1e-9);
        assert!(gl.total_loss.abs() < 1e-9);
    }

    #[test]
    fn cumulative_arrays_are_monotonic() {
        let elevs = vec![0.0, 10.0, 5.0, 15.0, 0.0];
        let gl = accumulate(&elevs, 3.0);
        for i in 1..gl.cum_gain.len() {
            assert!(gl.cum_gain[i] >= gl.cum_gain[i - 1]);
            assert!(gl.cum_loss[i] >= gl.cum_loss[i - 1]);
        }
    }

    #[test]
    fn accumulate_empty_input() {
        let gl = accumulate(&[], 3.0);
        assert_eq!(gl.cum_gain.len(), 0);
        assert_eq!(gl.total_gain, 0.0);
    }

    // — compute_gain_loss ─────────────────────────────────────────────────────

    #[test]
    fn spike_does_not_inflate_gain_or_loss() {
        // 5 points, middle one is a spike — median window covers all → spike filtered out
        let locations = vec![
            loc(0.0, 0.0, 100.0),
            loc(0.0001, 0.0, 100.0),
            loc(0.0002, 0.0, 145.0), // spike
            loc(0.0003, 0.0, 100.0),
            loc(0.0004, 0.0, 100.0),
        ];
        let gl = compute_gain_loss(&locations, 0.05, 3.0);
        assert!(gl.total_gain.abs() < 1e-6, "gain={}", gl.total_gain);
        assert!(gl.total_loss.abs() < 1e-6, "loss={}", gl.total_loss);
    }

    #[test]
    fn compute_gain_loss_empty() {
        let gl = compute_gain_loss(&[], 0.015, 3.0);
        assert_eq!(gl.cum_gain.len(), 0);
        assert_eq!(gl.total_gain, 0.0);
    }

    // — compute_slopes ────────────────────────────────────────────────────────

    #[test]
    fn flat_profile_has_zero_slope() {
        let points = vec![
            loc(0.0, 0.0, 100.0),
            loc(0.001, 0.0, 100.0),
            loc(0.002, 0.0, 100.0),
        ];
        let cum = cumulative_horizontal_distances(&points);
        let slopes = compute_slopes(&points, &cum);
        for s in &slopes {
            assert!(s.abs() < 1e-6, "slope={}", s);
        }
    }

    #[test]
    fn steady_climb_yields_positive_slope() {
        // ~111 m horizontal per step, +10 m elevation → ~9% grade
        let points = vec![
            loc(0.0, 0.0, 0.0),
            loc(0.001, 0.0, 10.0),
            loc(0.002, 0.0, 20.0),
            loc(0.003, 0.0, 30.0),
            loc(0.004, 0.0, 40.0),
        ];
        let cum = cumulative_horizontal_distances(&points);
        let slopes = compute_slopes(&points, &cum);
        assert_eq!(slopes.len(), 5);
        for s in &slopes {
            assert!(*s > 0.0, "expected positive slope, got {}", s);
        }
    }

    #[test]
    fn compute_slopes_empty() {
        let s = compute_slopes(&[], &[]);
        assert!(s.is_empty());
    }
}
