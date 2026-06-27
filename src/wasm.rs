use wasm_bindgen::prelude::*;

use crate::pace_model::{WeatherConditions, WeatherLookup};
use crate::{build_trace as core_build_trace, Location};

// ── Public entry points ───────────────────────────────────────────────────────

/// Parse a GPX file from raw bytes (e.g. a `Uint8Array` read on the JS side)
/// into a `Trace` — track-points, waypoints and metadata are all parsed
/// once here and stored in WASM memory.
///
/// Returns `null` when the GPX contains no valid track-points.
#[wasm_bindgen(js_name = "parseGpx")]
pub fn parse_gpx(bytes: &[u8]) -> Option<Trace> {
    parse_wasm_trace(bytes)
}

/// Parse a GPX file and compute the full analysis in one call: trace metrics,
/// waypoints, legs (Naismith), sections and stages (Minetti pace model).
///
/// Convenience wrapper around `parseGpx` + `trace.analyze()` for callers who
/// just want the JSON result and don't need the `Trace` handle (so there's
/// no `.free()` to manage). If you already have a `Trace`, call
/// `.analyze()` on it directly instead of parsing the bytes twice.
///
/// `options` — a JS object: `{ basePaceSPerKm, kFatigue, lifeBaseStopS, weather? }`.
/// `weather` is an optional array of
/// `{ name, temperatureC, humidityPct, windKmh, precipProbPct }`, matched against
/// checkpoint names; unmatched checkpoints use neutral weather.
///
/// Returns a JS object with `{ trace, waypoints, legs, sections, stages, metadata }`
/// or `null` when the GPX contains no valid track-points or `options` is malformed.
#[wasm_bindgen(js_name = "analyzeGpx")]
pub fn analyze_gpx(bytes: &[u8], options: JsValue) -> Option<JsValue> {
    let options: WasmAnalyzeOptions = serde_wasm_bindgen::from_value(options).ok()?;
    let trace = parse_wasm_trace(bytes)?;

    let analysis = compute_route_analysis(&trace, &options);
    let result = WasmGpxFull {
        trace: WasmTraceSummary {
            total_distance_km: trace.inner.total_distance,
            total_elevation_gain_m: trace.inner.total_elevation_gain,
            total_elevation_loss_m: trace.inner.total_elevation_loss,
            location_count: trace.inner.locations.len() as u32,
        },
        waypoints: analysis.waypoints,
        legs: analysis.legs,
        sections: analysis.sections,
        stages: analysis.stages,
        metadata: analysis.metadata,
    };

    serde_wasm_bindgen::to_value(&result).ok()
}

/// Parse GPX `bytes` into a `Trace`, including its waypoints and metadata.
fn parse_wasm_trace(bytes: &[u8]) -> Option<Trace> {
    let locations = crate::gpx::parse_trace_points(bytes);
    let inner = core_build_trace(&locations).ok()?;
    Some(Trace {
        inner,
        waypoints: crate::gpx::parse_waypoints(bytes),
        metadata: crate::gpx::parse_metadata(bytes),
    })
}

