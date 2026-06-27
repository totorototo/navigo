# navigo — demo

Vite web app that runs the navigo GPS library compiled to WebAssembly.
Brutalist design — monospace, black borders, no gradients.

## what it shows

Loads a real race GPX file (`public/grp-160-2026.gpx`) and renders:

- Stat cards — total distance, elevation gain / loss, location count, detected climbs, bounding box
- Elevation profile canvas — solid black fill, yellow climb zones, peak / valley markers
- Per-climb breakdown (distance, gain, grade, summit altitude, Garmin score)
- Checkpoints table — name, type, elevation, cutoff time
- Sections table — pace-modelled splits between checkpoints (distance, gain/loss, est. time, difficulty)
- Stages table — same, grouped by Start / LifeBase / Arrival

## prerequisites

- [Rust](https://rustup.rs/) + `wasm32-unknown-unknown` target
- [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/)
- Node.js ≥ 18

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
```

## run

```bash
cd demo
npm install
npm run dev        # → http://localhost:5173
```

`npm run dev` runs `wasm-pack build` first (compiles Rust → WASM), then
starts the Vite dev server.

## build for production

```bash
npm run build      # output in demo/dist/
npm run preview    # serve the production build locally
```

Output: ~60 kB WASM + ~14 kB JS (gzipped: ~4 kB).

## stack

| tool | role |
|---|---|
| `wasm-pack` | compiles Rust → `.wasm` + JS bindings |
| `vite-plugin-wasm` | lets Vite bundle `.wasm` files |
| `vite-plugin-top-level-await` | enables `await init()` at module top level |

## how it works

```
wasm-pack build --features wasm
        │
        ▼
demo/pkg/navigo.js + navigo_bg.wasm
        │
   Vite + vite-plugin-wasm
        │
   browser loads WASM
        │
fetch("/grp-160-2026.gpx") → Uint8Array
        │
parseGpx(bytes)   ← one copy JS → WASM; track-points, waypoints
        │           and metadata all parsed once
   trace: Trace (lives in WASM memory)
        │
   ┌────┴──────────────────────────────────────────────┐
   │  trace.total_distance, .climbs(), …  → free/cheap │
   │  trace.analyze(options)              → legs/      │
   │    sections/stages/waypoints, no bytes re-sent     │
   └─────────────────────────────────────────────────────┘
        │
   stats / canvas / climbs / checkpoints / sections / stages
        │
   trace.free()    ← release WASM memory
```
