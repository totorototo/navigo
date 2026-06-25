use crate::pace_model::{self, WeatherConditions};
use crate::trace::Trace;

/// Per-segment physiological metrics accumulated over a trace index range.
///
/// Shared by `leg`, `section`, and `stage` — the inner pace/fatigue/circadian/weather
/// loop is identical across all three; only the waypoint grouping differs.
pub struct SegmentMetrics {
    pub min_elevation: f64,
    pub max_elevation: f64,
    /// Maximum absolute slope (%) across the range.
    pub max_slope: f64,
    /// Total moving time in seconds.
    pub total_time: f64,
    /// Sum of seg_dist_km × terrain_pace_factor. Used to derive the average pace factor.
    pub total_weighted_dist_km: f64,
}

/// Accumulate metrics over trace points `[start_index, end_index)`.
///
/// `d_eff_m` (cumulative effort-weighted distance in **metres**) and `elapsed_s`
/// (cumulative moving-time clock in **seconds**) are mutated in place so the
/// caller can carry fatigue and circadian state across consecutive segments.
///
/// `clock_start` is the race start epoch or `None` for a neutral (1.0) circadian factor.
#[allow(clippy::too_many_arguments)]
pub fn compute(
    trace: &Trace,
    start_index: usize,
    end_index: usize,
    base_pace_s_per_km: f64,
    k_fatigue: f64,
    clock_start: Option<i64>,
    weather: WeatherConditions,
    d_eff_m: &mut f64,
    elapsed_s: &mut f64,
) -> SegmentMetrics {
    let mut min_elevation = trace.locations[start_index].altitude;
    let mut max_elevation = trace.locations[start_index].altitude;
    let mut max_slope = 0.0_f64;
    let mut total_time = 0.0_f64;
    let mut total_weighted_dist_km = 0.0_f64;

    let weather_factor = pace_model::weather_factor(weather);

    for j in start_index..end_index {
        let ele = trace.locations[j].altitude;
        min_elevation = min_elevation.min(ele);
        max_elevation = max_elevation.max(ele);
        max_slope = max_slope.max(trace.slopes[j].abs());

        let slope_frac = trace.slopes[j] / 100.0;
        // cumulative_distances is in km; differences give km segments directly.
        let seg_dist_km = trace.cumulative_distances[j + 1] - trace.cumulative_distances[j];

        let factors = pace_model::compute_factors(
            slope_frac,
            *d_eff_m / 1000.0,
            k_fatigue,
            clock_start,
            *elapsed_s,
            weather_factor,
        );

        let seg_time = seg_dist_km * base_pace_s_per_km * factors.combined;
        total_time += seg_time;
        *elapsed_s += seg_time;
        total_weighted_dist_km += seg_dist_km * factors.terrain;
        *d_eff_m += seg_dist_km * 1000.0 * factors.terrain;
    }

    SegmentMetrics {
        min_elevation,
        max_elevation,
        max_slope,
        total_time,
        total_weighted_dist_km,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pace_model::{weather_factor, WEATHER_NEUTRAL};
    use crate::Location;

    fn flat_trace(n: usize) -> Trace {
        let locs: Vec<Location> = (0..n)
            .map(|i| Location {
                longitude: i as f64 * 0.001,
                latitude: 0.0,
                altitude: 100.0,
            })
            .collect();
        Trace::new(&locs).unwrap()
    }

    #[test]
    fn flat_range_neutral_pace_factor() {
        let trace = flat_trace(5);
        let mut d_eff = 0.0;
        let mut elapsed = 0.0;
        let m = compute(
            &trace,
            0,
            trace.locations.len() - 1,
            500.0,
            pace_model::K_FATIGUE,
            None,
            WEATHER_NEUTRAL,
            &mut d_eff,
            &mut elapsed,
        );
        assert!((m.min_elevation - 100.0).abs() < 0.001);
        assert!((m.max_elevation - 100.0).abs() < 0.001);
        assert!(m.total_time > 0.0);
        assert!((m.total_time - elapsed).abs() < 1e-9);
    }

    #[test]
    fn state_carries_across_consecutive_calls() {
        let locs: Vec<Location> = (0..20)
            .map(|i| Location {
                longitude: 0.0,
                latitude: i as f64 * 0.001,
                altitude: 100.0 + i as f64 * 10.0,
            })
            .collect();
        let trace = Trace::new(&locs).unwrap();
        let last = trace.locations.len() - 1;
        let mid = last / 2;

        let mut d_eff = 0.0;
        let mut elapsed = 0.0;
        compute(
            &trace,
            0,
            mid,
            500.0,
            pace_model::K_FATIGUE,
            None,
            WEATHER_NEUTRAL,
            &mut d_eff,
            &mut elapsed,
        );
        let d_after_first = d_eff;
        compute(
            &trace,
            mid,
            last,
            500.0,
            pace_model::K_FATIGUE,
            None,
            WEATHER_NEUTRAL,
            &mut d_eff,
            &mut elapsed,
        );
        assert!(d_eff > d_after_first);
        assert!(elapsed > 0.0);
    }

    #[test]
    fn adverse_weather_increases_time() {
        let trace = flat_trace(5);
        let hot = crate::pace_model::WeatherConditions {
            temperature_c: 32.0,
            humidity_pct: 85.0,
            wind_kmh: 35.0,
            precip_prob_pct: 80.0,
        };
        let mut d_a = 0.0;
        let mut e_a = 0.0;
        let neutral = compute(
            &trace,
            0,
            trace.locations.len() - 1,
            500.0,
            pace_model::K_FATIGUE,
            None,
            WEATHER_NEUTRAL,
            &mut d_a,
            &mut e_a,
        );
        let mut d_b = 0.0;
        let mut e_b = 0.0;
        let adverse = compute(
            &trace,
            0,
            trace.locations.len() - 1,
            500.0,
            pace_model::K_FATIGUE,
            None,
            hot,
            &mut d_b,
            &mut e_b,
        );
        assert!(adverse.total_time > neutral.total_time);
        let expected = neutral.total_time * weather_factor(hot);
        assert!((adverse.total_time - expected).abs() < 1e-6);
        assert!((neutral.total_weighted_dist_km - adverse.total_weighted_dist_km).abs() < 1e-9);
    }

    #[test]
    fn descending_slope_reduces_time_below_flat() {
        // Gentle descent (5 m drop per point) — Minetti cost is below flat.
        let locs: Vec<Location> = (0..5)
            .map(|i| Location {
                longitude: i as f64 * 0.001,
                latitude: 0.0,
                altitude: 500.0 - i as f64 * 5.0,
            })
            .collect();
        let descending = Trace::new(&locs).unwrap();
        let flat = flat_trace(5);
        let last = flat.locations.len() - 1;

        let mut d_d = 0.0;
        let mut e_d = 0.0;
        let desc_m = compute(
            &descending,
            0,
            last,
            500.0,
            pace_model::K_FATIGUE,
            None,
            WEATHER_NEUTRAL,
            &mut d_d,
            &mut e_d,
        );

        let mut d_f = 0.0;
        let mut e_f = 0.0;
        let flat_m = compute(
            &flat,
            0,
            last,
            500.0,
            pace_model::K_FATIGUE,
            None,
            WEATHER_NEUTRAL,
            &mut d_f,
            &mut e_f,
        );

        assert!(desc_m.total_time < flat_m.total_time);
    }

    #[test]
    fn max_slope_tracked_correctly() {
        // 50 m gain per 0.111 km segment → very steep.
        let locs: Vec<Location> = (0..4)
            .map(|i| Location {
                longitude: i as f64 * 0.001,
                latitude: 0.0,
                altitude: i as f64 * 50.0,
            })
            .collect();
        let trace = Trace::new(&locs).unwrap();
        let mut d = 0.0;
        let mut e = 0.0;
        let m = compute(
            &trace,
            0,
            3,
            500.0,
            pace_model::K_FATIGUE,
            None,
            WEATHER_NEUTRAL,
            &mut d,
            &mut e,
        );
        assert!(m.max_slope > 10.0);
        assert_eq!(m.min_elevation, 0.0);
        assert_eq!(m.max_elevation, 100.0); // index 2 is the last iterated (end exclusive)
    }
}
