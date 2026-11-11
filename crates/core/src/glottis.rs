// SPDX-License-Identifier: GPL-3.0-or-later
//! Geometric glottis: a kinematic model of the vocal folds, ported from VocalTractLab's
//! `GeometricGlottis` (C) Peter Birkholz, GPL-3.0-or-later.
//!
//! The folds do not obey a mechanical equation of motion. The displacement of the lower and
//! upper fold edges follows a (phase-warped) sinusoid whose amplitude depends on the pressure
//! difference across the glottis (Titze 1989), so lung pressure switches phonation on and off.

use crate::iir::Iir;
use crate::tube::{LOWER_GLOTTIS, UPPER_GLOTTIS};
use std::f64::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Range {
    pub min: f64,
    pub max: f64,
}

/// Control parameters (the ones a gestural score would drive over time).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Control {
    /// Fundamental frequency (Hz), 40..600.
    pub f0: f64,
    /// Lung pressure (dPa), 0..20000. 1 cmH2O = 980.665 dPa.
    pub pressure: f64,
    /// Displacement of the lower fold edge at the vocal processes (cm), -0.05..0.3.
    pub lower_end_x: f64,
    /// Displacement of the upper fold edge at the vocal processes (cm), -0.05..0.3.
    pub upper_end_x: f64,
    /// Area of the chink between the arytenoids (cm^2), -0.25..0.25 (negative = virtual target).
    pub chink_area: f64,
    /// Phase lag of the upper edge behind the lower edge (rad), 0..pi.
    pub phase_lag: f64,
    /// Relative amplitude -1..1 (stiffness / friction; negative = virtual target, treated as 0).
    pub relative_amplitude: f64,
    /// Diplophonia: amount by which every second pulse is reduced, 0..1.
    pub double_pulsing: f64,
    /// Pulse skewness -0.5..0.5.
    pub pulse_skewness: f64,
    /// Flutter (%), 0..100.
    pub flutter: f64,
    /// Aspiration strength (dB), -40..0.
    pub aspiration_db: f64,
}

pub const CONTROL_RANGES: [(&str, Range); 11] = [
    ("f0", Range { min: 40.0, max: 600.0 }),
    ("pressure", Range { min: 0.0, max: 20000.0 }),
    ("lower_end_x", Range { min: -0.05, max: 0.3 }),
    ("upper_end_x", Range { min: -0.05, max: 0.3 }),
    ("chink_area", Range { min: -0.25, max: 0.25 }),
    ("phase_lag", Range { min: 0.0, max: 3.1415 }),
    ("relative_amplitude", Range { min: -1.0, max: 1.0 }),
    ("double_pulsing", Range { min: 0.0, max: 1.0 }),
    ("pulse_skewness", Range { min: -0.5, max: 0.5 }),
    ("flutter", Range { min: 0.0, max: 100.0 }),
    ("aspiration_db", Range { min: -40.0, max: 0.0 }),
];

impl Default for Control {
    /// VTL's neutral values: modal phonation at 120 Hz and 8000 dPa.
    fn default() -> Self {
        Control {
            f0: 120.0, pressure: 8000.0, lower_end_x: 0.01, upper_end_x: 0.02, chink_area: 0.05,
            phase_lag: 1.22, relative_amplitude: 1.0, double_pulsing: 0.05, pulse_skewness: 0.0,
            flutter: 25.0, aspiration_db: -10.0,
        }
    }
}

impl Control {
    /// Limits every parameter to its range.
    pub fn restricted(mut self) -> Self {
        let c = |v: f64, i: usize| v.clamp(CONTROL_RANGES[i].1.min, CONTROL_RANGES[i].1.max);
        self.f0 = c(self.f0, 0);
        self.pressure = c(self.pressure, 1);
        self.lower_end_x = c(self.lower_end_x, 2);
        self.upper_end_x = c(self.upper_end_x, 3);
        self.chink_area = c(self.chink_area, 4);
        self.phase_lag = c(self.phase_lag, 5);
        self.relative_amplitude = c(self.relative_amplitude, 6);
        self.double_pulsing = c(self.double_pulsing, 7);
        self.pulse_skewness = c(self.pulse_skewness, 8);
        self.flutter = c(self.flutter, 9);
        self.aspiration_db = c(self.aspiration_db, 10);
        self
    }

    /// Linear interpolation between two parameter sets (the synthesizer does this per sample).
    pub fn lerp(&self, other: &Control, t: f64) -> Control {
        let m = |a: f64, b: f64| (1.0 - t) * a + t * b;
        Control {
            f0: m(self.f0, other.f0), pressure: m(self.pressure, other.pressure),
            lower_end_x: m(self.lower_end_x, other.lower_end_x), upper_end_x: m(self.upper_end_x, other.upper_end_x),
            chink_area: m(self.chink_area, other.chink_area), phase_lag: m(self.phase_lag, other.phase_lag),
            relative_amplitude: m(self.relative_amplitude, other.relative_amplitude),
            double_pulsing: m(self.double_pulsing, other.double_pulsing),
            pulse_skewness: m(self.pulse_skewness, other.pulse_skewness), flutter: m(self.flutter, other.flutter),
            aspiration_db: m(self.aspiration_db, other.aspiration_db),
        }
    }
}

