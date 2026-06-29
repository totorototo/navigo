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
import init, { parseGpx, analyzeGpx, buildTrace } from "@totorototo/navigo/web";
await init();
```

---

## Entry functions

### `parseGpx(bytes: Uint8Array): Trace | null`

Parse a GPX file into a `Trace` — **only `<trkpt>` track-points are parsed**;
waypoints and metadata are intentionally skipped so this path stays lean.
Returns `null` when the file contains no valid track-points.

Call `parseWaypoints(bytes)` and/or `parseMetadata(bytes)` separately when you
need that data, or use `parseGpxAll(bytes)` to get a `Trace` with both
already attached. If you need the full race analysis in one shot, use
`analyzeGpx(bytes, options)` instead.

```js
const bytes = new Uint8Array(await file.arrayBuffer());
const trace = parseGpx(bytes);
```

### `parseGpxAll(bytes: Uint8Array): Trace | null`

Parse a GPX file into a `Trace` that also carries its waypoints and
metadata — use this when you'll call `.analyze()` or `.recalibrate()` on the
returned handle, since those read the trace's stored waypoints (plain
`parseGpx` always returns an empty waypoint list). Triple-scans `bytes`
once up front instead of `parseGpx`'s single scan; the cost is paid once,
not on every `.recalibrate()` call. Returns `null` when the file contains
no valid track-points.

```js
const bytes = new Uint8Array(await file.arrayBuffer());
const trace = parseGpxAll(bytes);
```

### `parseWaypoints(bytes: Uint8Array): Waypoint[]`

Parse only the `<wpt>` elements from raw GPX bytes. Returns an array of
waypoint objects (empty array when none found).

### `parseMetadata(bytes: Uint8Array): { name, description }`

Parse only the `<metadata>` block from raw GPX bytes. Returns an object with
`name` and `description` fields (each `null` when absent).

### `trace.analyze(options): object | null`

Method on `Trace`. Compute the race analysis (waypoints, legs, sections,
stages) using this trace's waypoints — requires a `Trace` from `parseGpxAll`
(or `analyzeGpx`'s internal full parse); plain `parseGpx` traces have no
waypoints, so this returns empty `legs` and `null` `sections`/`stages`. Pure
opaque-handle-in, JSON-out — no bytes cross the boundary again, and the
expensive trace computation (simplification, elevation, climbs) is never
repeated. Returns `null` on malformed `options`.

```js
const trace = parseGpxAll(bytes);
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
A `Trace` built via `buildTrace` (no GPX source) or plain `parseGpx` has no
waypoints, so `.analyze()` on either returns an empty `legs` and `null`
`sections`/`stages`.

### `analyzeGpx(bytes, options): object | null`

Convenience wrapper around `parseGpxAll` + `trace.analyze()` for callers who
just want the JSON result and don't need the `Trace` handle (so there's
no `.free()` to manage). If you already have a `Trace` from `parseGpxAll`,
call `.analyze()` on it directly instead of parsing the bytes twice. Returns
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

| Field            | Type             | Description                                            |
| ---------------- | ---------------- | ------------------------------------------------------ |
| `basePaceSPerKm` | `number`         | Flat-terrain pace in s/km (e.g. `500` = 8:20/km)       |
| `kFatigue`       | `number`         | Exponential fatigue coefficient (e.g. `0.002`)         |
| `lifeBaseStopS`  | `number`         | Default planned stop at LifeBase checkpoints (seconds) |
| `weather`        | `WeatherEntry[]` | Optional. Per-checkpoint forecast (see below)          |

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
    {
      name: "Chamonix",
      temperatureC: 32,
      humidityPct: 85,
      windKmh: 35,
      precipProbPct: 80,
    },
  ],
});
```

**`TraceSummary`**

```ts
{ totalDistanceKm: number, totalElevationGainM: number,
  totalElevationLossM: number, locationCount: number }
```

**`Waypoint`**

```ts
{ latitude: number, longitude: number, elevation: number | null,
  name: string, wptType: string | null, time: number | null,
  stopDuration: number | null }
```

**`LegStats`**

```ts
{ legId: number, sectionIdx: number, startIndex: number, endIndex: number,
  startLocation: string, endLocation: string,
  totalDistanceKm: number, totalElevationGainM: number,
  totalElevationLossM: number, avgSlope: number, maxSlope: number,
  minElevation: number, maxElevation: number, bearing: number,
  difficulty: number, estimatedDurationS: number }
