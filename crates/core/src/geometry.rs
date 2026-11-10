// SPDX-License-Identifier: GPL-3.0-or-later
//! Speaker geometry (skeleton editor model). JSON I/O lives in the TypeScript layer;
//! the core only receives plain numbers, which keeps it dependency-free.

pub const SPEED_OF_SOUND_CM_S: f64 = 35_000.0;
/// Accepted ranges. The web editor uses the same limits for its sliders and JSON validation.
pub const TRACT_LENGTH_RANGE_CM: (f64, f64) = (8.0, 22.0);
pub const ORAL_RATIO_RANGE: (f64, f64) = (0.25, 0.75);
pub const NASAL_SCALE_RANGE: (f64, f64) = (0.3, 2.5);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpeakerGeometry {
    /// Total vocal tract length (glottis to lips) in cm.
    pub tract_length_cm: f64,
    /// Oral cavity share of the tract (the remainder is pharyngeal), 0..1.
    pub oral_ratio: f64,
    /// Nasal cavity volume scale (1.0 = reference).
    pub nasal_volume_scale: f64,
}

impl Default for SpeakerGeometry {
    fn default() -> Self {
        Self { tract_length_cm: 17.5, oral_ratio: 0.5, nasal_volume_scale: 1.0 }
    }
}

fn fit(v: f64, (lo, hi): (f64, f64), fallback: f64) -> f64 {
    if v.is_finite() { v.clamp(lo, hi) } else { fallback }
}

impl SpeakerGeometry {
    /// Clamps every field into its accepted range (non-finite values fall back to the default).
    pub fn clamped(self) -> Self {
        let d = Self::default();
        Self {
            tract_length_cm: fit(self.tract_length_cm, TRACT_LENGTH_RANGE_CM, d.tract_length_cm),
            oral_ratio: fit(self.oral_ratio, ORAL_RATIO_RANGE, d.oral_ratio),
            nasal_volume_scale: fit(self.nasal_volume_scale, NASAL_SCALE_RANGE, d.nasal_volume_scale),
        }
    }

    pub fn oral_length_cm(&self) -> f64 {
        self.tract_length_cm * self.oral_ratio.clamp(0.0, 1.0)
    }

    pub fn pharyngeal_length_cm(&self) -> f64 {
        self.tract_length_cm - self.oral_length_cm()
    }

    /// Resonances of a uniform closed-open tube of the same length: F_n = (2n-1) c / 4L.
    /// A reference for the schwa-like neutral vocal tract, not a model of real vowels.
    pub fn neutral_formants(&self, count: usize) -> Vec<f64> {
        (0..count)
            .map(|n| (2 * n + 1) as f64 * SPEED_OF_SOUND_CM_S / (4.0 * self.tract_length_cm))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lengths_add_up() {
        let g = SpeakerGeometry { tract_length_cm: 16.0, oral_ratio: 0.25, nasal_volume_scale: 1.0 };
        assert_eq!(g.oral_length_cm(), 4.0);
        assert_eq!(g.pharyngeal_length_cm(), 12.0);
    }

    #[test]
    fn neutral_formants_of_17_5_cm_tract() {
        let f = SpeakerGeometry::default().neutral_formants(3);
        assert!((f[0] - 500.0).abs() < 1e-9);
        assert!((f[1] - 1500.0).abs() < 1e-9);
        assert!((f[2] - 2500.0).abs() < 1e-9);
    }

    #[test]
    fn clamps_and_replaces_non_finite() {
        let g = SpeakerGeometry { tract_length_cm: 99.0, oral_ratio: f64::NAN, nasal_volume_scale: -1.0 }.clamped();
        assert_eq!(g.tract_length_cm, TRACT_LENGTH_RANGE_CM.1);
        assert_eq!(g.oral_ratio, SpeakerGeometry::default().oral_ratio);
        assert_eq!(g.nasal_volume_scale, NASAL_SCALE_RANGE.0);
    }
}
