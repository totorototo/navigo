use crate::Location;

/// Numerically stable triangle area from three side lengths (Kahan 2014).
/// Sides are sorted descending so each subtraction stays well-conditioned.
/// Returns 0 for degenerate (collinear) triangles.
fn triangle_area(side_a: f64, side_b: f64, side_c: f64) -> f64 {
    let (mut a, mut b, mut c) = (side_a, side_b, side_c);
    if a < b {
        std::mem::swap(&mut a, &mut b);
    }
    if b < c {
        std::mem::swap(&mut b, &mut c);
    }
    if a < b {
        std::mem::swap(&mut a, &mut b);
    }
    let t = (a + (b + c)) * (c - (a - b)) * (c + (a - b)) * (a + (b - c));
    if t <= 0.0 {
        0.0
    } else {
        0.25 * t.sqrt()
    }
}

/// Perpendicular distance from `point` to the line through `line_start`–`line_end`.
/// Uses 2-D haversine (ignores altitude) so simplification preserves horizontal
/// path shape rather than being skewed by elevation noise.
/// Distances are in km (same unit as `Location::calculate_distance_to`).
pub fn perpendicular_distance(point: &Location, line_start: &Location, line_end: &Location) -> f64 {
    let line_dist = line_start.calculate_distance_to(line_end);
    if line_dist < 1e-6 {
        // Line is effectively a point — return distance to that point.
        return point.calculate_distance_to(line_start);
    }
    let d_start = line_start.calculate_distance_to(point);
    let d_end = line_end.calculate_distance_to(point);
    let area = triangle_area(line_dist, d_start, d_end);
    (2.0 * area) / line_dist
}

/// Douglas-Peucker simplification.
/// Returns the indices (ascending) of the points that survive simplification.
/// Endpoints are always kept. `epsilon` is in km (same unit as `calculate_distance_to`).
///
/// Uses an explicit work stack instead of recursion — no call-stack depth limit
/// regardless of how deeply the polyline subdivides.
pub fn douglas_peucker_indices(locations: &[Location], epsilon: f64) -> Vec<usize> {
    let n = locations.len();
    if n <= 2 {
        return (0..n).collect();
    }

    let mut keep = vec![false; n];
    keep[0] = true;
    keep[n - 1] = true;

    let mut stack: Vec<(usize, usize)> = vec![(0, n - 1)];

    while let Some((lo, hi)) = stack.pop() {
        let mut max_dist = 0.0f64;
        let mut max_idx = lo + 1;

        for i in (lo + 1)..hi {
            let d = perpendicular_distance(&locations[i], &locations[lo], &locations[hi]);
            if d > max_dist {
                max_dist = d;
                max_idx = i;
            }
        }

        if max_dist > epsilon {
            keep[max_idx] = true;
            stack.push((lo, max_idx));
            stack.push((max_idx, hi));
        }
    }

    (0..n).filter(|&i| keep[i]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Location;

    fn loc(lat: f64, lon: f64) -> Location {
        Location {
            latitude: lat,
            longitude: lon,
            altitude: 0.0,
        }
    }

    #[test]
    fn straight_line_simplifies_to_endpoints() {
        let points = vec![
            loc(0.0, 0.0),
            loc(0.0001, 0.0),
            loc(0.0002, 0.0),
            loc(0.0003, 0.0),
            loc(0.0004, 0.0),
        ];
        // epsilon = 0.1 km (100 m) — all interior points are collinear, so only endpoints kept
        let idx = douglas_peucker_indices(&points, 0.1);
        assert_eq!(idx, vec![0, 4]);
    }

    #[test]
    fn endpoints_always_kept() {
        let points = vec![
            loc(0.0, 0.0),
            loc(0.5, 0.0),
            loc(1.0, 5.0),
            loc(1.5, 0.0),
            loc(2.0, 0.0),
        ];
        let idx = douglas_peucker_indices(&points, 0.001);
        assert_eq!(idx[0], 0);
        assert_eq!(*idx.last().unwrap(), 4);
    }

    #[test]
    fn two_points_returned_unchanged() {
        let points = vec![loc(0.0, 0.0), loc(1.0, 1.0)];
        let idx = douglas_peucker_indices(&points, 1.0);
        assert_eq!(idx, vec![0, 1]);
    }

    #[test]
    fn single_point_returned_unchanged() {
        let points = vec![loc(0.0, 0.0)];
        let idx = douglas_peucker_indices(&points, 1.0);
        assert_eq!(idx, vec![0]);
    }

    #[test]
    fn peak_survives_with_tight_epsilon() {
        // Clear "peak" displaced far from the straight line — must survive.
        let points = vec![
            loc(0.0, 0.0),
            loc(0.0, 0.001),
            loc(5.0, 0.0005), // far off to the side
            loc(0.0, 0.002),
            loc(0.0, 0.003),
        ];
        let idx = douglas_peucker_indices(&points, 0.001);
        assert!(idx.contains(&0));
        assert!(idx.contains(&4));
        assert!(idx.contains(&2), "off-line peak should survive");
    }

    #[test]
    fn larger_epsilon_keeps_fewer_points() {
        let points: Vec<Location> = (0..20)
            .map(|i| loc(i as f64 * 0.001, (i % 3) as f64 * 0.0001))
            .collect();
        let loose = douglas_peucker_indices(&points, 0.5);
        let strict = douglas_peucker_indices(&points, 0.0001);
        assert!(loose.len() <= strict.len());
        assert!(loose.len() >= 2);
    }
}
