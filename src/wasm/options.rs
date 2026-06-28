use crate::pace_model::{WeatherConditions, WeatherLookup};

// ── Options for analyzeGpx / Trace::analyze / Trace::recalibrate ─────────────

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct WasmWeatherEntry {
    name: String,
    temperature_c: f64,
    humidity_pct: f64,
    wind_kmh: f64,
    precip_prob_pct: f64,
}

/// Shared by `WasmAnalyzeOptions` and `WasmRecalibrateOptions` — both carry an
/// optional `weather` array with the same shape.
fn weather_lookup_from(entries: &[WasmWeatherEntry]) -> WeatherLookup {
    if entries.is_empty() {
        return WeatherLookup::empty();
    }
    let (names, values) = entries
        .iter()
        .map(|w| {
            (
                w.name.clone(),
                WeatherConditions {
                    temperature_c: w.temperature_c,
                    humidity_pct: w.humidity_pct,
                    wind_kmh: w.wind_kmh,
                    precip_prob_pct: w.precip_prob_pct,
                },
            )
        })
        .unzip();
    WeatherLookup::new(names, values)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WasmAnalyzeOptions {
    base_pace_s_per_km: f64,
    k_fatigue: f64,
    life_base_stop_s: u32,
    #[serde(default)]
    weather: Vec<WasmWeatherEntry>,
}

impl WasmAnalyzeOptions {
    pub(crate) fn base_pace_s_per_km(&self) -> f64 {
        self.base_pace_s_per_km
    }

    pub(crate) fn k_fatigue(&self) -> f64 {
        self.k_fatigue
    }

    pub(crate) fn life_base_stop_s(&self) -> u32 {
        self.life_base_stop_s
    }

    pub(crate) fn weather_lookup(&self) -> WeatherLookup {
        weather_lookup_from(&self.weather)
    }
}

// ── Options for Trace::recalibrate ────────────────────────────────────────────

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WasmRecalibrateOptions {
    base_pace_s_per_km: f64,
    k_fatigue: f64,
    life_base_stop_s: u32,
    current_index: u32,
    actual_elapsed_s: f64,
    #[serde(default)]
    weather: Vec<WasmWeatherEntry>,
}

impl WasmRecalibrateOptions {
    pub(crate) fn base_pace_s_per_km(&self) -> f64 {
        self.base_pace_s_per_km
    }

    pub(crate) fn k_fatigue(&self) -> f64 {
        self.k_fatigue
    }

    pub(crate) fn life_base_stop_s(&self) -> u32 {
        self.life_base_stop_s
    }

    pub(crate) fn current_index(&self) -> usize {
        self.current_index as usize
    }

    pub(crate) fn actual_elapsed_s(&self) -> f64 {
        self.actual_elapsed_s
    }

    pub(crate) fn weather_lookup(&self) -> WeatherLookup {
        weather_lookup_from(&self.weather)
    }
}

#[cfg(test)]
impl WasmAnalyzeOptions {
    /// Used by `wasm.rs`'s pipeline tests, which need a concrete options
    /// value but live outside this module.
    pub(crate) fn sample() -> Self {
        Self {
            base_pace_s_per_km: 500.0,
            k_fatigue: 0.002,
            life_base_stop_s: 3600,
            weather: Vec::new(),
        }
    }
}

#[cfg(test)]
impl WasmRecalibrateOptions {
    /// Used by `wasm.rs`'s pipeline tests, which need a concrete options
    /// value but live outside this module.
    pub(crate) fn sample(current_index: u32, actual_elapsed_s: f64) -> Self {
        Self {
            base_pace_s_per_km: 500.0,
            k_fatigue: 0.002,
            life_base_stop_s: 3600,
            current_index,
            actual_elapsed_s,
            weather: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weather_lookup_empty_when_no_entries() {
        let options = WasmAnalyzeOptions {
            base_pace_s_per_km: 500.0,
            k_fatigue: 0.002,
            life_base_stop_s: 3600,
            weather: Vec::new(),
        };
        assert!((options.weather_lookup().factor_for("anything") - 1.0).abs() < 1e-9);
    }

    #[test]
    fn weather_lookup_converts_entries_and_matches_by_name() {
        let options = WasmAnalyzeOptions {
            base_pace_s_per_km: 500.0,
            k_fatigue: 0.002,
            life_base_stop_s: 3600,
            weather: vec![WasmWeatherEntry {
                name: "Chamonix".into(),
                temperature_c: 32.0,
                humidity_pct: 85.0,
                wind_kmh: 35.0,
                precip_prob_pct: 80.0,
            }],
        };
        let lookup = options.weather_lookup();
        assert!(lookup.factor_for("Chamonix") > 1.0);
        assert!((lookup.factor_for("Unknown") - 1.0).abs() < 1e-9);
    }
}
