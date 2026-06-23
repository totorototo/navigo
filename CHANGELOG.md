# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.0] - 2026-06-23

### Changed

- **Breaking:** `Trace` can no longer be empty. `Trace::new` and `build_trace` now
  return `Result<Trace, TraceError>`, rejecting empty input instead of silently
  producing a zeroed-out trace.
- **Breaking:** `Trace::area()` is now infallible and returns `Area` directly
  (previously `Result<Area, &str>`).
- **Breaking:** `Trace::get_section()` now returns `Result<Vec<Location>, TraceError>`
  instead of `Result<Vec<Location>, &str>`.
- The WASM `buildTrace()` binding now returns `WasmTrace | null` (`null` on empty
  input) instead of always constructing a trace.

### Added

- `TraceError` enum (`EmptyTrace`, `InvalidRange`, `IndexOutOfBounds`) implementing
  `std::error::Error`, replacing stringly-typed errors.
- `LICENSE` file (MIT), matching the `license` field already declared in `Cargo.toml`.

## [0.2.0] - 2026-06-23

### Added

- WebAssembly bindings (`wasm` feature): `build_wasm_trace` / `buildTrace`, with a
  `WasmTrace` class exposing scalar/array getters and query methods.
- Demo web app (Vite) under `demo/` showcasing the WASM bindings.
- CI: WASM build coverage and code coverage gating.

## [0.1.0] - 2026-06-23

### Added

- Initial release: `Location`, `Trace`, `Area`, `Elevation` / `GainLoss` types.
- Distance, bearing, elevation-change, area/radius containment helpers on `Location`.
- `Trace::new` precomputing Douglas-Peucker simplification, cumulative distances,
  denoised elevation gain/loss, smoothed slopes, peak/valley detection (AMPD), and
  Garmin-style climb segment detection.
- CI pipeline (build, clippy, fmt, tests) and crates.io publish workflow.

[Unreleased]: https://github.com/totorototo/navigo/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/totorototo/navigo/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/totorototo/navigo/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/totorototo/navigo/releases/tag/v0.1.0
