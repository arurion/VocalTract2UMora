//! Liljencrants-Fant glottal source helpers.

/// Dimensionless LF timing parameters (all relative to the fundamental period T0).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LfShape {
    /// Return-phase time constant (acoustic roundness / incomplete closure).
    pub ra: f64,
    /// Asymmetry of the open phase (pulse skew).
    pub rk: f64,
    /// Glottal-formant ratio (open-phase rise).
    pub rg: f64,
}

/// Fant (2004) regression from the single voice-quality parameter `Rd`
/// to (Ra, Rk, Rg). `rd` is clamped to the model's valid range 0.3..=2.7.
pub fn shape_from_rd(rd: f64) -> LfShape {
    let rd = rd.clamp(0.3, 2.7);
    let ra = (-1.0 + 4.8 * rd) / 100.0;
    let rk = (22.4 + 11.8 * rd) / 100.0;
    let rg = rk / (4.0 * ((0.11 * rd / (0.5 + 1.2 * rk)) - ra) / rk);
    LfShape { ra, rk, rg }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modal_voice_is_sane() {
        let s = shape_from_rd(1.0);
        assert!((s.ra - 0.038).abs() < 1e-9);
        assert!((s.rk - 0.342).abs() < 1e-9);
        assert!(s.rg.is_finite() && s.rg > 0.0);
    }

    #[test]
    fn rd_is_clamped() {
        assert_eq!(shape_from_rd(-5.0), shape_from_rd(0.3));
        assert_eq!(shape_from_rd(99.0), shape_from_rd(2.7));
    }
}
