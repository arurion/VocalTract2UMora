// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthesis loop: glottis model + tube + time-domain simulation + output filter.
//! Mirrors the per-sample order of VocalTractLab's `Synthesizer::add`:
//! glottis geometry -> tube -> pressures -> glottis time step -> acoustic step -> output.

use crate::glottis::GeometricGlottis;
use crate::iir::Iir;
use crate::tds::Tds;
use crate::tube::*;

/// Output bandwidth of the synthetic speech (Hz), as in VTL.
pub const SPEECH_BANDWIDTH_HZ: f64 = 12000.0;

pub struct Synthesizer {
    pub tube: Tube,
    pub glottis: GeometricGlottis,
    tds: Tds,
    out_filter: Iir,
    prev_flow: f64,
    dt: f64,
    last_flow: f64,
}

impl Synthesizer {
    pub fn new(tube: &Tube, sample_rate: f64) -> Self {
        Synthesizer {
            tube: tube.clone(),
            glottis: GeometricGlottis::new(sample_rate),
            tds: Tds::new(tube, sample_rate),
            out_filter: Iir::chebyshev(SPEECH_BANDWIDTH_HZ / sample_rate, false, 8),
            prev_flow: 0.0,
            dt: 1.0 / sample_rate,
            last_flow: 0.0,
        }
    }

    /// Replaces the vocal tract geometry (the glottal sections are driven by the glottis model).
    pub fn set_tube(&mut self, tube: &Tube) {
        self.tube = tube.clone();
        self.tds.set_tube(tube);
    }

    pub fn set_noise_seed(&mut self, seed: u64) { self.tds.set_noise_seed(seed); }

    /// Radiated volume velocity of the last sample (cm^3/s).
    pub fn last_flow(&self) -> f64 { self.last_flow }

    /// Minimum cross-section area (cm^2) over the supraglottal sections.
    pub fn min_supraglottal_area(&self) -> f64 {
        self.tube.sections[FIRST_PHARYNX..].iter().map(|s| s.area_cm2).fold(f64::MAX, f64::min)
    }

    /// Produces one audio sample, scaled to roughly [-1, 1].
    pub fn step(&mut self) -> f64 {
        self.glottis.calc_geometry();
        let (len, area) = self.glottis.tube_data();
        self.tube.set_glottis_geometry(len, area);
        self.tds.set_glottis(len, area);
        self.tds.set_aspiration_db(self.glottis.control.aspiration_db);
        self.tds.set_pressure_source(self.glottis.control.pressure, FIRST_TRACHEA);

        let p = [
            self.tds.section_pressure(LAST_TRACHEA),
            self.tds.section_pressure(LOWER_GLOTTIS),
            self.tds.section_pressure(UPPER_GLOTTIS),
            self.tds.section_pressure(FIRST_PHARYNX),
        ];
        self.glottis.inc_time(self.dt, p);

        let flow = self.tds.step();
        self.last_flow = flow;
        let pressure = (flow - self.prev_flow) / self.dt; // radiated pressure ~ d(flow)/dt
        self.prev_flow = flow;
        self.out_filter.process(pressure) * 1e-7
    }

    pub fn render(&mut self, samples: usize) -> Vec<f64> {
        (0..samples).map(|_| self.step()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn neutral_tube() -> Tube {
        let mut t = Tube::default();
        t.set_uniform_supraglottal(16.9, 4.0);
        t
    }

    /// Fundamental frequency estimate by autocorrelation (80..250 Hz).
    fn f0_estimate(x: &[f64], fs: f64) -> (f64, f64) {
        let (lo, hi) = ((fs / 250.0) as usize, (fs / 80.0) as usize);
        let e0: f64 = x.iter().map(|v| v * v).sum();
        let (mut best, mut best_lag) = (f64::MIN, lo);
        for lag in lo..=hi {
            let c: f64 = x.iter().zip(&x[lag..]).map(|(a, b)| a * b).sum();
            if c > best { best = c; best_lag = lag; }
        }
        (fs / best_lag as f64, best / e0)
    }

    #[test]
    fn voiced_sound_has_the_requested_f0_and_is_periodic() {
        let mut s = Synthesizer::new(&neutral_tube(), 44100.0);
        s.glottis.control.flutter = 0.0;
        s.glottis.control.f0 = 120.0;
        let audio = s.render(44100);
        assert!(audio.iter().all(|v| v.is_finite() && v.abs() < 5.0), "unstable output");
        let tail = &audio[22050..];
        let rms = (tail.iter().map(|v| v * v).sum::<f64>() / tail.len() as f64).sqrt();
        println!("rms {rms:.4}");
        assert!(rms > 1e-3, "silent: {rms}");
        let (f0, strength) = f0_estimate(&tail[..8000], 44100.0);
        println!("f0 {f0:.1} Hz, periodicity {strength:.2}");
        assert!((f0 - 120.0).abs() < 4.0, "f0 {f0}");
        assert!(strength > 0.6, "weakly periodic: {strength}");
    }

    #[test]
    fn pitch_follows_the_control() {
        let mut s = Synthesizer::new(&neutral_tube(), 44100.0);
        s.glottis.control.flutter = 0.0;
        s.glottis.control.f0 = 180.0;
        let audio = s.render(44100);
        let (f0, _) = f0_estimate(&audio[22050..30050], 44100.0);
        assert!((f0 - 180.0).abs() < 6.0, "f0 {f0}");
    }

    #[test]
    fn no_lung_pressure_is_silent() {
        let mut s = Synthesizer::new(&neutral_tube(), 44100.0);
        s.glottis.control.pressure = 0.0;
        let audio = s.render(8000);
        assert!(audio.iter().all(|v| v.abs() < 1e-9));
    }

    #[test]
    fn lower_pressure_is_quieter() {
        let rms = |pressure: f64| {
            let mut s = Synthesizer::new(&neutral_tube(), 44100.0);
            s.glottis.control.flutter = 0.0;
            s.glottis.control.pressure = pressure;
            let a = s.render(30000);
            let t = &a[15000..];
            (t.iter().map(|v| v * v).sum::<f64>() / t.len() as f64).sqrt()
        };
        let (soft, loud) = (rms(3000.0), rms(12000.0));
        println!("rms soft {soft:.4} loud {loud:.4}");
        assert!(loud > 1.5 * soft, "{soft} {loud}");
    }
}
