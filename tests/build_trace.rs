use navigo::{build_trace, Location};

#[test]
fn should_build_trace() {
    let paris = Location {
        longitude: 2.350987,
        latitude: 48.856667,
        altitude: 0.0,
    };
    let moscow = Location {
        longitude: 37.617634,
        latitude: 55.755787,
        altitude: 0.0,
    };
    let locations = vec![paris, moscow];
    let trace = build_trace(&locations).unwrap();

    assert!(!trace.locations().is_empty());
    assert!((trace.length() - 2486.340992526076).abs() < 1e-6);
}

#[test]
fn build_trace_rejects_empty_input() {
    assert!(build_trace(&[]).is_err());
}
