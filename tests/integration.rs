//! Integration tests covering the full analysis pipeline end-to-end.
//!
//! Tests exercise GPX parsing, trace construction, section/stage computation,
//! and calibration — the same paths the WASM layer calls.

use navigo::{
    build_trace, parse_metadata, parse_trace_points, parse_waypoints, AnalysisOptions,
    BoundaryKind, Location, Trace, Waypoint,
};

// ── GPX Parsing ──────────────────────────────────────────────────────────────

const MINIMAL_GPX: &[u8] = b"<?xml version=\"1.0\"?>\n\
<gpx>\n\
  <metadata><name>Test Route</name><desc>A short test</desc></metadata>\n\
  <trk><trkseg>\n\
    <trkpt lat=\"45.0\" lon=\"6.0\"><ele>1000</ele></trkpt>\n\
    <trkpt lat=\"45.001\" lon=\"6.0\"><ele>1010</ele></trkpt>\n\
    <trkpt lat=\"45.002\" lon=\"6.0\"><ele>1005</ele></trkpt>\n\
    <trkpt lat=\"45.003\" lon=\"6.0\"><ele>1020</ele></trkpt>\n\
    <trkpt lat=\"45.004\" lon=\"6.0\"><ele>1030</ele></trkpt>\n\
  </trkseg></trk>\n\
  <wpt lat=\"45.0\" lon=\"6.0\"><name>Start</name><type>Start</type><time>2025-06-15T06:00:00Z</time></wpt>\n\
  <wpt lat=\"45.002\" lon=\"6.0\"><name>Ravito</name><type>LifeBase</type><time>2025-06-15T10:00:00Z</time></wpt>\n\
  <wpt lat=\"45.004\" lon=\"6.0\"><name>Finish</name><type>Arrival</type><time>2025-06-15T14:00:00Z</time></wpt>\n\
</gpx>";

#[test]
fn parse_trace_points_extracts_all_trkpts() {
    let locs = parse_trace_points(MINIMAL_GPX);
    assert_eq!(locs.len(), 5);
    assert!((locs[0].latitude - 45.0).abs() < 1e-9);
    assert!((locs[0].longitude - 6.0).abs() < 1e-9);
    assert!((locs[0].altitude - 1000.0).abs() < 1e-9);
    assert!((locs[4].latitude - 45.004).abs() < 1e-9);
    assert!((locs[4].altitude - 1030.0).abs() < 1e-9);
}

#[test]
fn parse_waypoints_extracts_typed_wpts() {
    let wpts = parse_waypoints(MINIMAL_GPX);
    assert_eq!(wpts.len(), 3);
    assert_eq!(wpts[0].name, "Start");
    assert_eq!(wpts[0].wpt_type.as_deref(), Some("Start"));
    assert_eq!(wpts[1].wpt_type.as_deref(), Some("LifeBase"));
    assert_eq!(wpts[2].wpt_type.as_deref(), Some("Arrival"));
    // Times should be parsed
    assert!(wpts[0].time.is_some());
    assert!(wpts[2].time.is_some());
}

#[test]
fn parse_metadata_extracts_name_and_desc() {
    let meta = parse_metadata(MINIMAL_GPX);
    assert_eq!(meta.name.as_deref(), Some("Test Route"));
    assert_eq!(meta.description.as_deref(), Some("A short test"));
}

#[test]
fn parse_trace_points_handles_empty_gpx() {
    let locs = parse_trace_points(b"<gpx></gpx>");
    assert!(locs.is_empty());
}

#[test]
fn parse_trace_points_skips_malformed_trkpt() {
    // Missing lat attribute
    let gpx = br#"<gpx><trk><trkseg>
        <trkpt lon="6.0"><ele>100</ele></trkpt>
        <trkpt lat="45.0" lon="6.0"><ele>200</ele></trkpt>
    </trkseg></trk></gpx>"#;
    let locs = parse_trace_points(gpx);
    assert_eq!(locs.len(), 1);
    assert!((locs[0].altitude - 200.0).abs() < 1e-9);
}

#[test]
fn parse_trace_points_handles_single_quotes() {
    let gpx = br#"<gpx><trk><trkseg>
        <trkpt lat='48.0' lon='2.0'><ele>50</ele></trkpt>
    </trkseg></trk></gpx>"#;
    let locs = parse_trace_points(gpx);
    assert_eq!(locs.len(), 1);
    assert!((locs[0].latitude - 48.0).abs() < 1e-9);
}

// ── Trace Construction ───────────────────────────────────────────────────────

#[test]
fn trace_from_gpx_has_correct_properties() {
    let locs = parse_trace_points(MINIMAL_GPX);
    let trace = build_trace(&locs).unwrap();

    assert_eq!(trace.locations().len(), 5);
    assert!(trace.total_distance() > 0.0);
    assert!(trace.total_elevation_gain() >= 0.0);
    assert!(trace.total_elevation_loss() >= 0.0);
    // Cumulative distances are monotonically non-decreasing
    let dists = trace.cumulative_distances();
    for i in 1..dists.len() {
        assert!(dists[i] >= dists[i - 1]);
    }
}

