use wasm_bindgen::prelude::*;

use crate::{build_trace as core_build_trace, Location};

// ── Public entry points ───────────────────────────────────────────────────────

/// Parse a GPX file from raw bytes (e.g. a `Uint8Array` read on the JS side)
/// and build a `WasmTrace` from all `<trkpt>` elements.
///
/// Returns `null` when the GPX contains no valid track-points.
#[wasm_bindgen(js_name = "parseGpx")]
pub fn parse_gpx(bytes: &[u8]) -> Option<WasmTrace> {
    let locations = crate::gpx::parse_trace_points(bytes);
    core_build_trace(&locations)
        .ok()
        .map(|inner| WasmTrace { inner })
}

/// Parse a GPX file and compute the full analysis: trace metrics, waypoints,
/// legs (Naismith), sections and stages (Minetti pace model).
///
/// `base_pace_s_per_km` — flat-terrain pace in s/km (e.g. 500 = 8:20/km).
/// `k_fatigue`          — exponential fatigue coefficient (e.g. 0.002).
/// `life_base_stop_s`   — default planned stop at LifeBase checkpoints (seconds).
///
/// Returns a JS object with `{ trace, waypoints, legs, sections, stages, metadata }`
/// or `null` when the GPX contains no valid track-points.
#[wasm_bindgen(js_name = "parseGpxFull")]
pub fn parse_gpx_full(
    bytes: &[u8],
    base_pace_s_per_km: f64,
    k_fatigue: f64,
    life_base_stop_s: u32,
) -> Option<JsValue> {
    let locations = crate::gpx::parse_trace_points(bytes);
    let trace = core_build_trace(&locations).ok()?;
    let waypoints = crate::gpx::parse_waypoints(bytes);
    let metadata = crate::gpx::parse_metadata(bytes);
    let weather = crate::pace_model::WeatherLookup::empty();

    let legs = crate::leg::compute_from_waypoints(&trace, &waypoints);
    let sections =
        crate::section::compute_from_waypoints(&trace, &waypoints, base_pace_s_per_km, k_fatigue, life_base_stop_s, &weather);
    let stages =
        crate::stage::compute_from_waypoints(&trace, &waypoints, base_pace_s_per_km, k_fatigue, life_base_stop_s, &weather);

    let result = WasmGpxFull {
        trace: WasmTraceSummary {
            total_distance_km: trace.total_distance,
            total_elevation_gain_m: trace.total_elevation_gain,
            total_elevation_loss_m: trace.total_elevation_loss,
            location_count: trace.locations.len() as u32,
        },
        waypoints: waypoints
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
            .iter()
            .map(|l| WasmLegStats {
                leg_id: l.leg_id as u32,
                section_idx: l.section_idx as u32,
                start_index: l.start_index as u32,
                end_index: l.end_index as u32,
                start_location: l.start_location.clone(),
                end_location: l.end_location.clone(),
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
            ss.iter()
                .map(|s| WasmIntervalStats {
                    id: s.section_id as u32,
                    group_idx: s.stage_idx as u32,
                    start_index: s.start_index as u32,
                    end_index: s.end_index as u32,
                    start_location: s.start_location.clone(),
                    end_location: s.end_location.clone(),
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
            ss.iter()
                .map(|s| WasmIntervalStats {
                    id: s.stage_id as u32,
                    group_idx: 0,
                    start_index: s.start_index as u32,
                    end_index: s.end_index as u32,
                    start_location: s.start_location.clone(),
                    end_location: s.end_location.clone(),
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
            name: metadata.name,
            description: metadata.description,
        },
    };

    serde_wasm_bindgen::to_value(&result).ok()
}

// ── Serializable output types for parseGpxFull ────────────────────────────────

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
struct WasmIntervalStats {
    id: u32,
    /// stage_idx for sections, 0 for stages.
    group_idx: u32,
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
struct WasmGpxFull {
    trace: WasmTraceSummary,
    waypoints: Vec<WasmWaypoint>,
    legs: Vec<WasmLegStats>,
    sections: Option<Vec<WasmIntervalStats>>,
    stages: Option<Vec<WasmIntervalStats>>,
    metadata: WasmGpxMetadata,
}

/// Build a trace from a flat `Float64Array` of interleaved coordinates:
/// `[lon₀, lat₀, alt₀,  lon₁, lat₁, alt₁, …]`.
///
/// Data is copied from JS memory into WASM once here; all subsequent work
/// stays inside WASM.  Returns a `WasmTrace` class instance whose methods can
/// be called with near-zero boundary overhead, or `null` if `flat` carries no
/// points.
#[wasm_bindgen(js_name = "buildTrace")]
pub fn build_trace(flat: &[f64]) -> Option<WasmTrace> {
    let locations: Vec<Location> = flat
        .chunks_exact(3)
        .map(|c| Location {
            longitude: c[0],
            latitude: c[1],
            altitude: c[2],
        })
        .collect();
    core_build_trace(&locations)
        .ok()
        .map(|inner| WasmTrace { inner })
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
