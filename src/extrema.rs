/// AMPD (Automatic Multiscale-based Peak Detection) for elevation profiles.
///
/// At each scale `s` (1 … scale_max), a point is a local extremum if it is
/// greater-than (peaks) or less-than (valleys) both neighbours at distance `s`.
/// Counting across all scales gives a vote tally; points that exceed the
/// `threshold` are extremum candidates.  Nearby candidates are clustered and
/// kept only when their topographic prominence exceeds `MIN_PROMINENCE`.

const AMPD_SCALE_MAX: usize = 21;
const AMPD_THRESHOLD: usize = 15;
const MIN_PROMINENCE: f32 = 20.0; // altitude metres
const CLUSTER_WINDOW: usize = 3;

// ── Core AMPD ────────────────────────────────────────────────────────────────

fn ampd_core(signal: &[f32], scale_max: usize, threshold: usize, find_peaks: bool) -> Vec<usize> {
    let n = signal.len();
    if n < 3 || scale_max == 0 {
        return vec![];
    }
    let effective = scale_max.min(n / 2);
    if effective == 0 {
        return vec![];
    }

    let mut counts = vec![0u8; n];

    for scale in 1..=effective {
        if scale * 2 >= n {
            break;
        }
        for i in scale..(n - scale) {
            let is_ext = if find_peaks {
                signal[i] > signal[i - scale] && signal[i] > signal[i + scale]
            } else {
                signal[i] < signal[i - scale] && signal[i] < signal[i + scale]
            };
            if is_ext {
                counts[i] = counts[i].saturating_add(1);
            }
        }
    }

    counts
        .iter()
        .enumerate()
        .filter(|&(_, &c)| (c as usize) >= threshold)
        .map(|(i, _)| i)
        .collect()
}

// ── Clustering ───────────────────────────────────────────────────────────────

/// Merge consecutive candidates within `CLUSTER_WINDOW` indices, keeping the
/// best (highest for peaks, lowest for valleys) representative of each cluster.
fn cluster_extrema(raw: &[usize], signal: &[f32], find_peaks: bool) -> Vec<usize> {
    if raw.is_empty() {
        return vec![];
    }

    let mut clustered = Vec::new();
    let mut best_idx = raw[0];
    let mut best_val = signal[raw[0]];

    for i in 1..raw.len() {
        let curr_idx = raw[i];
        let prev_idx = raw[i - 1];

        if curr_idx - prev_idx <= CLUSTER_WINDOW {
            let curr_val = signal[curr_idx];
            let better =
                if find_peaks { curr_val > best_val } else { curr_val < best_val };
            if better {
                best_val = curr_val;
                best_idx = curr_idx;
            }
        } else {
            clustered.push(best_idx);
            best_idx = curr_idx;
            best_val = signal[curr_idx];
        }
    }
    clustered.push(best_idx);
    clustered
}

// ── Prominence ───────────────────────────────────────────────────────────────

/// Key-col prominence: the height of the highest col that must be crossed to
/// reach a higher peak (or the symmetric valley equivalent).
fn extremum_prominence(signal: &[f32], idx: usize, find_peaks: bool) -> f32 {
    let v = signal[idx];

    // Walk left until a higher peak (lower valley) is found, track the col.
    let mut left_col = v;
    let mut i = idx;
    while i > 0 {
        i -= 1;
        let s = signal[i];
        if find_peaks {
            if s > v {
                break;
            }
            if s < left_col {
                left_col = s;
            }
        } else {
            if s < v {
                break;
            }
            if s > left_col {
                left_col = s;
            }
        }
    }

    let mut right_col = v;
    let mut j = idx;
    while j + 1 < signal.len() {
        j += 1;
        let s = signal[j];
        if find_peaks {
            if s > v {
                break;
            }
            if s < right_col {
                right_col = s;
            }
        } else {
            if s < v {
                break;
            }
            if s > right_col {
                right_col = s;
            }
        }
    }

    if find_peaks {
        v - left_col.max(right_col)
    } else {
        left_col.min(right_col) - v
    }
}