#[test]
fn trace_single_point() {
    let locs = vec![Location {
        latitude: 45.0,
        longitude: 6.0,
        altitude: 1000.0,
    }];
    let trace = build_trace(&locs).unwrap();
    assert_eq!(trace.locations().len(), 1);
    assert!((trace.total_distance() - 0.0).abs() < 1e-9);
}

#[test]
fn trace_many_points_construction() {
    // 100 points in a straight line — trace should handle gracefully
    let locs: Vec<Location> = (0..100)
        .map(|i| Location {
            latitude: 45.0 + i as f64 * 0.0001,
            longitude: 6.0,
            altitude: 100.0,
        })
        .collect();
    let trace = build_trace(&locs).unwrap();
    // Should have at least 2 points and valid distance
    assert!(trace.locations().len() >= 2);
    assert!(trace.total_distance() > 0.0);
}

#[test]
fn trace_elevation_gain_loss_coherence() {
    // Saw-tooth: up 50, down 30, up 50, down 30 — gain should exceed loss
    let locs: Vec<Location> = vec![
        Location {
            latitude: 0.0,
            longitude: 0.0,
            altitude: 100.0,
        },
        Location {
            latitude: 0.001,
            longitude: 0.0,
            altitude: 150.0,
        },
        Location {
            latitude: 0.002,
            longitude: 0.0,
            altitude: 120.0,
        },
        Location {
            latitude: 0.003,
            longitude: 0.0,
            altitude: 170.0,
        },
        Location {
            latitude: 0.004,
            longitude: 0.0,
            altitude: 140.0,
        },
    ];
    let trace = build_trace(&locs).unwrap();
    assert!(trace.total_elevation_gain() > 0.0);
    assert!(trace.total_elevation_loss() > 0.0);
}

// ── Race Analysis Pipeline ───────────────────────────────────────────────────

fn build_race_trace() -> (Trace, Vec<Waypoint>) {
    let locs = parse_trace_points(MINIMAL_GPX);
    let wpts = parse_waypoints(MINIMAL_GPX);
    let trace = build_trace(&locs).unwrap();
    (trace, wpts)
}

#[test]
fn section_analysis_from_gpx() {
    let (trace, wpts) = build_race_trace();
    let sections =
        navigo::section::compute_from_waypoints(&trace, &wpts, &AnalysisOptions::default())
            .unwrap();

    // Start → LifeBase → Arrival = 2 sections
    assert_eq!(sections.len(), 2);
    assert_eq!(sections[0].start_location, "Start");
    assert_eq!(sections[0].end_location, "Ravito");
    assert_eq!(sections[1].start_location, "Ravito");
    assert_eq!(sections[1].end_location, "Finish");

    // Basic sanity
    for s in &sections {
        assert!(s.total_distance_km > 0.0);
        assert!(s.estimated_duration_s > 0.0);
        assert!(s.difficulty >= 1 && s.difficulty <= 5);
        assert!(s.pace_factor > 0.0);
    }
}

#[test]
fn stage_analysis_from_gpx() {
    let (trace, wpts) = build_race_trace();
    let stages =
        navigo::stage::compute_from_waypoints(&trace, &wpts, &AnalysisOptions::default()).unwrap();

    // Start → LifeBase → Arrival = 2 stages
    assert_eq!(stages.len(), 2);
    assert_eq!(stages[0].stage_id, 0);
    assert_eq!(stages[1].stage_id, 1);
}

#[test]
fn sections_and_stages_have_consistent_distances() {
    let (trace, wpts) = build_race_trace();
    let sections =
        navigo::section::compute_from_waypoints(&trace, &wpts, &AnalysisOptions::default())
            .unwrap();
    let stages =
        navigo::stage::compute_from_waypoints(&trace, &wpts, &AnalysisOptions::default()).unwrap();

    // Since there are no TimeBarriers, sections == stages
    assert_eq!(sections.len(), stages.len());
    for (sec, stg) in sections.iter().zip(stages.iter()) {
        assert!((sec.total_distance_km - stg.total_distance_km).abs() < 1e-6);
    }
}

#[test]
fn max_completion_time_from_waypoint_timestamps() {
    let (trace, wpts) = build_race_trace();
    let sections =
        navigo::section::compute_from_waypoints(&trace, &wpts, &AnalysisOptions::default())
            .unwrap();

    // Start at 06:00, LifeBase at 10:00 → 4h = 14400s
    assert_eq!(sections[0].max_completion_time, Some(14400));
    // LifeBase at 10:00, Arrival at 14:00 → 4h = 14400s
    assert_eq!(sections[1].max_completion_time, Some(14400));
}

// ── Calibration Pipeline ─────────────────────────────────────────────────────

