# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

navigo is a Rust crate for GPS/geospatial trace analysis: GPX parsing (no XML
dependency, byte-scanning only), Douglas-Peucker simplification, denoised
elevation gain/loss, peak/valley detection, Garmin-style climb qualification,
the Minetti 2002 metabolic cost model for pace prediction, and race-route
analysis (legs/sections/stages with fatigue, circadian and weather factors).
It compiles natively as a normal Rust crate (`rlib`) and also to WebAssembly
(`cdylib`, `wasm` feature) for use in the browser. Built bindings are published
to npm as `@totorototo/navigo`; the crate itself is published to crates.io.

`demo/` is a Vite + vanilla-JS app that loads the WASM build and visualizes a
real race GPX file — used to sanity-check the WASM API end-to-end. Its
`npm run dev`/`build` rebuild the WASM binary from the current `src/` tree
via `wasm-pack` before Vite starts (see `demo/package.json`), not from the
published `@totorototo/navigo` package — so it's the fastest way to manually
verify a change to core or `wasm/` code end to end.

## Commands

```bash
cargo build                          # native build
cargo test                           # run all tests
cargo test --all-features            # run tests incl. wasm feature
cargo test <name>                    # run a single test by name (substring match)
cargo fmt --check                    # CI enforces this — run `cargo fmt` to fix
cargo clippy --all-targets --all-features   # CI runs with RUSTFLAGS="-D warnings"
cargo llvm-cov --fail-under-lines 95 --fail-under-regions 90   # coverage gate used in CI

# WASM (requires `cargo install wasm-pack` + wasm32-unknown-unknown target)
wasm-pack build --target web -- --features wasm
wasm-pack build --target bundler -- --features wasm

# demo app (builds wasm via wasm-pack, then runs Vite)
cd demo && npm install
npm run dev         # http://localhost:5173
npm run build       # production build → demo/dist
```

There are no unit tests under `src/`'s own `#[cfg(test)]` modules in isolation
from integration tests — both exist (e.g. `src/wasm.rs`'s `pipeline_tests`,
`tests/build_trace.rs`). Run the whole suite with `cargo test --all-features`
before considering a change done; CI fails the build on any clippy warning.

## Architecture

**Pipeline, not a class hierarchy.** `Trace::new` (`src/trace.rs`) is the one
constructor and it front-loads all the expensive work in one pass — simplify,
distances, denoised elevation, slopes, peaks/valleys, climbs — so every
downstream method assumes precomputed, non-empty data. `Trace` is `pub use`'d
from `lib.rs` and the WASM layer wraps it rather than re-implementing it.

**Layering**, lowest to highest level:
- `location.rs`, `area.rs` — primitive geo types (haversine distance, bearing).
- `simplify.rs`, `elevation.rs`, `extrema.rs`, `climbs.rs` — algorithms run
  once inside `Trace::new` (Douglas-Peucker, median-smoothed gain/loss with
  hysteresis, AMPD peak detection, Garmin Climb Pro thresholds).
- `trace.rs` — the `Trace` struct and its query methods (point/index at
  distance, closest-point search, slicing).
- `gpx.rs`, `waypoint.rs` — GPX byte-scanning parser; waypoints are classified
  as section/stage boundaries by `wpt_type` (Start/LifeBase/Arrival/etc).
- `minetti.rs`, `pace_model.rs` — pure functions: metabolic cost vs. grade,
  fatigue decay, circadian rhythm, weather factors. No dependency on `Trace`.
- `leg.rs` → `section.rs` → `stage.rs` — each builds on the one below it by
  consuming consecutive waypoint pairs from the same `Trace`. `leg` is plain
  Naismith-rule geometry; `section` adds the full pace model; `stage` groups
  sections between Start/LifeBase/Arrival boundaries.
- `calibration.rs` — recomputes remaining ETAs mid-race from one known
  elapsed-time data point; depends on `section`/`stage` output shapes.
- `wasm/` (feature-gated) — `wasm.rs` is the `#[wasm_bindgen]` entry surface
  (`parseGpx`, `buildTrace`, `analyzeGpx`); `wasm/trace.rs` wraps `Trace` as a
  JS-visible class; `wasm/dto.rs` defines the serde-serializable shapes
  returned to JS; `wasm/options.rs` parses the JS options object. This module
  only adapts — it must not contain analysis logic that belongs in the core
  modules above.

**WASM memory model**: data lives in WASM linear memory; JS holds a thin
pointer. Only boundaries cross JS↔WASM — scalars are register-free, bulk
arrays (`locations_flat`, `cumulative_distances`, etc.) are copied once and
should be cached on the JS side. Every `Trace` obtained from WASM must have
`.free()` called (no GC bridge) — see `demo/` for the `FinalizationRegistry`
pattern.

**Error handling**: `Trace::new` is the only fallible entry point in the core
crate (`TraceError::EmptyTrace`) — once a `Trace` exists, all other methods
assume non-empty, valid data rather than re-checking.

## Conventions

- Public API additions go through `lib.rs`'s explicit `pub use` list, not
  blanket `pub mod` re-exports.
- New WASM-exposed functionality belongs in `src/wasm/`, with the underlying
  computation living in the relevant core module — `wasm/*` should stay thin.
- `cargo fmt` and zero clippy warnings are enforced in CI (`-D warnings`),
  along with a 95%/90% line/region coverage floor — keep new code covered by
  tests in the same PR.

## Before committing/pushing

1. Update the relevant README(s) (`README.md`, `demo/README.md`,
   `npm/README.md`) to reflect the change.
2. Bump the version in `Cargo.toml` (the npm package version is synced from
   it automatically during release CI — don't hand-edit `npm/*/package.json`).
3. Test the demo app (`cd demo && npm run dev` or `npm run build`) to confirm
   the change works end-to-end through the freshly built WASM binary.
