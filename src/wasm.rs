use wasm_bindgen::prelude::*;

use crate::{build_trace as core_build_trace, Location};

// ── Public entry point ────────────────────────────────────────────────────────

/// Build a trace from a flat `Float64Array` of interleaved coordinates:
/// `[lon₀, lat₀, alt₀,  lon₁, lat₁, alt₁, …]`.
///
/// Data is copied from JS memory into WASM once here; all subsequent work
/// stays inside WASM.  Returns a `WasmTrace` class instance whose methods can
/// be called with near-zero boundary overhead.
#[wasm_bindgen(js_name = "buildTrace")]
pub fn build_trace(flat: &[f64]) -> WasmTrace {
    let locations: Vec<Location> = flat
        .chunks_exact(3)
        .map(|c| Location {
            longitude: c[0],
            latitude: c[1],
            altitude: c[2],
        })
        .collect();
    WasmTrace {
        inner: core_build_trace(&locations),
    }
}

// ── WasmTrace class ───────────────────────────────────────────────────────────

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
pub struct WasmTrace {
    inner: crate::trace::Trace,
}

#[wasm_bindgen]
impl WasmTrace {
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

    /// Returns `{ longitude, latitude, altitude }` or `null`.
    pub fn point_at_distance(&self, dist_km: f64) -> JsValue {
        match self.inner.point_at_distance(dist_km) {
            Some(loc) => serde_wasm_bindgen::to_value(loc).unwrap_or(JsValue::NULL),
            None => JsValue::NULL,
        }
    }

    /// Returns `{ location: { longitude, latitude, altitude }, index, distance }` or `null`.
    pub fn find_closest_point(&self, longitude: f64, latitude: f64, altitude: f64) -> JsValue {
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
    ) -> JsValue {
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
    /// cumulative distances, or `null` on invalid input.
    pub fn slice_between_distances(&self, start_km: f64, end_km: f64) -> JsValue {
        match self.inner.slice_between_distances(start_km, end_km) {
            Some(locs) => js_sys::Float64Array::from(locs_to_flat(locs).as_slice()).into(),
            None => JsValue::NULL,
        }
    }

    /// Flat `Float64Array` `[lon, lat, alt, …]` for the index range (inclusive),
    /// or `null` on out-of-bounds input.
    pub fn get_section(&self, start_index: u32, end_index: u32) -> JsValue {
        match self
            .inner
            .get_section(start_index as usize, end_index as usize)
        {
            Ok(locs) => js_sys::Float64Array::from(locs_to_flat(&locs).as_slice()).into(),
            Err(_) => JsValue::NULL,
        }
    }

    /// Returns `{ min_longitude, max_longitude, min_latitude, max_latitude }` or `null`.
    pub fn area(&self) -> JsValue {
        match self.inner.area() {
            Ok(a) => serde_wasm_bindgen::to_value(&a).unwrap_or(JsValue::NULL),
            Err(_) => JsValue::NULL,
        }
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
}

// ── Private helpers ───────────────────────────────────────────────────────────

fn closest_result(result: Option<(&Location, usize, f64)>) -> JsValue {
    match result {
        Some((loc, idx, dist)) => serde_wasm_bindgen::to_value(&ClosestPointResult {
            location: loc,
            index: idx as u32,
            distance: dist,
        })
        .unwrap_or(JsValue::NULL),
        None => JsValue::NULL,
    }
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
