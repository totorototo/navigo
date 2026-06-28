use wasm_bindgen::prelude::*;

use crate::Location;

use super::options::{WasmAnalyzeOptions, WasmRecalibrateOptions};

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

// Plain (non-wasm_bindgen) constructor and accessors used internally by
// `wasm.rs` — kept out of the `#[wasm_bindgen] impl` block below so they
// aren't picked up as part of the JS-facing API.
impl Trace {
    pub(crate) fn new(
        inner: crate::trace::Trace,
        waypoints: Vec<crate::waypoint::Waypoint>,
        metadata: crate::gpx::GpxMetadata,
    ) -> Self {
        Self {
            inner,
            waypoints,
            metadata,
        }
    }

    pub(crate) fn inner(&self) -> &crate::trace::Trace {
        &self.inner
    }

    pub(crate) fn waypoints(&self) -> &[crate::waypoint::Waypoint] {
        &self.waypoints
    }

    pub(crate) fn metadata(&self) -> &crate::gpx::GpxMetadata {
        &self.metadata
    }
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
        let analysis = super::compute_route_analysis(self, &options);
        serde_wasm_bindgen::to_value(&analysis).ok()
    }

    /// Live, mid-race ETA recalibration — corrects the static `.analyze()`
    /// prediction against the runner's actual progress so far.
    ///
    /// `options` — a JS object: `{ basePaceSPerKm, kFatigue, lifeBaseStopS,
    /// currentIndex, actualElapsedS, weather? }`. `currentIndex` is the
    /// runner's current point on this trace (e.g. from `find_closest_point`);
    /// `actualElapsedS` is real seconds elapsed since race start.
    ///
    /// Returns `{ sections, stages }`, each either `null` (fewer than 2
    /// boundaries of that kind) or `{ calibration_factor,
    /// calibrated_base_pace_s_per_km, predicted_so_far_s, actual_elapsed_s,
    /// etas: [{ id, end_index, remaining_duration_s, cumulative_remaining_s }, …] }`.
    /// Returns `null` when `options` is malformed.
    pub fn recalibrate(&self, options: JsValue) -> Option<JsValue> {
        let options: WasmRecalibrateOptions = serde_wasm_bindgen::from_value(options).ok()?;
        let recalibration = super::compute_recalibration(self, &options);
        serde_wasm_bindgen::to_value(&recalibration).ok()
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