#[test]
fn recalibration_from_start() {
    let (trace, wpts) = build_race_trace();
    let result = navigo::calibration::recalibrate_from_current(
        &trace,
        &wpts,
        BoundaryKind::Section,
        0,
        0.0,
        &AnalysisOptions::default().life_base_stop(0),
    )
    .unwrap();

    // Factor should be 1.0 with zero elapsed
    assert!((result.calibration_factor - 1.0).abs() < 1e-9);
    // All intervals are remaining
    assert!(result.etas.iter().all(|e| e.remaining_duration_s > 0.0));
}

#[test]
fn recalibration_returns_none_without_boundaries() {
    let locs = vec![
        Location {
            latitude: 0.0,
            longitude: 0.0,
            altitude: 0.0,
        },
        Location {
            latitude: 0.001,
            longitude: 0.0,
            altitude: 0.0,
        },
    ];
    let trace = build_trace(&locs).unwrap();
    let wpts: Vec<Waypoint> = vec![];
    let result = navigo::calibration::recalibrate_from_current(
        &trace,
        &wpts,
        BoundaryKind::Section,
        0,
        0.0,
        &AnalysisOptions::default(),
    );
    assert!(result.is_none());
}

// ── Analysis Options Builder ─────────────────────────────────────────────────

#[test]
fn analysis_options_builder_overrides() {
    let opts = AnalysisOptions::default()
        .base_pace(600.0)
        .fatigue(0.005)
        .life_base_stop(1800);

    assert!((opts.base_pace_s_per_km - 600.0).abs() < 1e-9);
    assert!((opts.k_fatigue - 0.005).abs() < 1e-9);
    assert_eq!(opts.life_base_stop_s, 1800);
}

#[test]
fn different_pace_changes_duration() {
    let (trace, wpts) = build_race_trace();
    let fast = navigo::section::compute_from_waypoints(
        &trace,
        &wpts,
        &AnalysisOptions::default()
            .base_pace(400.0)
            .life_base_stop(0),
    )
    .unwrap();
    let slow = navigo::section::compute_from_waypoints(
        &trace,
        &wpts,
        &AnalysisOptions::default()
            .base_pace(600.0)
            .life_base_stop(0),
    )
    .unwrap();

    assert!(slow[0].estimated_duration_s > fast[0].estimated_duration_s);
}

// ── Edge Cases ───────────────────────────────────────────────────────────────

#[test]
fn duplicate_adjacent_points() {
    let locs = vec![
        Location {
            latitude: 45.0,
            longitude: 6.0,
            altitude: 100.0,
        },
        Location {
            latitude: 45.0,
            longitude: 6.0,
            altitude: 100.0,
        },
        Location {
            latitude: 45.001,
            longitude: 6.0,
            altitude: 110.0,
        },
    ];
    let trace = build_trace(&locs).unwrap();
    // Should not panic; distance is still valid
    assert!(trace.total_distance() >= 0.0);
}

#[test]
fn trace_with_altitude_spike() {
    // A single 500m spike in the middle should be smoothed by the denoising
    let locs: Vec<Location> = (0..10)
        .map(|i| Location {
            latitude: 45.0 + i as f64 * 0.0001,
            longitude: 6.0,
            altitude: if i == 5 { 600.0 } else { 100.0 },
        })
        .collect();
    let trace = build_trace(&locs).unwrap();
    // Denoised gain should be much less than the raw 500m spike
    assert!(trace.total_elevation_gain() < 100.0);
}

#[test]
fn trace_slopes_bounded() {
    // Slopes should be finite and reasonable even with extreme altitude changes
    let locs: Vec<Location> = (0..5)
        .map(|i| Location {
            latitude: 45.0 + i as f64 * 0.0001,
            longitude: 6.0,
            altitude: i as f64 * 100.0,
        })
        .collect();
    let trace = build_trace(&locs).unwrap();
    for &s in trace.slopes() {
        assert!(s.is_finite());
    }
}

// ── GPX with real fixture ────────────────────────────────────────────────────

#[test]
fn parse_real_gpx_fixture() {
    let gpx_bytes =
        std::fs::read("tests/fixtures/grp-160-2026.gpx").expect("fixture file should exist");
    let locs = parse_trace_points(&gpx_bytes);
    let wpts = parse_waypoints(&gpx_bytes);

    // Real GPX should have substantial data
    assert!(
        locs.len() > 100,
        "expected >100 track points, got {}",
        locs.len()
    );
    assert!(
        wpts.len() >= 2,
        "expected >=2 waypoints, got {}",
        wpts.len()
    );

    let trace = build_trace(&locs).unwrap();
    assert!(trace.total_distance() > 10.0); // At least 10 km

    // Section analysis should work end-to-end
    let sections =
        navigo::section::compute_from_waypoints(&trace, &wpts, &AnalysisOptions::default());
    assert!(sections.is_some());
    let sections = sections.unwrap();
    assert!(!sections.is_empty());

    // Every section should have positive distance and duration
    for s in &sections {
        assert!(
            s.total_distance_km > 0.0,
            "section {} has zero distance",
            s.section_id
        );
        assert!(
            s.estimated_duration_s > 0.0,
            "section {} has zero duration",
            s.section_id
        );
    }
}
