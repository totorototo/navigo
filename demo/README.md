# navigo — demo

Vite web app that runs the navigo GPS library compiled to WebAssembly.
Brutalist design — monospace, black borders, no gradients.

## what it shows

- Total distance, elevation gain / loss, location count, detected climbs
- Elevation profile canvas — solid black fill, yellow climb zones, peak / valley markers
- Per-climb breakdown (distance, gain, grade, summit altitude, Garmin score)

The sample trace is a synthetic 25 km Alpine route with three qualifying climbs,
generated in `src/sample-trace.js`.

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
buildTrace(Float64Array)   ← one copy JS → WASM
        │
   WasmTrace (lives in WASM memory)
        │
   ┌────┴────────────────────────────────────┐
   │  scalar getters   → free (registers)    │
   │  array getters    → copy once, cache JS │
   │  query methods    → cheap (scalars)     │
   └─────────────────────────────────────────┘
        │
   stats / canvas / climbs list
        │
   trace.free()    ← release WASM memory
```

