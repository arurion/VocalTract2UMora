// SPDX-License-Identifier: GPL-3.0-or-later
//! Liljencrants-Fant (LF) glottal source.
//!
//! Time is normalised to the fundamental period (T0 = 1) and the amplitude to the
//! excitation strength (Ee = 1, i.e. the derivative flow reaches -1 at closure).
//!
//! Derivative glottal flow `E(t)`:
//!   0 <= t <= Te :  E0 * exp(alpha t) * sin(wg t),          wg = pi / Tp
//!   Te <  t <= T0: -(1 / (eps Ta)) * (exp(-eps (t-Te)) - exp(-eps (T0-Te)))
//! with `eps Ta = 1 - exp(-eps (T0-Te))` (return-phase continuity) and `alpha` chosen so the
//! flow returns to zero (`integral E dt = 0`).

use crate::fft::fft;
use std::f64::consts::PI;

/// Dimensionless LF timing parameters (all relative to T0).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LfShape {
    /// Return-phase time constant `Ta / T0` (acoustic roundness / incomplete closure).
    pub ra: f64,
    /// Open-phase asymmetry `(Te - Tp) / Tp`.
    pub rk: f64,
    /// Glottal-formant ratio `Fg / F0 = 1 / (2 Tp)`.
    pub rg: f64,
}

/// Fant (2004) regression from the voice-quality parameter `Rd` to (Ra, Rk, Rg).
/// `rd` is clamped to the regression's valid range 0.3..=2.7.
pub fn shape_from_rd(rd: f64) -> LfShape {
    let rd = rd.clamp(0.3, 2.7);
    let ra = (-1.0 + 4.8 * rd) / 100.0;
    let rk = (22.4 + 11.8 * rd) / 100.0;
    let rg = rk / (4.0 * ((0.11 * rd / (0.5 + 1.2 * rk)) - ra));
    LfShape { ra, rk, rg }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LfError {
    /// A parameter is non-finite or out of range (Ra, Rg > 0 and 0 < Rk < 1 are required).
    InvalidParams,
    /// The timing is inconsistent (needs Te < T0 and Ta < T0 - Te) or no root was found.
    NoSolution,
}

/// A solved LF pulse (T0 = 1, Ee = 1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LfPulse {
    pub te: f64,
    pub tp: f64,
    pub ta: f64,
    pub alpha: f64,
    pub epsilon: f64,
}

impl LfPulse {
    pub fn solve(shape: LfShape) -> Result<LfPulse, LfError> {
        let LfShape { ra, rk, rg } = shape;
        if !(ra.is_finite() && rk.is_finite() && rg.is_finite()) || ra <= 0.0 || rg <= 0.0 || rk <= 0.0 || rk >= 1.0 {
            return Err(LfError::InvalidParams);
        }
        let tp = 0.5 / rg;
        let te = tp * (1.0 + rk);
        let ta = ra;
        let d = 1.0 - te;
        if te >= 1.0 || ta >= d {
            return Err(LfError::NoSolution);
        }

        // epsilon: f(e) = e*Ta - 1 + exp(-e d) has f<0 just above 0 and f(1/Ta) > 0.
        let f = |e: f64| e * ta - 1.0 + (-e * d).exp();
        let (mut lo, mut hi) = (1e-9, 1.0 / ta);
        if f(lo) >= 0.0 {
            return Err(LfError::NoSolution);
        }
        for _ in 0..200 {
            let mid = 0.5 * (lo + hi);
            if f(mid) < 0.0 { lo = mid } else { hi = mid }
        }
        let epsilon = 0.5 * (lo + hi);

        // alpha: area balance of the whole derivative pulse.
        let w = PI / tp;
        let (s, c) = ((w * te).sin(), (w * te).cos());
        let ret = -(1.0 / (1.0 - (-epsilon * d).exp()))
            * ((1.0 - (-epsilon * d).exp()) / epsilon - d * (-epsilon * d).exp());
        let g = |a: f64| -((a * s - w * c) + w * (-a * te).exp()) / (s * (a * a + w * w)) + ret;

        let (mut a0, mut g0) = (-300.0, g(-300.0));
        let mut bracket = None;
        let mut a = -300.0;
        while a < 600.0 {
            let a1 = a + 1.0;
            let g1 = g(a1);
            if g0.is_finite() && g1.is_finite() && g0 * g1 <= 0.0 {
                bracket = Some((a0, a1));
                break;
            }
            a0 = a1;
            g0 = g1;
            a = a1;
        }
        let (mut lo, mut hi) = bracket.ok_or(LfError::NoSolution)?;
        let glo = g(lo);
        for _ in 0..200 {
            let mid = 0.5 * (lo + hi);
            if g(mid) * glo <= 0.0 { hi = mid } else { lo = mid }
        }
        Ok(LfPulse { te, tp, ta, alpha: 0.5 * (lo + hi), epsilon })
    }