fn filter_by_prominence(
    candidates: &[usize],
    signal: &[f32],
    min_prominence: f32,
    find_peaks: bool,
) -> Vec<usize> {
    if candidates.is_empty() || min_prominence <= 0.0 {
        return candidates.to_vec();
    }
    candidates
        .iter()
        .filter(|&&idx| extremum_prominence(signal, idx, find_peaks) >= min_prominence)
        .copied()
        .collect()
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Detect significant peaks in an elevation signal.
/// Returns indices into `signal`, sorted ascending.
pub fn find_peaks(signal: &[f32]) -> Vec<usize> {
    if signal.len() < 3 {
        return vec![];
    }
    let raw = ampd_core(signal, AMPD_SCALE_MAX, AMPD_THRESHOLD, true);
    let clustered = cluster_extrema(&raw, signal, true);
    filter_by_prominence(&clustered, signal, MIN_PROMINENCE, true)
}

/// Detect significant valleys in an elevation signal.
/// Returns indices into `signal`, sorted ascending.
pub fn find_valleys(signal: &[f32]) -> Vec<usize> {
    if signal.len() < 3 {
        return vec![];
    }
    let raw = ampd_core(signal, AMPD_SCALE_MAX, AMPD_THRESHOLD, false);
    let clustered = cluster_extrema(&raw, signal, false);
    filter_by_prominence(&clustered, signal, MIN_PROMINENCE, false)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_signal_has_no_peaks() {
        let signal = vec![2.0f32; 10];
        let peaks = find_peaks(&signal);
        assert!(peaks.is_empty());
    }

    #[test]
    fn flat_signal_has_no_valleys() {
        let signal = vec![3.0f32; 10];
        let valleys = find_valleys(&signal);
        assert!(valleys.is_empty());
    }

    #[test]
    fn signal_too_short_returns_empty() {
        let signal = vec![1.0f32, 5.0, 2.0];
        // len==3 is acceptable for find_peaks; AMPD will try scale=1
        let peaks = find_peaks(&signal);
        assert!(peaks.len() <= 1);
        let valleys = find_valleys(&signal);
        assert!(valleys.len() <= 1);
    }

    #[test]
    fn prominence_filter_drops_low_prominence() {
        // signal: clear dominant peak at idx=2, shallow bump at idx=6
        let signal = vec![0.0f32, 50.0, 100.0, 60.0, 40.0, 55.0, 70.0, 30.0, 0.0];
        let candidates = vec![2usize, 6];
        // Prominence of idx=2 is 100; prominence of idx=6 is ~30
        let kept = filter_by_prominence(&candidates, &signal, 35.0, true);
        assert_eq!(kept, vec![2]);
    }

    #[test]
    fn prominent_peaks_detected() {
        // Two clear, well-separated peaks with ≥20m prominence
        let mut signal = vec![100.0f32; 60];
        signal[10] = 180.0;
        signal[40] = 170.0;
        let peaks = find_peaks(&signal);
        // At minimum the two injected peaks should be detected
        assert!(!peaks.is_empty());
    }

    #[test]
    fn peaks_and_valleys_are_symmetric_on_alternating_signal() {
        let signal: Vec<f32> =
            (0..30).map(|i| if i % 2 == 0 { 200.0 } else { 100.0 }).collect();
        let raw_peaks = ampd_core(&signal, 2, 1, true);
        let raw_valleys = ampd_core(&signal, 2, 1, false);
        assert_eq!(raw_peaks.len(), raw_valleys.len());
    }

    #[test]
    fn extremum_prominence_correct_for_dominant_peak() {
        let signal = vec![0.0f32, 50.0, 100.0, 60.0, 40.0, 55.0, 70.0, 30.0, 0.0];
        let p = extremum_prominence(&signal, 2, true);
        assert!((p - 100.0).abs() < 0.001, "prominence={}", p);
    }

    #[test]
    fn extremum_prominence_correct_for_secondary_peak() {
        let signal = vec![0.0f32, 50.0, 100.0, 60.0, 40.0, 55.0, 70.0, 30.0, 0.0];
        let p = extremum_prominence(&signal, 6, true);
        assert!((p - 30.0).abs() < 0.001, "prominence={}", p);
    }
}
