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
import { parseGpx, analyzeGpx, buildTrace } from "@totorototo/navigo";
```

## Usage (plain browser ESM)

```js
import init, {
  parseGpx,
  analyzeGpx,
  buildTrace,
} from "@totorototo/navigo/web";
await init();
```

---

## Entry functions

### `parseGpx(bytes: Uint8Array): Trace | null`

Parse a GPX file into a `Trace` — track-points, waypoints and metadata are
all parsed once and stored in WASM memory. Returns `null` when the file
contains no valid track-points.

```js
const bytes = new Uint8Array(await file.arrayBuffer());
const trace = parseGpx(bytes);
```

### `trace.analyze(options): object | null`

Method on `Trace`. Compute the race analysis (waypoints, legs, sections,
stages) using this trace's waypoints (parsed once by `parseGpx`). Pure
opaque-handle-in, JSON-out — no bytes cross the boundary again, and the
expensive trace computation (simplification, elevation, climbs) is never
repeated. Returns `null` on malformed `options`.

```js
const trace = parseGpx(bytes);
const analysis = trace.analyze({
  basePaceSPerKm: 500,
  kFatigue: 0.002,
  lifeBaseStopS: 3600,
});
// analysis.waypoints — Waypoint[]
// analysis.legs      — LegStats[]
// analysis.sections  — SectionStats[] | null
// analysis.stages    — StageStats[] | null
// analysis.metadata  — { name, description }
trace.free();
```

See **`options`** below for the full shape (including optional `weather`).
A `Trace` built via `buildTrace` (no GPX source) has no waypoints, so
`.analyze()` on it returns an empty `legs` and `null` `sections`/`stages`.

### `analyzeGpx(bytes, options): object | null`

Convenience wrapper around `parseGpx` + `trace.analyze()` for callers who
just want the JSON result and don't need the `Trace` handle (so there's
no `.free()` to manage). If you already have a `Trace`, call
`.analyze()` on it directly instead of parsing the bytes twice. Returns
`null` on empty input or malformed `options`, or the following object:

```js
const result = analyzeGpx(bytes, {
  basePaceSPerKm: 500,
  kFatigue: 0.002,
  lifeBaseStopS: 3600,
});
// result.trace      — TraceSummary
// result.metadata   — { name, description }
// result.waypoints  — Waypoint[]
// result.legs       — LegStats[]
// result.sections   — SectionStats[] | null
// result.stages     — StageStats[] | null
```

**`options`**

| Field            | Type                  | Description                                             |
| ---------------- | --------------------- | -------------------------------------------------------- |
| `basePaceSPerKm`  | `number`              | Flat-terrain pace in s/km (e.g. `500` = 8:20/km)         |
| `kFatigue`        | `number`              | Exponential fatigue coefficient (e.g. `0.002`)           |
| `lifeBaseStopS`   | `number`              | Default planned stop at LifeBase checkpoints (seconds)   |
| `weather`         | `WeatherEntry[]`      | Optional. Per-checkpoint forecast (see below)            |

**`WeatherEntry`** — matched against waypoint `name`; unmatched checkpoints use
neutral weather (no slowdown):

```ts
{ name: string, temperatureC: number, humidityPct: number,
  windKmh: number, precipProbPct: number }
```

```js
const result = analyzeGpx(bytes, {
  basePaceSPerKm: 500,
  kFatigue: 0.002,
  lifeBaseStopS: 3600,
  weather: [
    { name: "Chamonix", temperatureC: 32, humidityPct: 85, windKmh: 35, precipProbPct: 80 },
  ],
});
```

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

**`SectionStats`**

```ts
{ id: number, stage_idx: number, start_index: number, end_index: number,
  start_location: string, end_location: string,
  total_distance_km: number, total_elevation_gain_m: number,
  total_elevation_loss_m: number, avg_slope: number, max_slope: number,
  min_elevation: number, max_elevation: number, bearing: number,
  difficulty: number, estimated_duration_s: number, pace_factor: number,
  start_time: number | null, end_time: number | null,
  max_completion_time: number | null, cutoff_ratio: number | null,
  stop_duration: number | null }
```

**`StageStats`** — same shape as `SectionStats` minus `stage_idx`.

### `buildTrace(flat: Float64Array): Trace | null`

Build a `Trace` from interleaved coordinates
`[lon₀, lat₀, alt₀, lon₁, lat₁, alt₁, …]`. Returns `null` when the array
carries no points.

```js
const pts = new Float64Array([
  2.350987, 48.856667, 0, 37.617634, 55.755787, 200,
]);
const trace = buildTrace(pts);
```

---

## `Trace`

`Trace` lives in WASM linear memory; the JS object is just a pointer.
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

#### `analyze(options): object | null`

Race analysis (waypoints, legs, sections, stages) — see
[`trace.analyze(options)`](#traceanalyzeoptions-object--null) above for the
full shape and `options` reference.

#### `recalibrate(options): object | null`

Live, mid-race ETA recalibration — corrects the static `.analyze()`
prediction against the runner's actual progress. Compares `actualElapsedS`
against what the model predicted for the distance covered to solve a
calibration factor, then re-predicts remaining sections/stages with the
pace re-anchored to it and the circadian clock re-seeded to real elapsed
time. Returns `null` on malformed `options`.

```js
const currentIndex = trace.find_closest_point(lon, lat, alt)?.index;

const recalibration = trace.recalibrate({
  basePaceSPerKm: 500,
  kFatigue: 0.002,
  lifeBaseStopS: 3600,
  currentIndex,
  actualElapsedS: 5400, // real seconds elapsed since race start
});
// recalibration.sections — RecalibratedEtas | null  (checkpoint granularity)
// recalibration.stages   — RecalibratedEtas | null  (LifeBase granularity)
trace.free();
```

`sections` and `stages` solve independent calibration factors — each
re-predicts at its own boundary granularity with its own per-range weather
lookup. Either is `null` when that boundary kind has fewer than 2 typed
waypoints (e.g. a `Trace` with no GPX-sourced waypoints).

**`options`** — same `basePaceSPerKm` / `kFatigue` / `lifeBaseStopS` /
`weather` as [`analyze`](#options) above, plus:

| Field            | Type     | Description                                          |
| ---------------- | -------- | ----------------------------------------------------- |
| `currentIndex`   | `number` | Runner's current trace index (e.g. from `find_closest_point`) |
| `actualElapsedS` | `number` | Real seconds elapsed since race start                |

**`RecalibratedEtas`**

```ts
{ calibration_factor: number, calibrated_base_pace_s_per_km: number,
  predicted_so_far_s: number, actual_elapsed_s: number,
  etas: { id: number, end_index: number, remaining_duration_s: number,
          cumulative_remaining_s: number }[] }
```

The factor is `actualElapsedS / predictedSoFar`, clamped to `[0.5, 3.0]`
and ignored (kept at `1.0`) below 300s of predicted effort — too noisy
otherwise.

#### `free(): void`

Release WASM memory. **Required** — Rust's allocator has no GC bridge.

```js
const registry = new FinalizationRegistry((t) => t.free());
const trace = parseGpx(bytes);
registry.register(trace, trace);
```

---

Rust source: https://github.com/totorototo/navigo