/// Shared implementation behind `analyzeGpx` and `Trace::analyze`.
fn compute_route_analysis(trace: &Trace, options: &WasmAnalyzeOptions) -> WasmRouteAnalysis {
    let weather = options.weather_lookup();

    let legs = crate::leg::compute_from_waypoints(&trace.inner, &trace.waypoints);
    let sections = crate::section::compute_from_waypoints(
        &trace.inner,
        &trace.waypoints,
        options.base_pace_s_per_km,
        options.k_fatigue,
        options.life_base_stop_s,
        &weather,
    );
    let stages = crate::stage::compute_from_waypoints(
        &trace.inner,
        &trace.waypoints,
        options.base_pace_s_per_km,
        options.k_fatigue,
        options.life_base_stop_s,
        &weather,
    );

    WasmRouteAnalysis {
        waypoints: trace
            .waypoints
            .iter()
            .map(|w| WasmWaypoint {
                latitude: w.latitude,
                longitude: w.longitude,
                elevation: w.elevation,
                name: w.name.clone(),
                wpt_type: w.wpt_type.clone(),
                time: w.time,
                stop_duration: w.stop_duration,
            })
            .collect(),
        legs: legs
            .into_iter()
            .map(|l| WasmLegStats {
                leg_id: l.leg_id as u32,
                section_idx: l.section_idx as u32,
                start_index: l.start_index as u32,
                end_index: l.end_index as u32,
                start_location: l.start_location,
                end_location: l.end_location,
                total_distance_km: l.total_distance_km,
                total_elevation_gain_m: l.total_elevation_gain_m,
                total_elevation_loss_m: l.total_elevation_loss_m,
                avg_slope: l.avg_slope,
                max_slope: l.max_slope,
                min_elevation: l.min_elevation,
                max_elevation: l.max_elevation,
                bearing: l.bearing,
                difficulty: l.difficulty,
                estimated_duration_s: l.estimated_duration_s,
            })
            .collect(),
        sections: sections.map(|ss| {
            ss.into_iter()
                .map(|s| WasmSectionStats {
                    id: s.section_id as u32,
                    stage_idx: s.stage_idx as u32,
                    start_index: s.start_index as u32,
                    end_index: s.end_index as u32,
                    start_location: s.start_location,
                    end_location: s.end_location,
                    total_distance_km: s.total_distance_km,
                    total_elevation_gain_m: s.total_elevation_gain_m,
                    total_elevation_loss_m: s.total_elevation_loss_m,
                    avg_slope: s.avg_slope,
                    max_slope: s.max_slope,
                    min_elevation: s.min_elevation,
                    max_elevation: s.max_elevation,
                    start_time: s.start_time,
                    end_time: s.end_time,
                    bearing: s.bearing,
                    difficulty: s.difficulty,
                    estimated_duration_s: s.estimated_duration_s,
                    pace_factor: s.pace_factor,
                    max_completion_time: s.max_completion_time,
                    cutoff_ratio: s.cutoff_ratio,
                    stop_duration: s.stop_duration,
                })
                .collect()
        }),
        stages: stages.map(|ss| {
            ss.into_iter()
                .map(|s| WasmStageStats {
                    id: s.stage_id as u32,
                    start_index: s.start_index as u32,
                    end_index: s.end_index as u32,
                    start_location: s.start_location,
                    end_location: s.end_location,
                    total_distance_km: s.total_distance_km,
                    total_elevation_gain_m: s.total_elevation_gain_m,
                    total_elevation_loss_m: s.total_elevation_loss_m,
                    avg_slope: s.avg_slope,
                    max_slope: s.max_slope,
                    min_elevation: s.min_elevation,
                    max_elevation: s.max_elevation,
                    start_time: s.start_time,
                    end_time: s.end_time,
                    bearing: s.bearing,
                    difficulty: s.difficulty,
                    estimated_duration_s: s.estimated_duration_s,
                    pace_factor: s.pace_factor,
                    max_completion_time: s.max_completion_time,
                    cutoff_ratio: s.cutoff_ratio,
                    stop_duration: s.stop_duration,
                })
                .collect()
        }),
        metadata: WasmGpxMetadata {
            name: trace.metadata.name.clone(),
            description: trace.metadata.description.clone(),
        },
    }
}