```

**`SectionStats`**

```ts
{ id: number, stageIdx: number, startIndex: number, endIndex: number,
  startLocation: string, endLocation: string,
  totalDistanceKm: number, totalElevationGainM: number,
  totalElevationLossM: number, avgSlope: number, maxSlope: number,
  minElevation: number, maxElevation: number, bearing: number,
  difficulty: number, estimatedDurationS: number, paceFactor: number,
  startTime: number | null, endTime: number | null,
  maxCompletionTime: number | null, cutoffRatio: number | null,
  stopDuration: number | null }
```

**`StageStats`** — same shape as `SectionStats` minus `stageIdx`.

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

### Scalar properties (zero-copy — read from WASM registers)

| Property                    | Type           | Description                             |
| --------------------------- | -------------- | --------------------------------------- |
| `totalDistance`             | `number`       | Total distance (km)                     |
| `totalElevationGain`        | `number`       | Cumulative elevation gain (m)           |
| `totalElevationLoss`        | `number`       | Cumulative elevation loss (m)           |
| `locationCount`             | `number`       | Number of (D-P simplified) locations    |

### Array methods (copy WASM → JS heap; call once and cache)

| Method                         | Returns        | Description                             |
| ------------------------------ | -------------- | --------------------------------------- |
| `getLocationsFlat()`           | `Float64Array` | `[lon, lat, alt, …]` for every location |
| `getCumulativeDistances()`     | `Float64Array` | Distance at each location (km)          |
| `getCumulativeElevationGains()`| `Float64Array` | Cumulative gain at each location (m)    |
| `getCumulativeElevationLosses()`| `Float64Array`| Cumulative loss at each location (m)    |
| `getSlopes()`                  | `Float64Array` | Slope at each location (%)              |
| `getPeaks()`                   | `Uint32Array`  | Peak location indices                   |
| `getValleys()`                 | `Uint32Array`  | Valley location indices                 |

### Methods

#### `indexAtDistance(distKm: number): number`

Index of the first location at or beyond `distKm`.

#### `pointAtDistance(distKm: number): { longitude, latitude, altitude } | undefined`

Location object at the given cumulative distance, or `undefined`.

#### `findClosestPoint(lon, lat, alt): { location, index, distance } | undefined`

Nearest location to the given coordinates. `distance` is in km.

#### `findClosestPointFrom(lon, lat, alt, startFrom): { location, index, distance } | undefined`

Like `findClosestPoint` but only searches from `startFrom` onwards — use
this in live-tracking loops to avoid snapping to an earlier position.

#### `sliceBetweenDistances(startKm, endKm): Float64Array | undefined`

All points between two cumulative distances as a flat `[lon, lat, alt, …]`
array, or `undefined` on invalid input.

#### `getSection(startIndex, endIndex): Float64Array`

Points in the index range (inclusive) as `[lon, lat, alt, …]`. Throws on
out-of-bounds input.

#### `area(): { minLongitude, maxLongitude, minLatitude, maxLatitude }`

Bounding box of the trace.

#### `elevation(): { positive, negative }`

Raw (non-denoised) elevation totals (m).

#### `climbs(): ClimbStats[]`

Detected climbs, each with:

```ts
{ startIndex: number, endIndex: number, startDistKm: number,
  climbDistKm: number, elevationGain: number, summitElev: number,
  avgGradient: number }
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

Needs this trace's waypoints to know section/stage boundaries — build the
`Trace` with `parseGpxAll`, not plain `parseGpx` (which always returns an
empty waypoint list, so `sections`/`stages` would come back `null`).

```js
const currentIndex = trace.findClosestPoint(lon, lat, alt)?.index;

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

| Field            | Type     | Description                                                   |
| ---------------- | -------- | ------------------------------------------------------------- |
| `currentIndex`   | `number` | Runner's current trace index (e.g. from `find_closest_point`) |
| `actualElapsedS` | `number` | Real seconds elapsed since race start                         |

**`RecalibratedEtas`**

```ts
{ calibrationFactor: number, calibratedBasePaceSPerKm: number,
  predictedSoFarS: number, actualElapsedS: number,
  etas: { id: number, endIndex: number, remainingDurationS: number,
          cumulativeRemainingS: number }[] }
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
