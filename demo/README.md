# navigo demo

A Vite web application that uses the navigo library compiled to WebAssembly.

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
npm run dev
```

`npm run dev` builds the WASM package first (`wasm-pack build`) then starts
the Vite dev server at http://localhost:5173.

## build for production

```bash
npm run build   # outputs to demo/dist/
npm run preview # serve the production build locally
```

## how it works

```
wasm-pack build  →  demo/pkg/navigo.js + navigo_bg.wasm
                              │
                     Vite + vite-plugin-wasm
                              │
                       browser loads WASM
                              │
               buildTrace(Float64Array)  →  WasmTrace
                              │
              stats / profile canvas / climbs list
```

- Input coordinates are passed as a single flat `Float64Array`
  `[lon₀, lat₀, alt₀, lon₁, lat₁, alt₁, …]` — one copy JS → WASM.
- All computation runs inside WASM linear memory.
- Bulk arrays (`cumulative_distances`, `locations_flat`, …) are read once
  and cached in JS variables.
- `trace.free()` is called when done to release WASM memory.
