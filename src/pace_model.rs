use crate::minetti;

// ── Fatigue ───────────────────────────────────────────────────────────────────

/// Default fatigue coefficient. Presets: Low=0.001, Moderate=0.002, High=0.003, Very high=0.004.
pub const K_FATIGUE: f64 = 0.002;

/// Fraction of accumulated effort distance recovered at a LifeBase checkpoint.
pub const RECOVERY_LIFE_BASE: f64 = 0.20;

/// Default planned stop time at a LifeBase in seconds (used when no per-waypoint override is set).
pub const DEFAULT_LIFE_BASE_STOP_S: u32 = 3600;

/// Default flat-terrain pace when no user pace is provided (500 s/km = 8:20/km).
pub const DEFAULT_BASE_PACE_S_PER_KM: f64 = 500.0;

/// Exponential fatigue multiplier (≥ 1.0). More realistic than linear: late-race slowdown accelerates.
pub fn fatigue_factor(d_eff_km: f64, k: f64) -> f64 {
    (k * d_eff_km).exp()
}

// ── Circadian ─────────────────────────────────────────────────────────────────

/// Circadian pace multiplier (≥ 1.0) based on UTC time-of-day.
///
/// Smooth half-cosine bump centred at 3:30 UTC (peak sleep-deprivation), ±2 h window, up to +15%.
pub fn circadian_factor(unix_time_s: i64) -> f64 {
    let hours_utc = ((unix_time_s as f64) / 3600.0).rem_euclid(24.0);
    let diff = hours_utc - 3.5;
    if diff.abs() < 2.0 {
        let normalized = diff / 2.0;
        1.0 + 0.15 * 0.5 * (1.0 + (std::f64::consts::PI * normalized).cos())
    } else {
        1.0
    }
}

// ── Composite pace factor ─────────────────────────────────────────────────────

pub struct PaceFactors {
    /// Terrain-only Minetti factor. Drives effort-weighted distance (d_eff).
    pub terrain: f64,
    /// terrain × fatigue × circadian × weather. Applied to base pace to get segment time.
    pub combined: f64,
}

pub fn compute_factors(
    slope_frac: f64,
    d_eff_km: f64,
    k_fatigue: f64,
    clock_start: Option<i64>,
    now_s: f64,
    weather_factor: f64,
) -> PaceFactors {
    let terrain = minetti::pace_factor(slope_frac);
    let fatigue = fatigue_factor(d_eff_km, k_fatigue);
    let circadian = clock_start
        .map(|t0| circadian_factor(t0 + now_s as i64))
        .unwrap_or(1.0);
    PaceFactors {
        terrain,
        combined: terrain * fatigue * circadian * weather_factor,
    }
}

// ── Weather ───────────────────────────────────────────────────────────────────

pub const WEATHER_T_OPT: f64 = 12.0;
pub const WEATHER_HEAT_PER_C: f64 = 0.006;
pub const WEATHER_COLD_THRESHOLD_C: f64 = 0.0;
pub const WEATHER_COLD_PER_C: f64 = 0.003;
pub const WEATHER_WIND_THRESHOLD_KMH: f64 = 15.0;
pub const WEATHER_WIND_PER_KMH: f64 = 0.004;
pub const WEATHER_WIND_MAX: f64 = 0.20;
pub const WEATHER_PRECIP_MAX: f64 = 0.08;

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "wasm", derive(serde::Serialize, serde::Deserialize))]
pub struct WeatherConditions {
    pub temperature_c: f64,
    pub humidity_pct: f64,
    pub wind_kmh: f64,
    pub precip_prob_pct: f64,
}

/// Neutral forecast (cool, dry, calm). `weather_factor(WEATHER_NEUTRAL) == 1.0`.
pub const WEATHER_NEUTRAL: WeatherConditions = WeatherConditions {
    temperature_c: WEATHER_T_OPT,
    humidity_pct: 50.0,
    wind_kmh: 0.0,
    precip_prob_pct: 0.0,
};

