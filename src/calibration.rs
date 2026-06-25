use crate::location::Location;
use crate::pace_model::{WeatherLookup, RECOVERY_LIFE_BASE};
use crate::segment;
use crate::trace::Trace;
use crate::waypoint::Waypoint;

/// Calibration is ignored until the model predicts at least this many seconds of covered effort.
pub const MIN_CALIBRATION_PREDICTION_S: f64 = 300.0;

/// Calibration factor clamped to this range — prevents one bad GPS sample from producing absurd ETAs.
pub const CALIBRATION_MIN: f64 = 0.5;
pub const CALIBRATION_MAX: f64 = 3.0;

/// Which boundary set to use when partitioning a route into intervals.
pub enum BoundaryKind {
    /// Split on Start / TimeBarrier / LifeBase / Arrival (checkpoint granularity).
    Section,
    /// Split on Start / LifeBase / Arrival only (LifeBase granularity; ignores TimeBarriers).
    Stage,
}

fn is_boundary(wpt: &Waypoint, kind: &BoundaryKind) -> bool {
    match kind {
        BoundaryKind::Section => wpt.is_section_boundary(),
        BoundaryKind::Stage => wpt.is_stage_boundary(),
    }
}

/// Recalibrated ETA for one interval, relative to the runner's current position.
#[cfg_attr(feature = "wasm", derive(serde::Serialize))]
pub struct RecalibratedEta {
    pub id: usize,
    pub end_index: usize,
    /// Remaining moving + stop time within this interval (0 for completed intervals).
    pub remaining_duration_s: f64,
    /// Running sum of `remaining_duration_s` to the end of this interval (0 for completed).
    pub cumulative_remaining_s: f64,
}

/// Result of a live recalibration. Caller owns this value.
#[cfg_attr(feature = "wasm", derive(serde::Serialize))]
pub struct Recalibration {
    /// actual_elapsed / predicted_so_far (clamped + gated); 1.0 when not yet trusted.
    pub calibration_factor: f64,
    /// base_pace × calibration_factor — the pace used for forward predictions.
    pub calibrated_base_pace_s_per_km: f64,
    /// Model's moving-time prediction for the covered distance at the original pace.
    pub predicted_so_far_s: f64,
    pub actual_elapsed_s: f64,
    pub etas: Vec<RecalibratedEta>,
}

struct ResolvedRange {
    id: usize,
    start_index: usize,
    end_index: usize,
    end_wpt_name: String,
    end_wpt_type: Option<String>,
    end_wpt_stop_duration: Option<u32>,
}

/// Advance the physiological model across `[from, end)` of one resolved range at `pace`.
///
/// When `apply_end_effects` is true (runner reaches the checkpoint), applies LifeBase
/// fatigue recovery and populates `stop_out` with the planned stop time.
fn advance_range(
    trace: &Trace,
    range: &ResolvedRange,
    from: usize,
    end: usize,
    pace_s_per_km: f64,
    k_fatigue: f64,
    clock_start: Option<i64>,
    life_base_stop_s: u32,
    weather: &WeatherLookup,
    apply_end_effects: bool,
    d_eff_m: &mut f64,
    elapsed_s: &mut f64,
    stop_out: &mut f64,
) -> f64 {
    let range_weather = weather.find(&range.end_wpt_name);
    let metrics = segment::compute(
        trace,
        from,
        end,
        pace_s_per_km,
        k_fatigue,
        clock_start,
        range_weather,
        d_eff_m,
        elapsed_s,
    );

    *stop_out = 0.0;
    if apply_end_effects {
        if range.end_wpt_type.as_deref() == Some("LifeBase") {
            *d_eff_m *= 1.0 - RECOVERY_LIFE_BASE;
        }
        *stop_out = if let Some(sd) = range.end_wpt_stop_duration {
            sd as f64
        } else if range.end_wpt_type.as_deref() == Some("LifeBase") {
            life_base_stop_s as f64
        } else {
            0.0
        };
    }
    metrics.total_time
}

