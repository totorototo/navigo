# @totorototo/navigo

GPS/geospatial trace processing — distances, elevation, simplification, climb
detection — compiled from [Rust](https://github.com/totorototo/navigo) to
WebAssembly.

## Install

```bash
npm install @totorototo/navigo
```

## Usage (bundler — Vite, webpack, Rollup)

No manual `init()` needed; the bundler wires up the `.wasm` import for you.

```js
import { parseGpx, parseGpxFull, buildTrace } from "@totorototo/navigo";
```

## Usage (plain browser ESM)

```js
import init, {
  parseGpx,
  parseGpxFull,
  buildTrace,
} from "@totorototo/navigo/web";
await init();
```

---

## Entry functions

### `parseGpx(bytes: Uint8Array): WasmTrace | null`

Parse a GPX file and return a `WasmTrace`. Returns `null` when the file
contains no valid track-points.

```js
const bytes = new Uint8Array(await file.arrayBuffer());
const trace = parseGpx(bytes);
```

### `parseGpxFull(bytes, basePaceSPerKm, kFatigue, lifeBaseStopS): object | null`

Parse a GPX file and compute the full race analysis. Returns `null` on empty
input, or the following object:

```js
const result = parseGpxFull(bytes, 500, 0.002, 3600);
// result.trace      — TraceSummary
// result.metadata   — { name, description }
// result.waypoints  — Waypoint[]
// result.legs       — LegStats[]
// result.sections   — IntervalStats[] | null
// result.stages     — IntervalStats[] | null
```

**Parameters**

| Parameter        | Type         | Description                                            |
| ---------------- | ------------ | ------------------------------------------------------ |
| `bytes`          | `Uint8Array` | Raw GPX file bytes                                     |
| `basePaceSPerKm` | `number`     | Flat-terrain pace in s/km (e.g. `500` = 8:20/km)       |
| `kFatigue`       | `number`     | Exponential fatigue coefficient (e.g. `0.002`)         |
| `lifeBaseStopS`  | `number`     | Default planned stop at LifeBase checkpoints (seconds) |

**`TraceSummary`**

```ts
{ total_distance_km: number, total_elevation_gain_m: number,
  total_elevation_loss_m: number, location_count: number }
```

**`Waypoint`**

```ts
{ latitude: number, longitude: number, elevation: number | null,
  name: string, wpt_type: string | null, time: number | null,
  stop_duration: number | null }
```

**`LegStats`**

```ts
{ leg_id: number, section_idx: number, start_index: number, end_index: number,
  start_location: string, end_location: string,
  total_distance_km: number, total_elevation_gain_m: number,
  total_elevation_loss_m: number, avg_slope: number, max_slope: number,
  min_elevation: number, max_elevation: number, bearing: number,
  difficulty: number, estimated_duration_s: number }
```

**`IntervalStats`** (sections and stages share this shape)

```ts
{ id: number, group_idx: number, start_index: number, end_index: number,
  start_location: string, end_location: string,
  total_distance_km: number, total_elevation_gain_m: number,
  total_elevation_loss_m: number, avg_slope: number, max_slope: number,
  min_elevation: number, max_elevation: number, bearing: number,
  difficulty: number, estimated_duration_s: number, pace_factor: number,
  start_time: number | null, end_time: number | null,
  max_completion_time: number | null, cutoff_ratio: number | null,
  stop_duration: number | null }
```

### `buildTrace(flat: Float64Array): WasmTrace | null`

Build a `WasmTrace` from interleaved coordinates
`[lon₀, lat₀, alt₀, lon₁, lat₁, alt₁, …]`. Returns `null` when the array
carries no points.

```js
const pts = new Float64Array([
  2.350987, 48.856667, 0, 37.617634, 55.755787, 200,
]);
const trace = buildTrace(pts);
```

---

## `WasmTrace`

`WasmTrace` lives in WASM linear memory; the JS object is just a pointer.
Always call `.free()` when done, or use a `FinalizationRegistry`.

### Properties (getters — copy WASM → JS heap; call once and cache)

| Property                      | Type           | Description                             |
| ----------------------------- | -------------- | --------------------------------------- |
| `total_distance`              | `number`       | Total distance (km)                     |
| `total_elevation_gain`        | `number`       | Cumulative elevation gain (m)           |
| `total_elevation_loss`        | `number`       | Cumulative elevation loss (m)           |
| `location_count`              | `number`       | Number of (D-P simplified) locations    |
| `locations_flat`              | `Float64Array` | `[lon, lat, alt, …]` for every location |
| `cumulative_distances`        | `Float64Array` | Distance at each location (km)          |
| `cumulative_elevation_gains`  | `Float64Array` | Cumulative gain at each location (m)    |
| `cumulative_elevation_losses` | `Float64Array` | Cumulative loss at each location (m)    |
| `slopes`                      | `Float64Array` | Slope between consecutive locations (%) |
| `peaks`                       | `Uint32Array`  | Peak location indices                   |
| `valleys`                     | `Uint32Array`  | Valley location indices                 |

### Methods

#### `index_at_distance(distKm: number): number`

Index of the first location at or beyond `distKm`.

#### `point_at_distance(distKm: number): { longitude, latitude, altitude } | undefined`

Location object at the given cumulative distance, or `undefined`.

#### `find_closest_point(lon, lat, alt): { location, index, distance } | undefined`

Nearest location to the given coordinates. `distance` is in km.

#### `find_closest_point_from(lon, lat, alt, startFrom): { location, index, distance } | undefined`

Like `find_closest_point` but only searches from `startFrom` onwards — use
this in live-tracking loops to avoid snapping to an earlier position.

#### `slice_between_distances(startKm, endKm): Float64Array | undefined`

All points between two cumulative distances as a flat `[lon, lat, alt, …]`
array, or `undefined` on invalid input.

#### `get_section(startIndex, endIndex): Float64Array`

Points in the index range (inclusive) as `[lon, lat, alt, …]`. Throws on
out-of-bounds input.

#### `area(): { min_longitude, max_longitude, min_latitude, max_latitude }`

Bounding box of the trace.

#### `elevation(): { positive, negative }`

Raw (non-denoised) elevation totals (m).

#### `climbs(): ClimbStats[]`

Detected climbs, each with:

```ts
{ start_index: number, end_index: number, start_dist_km: number,
  climb_dist_km: number, elevation_gain: number, summit_elev: number,
  avg_gradient: number }
```

#### `free(): void`

Release WASM memory. **Required** — Rust's allocator has no GC bridge.

```js
const registry = new FinalizationRegistry((t) => t.free());
const trace = parseGpx(bytes);
registry.register(trace, trace);
```

---

Rust source: https://github.com/totorototo/navigo
