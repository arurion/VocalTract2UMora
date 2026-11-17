// SPDX-License-Identifier: GPL-3.0-or-later
//! Gestural score: sequences of gestures -> parameter targets -> smooth parameter curves.
//! Port of the core of VocalTractLab's `GesturalScore` (`calcTractParamTargets`,
//! `calcGlottisParamTargets`, `calcParamCurve`, `getParams`),
//! (C) Peter Birkholz, GPL-3.0-or-later.
//!
//! Every gesture sets a target for a time interval; the parameter follows the targets through a
//! fifth-order critically damped system with a time constant (tau) per target. Consonant
//! gestures own one articulator each (lips: LP, LD; tongue tip: TTX, TTY, TS3; tongue body: TCX,
//! TCY, TS2); the other parameters follow the average of the active consonant shapes, otherwise
//! the vowel. Differences from the original: curves end 10 ms after the score (the original
//! pads the rest with neutral values), and consonant shapes come from a `ConsonantEditor`,
//! which adds hand-made per-context overrides.

use crate::consonant::{ConsonantEditor, Params};
use crate::glottis::Control;
use crate::speaker::{Speaker, NUM_PARAMS};
use crate::vocal_tract::pi::*;

pub const CURVE_SAMPLING_RATE: usize = 400;
pub const MAX_CURVE_SAMPLES: usize = CURVE_SAMPLING_RATE * 60;
pub const REFERENCE_FREQUENCY: f64 = 1.0;
pub const DEFAULT_TIME_CONSTANT_S: f64 = 0.012;

/// Order of the glottis control parameters (the first two come from the F0 and pressure
/// gestures, the others from the glottal shape gestures).
pub const GLOTTIS_PARAM_NAMES: [&str; 11] = ["F0", "PR", "XB", "XT", "CA", "PL", "RA", "DP", "PS", "FL", "AS"];

#[derive(Debug, Clone, PartialEq)]
pub struct Gesture {
    pub duration_s: f64,
    /// Numeric value (velic opening, F0 in semitones, lung pressure in dPa).
    pub d_val: f64,
    pub slope: f64,
    /// Shape name (vowel, consonant or glottal shape).
    pub s_val: String,
    pub tau_s: f64,
    /// A neutral gesture does not set a target: the articulator follows the vowel.
    pub neutral: bool,
}

impl Gesture {
    pub fn shape(duration_s: f64, name: &str, tau_s: f64) -> Gesture {
        Gesture { duration_s, d_val: 0.0, slope: 0.0, s_val: name.to_string(), tau_s, neutral: false }
    }
    pub fn neutral(duration_s: f64) -> Gesture {
        Gesture { duration_s, d_val: 0.0, slope: 0.0, s_val: String::new(), tau_s: 0.005, neutral: true }
    }
    pub fn value(duration_s: f64, d_val: f64, slope: f64, tau_s: f64) -> Gesture {
        Gesture { duration_s, d_val, slope, s_val: String::new(), tau_s, neutral: false }
    }
}

