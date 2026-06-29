/// Metabolic cost of running in J·kg⁻¹·m⁻¹ for a given slope (Minetti et al. 2002).
///
/// Domain: [−0.45, +0.45] (clamped). Key values:
/// - slope =  0.00 → 3.6  (flat baseline)
/// - slope = −0.10 → ~2.8 (cheapest terrain, gentle descent)
/// - slope = +0.10 → ~6.0 (×1.67 flat)
/// - slope = +0.20 → ~9.8 (×2.72 flat)
pub fn cmet(slope: f64) -> f64 {
    let i = slope.clamp(-0.45, 0.45);
    // Horner's method: 155.4i⁵ − 30.4i⁴ − 43.3i³ + 46.3i² + 19.5i + 3.6
    i.mul_add(
        i.mul_add(i.mul_add(i.mul_add(155.4 * i - 30.4, -43.3), 46.3), 19.5),
        3.6,
    )
}

pub const CMET_FLAT: f64 = 3.6;
const INV_CMET_FLAT: f64 = 1.0 / 3.6;

/// Pace factor relative to flat terrain: `cmet(slope) / cmet(0)`.
///
/// Multiply a runner's flat pace by this to get the equivalent effort pace on this slope.
pub fn pace_factor(slope: f64) -> f64 {
    cmet(slope) * INV_CMET_FLAT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_terrain_baseline() {
        assert!((cmet(0.0) - 3.6).abs() < 1e-9);
    }

    #[test]
    fn gentle_descent_cheaper_than_flat() {
        assert!(cmet(-0.10) < CMET_FLAT);
    }

    #[test]
    fn climbs_cost_progressively_more() {
        assert!(cmet(0.10) > cmet(0.05));
        assert!(cmet(0.20) > cmet(0.10));
        assert!(cmet(0.30) > cmet(0.20));
    }

    #[test]
    fn steep_descent_costs_more_than_gentle() {
        assert!(cmet(-0.40) > cmet(-0.15));
    }

    #[test]
    fn clamped_at_domain_boundaries() {
        assert!((cmet(-0.45) - cmet(-0.99)).abs() < 1e-9);
        assert!((cmet(0.45) - cmet(0.99)).abs() < 1e-9);
    }

    #[test]
    fn flat_pace_factor_is_one() {
        assert!((pace_factor(0.0) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn ten_percent_slope_approx_1_67() {
        assert!((pace_factor(0.10) - 1.67).abs() < 0.05);
    }

    #[test]
    fn gentle_descent_pace_factor_below_one() {
        assert!(pace_factor(-0.10) < 1.0);
    }
}
