# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0] - 2026-06-25

### Added

- **GPX parser** (`gpx` module): zero-dependency byte-scanning parser —
  `parse_trace_points`, `parse_waypoints`, `parse_metadata` / `GpxMetadata`.
- **`Waypoint`** struct with `is_section_boundary()` (any typed waypoint) and
  `is_stage_boundary()` (Start / LifeBase / Arrival only) classification.
- **`parse_iso8601_to_epoch`** (`time` module): ISO 8601 string → Unix seconds,
  handles `Z` and `±HH:MM` offsets.
- **Minetti 2002 pace model** (`minetti` module): `cmet(slope)` — 5th-degree
  polynomial metabolic cost (J/(kg·m)), `pace_factor(slope)`, `CMET_FLAT`.
- **Extended pace model** (`pace_model` module): `fatigue_factor` (exponential
  decay with constant `K_FATIGUE`), `circadian_factor` (cosine model, −15% at
  03:30 UTC), `WeatherLookup` / `WeatherConditions` for per-checkpoint weather
  corrections (thermal, wind, precipitation).
- **`SegmentMetrics` / `compute`** (`segment` module): min/max elevation,
  max slope, weighted time and distance for any index range of a trace.
- **`LegStats` / `compute_from_waypoints`** (`leg` module): Naismith-based
  per-leg analysis — total distance, gain/loss, bearing, difficulty (1–5),
  estimated duration.
- **`SectionStats` / `compute_from_waypoints`** (`section` module):
  pace-modelled section analysis — `pace_factor`, `max_completion_time`,
  `cutoff_ratio`, `stop_duration`.
- **`StageStats` / `compute_from_waypoints`** (`stage` module): same metrics
  grouped by Start / LifeBase / Arrival boundaries.
- **`Recalibration` / `recalibrate_from_current`** (`calibration` module): live
  ETA recalibration from actual vs predicted elapsed time; factor clamped to
  `[0.5, 3.0]`, gated on ≥ 300 s predicted to avoid early noise.
- **WASM `parseGpx(Uint8Array)`**: parse GPX bytes directly into a `WasmTrace`.
- **WASM `parseGpxFull(bytes, basePace, kFatigue, lifeBaseStop)`**: one-call
  full analysis returning `{ trace, waypoints, legs, sections, stages, metadata }`
  as a plain JS object.
- **Demo**: updated to load a real GPX file (GRP 2026 Ultra Solo, 174.7 km,
  15 checkpoints) via `fetch`, displaying an elevation profile, Checkpoints,
  Sections and Stages tables, all driven by the Minetti pace model.
- Test suite expanded to 141 tests (was 42), 0 warnings.

## [0.3.1] - 2026-06-24

### Added

- Published WASM bindings to npm as
  [`@totorototo/navigo`](https://www.npmjs.com/package/@totorototo/navigo),
  built for both the `bundler` (Vite/webpack, zero-config) and `web`
  (plain ESM, manual `init()`) wasm-pack targets.
- CI: tagged releases now publish to npm via Trusted Publishing (OIDC) —
  no stored npm token — alongside the existing crates.io publish.

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

[Unreleased]: https://github.com/totorototo/navigo/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/totorototo/navigo/compare/v0.3.1...v0.4.0
[0.3.1]: https://github.com/totorototo/navigo/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/totorototo/navigo/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/totorototo/navigo/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/totorototo/navigo/releases/tag/v0.1.0