/// Speaker-specific, time-invariant parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Static {
    /// Rest thickness (cm), 0.3..1.0.
    pub rest_thickness: f64,
    /// Rest length (cm), 0.5..2.0.
    pub rest_length: f64,
    /// Rest f0 (Hz), 50..400.
    pub rest_f0: f64,
    /// Chink length (cm), 0.1..0.5.
    pub chink_length: f64,
}

impl Default for Static {
    fn default() -> Self {
        Static { rest_thickness: 0.45, rest_length: 1.6, rest_f0: 120.0, chink_length: 0.4 }
    }
}

impl Static {
    pub fn restricted(mut self) -> Self {
        self.rest_thickness = self.rest_thickness.clamp(0.3, 1.0);
        self.rest_length = self.rest_length.clamp(0.5, 2.0);
        self.rest_f0 = self.rest_f0.clamp(50.0, 400.0);
        self.chink_length = self.chink_length.clamp(0.1, 0.5);
        self
    }
}

/// Values derived from the parameters and the current phase.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Derived {
    pub length: f64,
    pub thickness: f64,
    pub amplitude: f64,
    /// Displacement of the lower / upper fold edge (cm).
    pub lower_cord_x: f64,
    pub upper_cord_x: f64,
    /// Glottal area at the lower / upper edge (cm^2).
    pub lower_area: f64,
    pub upper_area: f64,
    pub chink_width: f64,
}

pub struct GeometricGlottis {
    pub control: Control,
    pub statics: Static,
    pub derived: Derived,
    phase: f64,
    time_s: f64,
    supraglottal_pressure: f64,
    supraglottal_filter: Iir,
}

impl GeometricGlottis {
    pub fn new(sample_rate: f64) -> Self {
        let mut g = GeometricGlottis {
            control: Control::default(),
            statics: Static::default(),
            derived: Derived::default(),
            phase: 0.0,
            time_s: 0.0,
            supraglottal_pressure: 0.0,
            supraglottal_filter: Iir::chebyshev(25.0 / sample_rate, false, 4),
        };
        g.calc_geometry();
        g
    }

    pub fn reset_motion(&mut self) {
        self.supraglottal_filter.reset();
        self.time_s = 0.0;
        self.phase = 0.0;
        self.supraglottal_pressure = 0.0;
    }

    pub fn phase(&self) -> f64 { self.phase }

    /// Computes the fold geometry for the current parameters and phase.
    pub fn calc_geometry(&mut self) {
        self.control = self.control.restricted();
        self.statics = self.statics.restricted();
        let (c, s) = (self.control, self.statics);

        // Length follows sqrt(f0) (Titze 1989, four-parameter model).
        let length = s.rest_length * (c.f0 / s.rest_f0).sqrt();
        let thickness = (s.rest_thickness * s.rest_length) / length;

        // Phases of the lower and upper edges modulo 4 pi (two periods).
        let mut orig = [self.phase, self.phase + 4.0 * PI - c.phase_lag];
        for p in orig.iter_mut() {
            let n = (*p / (4.0 * PI)).floor();
            *p -= n * 4.0 * PI;
        }
        // Skew the pulses with a piecewise-linear phase warp.
        let sk = c.pulse_skewness;
        let alpha = [
            0.0,
            (1.0 + sk) * PI / 2.0,
            PI + (1.0 - sk) * PI / 2.0,
            2.0 * PI + (1.0 + sk) * PI / 2.0,
            3.0 * PI + (1.0 - sk) * PI / 2.0,
            4.0 * PI,
        ];
        let beta = [0.0, 0.5 * PI, 1.5 * PI, 2.5 * PI, 3.5 * PI, 4.0 * PI];
        let mut warped = orig;
        for k in 0..2 {
            for i in 0..5 {
                if orig[k] >= alpha[i] && orig[k] <= alpha[i + 1] {
                    let den = (alpha[i + 1] - alpha[i]).abs().max(1e-9) * (alpha[i + 1] - alpha[i]).signum();
                    warped[k] = beta[i] + (beta[i + 1] - beta[i]) * (orig[k] - alpha[i]) / den;
                    break;
                }
            }
        }

        // Amplitude: ~0.1 cm at 8000 dPa when length = rest length; scales with sqrt(pressure).
        let trans_pressure = (c.pressure - self.supraglottal_pressure).max(0.0);
        let mut amplitude = 0.0011 * trans_pressure.sqrt();
        amplitude *= s.rest_length / length;
        amplitude *= c.relative_amplitude.max(0.0);

        // Diplophonia: reduce every second pulse.
        let mut disp = [0.0; 2];
        for i in 0..2 {
            disp[i] = if warped[i] <= 1.5 * PI || warped[i] >= 3.5 * PI {
                amplitude * warped[i].sin()
            } else {
                -amplitude + amplitude * (warped[i].sin() + 1.0) * (1.0 - 0.5 * c.double_pulsing)
            };
        }

        // Areas at the upper and lower edge: chink + integral of the opening along the folds.
        let chink = c.chink_area.max(0.0);
        const SAMPLES: usize = 32;
        let delta = length / (SAMPLES - 1) as f64;
        let end_x = [c.lower_end_x, c.upper_end_x];
        let mut area = [0.0; 2];
        for i in 0..2 {
            let (mut prev_x, mut a) = (0.0, chink);
            for k in 1..=SAMPLES {
                let t = k as f64 / SAMPLES as f64;
                let x = (end_x[i] * t + disp[i] * (t * PI).sin()).max(0.0);
                a += delta * (prev_x + x);
                prev_x = x;
            }
            area[i] = a;
        }
        let chink_length = s.chink_length.max(1e-6);
        self.derived = Derived {
            length, thickness, amplitude,
            lower_cord_x: disp[0], upper_cord_x: disp[1],
            lower_area: area[0], upper_area: area[1],
            chink_width: chink / chink_length,
        };
    }

