use wasm_bindgen::prelude::*;

use crate::calibration::{self, BoundaryKind};
use crate::{build_trace as core_build_trace, Location};

mod dto;
mod options;
mod trace;

use dto::{
    WasmGpxFull, WasmGpxMetadata, WasmRecalibration, WasmRouteAnalysis, WasmSectionStats,
    WasmStageStats, WasmTraceSummary, WasmWaypoint,
};
use options::{WasmAnalyzeOptions, WasmRecalibrateOptions};
pub use trace::Trace;

// ── Console warning helper (no web-sys dep) ──────────────────────────────────

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console, js_name = warn)]
    fn console_warn(msg: &str);
}

/// Log a warning to the browser console (only in debug builds to avoid noise).
#[inline]
fn warn(msg: &str) {
    console_warn(msg);
}

// ── Public entry points ───────────────────────────────────────────────────────

/// Parse a GPX file from raw bytes (e.g. a `Uint8Array` read on the JS side)
/// into a `Trace` — **only `<trkpt>` track-points are parsed**; waypoints and
/// metadata are intentionally ignored so this path stays as lean as possible.
///
/// Call [`parseWaypoints`] and/or [`parseMetadata`] separately when you need
/// that data.  If you need the full race analysis in one shot, use
/// [`analyzeGpx`] instead.
///
/// Returns `null` when the GPX contains no valid track-points.
#[wasm_bindgen(js_name = "parseGpx")]
pub fn parse_gpx(bytes: &[u8]) -> Option<Trace> {
    let locations = crate::gpx::parse_trace_points(bytes);
    let inner = core_build_trace(&locations).ok()?;
    Some(Trace::new(
        inner,
        Vec::new(),
        crate::gpx::GpxMetadata {
            name: None,
            description: None,
        },
    ))
}

/// Parse only the `<wpt>` waypoints from raw GPX bytes.
///
/// Returns a JS array of `{ latitude, longitude, elevation, name, wptType,
/// time, stopDuration }` objects, or an empty array when none are found.
/// Call this only when you actually need waypoints — parsing is O(N) over
/// the full byte slice.
#[wasm_bindgen(js_name = "parseWaypoints")]
pub fn parse_waypoints_js(bytes: &[u8]) -> JsValue {
    let waypoints: Vec<WasmWaypoint> = crate::gpx::parse_waypoints(bytes)
        .iter()
        .map(Into::into)
        .collect();
    serde_wasm_bindgen::to_value(&waypoints).unwrap_or(JsValue::UNDEFINED)
}

/// Parse only the `<metadata>` block (or root-level `<name>`/`<desc>`) from
/// raw GPX bytes.
///
/// Returns a JS object `{ name, description }` (fields are `null` when absent).
/// Call this only when you actually need the metadata.
#[wasm_bindgen(js_name = "parseMetadata")]
pub fn parse_metadata_js(bytes: &[u8]) -> JsValue {
    let metadata = WasmGpxMetadata::from(&crate::gpx::parse_metadata(bytes));
    serde_wasm_bindgen::to_value(&metadata).unwrap_or(JsValue::UNDEFINED)
}

/// Parse a GPX file and compute the full analysis in one call: trace metrics,
/// waypoints, legs (Naismith), sections and stages (Minetti pace model).
///
/// Use this when you need **both** the race analysis (legs/sections/stages) and
/// bulk trace arrays in a single JS object, without holding a `Trace` handle.
/// It parses track-points, waypoints and metadata in a single `bytes` pass
/// each, then runs the full pipeline.
///
/// If you only need raw GPS data (elevation profile, distances, peaks…) use
/// [`parseGpx`] instead — it skips waypoints and metadata entirely.
/// If you already hold a `Trace` from `parseGpx`, you can pass waypoints
/// obtained from [`parseWaypoints`] to the server-side analysis instead of
/// calling this function and re-parsing the bytes.
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
    let options: WasmAnalyzeOptions = serde_wasm_bindgen::from_value(options)
        .map_err(|e| warn(&format!("navigo: analyzeGpx options parse error: {e}")))
        .ok()?;
    let trace = parse_wasm_trace(bytes)?;

    let analysis = compute_route_analysis(&trace, &options);
    let result = WasmGpxFull::new(WasmTraceSummary::from(trace.inner()), analysis);

    serde_wasm_bindgen::to_value(&result)
        .map_err(|e| warn(&format!("navigo: analyzeGpx serialization error: {e}")))
        .ok()
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
    core_build_trace(&locations).ok().map(|inner| {
        Trace::new(
            inner,
            Vec::new(),
            crate::gpx::GpxMetadata {
                name: None,
                description: None,
            },
        )
    })
}

