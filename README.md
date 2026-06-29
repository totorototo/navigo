# navigo

[![CI](https://github.com/totorototo/navigo/actions/workflows/ci.yml/badge.svg)](https://github.com/totorototo/navigo/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/navigo.svg)](https://crates.io/crates/navigo)
[![docs.rs](https://docs.rs/navigo/badge.svg)](https://docs.rs/navigo)
[![License](https://img.shields.io/crates/l/navigo.svg)](LICENSE)

GPS/geospatial analysis in Rust — trace processing, GPX parsing, Minetti pace model, race route analysis, and live recalibration.

# api / usage

## location

A location is a GPS coordinate defined by a longitude, a latitude, and an altitude.

```rust
let location = Location {
    longitude: 2.350987,
    latitude: 48.856667,
    altitude: 890.0,
};
```

- **distance to another location** (km):

```rust
let distance: f64 = paris.calculate_distance_to(&moscow);
```

- **bearing to another location** (degrees):

```rust
let bearing: f64 = paris.calculate_bearing_to(&moscow);
```

- **elevation change to another location**:

```rust
let elevation: Elevation = paris.calculate_elevation_to(&moscow);
// elevation.positive — gain in meters
// elevation.negative — loss in meters
```

- **check if inside a bounding box**:

```rust
let area = Area { min_latitude: 54.7, max_latitude: 56.7,
                  min_longitude: 36.6, max_longitude: 38.6 };
let is_in: bool = location.is_in_area(&area);
```

- **check if inside a radius** (km):

```rust
let is_in: bool = location.is_in_radius(&center, &70.0);
```

---

## trace

`Trace::new` ingests raw GPS locations and precomputes everything in one shot:

- Douglas-Peucker simplification (for traces > 1 000 points, ε = 15 m)
- Cumulative distances
- Denoised cumulative elevation gain / loss (median smoothing + hysteresis)
- Smoothed slope at each point
- Peaks and valleys (AMPD algorithm with prominence filter)
- Qualifying climb segments (Garmin-style thresholds)

A `Trace` is never empty — construction fails with `TraceError::EmptyTrace`
if `locations` is empty, so every other method can assume at least one point.

```rust
let trace: Result<Trace, TraceError> = Trace::new(&locations);
// or via the convenience wrapper:
let trace: Result<Trace, TraceError> = build_trace(&locations);
```

### precomputed fields

```rust
trace.locations                   // Vec<Location>  — simplified working set
trace.cumulative_distances        // Vec<f64>        — km from start, [0] == 0.0
trace.cumulative_elevation_gains  // Vec<f64>        — denoised gain in meters
trace.cumulative_elevation_losses // Vec<f64>        — denoised loss in meters
trace.slopes                      // Vec<f64>        — % grade at each point
trace.peaks                       // Vec<usize>      — indices of detected peaks
trace.valleys                     // Vec<usize>      — indices of detected valleys
trace.climbs                      // Vec<ClimbStats> — qualifying climb segments
trace.total_distance              // f64             — total distance in km
trace.total_elevation_gain        // f64             — total denoised gain in meters
trace.total_elevation_loss        // f64             — total denoised loss in meters
```

### methods

- **total length** (km):

```rust
let length: f64 = trace.length(); // alias for total_distance
```

- **location at a cumulative distance** (km):

```rust
let loc: Option<&Location> = trace.point_at_distance(42.0);
```

- **index at a cumulative distance** (binary search):

```rust
let idx: usize = trace.index_at_distance(42.0);
```

- **slice between two distance marks** (km, both ends inclusive):

```rust
let section: Option<&[Location]> = trace.slice_between_distances(10.0, 50.0);
```

- **closest location to a point** (early-stop heuristic for loop courses):

```rust
let (loc, idx, dist_km) = trace.find_closest_point(&target).unwrap();
// start search from a known index (e.g. to handle loop courses):
let result = trace.find_closest_point_from(&target, start_from);
```

- **bounding box** (never fails — a trace always has at least one point):

```rust
let area: Area = trace.area();
```

- **sub-section by index range** (inclusive):

```rust
let section: Result<Vec<Location>, TraceError> = trace.get_section(start_index, end_index);
```

### climb stats

```rust
pub struct ClimbStats {
    pub start_index:    usize, // valley index in trace.locations
    pub end_index:      usize, // summit index in trace.locations
    pub start_dist_km:  f64,
    pub climb_dist_km:  f64,
    pub elevation_gain: f64,   // meters
    pub summit_elev:    f64,   // meters
    pub avg_gradient:   f64,   // %
}
```

Climbs are qualified using Garmin Climb Pro thresholds:
`distance ≥ 500 m`, `average gradient ≥ 3 %`, `distance × gradient > 3 500 m·%`

---

## GPX parsing

Parse a `.gpx` file from raw bytes — no XML dependency, byte-scanning only.

```rust
use navigo::gpx::{parse_trace_points, parse_waypoints, parse_metadata};

let bytes = std::fs::read("route.gpx").unwrap();

// Extract track points as Vec<Location>
let locations = parse_trace_points(&bytes);

// Extract <wpt> elements as Vec<Waypoint>
let waypoints = parse_waypoints(&bytes);

// Extract <metadata> fields
let meta = parse_metadata(&bytes);
// meta.name        → Option<String>
// meta.description → Option<String>
```

---

## Waypoints

```rust
pub struct Waypoint {
    pub latitude:      f64,
    pub longitude:     f64,
    pub elevation:     Option<f64>,
    pub name:          String,
    pub description:   Option<String>,
    pub comment:       Option<String>,
    pub symbol:        Option<String>,
    pub wpt_type:      Option<String>,   // e.g. "Start", "LifeBase", "TimeBarrier"
    pub time:          Option<i64>,      // Unix timestamp (seconds)
    pub stop_duration: Option<u32>,      // planned stop at this point (seconds)
}
```

Boundary classification:

```rust
waypoint.is_section_boundary(); // true for any waypoint with a non-null type
waypoint.is_stage_boundary();   // true only for Start / LifeBase / Arrival
```

---

## Pace model

### Minetti 2002

```rust
use navigo::minetti::{cmet, pace_factor, CMET_FLAT};

let cost = cmet(0.10);          // metabolic cost at +10% grade (J/(kg·m))
let factor = pace_factor(0.10); // relative speed factor vs flat (= cmet/CMET_FLAT)
```

Domain: slope in `[-0.45, 0.45]` (clamped beyond).

### Fatigue, circadian & weather

```rust
use navigo::pace_model::{
    fatigue_factor, circadian_factor, WeatherLookup, WeatherConditions,
    K_FATIGUE, DEFAULT_BASE_PACE_S_PER_KM, DEFAULT_LIFE_BASE_STOP_S,
};

let fatigue   = fatigue_factor(d_eff_km, k_fatigue);  // exponential decay (≥ 1.0)
let circadian = circadian_factor(unix_time_s);         // cosine, −15% at 03:30 UTC

// WeatherConditions fields:
// temperature_c:   °C
// humidity_pct:    0–100
// wind_kmh:        km/h
// precip_prob_pct: 0–100

let weather = WeatherLookup::empty();
// or with per-checkpoint data:
let weather = WeatherLookup::new(
    vec!["La Mongie".to_string()],
    vec![WeatherConditions { temperature_c: 5.0, humidity_pct: 80.0, wind_kmh: 30.0, precip_prob_pct: 40.0 }],
);
let factor = weather.factor_for("La Mongie"); // combined thermal + wind + precip factor
```

Useful constants:

| Constant                     | Value   | Meaning                       |
| ---------------------------- | ------- | ----------------------------- |
| `K_FATIGUE`                  | `0.002` | Default fatigue coefficient   |
| `DEFAULT_BASE_PACE_S_PER_KM` | `500.0` | 8:20/km flat pace             |
| `DEFAULT_LIFE_BASE_STOP_S`   | `3600`  | Default LifeBase stop (1 h)   |
| `RECOVERY_LIFE_BASE`         | `0.20`  | 20 % effort reset at LifeBase |

---

## Route analysis

All three levels take a built `Trace` and a slice of `Waypoint`s derived from the same GPX file.

```rust
use navigo::gpx::{parse_trace_points, parse_waypoints};
use navigo::{build_trace, leg, section, stage, pace_model::WeatherLookup};

let bytes = std::fs::read("route.gpx").unwrap();
let trace = build_trace(&parse_trace_points(&bytes)).unwrap();
let waypoints = parse_waypoints(&bytes);
let weather = WeatherLookup::empty();

const BASE_PACE: f64 = 500.0;  // s/km on flat terrain
const K_FATIGUE: f64 = 0.002;
const LIFE_BASE_STOP: u32 = 3600; // 1 h planned stop at LifeBase checkpoints
```

### Legs

One `LegStats` per consecutive pair of section-boundary waypoints.

```rust
let legs: Vec<leg::LegStats> = leg::compute_from_waypoints(&trace, &waypoints);
// legs[i].leg_id
// legs[i].section_idx
// legs[i].start_location / end_location  (waypoint names)
// legs[i].total_distance_km
// legs[i].total_elevation_gain_m / total_elevation_loss_m
// legs[i].avg_slope / max_slope          (% grade)
// legs[i].min_elevation / max_elevation  (meters)
// legs[i].bearing                        (degrees from north)
// legs[i].difficulty                     (1–5, Naismith effort)
// legs[i].estimated_duration_s           (Naismith rule, no fatigue)
```

### Sections

Sections are legs enriched with pace-model data (Minetti + fatigue + circadian + weather).

```rust
let sections: Option<Vec<section::SectionStats>> =
    section::compute_from_waypoints(&trace, &waypoints, BASE_PACE, K_FATIGUE, LIFE_BASE_STOP, &weather);
// sections[i].section_id / stage_idx
// sections[i].start_location / end_location
// sections[i].total_distance_km
// sections[i].total_elevation_gain_m / total_elevation_loss_m
// sections[i].avg_slope / max_slope / min_elevation / max_elevation
// sections[i].start_time / end_time      (Unix timestamps from waypoint <time>, or None)
// sections[i].bearing / difficulty
// sections[i].pace_factor                — combined speed factor vs flat
// sections[i].estimated_duration_s       — moving time + planned stop
// sections[i].max_completion_time        — cutoff as Unix timestamp, or None
// sections[i].cutoff_ratio               — estimated_duration / time_budget (< 1.0 = ok)
// sections[i].stop_duration              — planned stop at end checkpoint (s), or None
```

### Stages

Stages group sections between Start / LifeBase / Arrival boundaries (TimeBarrier waypoints are skipped).

```rust
let stages: Option<Vec<stage::StageStats>> =
    stage::compute_from_waypoints(&trace, &waypoints, BASE_PACE, K_FATIGUE, LIFE_BASE_STOP, &weather);
// stages[i].stage_id
// stages[i].start_location / end_location
// stages[i].total_distance_km
// stages[i].total_elevation_gain_m / total_elevation_loss_m
// stages[i].avg_slope / max_slope / min_elevation / max_elevation
// stages[i].start_time / end_time
// stages[i].bearing / difficulty
// stages[i].pace_factor / estimated_duration_s
// stages[i].max_completion_time / cutoff_ratio / stop_duration
```

---

## Time utilities

```rust
use navigo::time::parse_iso8601_to_epoch;

// Supported formats: "2025-11-20T12:00:00Z", "2025-11-20T12:00:00+01:00"
let epoch: Result<i64, _> = parse_iso8601_to_epoch("2025-11-20T12:00:00Z");
```

---

## Live calibration

Recalibrate remaining ETAs mid-race given the actual elapsed time at a known position.

```rust
use navigo::calibration::{recalibrate_from_current, BoundaryKind};

let result = recalibrate_from_current(
    &trace, &waypoints, BoundaryKind::Section,
    current_index,   // trace point index snapped to current position
    actual_elapsed_s,
    BASE_PACE, K_FATIGUE, LIFE_BASE_STOP, &weather,
);

if let Some(cal) = result {
    cal.calibration_factor;            // clamped to [0.5, 3.0]
    cal.calibrated_base_pace_s_per_km; // adjusted flat pace
    for eta in &cal.etas {
        eta.id;
        eta.remaining_duration_s;
        eta.cumulative_remaining_s;
    }
}
```

The factor is only applied when `predicted_so_far ≥ 300 s` to avoid noise from very short segments.

---

## WebAssembly

The library can be compiled to WASM for use in web applications via the `wasm` feature.
Prebuilt bindings are published to npm as [`@totorototo/navigo`](https://www.npmjs.com/package/@totorototo/navigo) — `npm install @totorototo/navigo`.

### how it works

All data lives in WASM linear memory. The JS side holds a thin pointer (`Trace`).
Only the boundaries cross the WASM↔JS membrane — scalars are free (registers), bulk arrays are copied once on demand.

```
buildTrace(Float64Array)          ← one O(n) copy JS→WASM, null if no points
       │
       ▼
Trace stays in WASM memory
       │
       ├── trace.totalDistance         → free (register)
       ├── trace.findClosestPoint()    → free (scalars in/out)
       ├── trace.locationsFlat         → one O(n) copy, cache it
       └── trace.free()               ← you must call this (no GC bridge)
```

### build

```bash
cargo install wasm-pack
wasm-pack build --target web -- --features wasm      # ES modules — Vite, plain browser
wasm-pack build --target bundler -- --features wasm  # webpack / Rollup
```

### usage

**From raw coordinates (`buildTrace`)**

```js
import init, { buildTrace } from "./navigo.js";
await init();

// build — one copy in, all computation in WASM
const pts = new Float64Array([
  2.350987,
  48.856667,
  0, // lon, lat, alt
  37.617634,
  55.755787,
  200,
]);
const trace = buildTrace(pts);
// → Trace, or null if pts carries no points

// scalar getters — free
trace.totalDistance; // number (km)
trace.totalElevationGain; // number (m)
trace.totalElevationLoss; // number (m)
trace.locationCount; // number

// array getters — copy once, then cache on the JS side
const locs = trace.locationsFlat; // Float64Array [lon,lat,alt,…]
const dists = trace.cumulativeDistances; // Float64Array (km)
const gains = trace.cumulativeElevationGains; // Float64Array (m)
const losses = trace.cumulativeElevationLosses; // Float64Array (m)
const slopes = trace.slopes; // Float64Array (%)
const peaks = trace.peaks; // Uint32Array  (indices)
const valleys = trace.valleys; // Uint32Array  (indices)

// query methods — scalars in, one small object out
trace.pointAtDistance(42.0);
// → { longitude, latitude, altitude } | undefined

trace.indexAtDistance(42.0);
// → number

trace.findClosestPoint(lon, lat, alt);
// → { location: { longitude, latitude, altitude }, index, distance } | undefined

trace.findClosestPointFrom(lon, lat, alt, lastIndex);
// → same shape | undefined  (use on live-tracking loops)

trace.sliceBetweenDistances(10.0, 50.0);
// → Float64Array [lon,lat,alt,…] | undefined

trace.getSection(startIndex, endIndex);
// → Float64Array [lon,lat,alt,…]  (throws on out-of-bounds / invalid range)

trace.area();
// → { minLongitude, maxLongitude, minLatitude, maxLatitude }

trace.elevation();
// → { positive, negative }  (raw, non-denoised)

trace.climbs();
// → [{ startIndex, endIndex, startDistKm, climbDistKm,
//      elevationGain, summitElev, avgGradient }, …]

// always release when done — Rust allocator has no GC bridge
trace.free();
```

**From a GPX file (`parseGpx` / `trace.analyze()` / `analyzeGpx`)**

```js
import init, { parseGpx, analyzeGpx } from "./navigo.js";
await init();

const bytes = new Uint8Array(
  await fetch("/route.gpx").then((r) => r.arrayBuffer()),
);

// Parse once — track-points, waypoints and metadata are all stored on the
// returned Trace, so nothing needs to be re-sent for the steps below.
const trace = parseGpx(bytes);
// → Trace | null  (same getters/methods as buildTrace, plus .analyze())

const options = { basePaceSPerKm: 500, kFatigue: 0.002, lifeBaseStopS: 3600 };

// Race analysis from the trace you already have — no bytes cross the
// boundary again, and the expensive trace computation isn't repeated.
const analysis = trace.analyze(options);
// → {
//     waypoints: [{ latitude, longitude, elevation, name, wptType, time, … }],
//     legs:      [{ totalDistanceKm, totalElevationGainM, bearing, difficulty, … }],
//     sections:  [{ …leg fields, paceFactor, maxCompletionTime, cutoffRatio, … }],
//     stages:    [{ …same, grouped by Start/LifeBase/Arrival }],
//     metadata:  { name, description },
//   }
//   or null on malformed options

// Or, if you just want the JSON in one call and don't need the Trace handle:
const full = analyzeGpx(bytes, options);
// → { trace: { totalDistanceKm, totalElevationGainM, … }, ...analysis }
//   or null on parse failure
```

**Live recalibration (`trace.recalibrate()`)**

Once the race clock has started and the runner has a GPS fix, correct the
static `.analyze()` prediction against actual progress — see
[Live calibration](#live-calibration) above for the underlying model.

```js
const currentIndex = trace.findClosestPoint(lon, lat, alt)?.index;

const recalibration = trace.recalibrate({
  basePaceSPerKm: 500,
  kFatigue: 0.002,
  lifeBaseStopS: 3600,
  currentIndex,
  actualElapsedS: 5400, // real seconds since race start
});
// → {
//     sections: { calibrationFactor, calibratedBasePaceSPerKm,
//                  predictedSoFarS, actualElapsedS,
//                  etas: [{ id, endIndex, remainingDurationS, cumulativeRemainingS }, …] } | null,
//     stages:   { …same shape, at Start/LifeBase/Arrival granularity } | null,
//   }
//   or null on malformed options
//
// `sections` and `stages` solve independent calibration factors — each
// re-predicts at its own boundary granularity, with its own per-range
// weather lookup. Either is null when that boundary kind has fewer than
// 2 typed waypoints.

trace.free();
```

### memory management

`Trace` lives in WASM linear memory. The JS object is just a pointer — Rust cannot reclaim it when the JS variable is GC'd. Always call `.free()`, or register a `FinalizationRegistry`:

```js
const registry = new FinalizationRegistry((t) => t.free());
const trace = buildTrace(pts);
registry.register(trace, trace);
```

---

## license

[MIT](LICENSE)