/// Gesture sequence types, in the order of the original.
pub mod gt {
    pub const VOWEL: usize = 0;
    pub const LIP: usize = 1;
    pub const TONGUE_TIP: usize = 2;
    pub const TONGUE_BODY: usize = 3;
    pub const VELIC: usize = 4;
    pub const GLOTTAL_SHAPE: usize = 5;
    pub const F0: usize = 6;
    pub const PRESSURE: usize = 7;
    pub const COUNT: usize = 8;
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Target {
    pub duration: f64,
    pub value: f64,
    pub tau_s: f64,
    pub slope: f64,
}

#[derive(Debug, Clone, Default)]
pub struct GestureScore {
    pub sequences: [Vec<Gesture>; gt::COUNT],
}

/// Value and time constant of a sequence where no gesture is active.
fn neutral_gesture_of(seq_type: usize) -> (f64, f64) {
    match seq_type {
        gt::VELIC => (-0.1, 0.01),
        _ => (0.0, 0.005),
    }
}

/// Samples the response of the critically damped target-following system at 400 Hz.
pub fn calc_param_curve(targets: &[Target], final_point_index: usize) -> Vec<f64> {
    const EPS: f64 = 0.000000001;
    let mut curve = vec![0.0; final_point_index + 1];
    if targets.is_empty() { return curve; }
    let mut tg: Vec<Target> = targets.to_vec();
    let mut ti = 0usize;
    let mut target_pos = 0.0;
    if tg[0].tau_s.abs() < EPS { tg[0].tau_s = EPS; }
    let mut a = -1.0 / tg[0].tau_s;
    let (mut a2, mut a3, mut a4) = (a * a, a * a * a, a * a * a * a);
    let (mut c0, mut c1, mut c2, mut c3, mut c4) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for i in 0..=final_point_index {
        let t_s = i as f64 / CURVE_SAMPLING_RATE as f64;
        while t_s > target_pos + tg[ti].duration && ti < tg.len() - 1 {
            let target = tg[ti];
            let t = target.duration;
            let (t2, t3, t4) = (t * t, t * t * t, t * t * t * t);
            let e = (a * t).exp();
            let f0 = e * (c0 + c1 * t + c2 * t2 + c3 * t3 + c4 * t4) + (target.value + target.duration * target.slope);
            let f1 = e * ((c0 * a + c1) + (c1 * a + 2.0 * c2) * t + (c2 * a + 3.0 * c3) * t2 + (c3 * a + 4.0 * c4) * t3 + (c4 * a) * t4) + target.slope;
            let f2 = e * ((c0 * a2 + 2.0 * c1 * a + 2.0 * c2) + (c1 * a2 + 4.0 * c2 * a + 6.0 * c3) * t + (c2 * a2 + 6.0 * c3 * a + 12.0 * c4) * t2 + (c3 * a2 + 8.0 * c4 * a) * t3 + (c4 * a2) * t4);
            let f3 = e * ((c0 * a3 + 3.0 * c1 * a2 + 6.0 * c2 * a + 6.0 * c3) + (c1 * a3 + 6.0 * c2 * a2 + 18.0 * c3 * a + 24.0 * c4) * t + (c2 * a3 + 9.0 * c3 * a2 + 36.0 * c4 * a) * t2 + (c3 * a3 + 12.0 * c4 * a2) * t3 + (c4 * a3) * t4);
            let f4 = e * ((c0 * a4 + 4.0 * c1 * a3 + 12.0 * c2 * a2 + 24.0 * c3 * a + 24.0 * c4) + (c1 * a4 + 8.0 * c2 * a3 + 36.0 * c3 * a2 + 96.0 * c4 * a) * t + (c2 * a4 + 12.0 * c3 * a3 + 72.0 * c4 * a2) * t2 + (c3 * a4 + 16.0 * c4 * a3) * t3 + (c4 * a4) * t4);
            target_pos += target.duration;
            ti += 1;
            if tg[ti].tau_s.abs() < EPS { tg[ti].tau_s = EPS; }
            a = -1.0 / tg[ti].tau_s;
            a2 = a * a;
            a3 = a2 * a;
            a4 = a3 * a;
            let v = tg[ti].value;
            let sl = tg[ti].slope;
            c0 = f0 - v;
            c1 = f1 - c0 * a - sl;
            c2 = (f2 - c0 * a2 - c1 * a * 2.0) / 2.0;
            c3 = (f3 - c0 * a3 - c1 * a2 * 3.0 - c2 * a * 6.0) / 6.0;
            c4 = (f4 - c0 * a4 - c1 * a3 * 4.0 - c2 * a2 * 12.0 - c3 * a * 24.0) / 24.0;
        }
        let t = t_s - target_pos;
        let (t2, t3, t4) = (t * t, t * t * t, t * t * t * t);
        let target = tg[ti];
        curve[i] = (a * t).exp() * (c0 + c1 * t + c2 * t2 + c3 * t3 + c4 * t4) + (target.value + t * target.slope);
    }
    curve
}

pub fn f0_st(freq_hz: f64) -> f64 {
    12.0 * (freq_hz.max(1.0) / REFERENCE_FREQUENCY).ln() / 2f64.ln()
}

pub fn f0_hz(freq_st: f64) -> f64 {
    REFERENCE_FREQUENCY * 2f64.powf(freq_st / 12.0)
}

/// Parameter curves of a score, sampled at 400 Hz.
pub struct Curves {
    pub tract: Vec<Vec<f64>>,
    pub glottis: Vec<Vec<f64>>,
}

impl GestureScore {
    pub fn duration_s(&self) -> f64 {
        self.sequences.iter().map(|s| s.iter().map(|g| g.duration_s).sum::<f64>()).fold(0.0, f64::max)
    }