/// Apparent (feels-like) temperature. Humidity amplifies heat stress above 20 °C only.
pub fn apparent_temp_c(temp_c: f64, humidity_pct: f64) -> f64 {
    if temp_c <= 20.0 {
        return temp_c;
    }
    let excess_rh = (humidity_pct.clamp(0.0, 100.0) - 40.0).max(0.0) / 10.0;
    temp_c + excess_rh * (temp_c - 20.0) * 0.1
}

pub fn thermal_factor(temp_c: f64, humidity_pct: f64) -> f64 {
    let at = apparent_temp_c(temp_c, humidity_pct);
    if at > WEATHER_T_OPT {
        1.0 + (at - WEATHER_T_OPT) * WEATHER_HEAT_PER_C
    } else if temp_c < WEATHER_COLD_THRESHOLD_C {
        1.0 + (WEATHER_COLD_THRESHOLD_C - temp_c) * WEATHER_COLD_PER_C
    } else {
        1.0
    }
}

pub fn wind_factor(wind_kmh: f64) -> f64 {
    if wind_kmh <= WEATHER_WIND_THRESHOLD_KMH {
        return 1.0;
    }
    1.0 + ((wind_kmh - WEATHER_WIND_THRESHOLD_KMH) * WEATHER_WIND_PER_KMH).min(WEATHER_WIND_MAX)
}

pub fn precip_factor(precip_prob_pct: f64) -> f64 {
    1.0 + WEATHER_PRECIP_MAX * precip_prob_pct.clamp(0.0, 100.0) / 100.0
}

pub fn weather_factor(c: WeatherConditions) -> f64 {
    thermal_factor(c.temperature_c, c.humidity_pct)
        * wind_factor(c.wind_kmh)
        * precip_factor(c.precip_prob_pct)
}

/// Name-keyed weather forecast table. Unknown names resolve to `WEATHER_NEUTRAL`.
#[derive(Debug, Clone)]
pub struct WeatherLookup {
    names: Vec<String>,
    values: Vec<WeatherConditions>,
}

impl WeatherLookup {
    pub fn empty() -> Self {
        Self {
            names: Vec::new(),
            values: Vec::new(),
        }
    }

    pub fn new(names: Vec<String>, values: Vec<WeatherConditions>) -> Self {
        Self { names, values }
    }

    pub fn find(&self, name: &str) -> WeatherConditions {
        self.names
            .iter()
            .zip(self.values.iter())
            .find(|(n, _)| n.as_str() == name)
            .map(|(_, v)| *v)
            .unwrap_or(WEATHER_NEUTRAL)
    }

    pub fn factor_for(&self, name: &str) -> f64 {
        weather_factor(self.find(name))
    }
}

// ── Analysis options (builder) ────────────────────────────────────────────────

/// Grouped parameters for the pace model used by `section`, `stage` and
/// `calibration` computations.  Use the builder methods or `Default` for
/// sensible race-analysis defaults.
#[derive(Debug, Clone)]
pub struct AnalysisOptions {
    /// Flat-terrain pace in seconds per km.
    pub base_pace_s_per_km: f64,
    /// Exponential fatigue coefficient.
    pub k_fatigue: f64,
    /// Default planned stop at LifeBase checkpoints (seconds).
    pub life_base_stop_s: u32,
    /// Per-checkpoint weather forecast.
    pub weather: WeatherLookup,
}

impl Default for AnalysisOptions {
    fn default() -> Self {
        Self {
            base_pace_s_per_km: DEFAULT_BASE_PACE_S_PER_KM,
            k_fatigue: K_FATIGUE,
            life_base_stop_s: DEFAULT_LIFE_BASE_STOP_S,
            weather: WeatherLookup::empty(),
        }
    }
}

impl AnalysisOptions {
    /// Start building options from the defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set flat-terrain pace (seconds per km).
    pub fn base_pace(mut self, s_per_km: f64) -> Self {
        self.base_pace_s_per_km = s_per_km;
        self
    }

    /// Set the exponential fatigue coefficient.
    pub fn fatigue(mut self, k: f64) -> Self {
        self.k_fatigue = k;
        self
    }

    /// Set the default LifeBase stop duration (seconds).
    pub fn life_base_stop(mut self, seconds: u32) -> Self {
        self.life_base_stop_s = seconds;
        self
    }