// ── Options for analyzeGpx / Trace::analyze ───────────────────────────────────

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct WasmWeatherEntry {
    name: String,
    temperature_c: f64,
    humidity_pct: f64,
    wind_kmh: f64,
    precip_prob_pct: f64,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct WasmAnalyzeOptions {
    base_pace_s_per_km: f64,
    k_fatigue: f64,
    life_base_stop_s: u32,
    #[serde(default)]
    weather: Vec<WasmWeatherEntry>,
}

impl WasmAnalyzeOptions {
    fn weather_lookup(&self) -> WeatherLookup {
        if self.weather.is_empty() {
            return WeatherLookup::empty();
        }
        let (names, values) = self
            .weather
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
}

#[cfg(test)]
mod pipeline_tests {
    use super::*;

    // 5 track-points plus 3 typed waypoints (Start / LifeBase / Arrival),
    // each waypoint coinciding exactly with a track-point so
    // `find_closest_point_from` resolves to distinct, increasing indices.
    const SAMPLE_GPX: &[u8] = br#"<?xml version="1.0"?>
<gpx>
<trk><trkseg>
<trkpt lat="45.000" lon="7.000"><ele>1000</ele></trkpt>
<trkpt lat="45.010" lon="7.010"><ele>1100</ele></trkpt>
<trkpt lat="45.020" lon="7.020"><ele>1050</ele></trkpt>
<trkpt lat="45.030" lon="7.030"><ele>1200</ele></trkpt>
<trkpt lat="45.040" lon="7.040"><ele>1150</ele></trkpt>
</trkseg></trk>
<wpt lat="45.000" lon="7.000"><name>Start</name><type>Start</type><time>2025-11-20T06:00:00Z</time></wpt>
<wpt lat="45.020" lon="7.020"><name>Life Base</name><type>LifeBase</type><time>2025-11-20T10:00:00Z</time><stopDuration>1800</stopDuration></wpt>
<wpt lat="45.040" lon="7.040"><name>Arrival</name><type>Arrival</type><time>2025-11-20T16:00:00Z</time></wpt>
</gpx>"#;

    fn sample_options() -> WasmAnalyzeOptions {
        WasmAnalyzeOptions {
            base_pace_s_per_km: 500.0,
            k_fatigue: 0.002,
            life_base_stop_s: 3600,
            weather: Vec::new(),
        }
    }

    #[test]
    fn parse_gpx_stores_waypoints_and_metadata_on_the_trace() {
        let trace = parse_gpx(SAMPLE_GPX).expect("sample GPX should parse");
        assert_eq!(trace.inner.locations.len(), 5);
        assert_eq!(trace.waypoints.len(), 3);
        assert_eq!(trace.waypoints[0].name, "Start");
        // No <metadata> block in the fixture, so parse_metadata falls back to
        // the first <name> anywhere in the document — here, the waypoint's.
        assert_eq!(trace.metadata.name.as_deref(), Some("Start"));
    }

    #[test]
    fn analyze_computes_legs_sections_and_stages_from_a_parsed_trace() {
        let trace = parse_gpx(SAMPLE_GPX).expect("sample GPX should parse");
        let analysis = compute_route_analysis(&trace, &sample_options());

        assert_eq!(analysis.waypoints.len(), 3);
        assert_eq!(analysis.legs.len(), 2);

        let sections = analysis
            .sections
            .expect("3 typed waypoints should yield sections");
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].stage_idx, 0);
        // Start (06:00) -> LifeBase (10:00): a 4h cutoff window.
        assert_eq!(sections[0].max_completion_time, Some(4 * 3600));

        let stages = analysis
            .stages
            .expect("Start/LifeBase/Arrival should yield stages");
        assert_eq!(stages.len(), 2);
    }

    #[test]
    fn build_trace_path_has_no_waypoints_so_analyze_returns_empty_route_data() {
        let flat = [7.0, 45.0, 1000.0, 7.01, 45.01, 1100.0];
        let trace = build_trace(&flat).expect("flat coords should build a trace");
        let analysis = compute_route_analysis(&trace, &sample_options());

        assert!(analysis.waypoints.is_empty());
        assert!(analysis.legs.is_empty());
        assert!(analysis.sections.is_none());
        assert!(analysis.stages.is_none());
    }
}