    /// Targets of the 19 vocal tract parameters.
    pub fn calc_tract_targets(&self, sp: &Speaker, editor: &ConsonantEditor) -> Vec<Vec<Target>> {
        const NT: usize = 5;
        let seq: Vec<&Vec<Gesture>> = (0..NT).map(|i| &self.sequences[i]).collect();
        let mut index = [0usize; NT];
        let mut gesture_pos = [0.0f64; NT];
        let mut pos = 0.0;
        let mut slices: Vec<[Gesture; NT]> = vec![];
        while (0..NT).any(|i| index[i] < seq[i].len()) {
            let mut k: i64 = -1;
            let mut nearest = 1_000_000.0;
            for i in 0..NT {
                if index[i] < seq[i].len() {
                    let end = gesture_pos[i] + seq[i][index[i]].duration_s;
                    if end < nearest { nearest = end; k = i as i64; }
                }
            }
            if k != -1 {
                if nearest > pos {
                    let slice_duration = nearest - pos;
                    let mut slice: [Gesture; NT] = std::array::from_fn(|i| {
                        let mut g = if index[i] < seq[i].len() {
                            seq[i][index[i]].clone()
                        } else {
                            let (v, tau) = neutral_gesture_of(i);
                            Gesture { duration_s: 0.0, d_val: v, slope: 0.0, s_val: String::new(), tau_s: tau, neutral: true }
                        };
                        g.duration_s = slice_duration;
                        g
                    });
                    if slices.len() < 1023 { slices.push(std::mem::replace(&mut slice, std::array::from_fn(|_| Gesture::neutral(0.0)))); }
                }
                pos = nearest;
                gesture_pos[k as usize] = nearest;
                index[k as usize] += 1;
            }
        }
        // Neutral consonant gestures keep the name of the previous slice.
        for i in 1..=3 {
            for k in 1..slices.len() {
                if slices[k][i].s_val.is_empty() || slices[k][i].neutral {
                    let prev = slices[k - 1][i].s_val.clone();
                    slices[k][i].s_val = prev;
                    slices[k][i].neutral = true;
                }
            }
        }

        let neutral = sp.neutral_params();
        let mut targets: Vec<Vec<Target>> = vec![vec![]; NUM_PARAMS];
        for slice in &slices {
            let vowel_name = slice[gt::VOWEL].s_val.clone();
            let vowel_tau = slice[gt::VOWEL].tau_s;
            let mut shape = sp.shape(&vowel_name);
            if shape.is_none() { shape = sp.shape("@"); }
            let mut target_value: Params = shape.map(|s| s.params).unwrap_or(neutral);
            let mut target_tau = [vowel_tau; NUM_PARAMS];
            let coords = editor.vowel_coords(&target_value);
            let _ = coords;

            let mut cons_params: [Option<Params>; 3] = [None, None, None];
            let (mut n_active, mut tau_sum) = (0usize, 0.0);
            let mut param_sum = [0.0; NUM_PARAMS];
            for k in 0..3 {
                let g = &slice[1 + k];
                if !g.neutral {
                    if let Some(t) = editor.target(&g.s_val, &vowel_name, &target_value) {
                        n_active += 1;
                        tau_sum += g.tau_s;
                        for m in 0..NUM_PARAMS { param_sum[m] += t.params[m]; }
                        cons_params[k] = Some(t.params);
                    }
                }
            }
            if n_active > 0 {
                for m in 0..NUM_PARAMS {
                    target_value[m] = param_sum[m] / n_active as f64;
                    target_tau[m] = tau_sum / n_active as f64;
                }
                if let Some(c) = &cons_params[0] { target_value[LP] = c[LP]; target_value[LD] = c[LD]; }
                if let Some(c) = &cons_params[1] { target_value[TTX] = c[TTX]; target_value[TTY] = c[TTY]; target_value[TS3] = c[TS3]; }
                if let Some(c) = &cons_params[2] { target_value[TCX] = c[TCX]; target_value[TCY] = c[TCY]; target_value[TS2] = c[TS2]; }
            }
            if !slice[gt::VELIC].neutral {
                target_value[VO] = slice[gt::VELIC].d_val;
                target_tau[VO] = slice[gt::VELIC].tau_s;
            }
            for k in 0..NUM_PARAMS {
                targets[k].push(Target { duration: slice[gt::VOWEL].duration_s, value: target_value[k], tau_s: target_tau[k], slope: 0.0 });
            }
        }
        // Articulators move at speeds that depend on the direction of the change.
        for k in 0..NUM_PARAMS {
            for i in 1..targets[k].len() {
                let prev = targets[k][i - 1].value;
                let this = &mut targets[k][i];
                let mut factor = if this.value > prev {
                    sp.anatomy.positive_velocity_factor[k]
                } else {
                    sp.anatomy.negative_velocity_factor[k]
                };
                if factor.abs() < 0.001 { factor = 0.001; }
                this.tau_s /= factor;
            }
        }
        targets
    }

