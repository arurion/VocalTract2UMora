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

    /// Resonance peaks (Hz) of a tube: white-noise flow into the first pharynx section, Welch
    /// estimate of the transfer to the radiated flow.
    fn resonance_peaks(tube: &Tube) -> Vec<(f64, f64)> {
        use crate::fft::fft;
        let fs = 44100.0;
        let mut tds = crate::tds::Tds::new(tube, fs);
        tds.generate_noise = false;
        let mut state = 0x1234_5678_9ABC_DEF1u64;
        let mut rnd = move || { state ^= state << 13; state ^= state >> 7; state ^= state << 17; (state >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0 };
        let total = 1 << 16;
        let (mut x, mut y) = (Vec::with_capacity(total), Vec::with_capacity(total));
        for _ in 0..total {
            let v = 100.0 * rnd();
            tds.set_flow_source(v, FIRST_PHARYNX);
            y.push(tds.step());
            x.push(v);
        }
        let nfft = 4096;
        let (mut pxx, mut pxy) = (vec![0.0; nfft / 2], vec![0.0; nfft / 2]);
        let mut start = 4096;
        while start + nfft <= total {
            let win = |i: usize| 0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / nfft as f64).cos();
            let (mut xr, mut xi) = ((0..nfft).map(|i| x[start + i] * win(i)).collect::<Vec<_>>(), vec![0.0; nfft]);
            let (mut yr, mut yi) = ((0..nfft).map(|i| y[start + i] * win(i)).collect::<Vec<_>>(), vec![0.0; nfft]);
            fft(&mut xr, &mut xi);
            fft(&mut yr, &mut yi);
            for k in 0..nfft / 2 {
                let (ax, ay) = ((xr[k] * xr[k] + xi[k] * xi[k]).sqrt(), (yr[k] * yr[k] + yi[k] * yi[k]).sqrt());
                pxx[k] += ax * ax;
                pxy[k] += ay * ax;
            }
            start += nfft / 2;
        }
        let h: Vec<f64> = (0..nfft / 2).map(|k| pxy[k] / pxx[k].max(1e-30)).collect();
        let bin = fs / nfft as f64;
        (3..nfft / 2 - 1).filter(|&k| h[k] > h[k - 1] && h[k] >= h[k + 1] && k as f64 * bin > 150.0 && k as f64 * bin < 4500.0).map(|k| (k as f64 * bin, h[k])).collect()
    }

    /// Height of the highest resonance peak within a frequency band, relative to the highest
    /// peak overall (0 if there is none).
    fn peak_in(peaks: &[(f64, f64)], lo: f64, hi: f64) -> f64 {
        let max = peaks.iter().map(|x| x.1).fold(0.0, f64::max);
        peaks.iter().filter(|x| x.0 >= lo && x.0 <= hi).map(|x| x.1 / max).fold(0.0, f64::max)
    }

    #[test]
    fn vowel_shapes_from_the_articulatory_model_have_their_typical_formants() {
        let sp = crate::speaker::Speaker::from_xml(include_str!("../../../resources/JD3.speaker")).unwrap();
        let peaks_of = |name: &str| {
            let mut vt = crate::vocal_tract::VocalTract::from_speaker(&sp);
            vt.set_params(&sp.shape(name).unwrap().params);
            vt.calculate_all();
            let mut t = vt.get_tube();
            t.set_glottis_area(0.05);
            resonance_peaks(&t)
        };
        let (a, i, u) = (peaks_of("a"), peaks_of("i"), peaks_of("u"));
        let show = |p: &[(f64, f64)]| {
            let max = p.iter().map(|x| x.1).fold(0.0, f64::max);
            p.iter().filter(|x| x.1 >= 0.03 * max).take(6).map(|x| format!("{:.0} Hz ({:.0}%)", x.0, 100.0 * x.1 / max)).collect::<Vec<_>>().join(", ")
        };
        println!("/a/: {}\n/i/: {}\n/u/: {}", show(&a), show(&i), show(&u));
        // Typical vowel formants: /a/ high F1 and mid F2, /i/ low F1 and high F2, /u/ low F1 and F2.
        assert!(peak_in(&a, 600.0, 950.0) > 0.1, "/a/ F1");
        assert!(peak_in(&a, 1000.0, 1700.0) > 0.1, "/a/ F2");
        assert!(peak_in(&i, 200.0, 450.0) > 0.03, "/i/ F1");
        assert!(peak_in(&i, 1900.0, 2900.0) > 0.1, "/i/ F2");
        assert!(peak_in(&u, 200.0, 450.0) > 0.1, "/u/ F1");
        assert!(peak_in(&u, 600.0, 1100.0) > 0.1, "/u/ F2");
        // The classic ordering of F2: /i/ > /a/ > /u/, and of F1: /a/ > /u/.
        let strongest = |p: &[(f64, f64)], lo: f64, hi: f64| p.iter().filter(|x| x.0 >= lo && x.0 <= hi).fold((0.0, 0.0), |m, x| if x.1 > m.1 { *x } else { m }).0;
        let (f2_i, f2_a, f2_u) = (strongest(&i, 1900.0, 2900.0), strongest(&a, 1000.0, 1700.0), strongest(&u, 600.0, 1100.0));
        assert!(f2_i > f2_a && f2_a > f2_u, "F2: i {f2_i}, a {f2_a}, u {f2_u}");
        assert!(strongest(&a, 600.0, 950.0) > strongest(&u, 200.0, 450.0));
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