#[cfg(test)]
mod analyze_options_tests {
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

// ── Serializable output types for analyzeGpx / Trace::analyze ────────────────

#[derive(serde::Serialize)]
struct WasmTraceSummary {
    total_distance_km: f64,
    total_elevation_gain_m: f64,
    total_elevation_loss_m: f64,
    location_count: u32,
}

#[derive(serde::Serialize)]
struct WasmWaypoint {
    latitude: f64,
    longitude: f64,
    elevation: Option<f64>,
    name: String,
    wpt_type: Option<String>,
    time: Option<i64>,
    stop_duration: Option<u32>,
}

#[derive(serde::Serialize)]
struct WasmLegStats {
    leg_id: u32,
    section_idx: u32,
    start_index: u32,
    end_index: u32,
    start_location: String,
    end_location: String,
    total_distance_km: f64,
    total_elevation_gain_m: f64,
    total_elevation_loss_m: f64,
    avg_slope: f64,
    max_slope: f64,
    min_elevation: f64,
    max_elevation: f64,
    bearing: f64,
    difficulty: u8,
    estimated_duration_s: f64,
}

#[derive(serde::Serialize)]
struct WasmSectionStats {
    id: u32,
    /// Index of the stage this section belongs to.
    stage_idx: u32,
    start_index: u32,
    end_index: u32,
    start_location: String,
    end_location: String,
    total_distance_km: f64,
    total_elevation_gain_m: f64,
    total_elevation_loss_m: f64,
    avg_slope: f64,
    max_slope: f64,
    min_elevation: f64,
    max_elevation: f64,
    start_time: Option<i64>,
    end_time: Option<i64>,
    bearing: f64,
    difficulty: u8,
    estimated_duration_s: f64,
    pace_factor: f64,
    max_completion_time: Option<i64>,
    cutoff_ratio: Option<f64>,
    stop_duration: Option<u32>,
}

#[derive(serde::Serialize)]
struct WasmStageStats {
    id: u32,
    start_index: u32,
    end_index: u32,
    start_location: String,
    end_location: String,
    total_distance_km: f64,
    total_elevation_gain_m: f64,
    total_elevation_loss_m: f64,
    avg_slope: f64,
    max_slope: f64,
    min_elevation: f64,
    max_elevation: f64,
    start_time: Option<i64>,
    end_time: Option<i64>,
    bearing: f64,
    difficulty: u8,
    estimated_duration_s: f64,
    pace_factor: f64,
    max_completion_time: Option<i64>,
    cutoff_ratio: Option<f64>,
    stop_duration: Option<u32>,
}

#[derive(serde::Serialize)]
struct WasmGpxMetadata {
    name: Option<String>,
    description: Option<String>,
}

#[derive(serde::Serialize)]
struct WasmRouteAnalysis {
    waypoints: Vec<WasmWaypoint>,
    legs: Vec<WasmLegStats>,
    sections: Option<Vec<WasmSectionStats>>,
    stages: Option<Vec<WasmStageStats>>,
    metadata: WasmGpxMetadata,
}

#[derive(serde::Serialize)]
struct WasmGpxFull {
    trace: WasmTraceSummary,
    waypoints: Vec<WasmWaypoint>,
    legs: Vec<WasmLegStats>,
    sections: Option<Vec<WasmSectionStats>>,
    stages: Option<Vec<WasmStageStats>>,
    metadata: WasmGpxMetadata,
}

/// Build a trace from a flat `Float64Array` of interleaved coordinates:
/// `[lon₀, lat₀, alt₀,  lon₁, lat₁, alt₁, …]`.
///
/// Data is copied from JS memory into WASM once here; all subsequent work
/// stays inside WASM.  Returns a `Trace` class instance whose methods can
/// be called with near-zero boundary overhead, or `null` if `flat` carries no
/// points.
///
/// There's no GPX source here, so `.analyze()` on this trace will see no
/// waypoints (empty `legs`, `sections`/`stages` both `null`).
#[wasm_bindgen(js_name = "buildTrace")]
pub fn build_trace(flat: &[f64]) -> Option<Trace> {
    let locations: Vec<Location> = flat
        .chunks_exact(3)
        .map(|c| Location {
            longitude: c[0],
            latitude: c[1],
            altitude: c[2],
        })
        .collect();
    core_build_trace(&locations).ok().map(|inner| Trace {
        inner,
        waypoints: Vec::new(),
        metadata: crate::gpx::GpxMetadata {
            name: None,
            description: None,
        },
    })
}

// ── Trace class ───────────────────────────────────────────────────────────

/// Opaque handle to a computed `Trace` that lives entirely in WASM memory.
///
/// **Memory note**: call `.free()` when done (or register a `FinalizationRegistry`)
/// because Rust's allocator cannot reclaim this memory from JS GC alone.
///
/// ```js
/// const registry = new FinalizationRegistry(t => t.free());
/// const trace = buildTrace(pts);
/// registry.register(trace, trace);
/// ```
#[wasm_bindgen]
pub struct Trace {
    inner: crate::trace::Trace,
    waypoints: Vec<crate::waypoint::Waypoint>,
    metadata: crate::gpx::GpxMetadata,
}

#[wasm_bindgen]
impl Trace {
    // ── Scalar getters — free boundary crossing (passed in registers) ──────────