/// Parse GPX `bytes` into a `Trace` that carries **all three** — track-points,
/// waypoints and metadata — in a single (triple-scan) pass.
///
/// Used only internally by `analyzeGpx`, where waypoints are required for the
/// legs/sections/stages pipeline. Public consumers who need waypoints
/// separately should call `parse_waypoints_js` / `parse_metadata_js`.
fn parse_wasm_trace(bytes: &[u8]) -> Option<Trace> {
    let parsed = crate::gpx::parse_all(bytes);
    let inner = core_build_trace(&parsed.locations).ok()?;
    Some(Trace::new(inner, parsed.waypoints, parsed.metadata))
}

/// Shared implementation behind `analyzeGpx` and `Trace::analyze`.
fn compute_route_analysis(trace: &Trace, options: &WasmAnalyzeOptions) -> WasmRouteAnalysis {
    let analysis_opts = options.to_analysis_options();

    let legs = crate::leg::compute_from_waypoints(trace.inner(), trace.waypoints());
    let sections =
        crate::section::compute_from_waypoints(trace.inner(), trace.waypoints(), &analysis_opts);
    let stages =
        crate::stage::compute_from_waypoints(trace.inner(), trace.waypoints(), &analysis_opts);

    WasmRouteAnalysis::new(
        trace.waypoints().iter().map(Into::into).collect(),
        legs.into_iter().map(Into::into).collect(),
        sections.map(|ss| ss.into_iter().map(WasmSectionStats::from).collect()),
        stages.map(|ss| ss.into_iter().map(WasmStageStats::from).collect()),
        trace.metadata().into(),
    )
}