    /// Targets of the glottis control parameters (order: `GLOTTIS_PARAM_NAMES`).
    pub fn calc_glottis_targets(&self, sp: &Speaker) -> Vec<Vec<Target>> {
        let neutral = Control::default();
        let neutral_values = [neutral.f0, neutral.pressure, neutral.lower_end_x, neutral.upper_end_x, neutral.chink_area,
            neutral.phase_lag, neutral.relative_amplitude, neutral.double_pulsing, neutral.pulse_skewness, neutral.flutter, neutral.aspiration_db];
        let mut targets: Vec<Vec<Target>> = vec![vec![]; GLOTTIS_PARAM_NAMES.len()];
        for g in &self.sequences[gt::GLOTTAL_SHAPE] {
            let shape = sp.glottis_shape(&g.s_val);
            for k in 2..GLOTTIS_PARAM_NAMES.len() {
                let value = shape.and_then(|s| s.get(GLOTTIS_PARAM_NAMES[k])).unwrap_or(neutral_values[k]);
                targets[k].push(Target { duration: g.duration_s, value, tau_s: g.tau_s, slope: 0.0 });
            }
        }
        for g in &self.sequences[gt::F0] {
            targets[0].push(Target { duration: g.duration_s, value: g.d_val, tau_s: g.tau_s, slope: g.slope });
        }
        for g in &self.sequences[gt::PRESSURE] {
            targets[1].push(Target { duration: g.duration_s, value: g.d_val, tau_s: g.tau_s, slope: g.slope });
        }
        targets
    }

    /// Sampled parameter curves of the whole score.
    pub fn calc_curves(&self, sp: &Speaker, editor: &ConsonantEditor) -> Curves {
        let final_index = (((self.duration_s() + 0.010) * CURVE_SAMPLING_RATE as f64) as usize).min(MAX_CURVE_SAMPLES - 1);
        let neutral = Control::default();
        let neutral_glottis = [f0_st(neutral.f0), neutral.pressure, neutral.lower_end_x, neutral.upper_end_x, neutral.chink_area,
            neutral.phase_lag, neutral.relative_amplitude, neutral.double_pulsing, neutral.pulse_skewness, neutral.flutter, neutral.aspiration_db];
        let tract_neutral = sp.neutral_params();
        let tt = self.calc_tract_targets(sp, editor);
        let tract = (0..NUM_PARAMS).map(|i| {
            if tt[i].is_empty() { vec![tract_neutral[i]; final_index + 1] } else { calc_param_curve(&tt[i], final_index) }
        }).collect();
        let gt_ = self.calc_glottis_targets(sp);
        let glottis = (0..GLOTTIS_PARAM_NAMES.len()).map(|i| {
            let mut c = if gt_[i].is_empty() { vec![neutral_glottis[i]; final_index + 1] } else { calc_param_curve(&gt_[i], final_index) };
            if i == 0 { for v in c.iter_mut() { *v = f0_hz(*v); } }
            c
        }).collect();
        Curves { tract, glottis }
    }
}

impl Curves {
    pub fn num_samples(&self) -> usize { self.tract[0].len() }

