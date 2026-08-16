//! Delta-over-time rate computation, shared by the counter-based collectors.
//!
//! Network and disk both read monotonic byte counters and divide the delta by
//! the elapsed wall time. Both did that division unguarded, so a zero or
//! non-finite elapsed produced `inf` — and `serde_json` serializes a
//! non-finite `f64` as `null`, which reaches the GUI as a missing field rather
//! than a number. Every such division now goes through here.

/// Bytes (or any unit) per second from a monotonic counter pair.
///
/// `cur < prev` — a counter reset, or an interface that was renamed onto a
/// recycled slot — saturates to a zero delta rather than wrapping.
pub fn rate(cur: u64, prev: u64, elapsed_s: f64) -> f64 {
    rate_from_delta(cur.saturating_sub(prev) as f64, elapsed_s)
}

/// Rate for callers that compute their own delta (wrap-around counters, unit
/// conversions). Returns 0.0 rather than a non-finite value when `elapsed_s`
/// is zero, negative, or not finite.
pub fn rate_from_delta(delta: f64, elapsed_s: f64) -> f64 {
    if !elapsed_s.is_finite() || elapsed_s <= 0.0 {
        return 0.0;
    }
    let r = delta / elapsed_s;
    if r.is_finite() { r } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_bytes_per_second() {
        assert_eq!(rate(1_000, 0, 1.0), 1_000.0);
        assert_eq!(rate(1_500, 1_000, 0.5), 1_000.0);
        assert_eq!(rate(0, 0, 1.0), 0.0);
    }

    #[test]
    fn counter_reset_saturates_instead_of_wrapping() {
        // Interface counter reset: cur < prev. Must not underflow into a
        // gigantic bogus rate.
        assert_eq!(rate(5, 1_000_000, 1.0), 0.0);
        assert_eq!(rate(0, u64::MAX, 1.0), 0.0);
    }

    /// The bug this module exists to prevent: `delta / 0.0` is `inf`, and
    /// `serde_json` turns a non-finite f64 into `null`.
    #[test]
    fn non_positive_elapsed_yields_zero_not_infinity() {
        for elapsed in [0.0, -1.0, -0.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let r = rate(1_000, 0, elapsed);
            assert!(r.is_finite(), "elapsed {elapsed} produced {r}");
            assert_eq!(r, 0.0, "elapsed {elapsed} produced {r}");
        }
    }

    #[test]
    fn results_always_serialize_as_json_numbers() {
        let r = rate(1_000, 0, 0.0);
        assert_eq!(serde_json::to_string(&r).unwrap(), "0.0");
    }

    #[test]
    fn huge_delta_over_tiny_elapsed_stays_finite() {
        let r = rate_from_delta(f64::MAX, f64::MIN_POSITIVE);
        assert!(r.is_finite());
        assert_eq!(r, 0.0);
    }

    #[test]
    fn delta_variant_handles_unit_conversion() {
        // RAPL-style: microjoules converted to joules, divided by seconds.
        assert_eq!(rate_from_delta(2_000_000.0 / 1_000_000.0, 2.0), 1.0);
    }
}
