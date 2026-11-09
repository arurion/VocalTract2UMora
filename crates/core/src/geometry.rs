//! Speaker geometry (skeleton editor model). JSON I/O lives in the TypeScript layer;
//! the core only receives plain numbers, which keeps it dependency-free.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpeakerGeometry {
    /// Total vocal tract length in cm.
    pub tract_length_cm: f64,
    /// Oral cavity share of the tract (remainder is pharyngeal), 0..1.
    pub oral_ratio: f64,
    /// Nasal cavity volume scale (1.0 = reference).
    pub nasal_volume_scale: f64,
}

impl Default for SpeakerGeometry {
    fn default() -> Self {
        Self { tract_length_cm: 17.5, oral_ratio: 0.5, nasal_volume_scale: 1.0 }
    }
}

impl SpeakerGeometry {
    pub fn oral_length_cm(&self) -> f64 {
        self.tract_length_cm * self.oral_ratio.clamp(0.0, 1.0)
    }
    pub fn pharyngeal_length_cm(&self) -> f64 {
        self.tract_length_cm - self.oral_length_cm()
    }
}