    /// Linear interpolation of the curves at `pos_s`.
    pub fn params_at(&self, pos_s: f64) -> (Params, [f64; 11]) {
        let n = self.num_samples();
        let sp_period = 1.0 / CURVE_SAMPLING_RATE as f64;
        let mut index = (pos_s * CURVE_SAMPLING_RATE as f64) as i64;
        let mut s = (pos_s - index as f64 * sp_period) / sp_period;
        let mut s1 = 1.0 - s;
        if index < 0 { index = 0; }
        if index as usize >= n - 1 {
            index = n as i64 - 2;
            s = 1.0;
            s1 = 0.0;
        }
        let i = index as usize;
        let mut tract = [0.0; NUM_PARAMS];
        for p in 0..NUM_PARAMS { tract[p] = s1 * self.tract[p][i] + s * self.tract[p][i + 1]; }
        let mut gl = [0.0; 11];
        for p in 0..11 { gl[p] = s1 * self.glottis[p][i] + s * self.glottis[p][i + 1]; }
        (tract, gl)
    }
}

impl Control {
    /// Builds the glottis control from the 11 values of a score (order: `GLOTTIS_PARAM_NAMES`).
    pub fn from_score_values(v: &[f64; 11]) -> Control {
        Control {
            f0: v[0], pressure: v[1], lower_end_x: v[2], upper_end_x: v[3], chink_area: v[4], phase_lag: v[5],
            relative_amplitude: v[6], double_pulsing: v[7], pulse_skewness: v[8], flutter: v[9], aspiration_db: v[10],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const JD3: &str = include_str!("../../../resources/JD3.speaker");
    const SCORE: &str = include_str!("../tests/data/vtl_score.txt");

    /// The same score as `tools/vtl_reference/dump_score.cpp`.
    fn test_score() -> GestureScore {
        let mut s = GestureScore::default();
        s.sequences[gt::VOWEL] = vec![Gesture::shape(0.20, "a", 0.012), Gesture::shape(0.40, "i", 0.016)];
        s.sequences[gt::TONGUE_BODY] = vec![Gesture::neutral(0.16), Gesture::shape(0.08, "tb-velar-closure", 0.012), Gesture::neutral(0.36)];
        s.sequences[gt::LIP] = vec![Gesture::neutral(0.30), Gesture::shape(0.06, "ll-labial-closure", 0.010), Gesture::neutral(0.24)];
        s.sequences[gt::TONGUE_TIP] = vec![Gesture::neutral(0.40), Gesture::shape(0.06, "tt-alveolar-fricative", 0.014), Gesture::neutral(0.14)];
        let velic = |d: f64, v: f64, neutral: bool| Gesture { duration_s: d, d_val: v, slope: 0.0, s_val: String::new(), tau_s: 0.01, neutral };
        s.sequences[gt::VELIC] = vec![velic(0.30, -0.1, true), velic(0.10, 0.5, false), velic(0.20, -0.1, true)];
        s.sequences[gt::GLOTTAL_SHAPE] = vec![Gesture::shape(0.15, "modal", 0.012), Gesture::shape(0.10, "voiceless-plosive", 0.012), Gesture::shape(0.35, "modal", 0.012)];
        s.sequences[gt::F0] = vec![Gesture::value(0.01, 80.0, 0.0, 0.03), Gesture::value(0.30, 83.0, 1.0, 0.03), Gesture::value(0.29, 78.0, 0.0, 0.03)];
        s.sequences[gt::PRESSURE] = vec![Gesture::value(0.01, 0.0, 0.0, 0.005), Gesture::value(0.50, 8000.0, 0.0, 0.005), Gesture::value(0.09, 0.0, 0.0, 0.005)];
        s
    }

    #[test]
    fn parameter_curves_match_the_original() {
        let sp = Speaker::from_xml(JD3).unwrap();
        let editor = ConsonantEditor::from_speaker(&sp).unwrap();
        let curves = test_score().calc_curves(&sp, &editor);
        let mut worst: f64 = 0.0;
        let mut rows = 0;
        for line in SCORE.lines() {
            let Some(rest) = line.strip_prefix("T ") else { continue };
            let (time, vals) = rest.split_once(' ').unwrap();
            let t: f64 = time.parse().unwrap();
            let (tract_s, glottis_s) = vals.split_once('|').unwrap();
            let want_tract: Vec<f64> = tract_s.split_whitespace().map(|x| x.parse().unwrap()).collect();
            let want_glottis: Vec<f64> = glottis_s.split_whitespace().map(|x| x.parse().unwrap()).collect();
            let (tract, glottis) = curves.params_at(t);
            for i in 0..NUM_PARAMS {
                let e = (tract[i] - want_tract[i]).abs();
                if e > worst { worst = e; }
                assert!(e < 2e-6, "t={t} tract param {i}: {} vs {}", tract[i], want_tract[i]);
            }
            for i in 0..11 {
                let e = (glottis[i] - want_glottis[i]).abs() / want_glottis[i].abs().max(1.0);
                assert!(e < 2e-6, "t={t} glottis param {i}: {} vs {}", glottis[i], want_glottis[i]);
            }
            rows += 1;
        }
        println!("{rows} time points, worst tract error {worst:.2e}");
        assert!(rows >= 48);
    }

    #[test]
    fn param_curve_follows_a_single_target_exactly() {
        let t = [Target { duration: 1.0, value: 3.0, tau_s: 0.01, slope: 0.0 }];
        let c = calc_param_curve(&t, 100);
        assert!(c.iter().all(|v| (v - 3.0).abs() < 1e-12));
    }

    #[test]
    fn param_curve_is_smooth_and_converges_to_the_next_target() {
        let t = [
            Target { duration: 0.1, value: 0.0, tau_s: 0.01, slope: 0.0 },
            Target { duration: 0.5, value: 1.0, tau_s: 0.02, slope: 0.0 },
        ];
        let c = calc_param_curve(&t, 200);
        assert!(c[39].abs() < 1e-9, "no movement before the second target");
        assert!((c[200] - 1.0).abs() < 1e-3, "converged {}", c[200]);
        // No overshoot and monotonic rise (critically damped).
        assert!(c.windows(2).all(|w| w[1] >= w[0] - 1e-9));
        assert!(c.iter().all(|v| *v <= 1.0 + 1e-6));
        // Continuous slope at the junction (first derivative matches).
        let d = |i: usize| (c[i + 1] - c[i]) * CURVE_SAMPLING_RATE as f64;
        assert!(d(40).abs() < 2.0, "no jump of the velocity at the switch: {}", d(40));
    }

    #[test]
    fn f0_conversion_round_trips() {
        assert!((f0_hz(f0_st(120.0)) - 120.0).abs() < 1e-9);
        assert!((f0_st(2.0) - 12.0).abs() < 1e-12);
    }

    #[test]
    fn consonant_gesture_owns_only_its_articulator() {
        let sp = Speaker::from_xml(JD3).unwrap();
        let editor = ConsonantEditor::from_speaker(&sp).unwrap();
        let mut s = GestureScore::default();
        s.sequences[gt::VOWEL] = vec![Gesture::shape(0.3, "a", 0.012)];
        s.sequences[gt::LIP] = vec![Gesture::shape(0.3, "ll-labial-closure", 0.012)];
        let t = s.calc_tract_targets(&sp, &editor);
        let lip = editor.target("ll-labial-closure", "a", &sp.shape("a").unwrap().params).unwrap().params;
        // Lips take the consonant's lip parameters; with a single active consonant all
        // other parameters equal that consonant's shape as well.
        assert!((t[LP][0].value - lip[LP]).abs() < 1e-12);
        assert!((t[LD][0].value - lip[LD]).abs() < 1e-12);
        assert!((t[TCX][0].value - lip[TCX]).abs() < 1e-12);
    }
}
