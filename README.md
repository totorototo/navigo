# navigo

[![CI](https://github.com/totorototo/navigo/actions/workflows/ci.yml/badge.svg)](https://github.com/totorototo/navigo/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/navigo.svg)](https://crates.io/crates/navigo)
[![docs.rs](https://docs.rs/navigo/badge.svg)](https://docs.rs/navigo)
[![License](https://img.shields.io/crates/l/navigo.svg)](LICENSE)

simply manipulate GPS/geospatial data — in rust

See [CHANGELOG.md](CHANGELOG.md) for release notes.

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

## WebAssembly

The library can be compiled to WASM for use in web applications via the `wasm` feature.
Prebuilt bindings are published to npm as [`@totorototo/navigo`](https://www.npmjs.com/package/@totorototo/navigo) — `npm install @totorototo/navigo`.

### how it works

All data lives in WASM linear memory. The JS side holds a thin pointer (`WasmTrace`).
Only the boundaries cross the WASM↔JS membrane — scalars are free (registers), bulk arrays are copied once on demand.

```
buildTrace(Float64Array)          ← one O(n) copy JS→WASM, null if no points
       │
       ▼
WasmTrace stays in WASM memory
       │
       ├── trace.total_distance        → free (register)
       ├── trace.find_closest_point()  → free (scalars in/out)
       ├── trace.locations_flat        → one O(n) copy, cache it
       └── trace.free()               ← you must call this (no GC bridge)
```

### build

```bash
cargo install wasm-pack
wasm-pack build --target web       # ES modules — Vite, plain browser
wasm-pack build --target bundler   # webpack / Rollup
```

### usage

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
// → WasmTrace, or null if pts carries no points

// scalar getters — free
trace.total_distance; // number (km)
trace.total_elevation_gain; // number (m)
trace.total_elevation_loss; // number (m)
trace.location_count; // number

// array getters — copy once, then cache on the JS side
const locs = trace.locations_flat; // Float64Array [lon,lat,alt,…]
const dists = trace.cumulative_distances; // Float64Array (km)
const gains = trace.cumulative_elevation_gains; // Float64Array (m)
const losses = trace.cumulative_elevation_losses; // Float64Array (m)
const slopes = trace.slopes; // Float64Array (%)
const peaks = trace.peaks; // Uint32Array  (indices)
const valleys = trace.valleys; // Uint32Array  (indices)

// query methods — scalars in, one small object out
trace.point_at_distance(42.0);
// → { longitude, latitude, altitude } | undefined

trace.index_at_distance(42.0);
// → number

trace.find_closest_point(lon, lat, alt);
// → { location: { longitude, latitude, altitude }, index, distance } | undefined

trace.find_closest_point_from(lon, lat, alt, lastIndex);
// → same shape | undefined  (use on live-tracking loops)

trace.slice_between_distances(10.0, 50.0);
// → Float64Array [lon,lat,alt,…] | undefined

trace.get_section(startIndex, endIndex);
// → Float64Array [lon,lat,alt,…]  (throws on out-of-bounds / invalid range)

trace.area();
// → { min_longitude, max_longitude, min_latitude, max_latitude }

trace.elevation();
// → { positive, negative }  (raw, non-denoised)

trace.climbs();
// → [{ start_index, end_index, start_dist_km, climb_dist_km,
//      elevation_gain, summit_elev, avg_gradient }, …]

// always release when done — Rust allocator has no GC bridge
trace.free();
```

### memory management

`WasmTrace` lives in WASM linear memory. The JS object is just a pointer — Rust cannot reclaim it when the JS variable is GC'd. Always call `.free()`, or register a `FinalizationRegistry`:

```js
const registry = new FinalizationRegistry((t) => t.free());
const trace = buildTrace(pts);
registry.register(trace, trace);
```

---

## license

[MIT](LICENSE)