/// Shared implementation behind `Trace::recalibrate`.
///
/// Runs `recalibrate_from_current` once per boundary kind — sections
/// (checkpoint granularity) and stages (LifeBase granularity) — since each
/// re-solves its own calibration factor against its own weather-per-range
/// lookups rather than sharing one result.
fn compute_recalibration(trace: &Trace, options: &WasmRecalibrateOptions) -> WasmRecalibration {
    let analysis_opts = options.to_analysis_options();

    let sections = calibration::recalibrate_from_current(
        trace.inner(),
        trace.waypoints(),
        BoundaryKind::Section,
        options.current_index(),
        options.actual_elapsed_s(),
        &analysis_opts,
    );
    let stages = calibration::recalibrate_from_current(
        trace.inner(),
        trace.waypoints(),
        BoundaryKind::Stage,
        options.current_index(),
        options.actual_elapsed_s(),
        &analysis_opts,
    );

    WasmRecalibration::new(sections, stages)
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

    #[test]
    fn parse_gpx_returns_trace_points_only_no_waypoints_no_metadata() {
        let trace = parse_gpx(SAMPLE_GPX).expect("sample GPX should parse");
        assert_eq!(trace.inner().locations.len(), 5);
        // parseGpx is the lean path — waypoints and metadata are NOT loaded.
        assert!(trace.waypoints().is_empty());
        assert!(trace.metadata().name.is_none());
    }

    #[test]
    fn parse_waypoints_js_returns_all_waypoints() {
        // Test the underlying GPX parser — serde_wasm_bindgen::to_value cannot
        // run on native targets, so we validate the raw parse result instead.
        let waypoints = crate::gpx::parse_waypoints(SAMPLE_GPX);
        assert_eq!(waypoints.len(), 3);
        assert_eq!(waypoints[0].name, "Start");
        assert_eq!(waypoints[1].name, "Life Base");
        assert_eq!(waypoints[2].name, "Arrival");
    }

    #[test]
    fn parse_metadata_js_falls_back_to_first_name_tag() {
        // Test the underlying GPX parser — serde_wasm_bindgen::to_value cannot
        // run on native targets, so we validate the raw parse result instead.
        let metadata = crate::gpx::parse_metadata(SAMPLE_GPX);
        // No <metadata> block in the fixture, so falls back to the first <name>
        // anywhere in the document — here, the first waypoint's name.
        assert_eq!(metadata.name.as_deref(), Some("Start"));
    }

    // Asserts on the serialized JSON shape (what JS actually receives) rather
    // than poking at DTO internals, so the DTOs don't need test-only accessors.
    #[test]
    fn analyze_computes_legs_sections_and_stages_from_a_parsed_trace() {
        // parse_wasm_trace (the full path) loads waypoints — required for analysis.
        let trace = parse_wasm_trace(SAMPLE_GPX).expect("sample GPX should parse");
        let analysis = compute_route_analysis(&trace, &WasmAnalyzeOptions::sample());
        let json = serde_json::to_value(&analysis).expect("analysis should serialize");

        assert_eq!(json["waypoints"].as_array().unwrap().len(), 3);
        assert_eq!(json["legs"].as_array().unwrap().len(), 2);

        let sections = json["sections"]
            .as_array()
            .expect("3 typed waypoints should yield sections");
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0]["stageIdx"], 0);
        // Start (06:00) -> LifeBase (10:00): a 4h cutoff window.
        assert_eq!(sections[0]["maxCompletionTime"], 4 * 3600);

        let stages = json["stages"]
            .as_array()
            .expect("Start/LifeBase/Arrival should yield stages");
        assert_eq!(stages.len(), 2);
    }

    #[test]
    fn build_trace_path_has_no_waypoints_so_analyze_returns_empty_route_data() {
        let flat = [7.0, 45.0, 1000.0, 7.01, 45.01, 1100.0];
        let trace = build_trace(&flat).expect("flat coords should build a trace");
        let analysis = compute_route_analysis(&trace, &WasmAnalyzeOptions::sample());
        let json = serde_json::to_value(&analysis).expect("analysis should serialize");

        assert!(json["waypoints"].as_array().unwrap().is_empty());
        assert!(json["legs"].as_array().unwrap().is_empty());
        assert!(json["sections"].is_null());
        assert!(json["stages"].is_null());
    }

    // Asserts on the serialized JSON shape (what JS actually receives) rather
    // than poking at DTO internals, so the DTOs don't need test-only accessors.
    #[test]
    fn recalibrate_computes_section_and_stage_etas_from_a_parsed_trace() {
        // parse_wasm_trace (the full path) loads waypoints — required for recalibration.
        let trace = parse_wasm_trace(SAMPLE_GPX).expect("sample GPX should parse");
        // current_index=2 puts the runner mid-route; actual_elapsed_s well above
        // the 300s gate so a non-trivial calibration factor is solved.
        let recalibration =
            compute_recalibration(&trace, &WasmRecalibrateOptions::sample(2, 3600.0));
        let json = serde_json::to_value(&recalibration).expect("recalibration should serialize");

        let sections = json["sections"]
            .as_object()
            .expect("Start/LifeBase/Arrival should yield section etas");
        assert_eq!(sections["etas"].as_array().unwrap().len(), 2);
        assert!(sections["calibrationFactor"].as_f64().unwrap() > 0.0);

        let stages = json["stages"]
            .as_object()
            .expect("Start/LifeBase/Arrival should yield stage etas");
        assert_eq!(stages["etas"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn recalibrate_returns_null_for_both_kinds_with_fewer_than_two_boundaries() {
        let flat = [7.0, 45.0, 1000.0, 7.01, 45.01, 1100.0];
        let trace = build_trace(&flat).expect("flat coords should build a trace");
        let recalibration = compute_recalibration(&trace, &WasmRecalibrateOptions::sample(0, 0.0));
        let json = serde_json::to_value(&recalibration).expect("recalibration should serialize");

        assert!(json["sections"].is_null());
        assert!(json["stages"].is_null());
    }
}
