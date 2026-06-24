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
import { buildTrace } from "@totorototo/navigo";

const pts = new Float64Array([
  2.350987, 48.856667, 0, // lon, lat, alt
  37.617634, 55.755787, 200,
]);
const trace = buildTrace(pts); // WasmTrace, or null if pts carries no points

trace.total_distance; // km
trace.total_elevation_gain; // m
trace.climbs(); // [{ start_index, end_index, ... }, ...]

trace.free(); // required — Rust's allocator has no GC bridge
```

## Usage (plain browser ESM)

```js
import init, { buildTrace } from "@totorototo/navigo/web";
await init();

const trace = buildTrace(pts);
```

## Memory management

`WasmTrace` lives in WASM linear memory; the JS object is just a pointer.
Always call `.free()`, or register a `FinalizationRegistry`:

```js
const registry = new FinalizationRegistry((t) => t.free());
const trace = buildTrace(pts);
registry.register(trace, trace);
```

Full API reference and the Rust source: https://github.com/totorototo/navigo