    /// Attach a weather forecast lookup table.
    pub fn weather(mut self, lookup: WeatherLookup) -> Self {
        self.weather = lookup;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fatigue_zero_effort_is_one() {
        assert!((fatigue_factor(0.0, K_FATIGUE) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn fatigue_grows_faster_than_linear() {
        let d = 100.0;
        let linear = 1.0 + K_FATIGUE * d;
        assert!(fatigue_factor(d, K_FATIGUE) > linear);
    }

    #[test]
    fn fatigue_ultra_finish_approx_1_60() {
        assert!((fatigue_factor(234.0, K_FATIGUE) - 1.60).abs() < 0.01);
    }

    #[test]
    fn circadian_noon_utc_is_neutral() {
        assert!((circadian_factor(12 * 3600) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn circadian_peak_at_3_30_utc() {
        assert!((circadian_factor(3 * 3600 + 30 * 60) - 1.15).abs() < 1e-9);
    }

    #[test]
    fn circadian_always_at_least_one() {
        let mut t = 0i64;
        while t < 24 * 3600 {
            assert!(circadian_factor(t) >= 1.0);
            t += 600;
        }
    }

    #[test]
    fn circadian_wraps_across_midnight() {
        let t0 = 3i64 * 3600 + 30 * 60;
        let t2 = 2 * 24 * 3600 + 3 * 3600 + 30 * 60;
        assert!((circadian_factor(t0) - circadian_factor(t2)).abs() < 1e-9);
    }

    #[test]
    fn compute_factors_flat_fresh_neutral_is_one() {
        let f = compute_factors(0.0, 0.0, K_FATIGUE, None, 0.0, 1.0);
        assert!((f.terrain - 1.0).abs() < 1e-9);
        assert!((f.combined - 1.0).abs() < 1e-9);
    }

    #[test]
    fn compute_factors_combined_is_product_of_all() {
        let slope_frac = 0.08;
        let d_eff_km = 50.0;
        let clock_start: i64 = 1_700_000_000;
        let now_s = 3600.0;
        let wf = 1.05;

        let f = compute_factors(
            slope_frac,
            d_eff_km,
            K_FATIGUE,
            Some(clock_start),
            now_s,
            wf,
        );
        let expected = minetti::pace_factor(slope_frac)
            * fatigue_factor(d_eff_km, K_FATIGUE)
            * circadian_factor(clock_start + now_s as i64)
            * wf;
        assert!((f.combined - expected).abs() < 1e-12);
    }

    #[test]
    fn weather_neutral_returns_one() {
        assert!((weather_factor(WEATHER_NEUTRAL) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn thermal_heat_and_humidity_worse() {
        let dry = thermal_factor(30.0, 40.0);
        let humid = thermal_factor(30.0, 90.0);
        assert!(dry > 1.0);
        assert!(humid > dry);
    }

    #[test]
    fn thermal_cold_penalty() {
        assert!(((1.0 + 10.0 * WEATHER_COLD_PER_C) - thermal_factor(-10.0, 50.0)).abs() < 1e-9);
    }

    #[test]
    fn wind_capped_at_max() {
        assert!((wind_factor(1000.0) - (1.0 + WEATHER_WIND_MAX)).abs() < 1e-9);
    }

    #[test]
    fn precip_zero_is_neutral_and_hundred_is_max() {
        assert!((precip_factor(0.0) - 1.0).abs() < 1e-9);
        assert!((precip_factor(100.0) - (1.0 + WEATHER_PRECIP_MAX)).abs() < 1e-9);
    }

    #[test]
    fn weather_lookup_empty_is_neutral() {
        assert!((WeatherLookup::empty().factor_for("anything") - 1.0).abs() < 1e-9);
    }

    #[test]
    fn weather_lookup_finds_by_name() {
        let hot = WeatherConditions {
            temperature_c: 32.0,
            humidity_pct: 85.0,
            wind_kmh: 35.0,
            precip_prob_pct: 80.0,
        };
        let lookup = WeatherLookup::new(vec!["Chamonix".into()], vec![hot]);
        assert!(lookup.factor_for("Chamonix") > 1.0);
        assert!((lookup.factor_for("Unknown") - 1.0).abs() < 1e-9);
    }
}