    /// Advances the model by one sample. `pressure_dpa` = subglottal, lower glottis, upper
    /// glottis, supraglottal (the last one is low-pass filtered and limits the amplitude).
    pub fn inc_time(&mut self, dt: f64, pressure_dpa: [f64; 4]) {
        self.supraglottal_pressure = self.supraglottal_filter.process(pressure_dpa[3]).clamp(-40000.0, 40000.0);
        // Flutter (Klatt & Klatt 1990): slow quasi-random f0 jitter.
        let mut f0 = self.control.f0;
        f0 += (self.control.flutter / 50.0) * (f0 / 100.0)
            * ((2.0 * PI * 12.7 * self.time_s).sin() + (2.0 * PI * 7.1 * self.time_s).sin() + (2.0 * PI * 4.7 * self.time_s).sin());
        self.phase += f0 * dt * 2.0 * PI;
        self.time_s += dt;
    }

    /// Lengths (cm) and areas (cm^2) of the two glottal tube sections.
    pub fn tube_data(&self) -> ([f64; 2], [f64; 2]) {
        let l = self.derived.thickness * 0.5;
        ([l, l], [self.derived.lower_area, self.derived.upper_area])
    }
}

/// Indices of the glottal sections in the tube (for callers that read pressures).
pub const GLOTTIS_SECTIONS: [usize; 2] = [LOWER_GLOTTIS, UPPER_GLOTTIS];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vibration_amplitude_is_0_1_cm_at_8000_dpa() {
        let mut g = GeometricGlottis::new(44100.0);
        g.control.pressure = 8000.0;
        g.control.f0 = g.statics.rest_f0;
        g.control.relative_amplitude = 1.0;
        g.calc_geometry();
        assert!((g.derived.amplitude - 0.0984).abs() < 0.002, "{}", g.derived.amplitude);
    }

    #[test]
    fn no_pressure_no_vibration() {
        let mut g = GeometricGlottis::new(44100.0);
        g.control.pressure = 0.0;
        let mut areas = vec![];
        for _ in 0..4000 {
            g.calc_geometry();
            areas.push(g.derived.upper_area);
            g.inc_time(1.0 / 44100.0, [0.0; 4]);
        }
        let (lo, hi) = areas.iter().fold((f64::MAX, f64::MIN), |(l, h), &a| (l.min(a), h.max(a)));
        assert!(hi - lo < 1e-9, "{lo} {hi}");
    }

    #[test]
    fn area_oscillates_at_f0() {
        let mut g = GeometricGlottis::new(44100.0);
        g.control.flutter = 0.0;
        g.control.double_pulsing = 0.0;
        g.control.f0 = 100.0;
        let mut areas = vec![];
        for _ in 0..44100 {
            g.calc_geometry();
            areas.push(g.derived.lower_area);
            g.inc_time(1.0 / 44100.0, [0.0; 4]);
        }
        // Count rising crossings of the mean: one per period.
        let mean = areas.iter().sum::<f64>() / areas.len() as f64;
        let n = areas.windows(2).filter(|w| w[0] < mean && w[1] >= mean).count();
        assert!((98..=102).contains(&n), "{n} periods in 1 s");
        // The glottis closes (area ~ chink only, i.e. folds touch) and opens widely.
        let (lo, hi) = areas.iter().fold((f64::MAX, f64::MIN), |(l, h), &a| (l.min(a), h.max(a)));
        assert!(hi > 3.0 * lo.max(0.01), "{lo} {hi}");
    }

    #[test]
    fn parameters_are_restricted() {
        let c = Control { f0: 5000.0, pressure: -3.0, ..Control::default() }.restricted();
        assert_eq!(c.f0, 600.0);
        assert_eq!(c.pressure, 0.0);
    }
}