/// Recalibrate remaining-interval ETAs from the runner's live progress.
///
/// `kind` selects section- vs stage-granularity boundaries. `current_index` is the
/// runner's current trace point; `actual_elapsed_s` is real seconds elapsed since race start.
///
/// Phase 1 replays covered intervals at the original `base_pace_s_per_km` to get the
/// model's prediction so far. Phase 2 forward-predicts remaining intervals at the
/// calibrated pace, re-seeding the circadian clock to `actual_elapsed_s`.
///
/// Returns `None` when fewer than 2 boundaries exist.
pub fn recalibrate_from_current(
    trace: &Trace,
    waypoints: &[Waypoint],
    kind: BoundaryKind,
    current_index: usize,
    actual_elapsed_s: f64,
    base_pace_s_per_km: f64,
    k_fatigue: f64,
    life_base_stop_s: u32,
    weather: &WeatherLookup,
) -> Option<Recalibration> {
    // ── Resolve boundary waypoints onto trace index ranges ─────────────────────
    let boundary_wpts: Vec<&Waypoint> = waypoints
        .iter()
        .filter(|w| is_boundary(w, &kind))
        .collect();
    if boundary_wpts.len() < 2 {
        return None;
    }

    let mut resolved: Vec<ResolvedRange> = Vec::new();
    let mut search_start = 0usize;

    for i in 0..boundary_wpts.len() - 1 {
        let start_wpt = boundary_wpts[i];
        let end_wpt = boundary_wpts[i + 1];
        let start_target = Location {
            longitude: start_wpt.longitude,
            latitude: start_wpt.latitude,
            altitude: 0.0,
        };
        let end_target = Location {
            longitude: end_wpt.longitude,
            latitude: end_wpt.latitude,
            altitude: 0.0,
        };
        let start_index = match trace.find_closest_point_from(&start_target, search_start) {
            Some((_, idx, _)) => idx,
            None => continue,
        };
        let end_index = match trace.find_closest_point_from(&end_target, start_index + 1) {
            Some((_, idx, _)) => idx,
            None => continue,
        };
        search_start = end_index;
        if start_index >= end_index {
            continue;
        }
        resolved.push(ResolvedRange {
            id: i,
            start_index,
            end_index,
            end_wpt_name: end_wpt.name.clone(),
            end_wpt_type: end_wpt.wpt_type.clone(),
            end_wpt_stop_duration: end_wpt.stop_duration,
        });
    }

    let clock_start = boundary_wpts[0].time;

    // ── Phase 1: replay covered intervals at the original pace ─────────────────
    let mut d_eff_m = 0.0_f64;
    let mut elapsed_s = 0.0_f64;
    let mut predicted_so_far = 0.0_f64;
    let mut predicted_stops_so_far = 0.0_f64;
    let mut stop_scratch = 0.0_f64;
    let mut current_range: usize = resolved.len();
    let mut found_current = false;

    for (idx, rs) in resolved.iter().enumerate() {
        if current_index >= rs.end_index {
            predicted_so_far += advance_range(
                trace,
                rs,
                rs.start_index,
                rs.end_index,
                base_pace_s_per_km,
                k_fatigue,
                clock_start,
                life_base_stop_s,
                weather,
                true,
                &mut d_eff_m,
                &mut elapsed_s,
                &mut stop_scratch,
            );
            predicted_stops_so_far += stop_scratch;
        } else {
            current_range = idx;
            found_current = true;
            if current_index > rs.start_index {
                predicted_so_far += advance_range(
                    trace,
                    rs,
                    rs.start_index,
                    current_index,
                    base_pace_s_per_km,
                    k_fatigue,
                    clock_start,
                    life_base_stop_s,
                    weather,
                    false,
                    &mut d_eff_m,
                    &mut elapsed_s,
                    &mut stop_scratch,
                );
            }
            break;
        }
    }

    // ── Solve calibration factor ───────────────────────────────────────────────
    // Compare moving time to moving time: strip planned stops already incurred.
    let calibration_factor = {
        let moving_elapsed = actual_elapsed_s - predicted_stops_so_far;
        if moving_elapsed > 0.0 && predicted_so_far >= MIN_CALIBRATION_PREDICTION_S {
            (moving_elapsed / predicted_so_far).clamp(CALIBRATION_MIN, CALIBRATION_MAX)
        } else {
            1.0
        }
    };
    let calibrated_pace = base_pace_s_per_km * calibration_factor;

    // ── Phase 2: forward-predict remaining intervals at the calibrated pace ────
    // Re-seed the circadian clock to the runner's real elapsed time.
    elapsed_s = actual_elapsed_s;

    let mut etas: Vec<RecalibratedEta> = Vec::with_capacity(resolved.len());
    let mut cumulative = 0.0_f64;

    for (idx, rs) in resolved.iter().enumerate() {
        let is_remaining =
            idx > current_range || (idx == current_range && found_current);
        let remaining = if is_remaining {
            let from = if idx == current_range && current_index > rs.start_index {
                current_index
            } else {
                rs.start_index
            };
            let mut stop_secs = 0.0;
            let moving = advance_range(
                trace,
                rs,
                from,
                rs.end_index,
                calibrated_pace,
                k_fatigue,
                clock_start,
                life_base_stop_s,
                weather,
                true,
                &mut d_eff_m,
                &mut elapsed_s,
                &mut stop_secs,
            );
            let r = moving + stop_secs;
            cumulative += r;
            r
        } else {
            0.0
        };

        etas.push(RecalibratedEta {
            id: rs.id,
            end_index: rs.end_index,
            remaining_duration_s: remaining,
            cumulative_remaining_s: if remaining > 0.0 { cumulative } else { 0.0 },
        });
    }

    Some(Recalibration {
        calibration_factor,
        calibrated_base_pace_s_per_km: calibrated_pace,
        predicted_so_far_s: predicted_so_far,
        actual_elapsed_s,
        etas,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pace_model::{DEFAULT_BASE_PACE_S_PER_KM, K_FATIGUE};

    fn build_flat_trace(n: usize) -> Trace {
        let locs: Vec<Location> = (0..n)
            .map(|i| Location {
                longitude: 0.0,
                latitude: i as f64 * 0.001,
                altitude: 100.0,
            })
            .collect();
        Trace::new(&locs).unwrap()
    }

    fn make_waypoint(lat: f64, name: &str, wpt_type: Option<&str>) -> Waypoint {
        Waypoint {
            latitude: lat,
            longitude: 0.0,
            elevation: None,
            name: name.to_string(),
            description: None,
            comment: None,
            symbol: None,
            wpt_type: wpt_type.map(String::from),
            time: None,
            stop_duration: None,
        }
    }

    // Start / TB1 / LB1 / LB2 / Arrival — 4 section ranges, 3 stage ranges.
    fn recal_waypoints() -> Vec<Waypoint> {
        vec![
            make_waypoint(0.000, "Start", Some("Start")),
            make_waypoint(0.005, "TB1", Some("TimeBarrier")),
            make_waypoint(0.010, "LB1", Some("LifeBase")),
            make_waypoint(0.020, "LB2", Some("LifeBase")),
            make_waypoint(0.029, "Arrival", Some("Arrival")),
        ]
    }

    #[test]
    fn returns_none_with_fewer_than_two_boundaries() {
        let trace = build_flat_trace(30);
        let waypoints = vec![make_waypoint(0.0, "Start", Some("Start"))];
        assert!(recalibrate_from_current(
            &trace,
            &waypoints,
            BoundaryKind::Section,
            0,
            0.0,
            DEFAULT_BASE_PACE_S_PER_KM,
            K_FATIGUE,
            0,
            &WeatherLookup::empty()
        )
        .is_none());
    }

    #[test]
    fn no_elapsed_keeps_factor_one_and_coherent_cumulative() {
        let trace = build_flat_trace(30);
        let result = recalibrate_from_current(
            &trace,
            &recal_waypoints(),
            BoundaryKind::Section,
            0,
            0.0,
            DEFAULT_BASE_PACE_S_PER_KM,
            K_FATIGUE,
            0,
            &WeatherLookup::empty(),
        )
        .unwrap();

        assert!((result.calibration_factor - 1.0).abs() < 1e-9);
        assert_eq!(result.etas.len(), 4);

        let sum: f64 = result.etas.iter().map(|e| e.remaining_duration_s).sum();
        assert!(
            (sum - result.etas.last().unwrap().cumulative_remaining_s).abs() < 1e-6
        );
    }

    #[test]
    fn slower_runner_gets_higher_factor() {
        let trace = build_flat_trace(30);
        let fast = recalibrate_from_current(
            &trace,
            &recal_waypoints(),
            BoundaryKind::Section,
            10,
            300.0,
            DEFAULT_BASE_PACE_S_PER_KM,
            K_FATIGUE,
            0,
            &WeatherLookup::empty(),
        )
        .unwrap();
        let slow = recalibrate_from_current(
            &trace,
            &recal_waypoints(),
            BoundaryKind::Section,
            10,
            1200.0,
            DEFAULT_BASE_PACE_S_PER_KM,
            K_FATIGUE,
            0,
            &WeatherLookup::empty(),
        )
        .unwrap();
        assert!(slow.calibration_factor > fast.calibration_factor);
        assert!(
            slow.etas.last().unwrap().cumulative_remaining_s
                > fast.etas.last().unwrap().cumulative_remaining_s
        );
    }

    #[test]
    fn completed_intervals_report_zero() {
        let trace = build_flat_trace(30);
        let result = recalibrate_from_current(
            &trace,
            &recal_waypoints(),
            BoundaryKind::Section,
            15,
            0.0,
            DEFAULT_BASE_PACE_S_PER_KM,
            K_FATIGUE,
            0,
            &WeatherLookup::empty(),
        )
        .unwrap();
        assert_eq!(result.etas.len(), 4);
        assert_eq!(result.etas[0].remaining_duration_s, 0.0);
        assert_eq!(result.etas[0].cumulative_remaining_s, 0.0);
        assert_eq!(result.etas[1].remaining_duration_s, 0.0);
        assert!(result.etas[2].remaining_duration_s > 0.0);
        assert!(result.etas[3].remaining_duration_s > 0.0);
        assert!(
            result.etas[3].cumulative_remaining_s > result.etas[2].cumulative_remaining_s
        );
    }

    #[test]
    fn stage_kind_ignores_time_barriers() {
        let trace = build_flat_trace(30);
        let as_sections = recalibrate_from_current(
            &trace,
            &recal_waypoints(),
            BoundaryKind::Section,
            0,
            0.0,
            DEFAULT_BASE_PACE_S_PER_KM,
            K_FATIGUE,
            0,
            &WeatherLookup::empty(),
        )
        .unwrap();
        let as_stages = recalibrate_from_current(
            &trace,
            &recal_waypoints(),
            BoundaryKind::Stage,
            0,
            0.0,
            DEFAULT_BASE_PACE_S_PER_KM,
            K_FATIGUE,
            0,
            &WeatherLookup::empty(),
        )
        .unwrap();
        assert_eq!(as_sections.etas.len(), 4);
        assert_eq!(as_stages.etas.len(), 3);

        // Stage 0 (Start→LB1) spans sections 0 + 1.
        let stage0 = as_stages.etas[0].remaining_duration_s;
        let sec01 = as_sections.etas[0].remaining_duration_s
            + as_sections.etas[1].remaining_duration_s;
        assert!((stage0 - sec01).abs() < 1e-6);
    }

    #[test]
    fn planned_stop_does_not_shift_calibration_factor() {
        let trace = build_flat_trace(30);
        let stop_s: u32 = 1800;
        let moving_elapsed = 1500.0_f64;

        // Waypoints with LB1 having a known stop time.
        let waypoints_with_stop = {
            let mut w = recal_waypoints();
            w[2].stop_duration = Some(stop_s); // LB1
            w
        };

        let no_stop = recalibrate_from_current(
            &trace,
            &recal_waypoints(),
            BoundaryKind::Section,
            15,
            moving_elapsed,
            DEFAULT_BASE_PACE_S_PER_KM,
            K_FATIGUE,
            0,
            &WeatherLookup::empty(),
        )
        .unwrap();
        let with_stop = recalibrate_from_current(
            &trace,
            &waypoints_with_stop,
            BoundaryKind::Section,
            15,
            moving_elapsed + stop_s as f64,
            DEFAULT_BASE_PACE_S_PER_KM,
            K_FATIGUE,
            0,
            &WeatherLookup::empty(),
        )
        .unwrap();

        assert!(no_stop.calibration_factor > 1.0);
        assert!(
            (no_stop.calibration_factor - with_stop.calibration_factor).abs() < 1e-9
        );
    }

    #[test]
    fn calibration_factor_clamped_at_max() {
        let trace = build_flat_trace(30);
        // current_index=10 gives ~555 s predicted (10 × 0.111 km × 500 s/km) > 300 s gate.
        // actual_elapsed=4× predicted → raw factor ≈ 4.0 → clamped to CALIBRATION_MAX.
        let result = recalibrate_from_current(
            &trace,
            &recal_waypoints(),
            BoundaryKind::Section,
            10,
            2500.0,
            DEFAULT_BASE_PACE_S_PER_KM,
            K_FATIGUE,
            0,
            &WeatherLookup::empty(),
        )
        .unwrap();
        assert!((result.calibration_factor - CALIBRATION_MAX).abs() < 1e-9);
    }

    #[test]
    fn calibration_factor_clamped_at_min() {
        let trace = build_flat_trace(30);
        // actual_elapsed far below predicted → raw factor << 0.5 → clamped to CALIBRATION_MIN.
        let result = recalibrate_from_current(
            &trace,
            &recal_waypoints(),
            BoundaryKind::Section,
            10,
            10.0,
            DEFAULT_BASE_PACE_S_PER_KM,
            K_FATIGUE,
            0,
            &WeatherLookup::empty(),
        )
        .unwrap();
        assert!((result.calibration_factor - CALIBRATION_MIN).abs() < 1e-9);
    }
}