    /// Derivative glottal flow at normalised time `t` (any real; wrapped into [0, 1)).
    pub fn dflow(&self, t: f64) -> f64 {
        let t = t - t.floor();
        if t <= self.te {
            let w = PI / self.tp;
            -((self.alpha * (t - self.te)).exp()) * (w * t).sin() / (w * self.te).sin()
        } else {
            let d = 1.0 - self.te;
            -(1.0 / (self.epsilon * self.ta))
                * ((-self.epsilon * (t - self.te)).exp() - (-self.epsilon * d).exp())
        }
    }

    /// One period of the derivative flow sampled at `n` points.
    pub fn render_period(&self, n: usize) -> Vec<f64> {
        (0..n).map(|i| self.dflow(i as f64 / n as f64)).collect()
    }
}

// ---------------------------------------------------------------------------------------
// Voice editor model + preview
// ---------------------------------------------------------------------------------------

/// Subglottal pressure (cmH2O) at which the preview treats the folds as vibrating, and the
/// reference pressure at which the source gain is 1.
pub const PHONATION_THRESHOLD_CMH2O: f64 = 2.5;
pub const REFERENCE_PRESSURE_CMH2O: f64 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoiceParams {
    pub f0: f64,
    pub shape: LfShape,
    /// Subglottal (lung) pressure, cmH2O.
    pub breath_cmh2o: f64,
    /// Aspiration noise level: noise RMS relative to the voiced source RMS at the reference pressure.
    pub aspiration: f64,
}

impl VoiceParams {
    pub fn from_rd(rd: f64, f0: f64) -> Self {
        VoiceParams { f0, shape: shape_from_rd(rd), breath_cmh2o: REFERENCE_PRESSURE_CMH2O, aspiration: 0.0 }
    }
}

/// Flow gain from lung pressure. Orifice flow scales roughly as Ps^1.5 (area grows with
/// pressure while velocity grows as sqrt(Ps)); this is a preview-level model, not a fit.
pub fn flow_gain(breath_cmh2o: f64) -> f64 {
    (breath_cmh2o.clamp(0.0, 40.0) / REFERENCE_PRESSURE_CMH2O).powf(1.5)
}

#[derive(Debug, Clone)]
pub struct Preview {
    /// One period of dUg/dt (Ee-normalised, scaled by the pressure gain).
    pub dflow: Vec<f64>,
    /// One period of Ug (integral of `dflow`, in units of Ee*T0).
    pub flow: Vec<f64>,
    /// Magnitude spectrum in dB of `dflow` + aspiration noise, bins of `bin_hz`.
    pub spectrum_db: Vec<f64>,
    pub bin_hz: f64,
    /// Regression of harmonic level (dB) against log2(frequency), harmonics up to 5 kHz.
    pub tilt_db_per_oct: f64,
    /// Fitted level at 1 kHz (dB), for drawing the tilt line.
    pub tilt_level_1k_db: f64,
    /// Level difference between the first and second harmonic (dB).
    pub h1_h2_db: f64,
    pub voiced: bool,
    pub pulse: LfPulse,
}

struct Xorshift(u64);
impl Xorshift {
    fn uniform(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
    }
}

const SPECTRUM_FFT: usize = 8192;