    #[wasm_bindgen(getter)]
    pub fn total_distance(&self) -> f64 {
        self.inner.total_distance
    }

    #[wasm_bindgen(getter)]
    pub fn total_elevation_gain(&self) -> f64 {
        self.inner.total_elevation_gain
    }

    #[wasm_bindgen(getter)]
    pub fn total_elevation_loss(&self) -> f64 {
        self.inner.total_elevation_loss
    }

    /// Number of (possibly D-P simplified) locations.
    #[wasm_bindgen(getter)]
    pub fn location_count(&self) -> u32 {
        self.inner.locations.len() as u32
    }

    // ── Array getters — O(n) copy from WASM → JS heap; call once and cache ────

    /// Flat `Float64Array` `[lon₀, lat₀, alt₀,  lon₁, lat₁, alt₁, …]`.
    #[wasm_bindgen(getter)]
    pub fn locations_flat(&self) -> Vec<f64> {
        self.inner
            .locations
            .iter()
            .flat_map(|l| [l.longitude, l.latitude, l.altitude])
            .collect()
    }

    #[wasm_bindgen(getter)]
    pub fn cumulative_distances(&self) -> Vec<f64> {
        self.inner.cumulative_distances.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn cumulative_elevation_gains(&self) -> Vec<f64> {
        self.inner.cumulative_elevation_gains.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn cumulative_elevation_losses(&self) -> Vec<f64> {
        self.inner.cumulative_elevation_losses.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn slopes(&self) -> Vec<f64> {
        self.inner.slopes.clone()
    }

    /// Peak indices as `Uint32Array`.
    #[wasm_bindgen(getter)]
    pub fn peaks(&self) -> Vec<u32> {
        self.inner.peaks.iter().map(|&i| i as u32).collect()
    }

    /// Valley indices as `Uint32Array`.
    #[wasm_bindgen(getter)]
    pub fn valleys(&self) -> Vec<u32> {
        self.inner.valleys.iter().map(|&i| i as u32).collect()
    }

    // ── Query methods — scalars in, cheap crossing; complex results via serde ──

    /// Index of the first location at or beyond `dist_km`.
    pub fn index_at_distance(&self, dist_km: f64) -> u32 {
        self.inner.index_at_distance(dist_km) as u32
    }

    /// Returns `{ longitude, latitude, altitude }`, or `undefined` when the
    /// distance is negative / the trace has no matching point.
    pub fn point_at_distance(&self, dist_km: f64) -> Option<JsValue> {
        self.inner
            .point_at_distance(dist_km)
            .and_then(|loc| serde_wasm_bindgen::to_value(loc).ok())
    }

    /// Returns `{ location: { longitude, latitude, altitude }, index, distance }`,
    /// or `undefined` when no closest point is found.
    pub fn find_closest_point(
        &self,
        longitude: f64,
        latitude: f64,
        altitude: f64,
    ) -> Option<JsValue> {
        let target = Location {
            longitude,
            latitude,
            altitude,
        };
        closest_result(self.inner.find_closest_point(&target))
    }

    /// Like `find_closest_point` but only searches from `start_from` onwards —
    /// use this on live-tracking loops to avoid snapping to an earlier position.
    pub fn find_closest_point_from(
        &self,
        longitude: f64,
        latitude: f64,
        altitude: f64,
        start_from: u32,
    ) -> Option<JsValue> {
        let target = Location {
            longitude,
            latitude,
            altitude,
        };
        closest_result(
            self.inner
                .find_closest_point_from(&target, start_from as usize),
        )
    }

    /// Flat `Float64Array` `[lon, lat, alt, …]` for all points between the two
    /// cumulative distances, or `undefined` on invalid input.
    pub fn slice_between_distances(&self, start_km: f64, end_km: f64) -> Option<Vec<f64>> {
        self.inner
            .slice_between_distances(start_km, end_km)
            .map(locs_to_flat)
    }

    /// Flat `Float64Array` `[lon, lat, alt, …]` for the index range (inclusive).
    /// Throws on out-of-bounds or invalid input.
    pub fn get_section(&self, start_index: u32, end_index: u32) -> Result<Vec<f64>, JsError> {
        self.inner
            .get_section(start_index as usize, end_index as usize)
            .map(|locs| locs_to_flat(&locs))
            .map_err(|e| JsError::new(&e.to_string()))
    }

    /// Returns `{ min_longitude, max_longitude, min_latitude, max_latitude }`.
    pub fn area(&self) -> JsValue {
        serde_wasm_bindgen::to_value(&self.inner.area()).unwrap_or(JsValue::NULL)
    }

    /// Returns `{ positive, negative }` (raw, non-denoised elevation totals).
    pub fn elevation(&self) -> JsValue {
        serde_wasm_bindgen::to_value(&self.inner.elevation()).unwrap_or(JsValue::NULL)
    }

    /// Returns an array of climb objects:
    /// `[{ start_index, end_index, start_dist_km, climb_dist_km, elevation_gain, summit_elev, avg_gradient }, …]`
    pub fn climbs(&self) -> JsValue {
        serde_wasm_bindgen::to_value(&self.inner.climbs).unwrap_or(JsValue::UNDEFINED)
    }

    /// Compute the race analysis (waypoints, legs, sections, stages) using
    /// this trace's waypoints (parsed once by `parseGpx`). Pure
    /// opaque-handle-in, JSON-out — no bytes cross the boundary again, and
    /// the expensive trace computation (simplification, elevation, climbs)
    /// is never repeated.
    ///
    /// Returns a JS object with `{ waypoints, legs, sections, stages, metadata }`
    /// or `null` when `options` is malformed.
    pub fn analyze(&self, options: JsValue) -> Option<JsValue> {
        let options: WasmAnalyzeOptions = serde_wasm_bindgen::from_value(options).ok()?;
        let analysis = compute_route_analysis(self, &options);
        serde_wasm_bindgen::to_value(&analysis).ok()
    }
}

// ── Private helpers ───────────────────────────────────────────────────────────

fn closest_result(result: Option<(&Location, usize, f64)>) -> Option<JsValue> {
    let (loc, idx, dist) = result?;
    serde_wasm_bindgen::to_value(&ClosestPointResult {
        location: loc,
        index: idx as u32,
        distance: dist,
    })
    .ok()
}

fn locs_to_flat(locs: &[Location]) -> Vec<f64> {
    locs.iter()
        .flat_map(|l| [l.longitude, l.latitude, l.altitude])
        .collect()
}

#[derive(serde::Serialize)]
struct ClosestPointResult<'a> {
    location: &'a Location,
    index: u32,
    distance: f64,
}
