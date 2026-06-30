use wasm_bindgen::prelude::*;

use crate::{build_trace as core_build_trace, Location};

mod dto;
mod js;
mod options;
mod pipeline;
mod trace;

use dto::{WasmGpxFull, WasmGpxMetadata, WasmTraceSummary, WasmWaypoint};
use js::{deserialize, serialize, serialize_or_undefined};
use options::WasmAnalyzeOptions;
use pipeline::{compute_route_analysis, parse_wasm_trace};
pub use trace::WasmTrace as Trace;

// ── Public entry points ───────────────────────────────────────────────────────

/// Parse a GPX file from raw bytes (e.g. a `Uint8Array` read on the JS side)
/// into a `Trace` — **only `<trkpt>` track-points are parsed**; waypoints and
/// metadata are intentionally ignored so this path stays as lean as possible.
///
/// Call [`parseWaypoints`] and/or [`parseMetadata`] separately when you need
/// that data, or use [`parseGpxAll`] to get a `Trace` with both
/// already attached. If you need the full race analysis in one shot, use
/// [`analyzeGpx`] instead.
///
/// Failure contract:
/// - returns `null` when the GPX contains no valid track-points;
/// - never throws for malformed GPX bytes — invalid/missing track-points are
///   treated as an empty parse result.
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

/// Parse a GPX file into a `Trace` that also carries its waypoints and
/// metadata, so `.analyze()` / `.recalibrate()` work directly on the
/// returned handle — no separate [`parseWaypoints`] call or extra setup
/// needed. Slower than the lean [`parseGpx`] (triple-scans `bytes` instead
/// of once for track-points only); use this when you'll call `.recalibrate()`
/// repeatedly over the trace's lifetime, since the scan only happens once.
///
/// Failure contract:
/// - returns `null` when the GPX contains no valid track-points;
/// - never throws for malformed GPX bytes — invalid/missing track-points are
///   treated as an empty parse result.
#[wasm_bindgen(js_name = "parseGpxAll")]
pub fn parse_gpx_all(bytes: &[u8]) -> Option<Trace> {
    parse_wasm_trace(bytes)
}

/// Parse only the `<wpt>` waypoints from raw GPX bytes.
///
/// Returns a JS array of `{ latitude, longitude, elevation, name, wptType,
/// time, stopDuration }` objects, or an empty array when none are found.
/// Call this only when you actually need waypoints — parsing is O(N) over
/// the full byte slice.
///
/// Failure contract:
/// - returns `[]` when no waypoints are found;
/// - returns `undefined` only if JS serialization itself fails, after logging a
///   warning to `console.warn`.
#[wasm_bindgen(js_name = "parseWaypoints")]
pub fn parse_waypoints_js(bytes: &[u8]) -> JsValue {
    let waypoints: Vec<WasmWaypoint> = crate::gpx::parse_waypoints(bytes)
        .iter()
        .map(Into::into)
        .collect();
    serialize_or_undefined(&waypoints, "parseWaypoints")
}

/// Parse only the `<metadata>` block (or root-level `<name>`/`<desc>`) from
/// raw GPX bytes.
///
/// Returns a JS object `{ name, description }` (fields are `null` when absent).
/// Call this only when you actually need the metadata.
///
/// Failure contract:
/// - missing metadata still returns an object with `null` fields;
/// - returns `undefined` only if JS serialization itself fails, after logging a
///   warning to `console.warn`.
#[wasm_bindgen(js_name = "parseMetadata")]
pub fn parse_metadata_js(bytes: &[u8]) -> JsValue {
    let metadata = WasmGpxMetadata::from(&crate::gpx::parse_metadata(bytes));
    serialize_or_undefined(&metadata, "parseMetadata")
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
/// [`parseGpx`] instead — it skips waypoints and metadata entirely. If you
/// already hold a `Trace` and want to add waypoints to it, re-parse via
/// [`parseGpxAll`] rather than calling this function (which
/// discards the `Trace` handle, returning only JSON).
///
/// `options` — a JS object: `{ basePaceSPerKm, kFatigue, lifeBaseStopS, weather? }`.
/// `weather` is an optional array of
/// `{ name, temperatureC, humidityPct, windKmh, precipProbPct }`, matched against
/// checkpoint names; unmatched checkpoints use neutral weather.
///
/// Returns a JS object with `{ trace, waypoints, legs, sections, stages, metadata }`
/// or `null` when the GPX contains no valid track-points or `options` is malformed.
///
/// Failure contract:
/// - parse/validation failures return `null`;
/// - malformed `options` log a warning to `console.warn` before returning `null`;
/// - serialization failures also log a warning and return `null`;
/// - this function does not throw for malformed GPX/options input.
#[wasm_bindgen(js_name = "analyzeGpx")]
pub fn analyze_gpx(bytes: &[u8], options: JsValue) -> Option<JsValue> {
    let options: WasmAnalyzeOptions = deserialize(options, "analyzeGpx options")?;
    let trace = parse_wasm_trace(bytes)?;

    let analysis = compute_route_analysis(&trace, &options);
    let result = WasmGpxFull::new(WasmTraceSummary::from(trace.inner()), analysis);

    serialize(&result, "analyzeGpx")
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
///
/// Failure contract:
/// - returns `null` when `flat` contains no complete points;
/// - silently ignores a trailing 1- or 2-value remainder in `flat`
///   (`chunks_exact(3)` semantics);
/// - does not throw on malformed numeric payloads.
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