pub fn preview(p: &VoiceParams, fs: f64, wave_points: usize) -> Result<Preview, LfError> {
    if !(p.f0.is_finite() && p.f0 > 20.0 && p.f0 < fs / 4.0 && fs > 8000.0) {
        return Err(LfError::InvalidParams);
    }
    let pulse = LfPulse::solve(p.shape)?;
    let gain = flow_gain(p.breath_cmh2o);
    let voiced = p.breath_cmh2o > PHONATION_THRESHOLD_CMH2O;
    let vgain = if voiced { gain } else { 0.0 };

    // Waveforms (one period).
    let n = wave_points.max(16);
    let dflow: Vec<f64> = pulse.render_period(n).iter().map(|v| v * vgain).collect();
    let mut acc = 0.0;
    let flow: Vec<f64> = dflow.iter().map(|v| { acc += v / n as f64; acc }).collect();

    // Reference RMS of the (unscaled) pulse sets the noise scale.
    let rms_ref = {
        let one = pulse.render_period(1024);
        (one.iter().map(|v| v * v).sum::<f64>() / one.len() as f64).sqrt()
    };
    let ug_peak = {
        let one = pulse.render_period(1024);
        let mut a = 0.0;
        one.iter().map(|v| { a += v / 1024.0; a }).fold(f64::MIN, f64::max).max(1e-12)
    };

    // Spectrum: render with a phase accumulator, add flow-modulated aspiration noise, Hann + FFT.
    let mut re = vec![0.0; SPECTRUM_FFT];
    let mut im = vec![0.0; SPECTRUM_FFT];
    let mut rng = Xorshift(0x9E37_79B9_7F4A_7C15);
    let mut phase = 0.0;
    let mut ug = 0.0;
    let dt = p.f0 / fs;
    for i in 0..SPECTRUM_FFT {
        let d = pulse.dflow(phase);
        ug += d * dt;
        if phase + dt >= 1.0 { ug = 0.0; }
        let modulation = 0.3 + 0.7 * (ug / ug_peak).clamp(0.0, 1.0);
        let noise = p.aspiration * rms_ref * gain * modulation * rng.uniform() * 3f64.sqrt();
        let win = 0.5 - 0.5 * (2.0 * PI * i as f64 / SPECTRUM_FFT as f64).cos();
        re[i] = (d * vgain + noise) * win;
        phase += dt;
        if phase >= 1.0 { phase -= 1.0; }
    }
    fft(&mut re, &mut im);
    let win_sum = SPECTRUM_FFT as f64 * 0.5;
    let spectrum_db: Vec<f64> = (0..SPECTRUM_FFT / 2)
        .map(|k| 20.0 * ((re[k] * re[k] + im[k] * im[k]).sqrt() / (win_sum / 2.0)).max(1e-9).log10())
        .collect();

    // Harmonic levels from the exact Fourier series of one period (no noise, no leakage).
    let m = 2048;
    let one = pulse.render_period(m);
    let (mut hr, mut hi) = (vec![0.0; m], vec![0.0; m]);
    hr.copy_from_slice(&one);
    fft(&mut hr, &mut hi);
    let harmonic_db = |k: usize| 20.0 * (2.0 * (hr[k] * hr[k] + hi[k] * hi[k]).sqrt() / m as f64 * gain.max(1e-9)).max(1e-9).log10();
    let kmax = ((5000.0 / p.f0) as usize).clamp(2, m / 2 - 1);
    let (mut sx, mut sy, mut sxx, mut sxy) = (0.0, 0.0, 0.0, 0.0);
    for k in 1..=kmax {
        let x = (k as f64 * p.f0 / 1000.0).log2();
        let y = harmonic_db(k);
        sx += x; sy += y; sxx += x * x; sxy += x * y;
    }
    let cnt = kmax as f64;
    let denom = cnt * sxx - sx * sx;
    let slope = if denom.abs() > 1e-12 { (cnt * sxy - sx * sy) / denom } else { 0.0 };
    let level = (sy - slope * sx) / cnt;

    Ok(Preview {
        dflow,
        flow,
        spectrum_db,
        bin_hz: fs / SPECTRUM_FFT as f64,
        tilt_db_per_oct: if voiced { slope } else { f64::NAN },
        tilt_level_1k_db: if voiced { level } else { f64::NAN },
        h1_h2_db: if voiced { harmonic_db(1) - harmonic_db(2) } else { f64::NAN },
        voiced,
        pulse,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rd_regression_matches_published_values() {
        let s = shape_from_rd(1.0);
        assert!((s.ra - 0.038).abs() < 1e-9);
        assert!((s.rk - 0.342).abs() < 1e-9);
        // Rg for modal voice is ~1.0 (guards against the earlier rk^2 formula bug: 0.353).
        assert!((s.rg - 1.033).abs() < 0.005, "rg = {}", s.rg);
    }

    #[test]
    fn rd_is_clamped() {
        assert_eq!(shape_from_rd(-5.0), shape_from_rd(0.3));
        assert_eq!(shape_from_rd(99.0), shape_from_rd(2.7));
    }

    #[test]
    fn solves_across_rd_range_with_zero_net_flow() {
        let mut rd = 0.3;
        while rd <= 2.7 {
            let p = LfPulse::solve(shape_from_rd(rd)).unwrap_or_else(|e| panic!("rd={rd}: {e:?}"));
            let n = 200_000;
            let area: f64 = (0..n).map(|i| p.dflow((i as f64 + 0.5) / n as f64)).sum::<f64>() / n as f64;
            assert!(area.abs() < 1e-4, "rd={rd} area={area}");
            // E(Te) = -Ee on both sides of the Te junction.
            assert!((p.dflow(p.te - 1e-9) + 1.0).abs() < 1e-3, "rd={rd}");
            assert!((p.dflow(p.te + 1e-9) + 1.0).abs() < 1e-3, "rd={rd}");
            rd += 0.2;
        }
    }

    #[test]
    fn flow_is_non_negative_and_closes() {
        let p = LfPulse::solve(shape_from_rd(1.0)).unwrap();
        let n = 4096;
        let mut ug = 0.0;
        let mut min = f64::MAX;
        for i in 0..n {
            ug += p.dflow(i as f64 / n as f64) / n as f64;
            min = min.min(ug);
        }
        assert!(min > -1e-3, "flow dipped negative: {min}");
        assert!(ug.abs() < 1e-3, "flow does not return to zero: {ug}");
    }

    #[test]
    fn agrees_with_vocaltractlab_reference_values() {
        // Reference (epsilon, alpha) computed from the equations used by VocalTractLab's LfPulse
        // (OQ/SQ/TL parameterisation: Te = OQ, Tp = Te*SQ/(1+SQ), Ta = TL; T0 = 1).
        // Only the numbers are used here; no VTL code is included.
        // (OQ, SQ, TL) -> (Rg, Rk, Ra) = (1/(2Tp), (Te-Tp)/Tp, TL)
        let cases = [
            (1.333333333, 0.333333333, 0.02, 50.000000, 4.236976),
            (1.25, 0.5, 0.04, 24.998864, 1.229362),
            (1.190476190, 0.666666667, 0.08, 12.176021, 0.024308),
        ];
        for (rg, rk, ra, eps, alpha) in cases {
            let p = LfPulse::solve(LfShape { ra, rk, rg }).unwrap();
            assert!((p.epsilon - eps).abs() / eps < 1e-3, "epsilon {} vs {}", p.epsilon, eps);
            assert!((p.alpha - alpha).abs() < 2e-3, "alpha {} vs {}", p.alpha, alpha);
        }
    }

    #[test]
    fn rejects_impossible_timing() {
        // Ta larger than the remaining closing time.
        assert_eq!(LfPulse::solve(LfShape { ra: 0.6, rk: 0.3, rg: 1.0 }), Err(LfError::NoSolution));
        assert_eq!(LfPulse::solve(LfShape { ra: -1.0, rk: 0.3, rg: 1.0 }), Err(LfError::InvalidParams));
        assert_eq!(LfPulse::solve(LfShape { ra: 0.04, rk: 1.2, rg: 1.0 }), Err(LfError::InvalidParams));
    }

    #[test]
    fn pressed_voice_is_brighter_than_breathy() {
        // Lower Rd = pressed = flatter spectrum (smaller negative tilt).
        let tilt = |rd: f64| preview(&VoiceParams::from_rd(rd, 120.0), 48000.0, 256).unwrap().tilt_db_per_oct;
        assert!(tilt(0.5) > tilt(1.0) && tilt(1.0) > tilt(2.0), "{} {} {}", tilt(0.5), tilt(1.0), tilt(2.0));
    }

    #[test]
    fn below_threshold_is_unvoiced_and_noise_scales_with_aspiration() {
        let mut p = VoiceParams::from_rd(1.0, 120.0);
        p.breath_cmh2o = 1.0;
        p.aspiration = 0.5;
        let v = preview(&p, 48000.0, 128).unwrap();
        assert!(!v.voiced && v.tilt_db_per_oct.is_nan());
        assert!(v.dflow.iter().all(|x| *x == 0.0));
        let mean: f64 = v.spectrum_db.iter().sum::<f64>() / v.spectrum_db.len() as f64;
        assert!(mean > -100.0, "noise floor missing: {mean}");
    }
}
