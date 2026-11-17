// SPDX-License-Identifier: GPL-3.0-or-later
//! Renders a gestural score to audio, and records the traces that make voiceless consonants
//! visible: the narrowest cross-section of the tract (closure), the glottal opening and
//! vibration, the lung pressure, and the sagittal contours with the tongue-palate contact.
//!
//! As in VocalTractLab, the vocal tract is calculated at the 400 Hz curve rate and the tube is
//! interpolated linearly for every audio sample.

use crate::consonant::ConsonantEditor;
use crate::geom::*;
use crate::gesture::{Curves, GestureScore};
use crate::glottis::Control;
use crate::speaker::Speaker;
use crate::synth::Synthesizer;
use crate::tube::*;
use crate::vocal_tract::{VocalTract, NUM_PROFILE_SAMPLES};

#[derive(Debug, Clone)]
pub struct RenderOptions {
    pub sample_rate: f64,
    pub noise_seed: u64,
    /// Interval of the trace samples in samples (44 at 44.1 kHz is about 1 ms).
    pub trace_every: usize,
    /// Record the sagittal contours (one frame per 400 Hz curve step).
    pub record_geometry: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        RenderOptions { sample_rate: 44100.0, noise_seed: 1, trace_every: 44, record_geometry: false }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TraceSample {
    pub t_s: f64,
    /// Narrowest cross-section of the supraglottal tract (cm^2) and the section it is in.
    pub min_area_cm2: f64,
    pub min_area_section: usize,
    /// Area of the upper glottal section (cm^2): small = closed or closing.
    pub glottal_area_cm2: f64,
    /// Vibration amplitude of the vocal folds (cm): 0 = no vibration.
    pub glottal_amplitude_cm: f64,
    pub lung_pressure_dpa: f64,
    /// Radiated volume velocity (cm^3/s).
    pub flow: f64,
}

/// Sagittal contours at one curve step, for the scrub preview.
#[derive(Debug, Clone, Default)]
pub struct GeometryFrame {
    pub t_s: f64,
    pub tongue: Vec<P2>,
    pub upper: Vec<P2>,
    pub lower: Vec<P2>,
    /// Indices into `tongue` (the midline contour) of the points that touch the palate / teeth.
    pub contact: Vec<usize>,
    /// Smallest distance (cm) between the tongue outline and the upper outline.
    pub tongue_palate_gap_cm: f64,
}

pub struct Rendered {
    pub audio: Vec<f64>,
    pub sample_rate: f64,
    pub traces: Vec<TraceSample>,
    pub frames: Vec<GeometryFrame>,
}

/// Distance from `p` to the segment `a`-`b`.
fn point_segment_distance(p: P2, a: P2, b: P2) -> f64 {
    let ab = b - a;
    let l2 = ab.square_magnitude();
    let t = if l2 > 0.0 { (scalar_product2(p - a, ab) / l2).clamp(0.0, 1.0) } else { 0.0 };
    (p - (a + ab * t)).magnitude()
}

/// Midline (sagittal plane) contour of the tongue surface, from the root to the tip.
pub fn tongue_midline(vt: &VocalTract) -> Vec<P2> {
    let s = &vt.surface[crate::vocal_tract::si::TONGUE];
    (0..crate::vocal_tract::NUM_DYNAMIC_TONGUE_RIBS + 1)
        .map(|i| s.get_vertex(i, s.num_rib_points / 2).to_p2())
        .collect()
}

/// Tongue-palate contact: points of the tongue midline closer than `threshold_cm` to the upper
/// outline (palate, velum, gums, teeth). The model keeps the tongue about 0.05 cm below the
/// palate, so a threshold of about 0.08 cm marks contact. Returns the indices into
/// `tongue_midline` and the smallest gap.
pub fn tongue_palate_contact(vt: &VocalTract, threshold_cm: f64) -> (Vec<usize>, f64) {
    let up: Vec<P2> = (0..vt.upper_outline.num_points()).map(|i| vt.upper_outline.get_control_point(i as i64)).collect();
    let mid = tongue_midline(vt);
    let mut contact = vec![];
    let mut min_gap = f64::MAX;
    // Only the part of the tongue under the palate counts (not the root in the pharynx).
    for i in mid.len() / 4..mid.len() {
        let d = up.windows(2).map(|w| point_segment_distance(mid[i], w[0], w[1])).fold(f64::MAX, f64::min);
        if d < min_gap { min_gap = d; }
        if d < threshold_cm { contact.push(i); }
    }
    (contact, min_gap)
}

fn outline_points(s: &crate::splines::LineStrip2D) -> Vec<P2> {
    (0..s.num_points()).map(|i| s.get_control_point(i as i64)).collect()
}

/// Renders the score. The audio is at `opts.sample_rate`, scaled by the synthesizer (about +-0.3).
pub fn render_score(sp: &Speaker, editor: &ConsonantEditor, score: &GestureScore, opts: &RenderOptions) -> Rendered {
    let fs = opts.sample_rate;
    let curves: Curves = score.calc_curves(sp, editor);
    let n_curve = curves.num_samples();
    let total = (score.duration_s() * fs) as usize;

    let mut vt = VocalTract::from_speaker(sp);
    let mut synth = Synthesizer::new(&Tube::default(), fs);
    synth.set_noise_seed(opts.noise_seed);

    let mut frames: Vec<GeometryFrame> = vec![];
    let tube_at = |vt: &mut VocalTract, frames: &mut Vec<GeometryFrame>, idx: usize, record: bool| -> Tube {
        let mut params = [0.0; crate::speaker::NUM_PARAMS];
        for p in 0..params.len() { params[p] = curves.tract[p][idx]; }
        vt.set_params(&params);
        vt.calculate_all();
        if record {
            let (contact, gap) = tongue_palate_contact(vt, 0.08);
            frames.push(GeometryFrame {
                t_s: idx as f64 / crate::gesture::CURVE_SAMPLING_RATE as f64,
                tongue: tongue_midline(vt),
                upper: outline_points(&vt.upper_outline),
                lower: outline_points(&vt.lower_outline),
                contact,
                tongue_palate_gap_cm: gap,
            });
        }
        vt.get_tube()
    };

    let mut audio = Vec::with_capacity(total);
    let mut traces = Vec::with_capacity(total / opts.trace_every.max(1) + 1);
    let (mut left_idx, mut left_tube, mut right_tube): (i64, Tube, Tube) = (-1, Tube::default(), Tube::default());
    let mut tube = Tube::default();
    for n in 0..total {
        let pos = n as f64 / fs;
        let mut needed = (pos * crate::gesture::CURVE_SAMPLING_RATE as f64) as i64;
        if needed > n_curve as i64 - 2 { needed = n_curve as i64 - 2; }
        if needed != left_idx {
            if left_idx != -1 && needed == left_idx + 1 {
                std::mem::swap(&mut left_tube, &mut right_tube);
                right_tube = tube_at(&mut vt, &mut frames, (needed + 1) as usize, opts.record_geometry);
            } else {
                left_tube = tube_at(&mut vt, &mut frames, needed as usize, opts.record_geometry);
                right_tube = tube_at(&mut vt, &mut frames, (needed + 1) as usize, opts.record_geometry);
            }
            left_idx = needed;
        }
        let ratio = (pos - needed as f64 / crate::gesture::CURVE_SAMPLING_RATE as f64) * crate::gesture::CURVE_SAMPLING_RATE as f64;
        tube.interpolate_supraglottal(&left_tube, &right_tube, ratio);

        let (_tract, gl) = curves.params_at(pos);
        synth.glottis.control = Control::from_score_values(&gl);
        synth.set_tube(&tube);
        let a = synth.step();
        audio.push(a);

        if opts.trace_every > 0 && n % opts.trace_every == 0 {
            let (mut min_area, mut min_sec) = (f64::MAX, FIRST_PHARYNX);
            for i in FIRST_PHARYNX..NUM_SECTIONS {
                if tube.sections[i].area_cm2 < min_area { min_area = tube.sections[i].area_cm2; min_sec = i; }
            }
            traces.push(TraceSample {
                t_s: pos,
                min_area_cm2: min_area,
                min_area_section: min_sec,
                glottal_area_cm2: synth.glottis.derived.upper_area,
                glottal_amplitude_cm: synth.glottis.derived.amplitude,
                lung_pressure_dpa: synth.glottis.control.pressure,
                flow: synth.last_flow(),
            });
        }
    }
    let _ = NUM_PROFILE_SAMPLES;
    Rendered { audio, sample_rate: fs, traces, frames }
}

impl Rendered {
    /// 16-bit mono PCM WAV. `peak` scales the loudest sample to that fraction of full scale.
    pub fn to_wav(&self, peak: f64) -> Vec<u8> {
        let max = self.audio.iter().fold(0.0f64, |m, v| m.max(v.abs())).max(1e-12);
        let gain = peak / max;
        wav_bytes(&self.audio.iter().map(|v| (v * gain).clamp(-1.0, 1.0)).collect::<Vec<_>>(), self.sample_rate as u32)
    }
}

/// Encodes samples in -1..1 as a 16-bit mono PCM WAV file.
pub fn wav_bytes(samples: &[f64], sample_rate: u32) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + samples.len() * 2);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&((s * 32767.0).round() as i16).to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gesture::{gt, Gesture};

    const JD3: &str = include_str!("../../../resources/JD3.speaker");

    /// A /ka/-like score: vowel /a/ for the whole time, velar closure at 0.15 s with a
    /// voiceless glottal gesture.
    fn ka_score() -> GestureScore {
        let mut s = GestureScore::default();
        s.sequences[gt::VOWEL] = vec![Gesture::shape(0.6, "a", 0.012)];
        s.sequences[gt::TONGUE_BODY] = vec![Gesture::neutral(0.12), Gesture::shape(0.10, "tb-velar-closure", 0.006), Gesture::neutral(0.38)];
        s.sequences[gt::GLOTTAL_SHAPE] = vec![Gesture::shape(0.14, "modal", 0.012), Gesture::shape(0.12, "voiceless-plosive", 0.012), Gesture::shape(0.34, "modal", 0.012)];
        s.sequences[gt::F0] = vec![Gesture::value(0.6, 82.9, 0.0, 0.03)];
        s.sequences[gt::PRESSURE] = vec![Gesture::value(0.6, 8000.0, 0.0, 0.005)];
        s
    }

    fn setup() -> (Speaker, ConsonantEditor) {
        let sp = Speaker::from_xml(JD3).unwrap();
        let ed = ConsonantEditor::from_speaker(&sp).unwrap();
        (sp, ed)
    }

    #[test]
    fn a_velar_stop_shows_closure_open_glottis_burst_and_voicing() {
        let (sp, ed) = setup();
        let opts = RenderOptions { record_geometry: true, ..RenderOptions::default() };
        let r = render_score(&sp, &ed, &ka_score(), &opts);
        assert_eq!(r.audio.len(), (0.6 * 44100.0) as usize);
        assert!(r.audio.iter().all(|v| v.is_finite() && v.abs() < 5.0));

        let at = |t: f64| r.traces.iter().min_by(|a, b| (a.t_s - t).abs().partial_cmp(&(b.t_s - t).abs()).unwrap()).unwrap();
        // The tract is open early on and closes during the gesture.
        assert!(at(0.05).min_area_cm2 > 0.3, "open at the start: {}", at(0.05).min_area_cm2);
        let closed = r.traces.iter().filter(|t| t.min_area_cm2 < 0.05).collect::<Vec<_>>();
        assert!(!closed.is_empty(), "the velar closure must appear in the trace");
        let (c0, c1) = (closed.first().unwrap().t_s, closed.last().unwrap().t_s);
        assert!(c0 > 0.12 && c1 < 0.30 && c1 - c0 > 0.03, "closure {c0:.3}..{c1:.3} s");
        // The vocal folds stop vibrating while the glottis is open and resume afterwards.
        let voiceless = r.traces.iter().filter(|t| t.glottal_amplitude_cm < 0.01).collect::<Vec<_>>();
        assert!(!voiceless.is_empty(), "devoicing must appear");
        assert!(at(0.05).glottal_amplitude_cm > 0.05 && at(0.5).glottal_amplitude_cm > 0.05);
        // Tongue-palate contact is detected during the closure and not at the start.
        let contact_frames: Vec<&GeometryFrame> = r.frames.iter().filter(|f| !f.contact.is_empty()).collect();
        assert!(!contact_frames.is_empty(), "contact during the velar closure");
        let t_contact = contact_frames[0].t_s;
        assert!(t_contact > 0.1, "no contact in the vowel before the closure: {t_contact}");
        // Sound: strong in the vowels, much weaker during the closure.
        let rms = |a: f64, b: f64| {
            let (i0, i1) = ((a * 44100.0) as usize, (b * 44100.0) as usize);
            (r.audio[i0..i1].iter().map(|v| v * v).sum::<f64>() / (i1 - i0) as f64).sqrt()
        };
        let (vowel, closure) = (rms(0.05, 0.10), rms(c0 + 0.01, c1.min(c0 + 0.05)));
        println!("closure {c0:.3}..{c1:.3} s, rms vowel {vowel:.4}, closure {closure:.5}, after {:.4}", rms(0.45, 0.55));
        assert!(closure < 0.4 * vowel, "silence during closure: {closure} vs {vowel}");
        assert!(rms(0.45, 0.55) > 0.3 * vowel, "vowel resumes");
    }

    #[test]
    fn the_wav_header_is_valid() {
        let w = wav_bytes(&[0.0, 0.5, -0.5], 44100);
        assert_eq!(&w[0..4], b"RIFF");
        assert_eq!(&w[8..16], b"WAVEfmt ");
        assert_eq!(u32::from_le_bytes([w[24], w[25], w[26], w[27]]), 44100);
        assert_eq!(w.len(), 44 + 6);
        assert_eq!(i16::from_le_bytes([w[48], w[49]]), -16384);
    }

    #[test]
    fn renders_are_reproducible() {
        let (sp, ed) = setup();
        let mut s = ka_score();
        for g in s.sequences.iter_mut() { for x in g.iter_mut() { x.duration_s *= 0.25; } }
        let opts = RenderOptions::default();
        let a = render_score(&sp, &ed, &s, &opts);
        let b = render_score(&sp, &ed, &s, &opts);
        assert_eq!(a.audio, b.audio);
    }
}
