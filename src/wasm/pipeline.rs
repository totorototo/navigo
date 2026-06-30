use crate::calibration::{self, BoundaryKind};

use super::dto::{WasmRecalibration, WasmRouteAnalysis, WasmSectionStats, WasmStageStats};
use super::options::{WasmAnalyzeOptions, WasmRecalibrateOptions};
use super::Trace;

/// Parse GPX `bytes` into a `Trace` that carries **all three** — track-points,
/// waypoints and metadata — in a single (triple-scan) pass.
///
/// Shared by `analyzeGpx` and the public `parseGpxAll`, both of
/// which need waypoints for the legs/sections/stages pipeline.
pub(super) fn parse_wasm_trace(bytes: &[u8]) -> Option<Trace> {
    let parsed = crate::gpx::parse_all(bytes);
    let inner = crate::build_trace(&parsed.locations).ok()?;
    Some(Trace::new(inner, parsed.waypoints, parsed.metadata))
}

/// Shared implementation behind `analyzeGpx` and `Trace::analyze`.
pub(super) fn compute_route_analysis(
    trace: &Trace,
    options: &WasmAnalyzeOptions,
) -> WasmRouteAnalysis {
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
pub(super) fn compute_recalibration(
    trace: &Trace,
    options: &WasmRecalibrateOptions,
) -> WasmRecalibration {
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
mod tests {
    use super::super::{build_trace, parse_gpx, parse_gpx_all};
    use super::*;
    use crate::wasm::dto::WasmWaypoint;

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
    fn parse_gpx_all_loads_waypoints() {
        let trace = parse_gpx_all(SAMPLE_GPX).expect("sample GPX should parse");
        assert_eq!(trace.inner().locations.len(), 5);
        assert_eq!(trace.waypoints().len(), 3);
        assert_eq!(trace.waypoints()[0].name, "Start");
    }

    #[test]
    fn parse_waypoints_js_returns_all_waypoints() {
        // Test the underlying GPX parser — serde_wasm_bindgen::to_value cannot
        // run on native targets, so we validate the raw parse result instead.
        let waypoints: Vec<WasmWaypoint> = crate::gpx::parse_waypoints(SAMPLE_GPX)
            .iter()
            .map(Into::into)
            .collect();
        let json = serde_json::to_value(&waypoints).expect("waypoints should serialize");

        assert_eq!(json.as_array().unwrap().len(), 3);
        assert_eq!(json[0]["name"], "Start");
        assert_eq!(json[1]["name"], "Life Base");
        assert_eq!(json[2]["name"], "Arrival");
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
