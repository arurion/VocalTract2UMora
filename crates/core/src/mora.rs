// SPDX-License-Identifier: GPL-3.0-or-later
//! Japanese morae as gestural scores.
//!
//! A mora (consonant + vowel, e.g. か = k + a) becomes a score with: a short silent lead where
//! the tract already sits in the consonant position, the consonant gestures (closure, release,
//! frication) on the articulators that form it, a glottal gesture (open for voiceless
//! consonants), and a long steady vowel. The timing was tuned with the renderer so that a
//! voiceless stop has a closure of about 60 ms and a VOT of about 45 ms.
//!
//! Palatalised morae (きゃ, しゃ, ちゃ ...) use the context of /i/ for the consonant target and
//! a short /i/ glide before the vowel.

use crate::gesture::{gt, Gesture, GestureScore};

/// How a consonant is produced; decides the timing and how the oto parameters are derived.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Manner {
    /// Closure and release (k t p g d b).
    Plosive,
    /// Constriction with frication (s sh h-palatal z j f).
    Fricative,
    /// Closure followed by frication (ch ts).
    Affricate,
    /// A very short closure (r).
    Tap,
    /// Vowel-like glide (y w).
    Glide,
    /// Glottal fricative (h).
    Glottal,
    /// Closure with a lowered velum (m n ng). Needs the nasal cavity.
    Nasal,
    /// No consonant (a vowel alone).
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Articulator {
    Lip,
    TongueTip,
    TongueBody,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConsonantSpec {
    /// Romaji symbol (k, sh, ch ...).
    pub symbol: &'static str,
    pub manner: Manner,
    pub voiced: bool,
    /// Gestures on the articulators, in time order; the closure comes first for affricates.
    pub gestures: Vec<(Articulator, &'static str)>,
    /// For glides: the vowel the glide starts from.
    pub glide_from: Option<&'static str>,
}

fn c(symbol: &'static str, manner: Manner, voiced: bool, gestures: &[(Articulator, &'static str)]) -> ConsonantSpec {
    ConsonantSpec { symbol, manner, voiced, gestures: gestures.to_vec(), glide_from: None }
}

/// The consonants of Japanese (without the palatalisation, which is a separate flag).
pub fn consonant_table() -> Vec<ConsonantSpec> {
    use Articulator::*;
    use Manner::*;
    let mut v = vec![
        c("k", Plosive, false, &[(TongueBody, "tb-velar-closure")]),
        c("g", Plosive, true, &[(TongueBody, "tb-velar-closure")]),
        c("t", Plosive, false, &[(TongueTip, "tt-alveolar-closure")]),
        c("d", Plosive, true, &[(TongueTip, "tt-alveolar-closure")]),
        c("p", Plosive, false, &[(Lip, "ll-labial-closure")]),
        c("b", Plosive, true, &[(Lip, "ll-labial-closure")]),
        c("s", Fricative, false, &[(TongueTip, "tt-alveolar-fricative")]),
        c("z", Fricative, true, &[(TongueTip, "tt-alveolar-fricative")]),
        c("sh", Fricative, false, &[(TongueTip, "tt-postalveolar-fricative")]),
        c("j", Fricative, true, &[(TongueTip, "tt-postalveolar-fricative")]),
        c("ch", Affricate, false, &[(TongueTip, "tt-postalveolar-closure"), (TongueTip, "tt-postalveolar-fricative")]),
        c("ts", Affricate, false, &[(TongueTip, "tt-alveolar-closure"), (TongueTip, "tt-alveolar-fricative")]),
        c("hy", Fricative, false, &[(TongueBody, "tb-palatal-fricative")]),
        c("f", Fricative, false, &[(Lip, "ll-labial-fricative")]),
        c("r", Tap, true, &[(TongueTip, "tt-alveolar-closure")]),
        c("h", Glottal, false, &[]),
        c("n", Nasal, true, &[(TongueTip, "tt-alveolar-closure")]),
        c("m", Nasal, true, &[(Lip, "ll-labial-closure")]),
        c("ng", Nasal, true, &[(TongueBody, "tb-velar-closure")]),
    ];
    let mut y = c("y", Glide, true, &[]);
    y.glide_from = Some("i");
    let mut w = c("w", Glide, true, &[]);
    w.glide_from = Some("u");
    v.push(y);
    v.push(w);
    v.push(c("", None, true, &[]));
    // Moraic nasal realised as a nasalised vowel (no closure; the velum is open). The closing
    // variants (n m ng) are the consonants above, used without a vowel.
    v.push(c("N", Nasal, true, &[]));
    v
}

/// Which nasal a moraic ん is, from the consonant symbol of the next mora (`None`: end of the
/// line). It assimilates in place: m before bilabials, ng before velars, n before alveolars,
/// and a nasalised vowel `N` before vowels, fricatives (s sh h f), glides and at the end.
pub fn moraic_nasal_symbol(next: Option<&str>) -> &'static str {
    match next {
        Some("m" | "p" | "b") => "m",
        Some("k" | "g" | "ng") => "ng",
        Some("n" | "t" | "d" | "ch" | "ts" | "j" | "z" | "r") => "n",
        _ => "N",
    }
}

/// Key moments of a mora score (seconds), known from the construction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MoraTiming {
    /// The lung pressure is switched on (or, in a sequence, the start of this mora's window).
    pub pressure_on: f64,
    /// End of the consonant gestures (the nominal release).
    pub consonant_end: f64,
    pub total: f64,
}

#[derive(Debug, Clone)]
pub struct MoraOptions {
    /// Fundamental frequency (Hz).
    pub f0_hz: f64,
    /// Lung pressure (dPa).
    pub pressure_dpa: f64,
    /// Duration of the steady vowel after the consonant (s).
    pub vowel_hold_s: f64,
    /// Silent lead before the lung pressure starts (s).
    pub lead_s: f64,
    /// Palatalised mora (きゃ ...): the consonant is formed in the context of /i/.
    pub palatalised: bool,
    /// Duration of a moraic nasal ん (s): the nasal is held for the whole mora, without a vowel.
    pub moraic_nasal_s: f64,
}

impl Default for MoraOptions {
    fn default() -> Self {
        MoraOptions { f0_hz: 120.0, pressure_dpa: 8000.0, vowel_hold_s: 0.40, lead_s: 0.05, palatalised: false, moraic_nasal_s: 0.20 }
    }
}

const TAU_CLOSURE: f64 = 0.006;
const TAU_FRICATIVE: f64 = 0.008;
const TAU_VOWEL: f64 = 0.012;
const TAU_GLOTTIS: f64 = 0.012;
pub const TAIL_S: f64 = 0.05;

/// One mora of a sequence.
#[derive(Debug, Clone)]
pub struct MoraItem {
    pub consonant: ConsonantSpec,
    /// "a", "i", "u", "e" or "o".
    pub vowel: String,
    pub palatalised: bool,
    /// Moraic nasal ん: `consonant` is held for the whole mora (`MoraOptions::moraic_nasal_s`)
    /// and `vowel` is only the context (the previous vowel), no vowel is spoken after it.
    pub moraic: bool,
}

/// Where a mora lies in a sequence (seconds from the start of the file).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Slot {
    pub start: f64,
    pub consonant_end: f64,
    pub end: f64,
}

/// (duration of the consonant gestures, time up to which the glottis stays open) by manner.
fn consonant_timing(m: Manner) -> (f64, f64) {
    match m {
        Manner::Plosive => (0.10, 0.10),
        Manner::Fricative => (0.15, 0.14),
        Manner::Affricate => (0.15, 0.14),
        Manner::Tap => (0.03, 0.0),
        Manner::Glide => (0.06, 0.0),
        Manner::Glottal => (0.12, 0.11),
        Manner::Nasal => (0.10, 0.0),
        Manner::None => (0.02, 0.0),
    }
}

/// Appends `g` to `seq` at time `start`, filling the gap since `cursor` with `filler`. The very
/// first gesture of a sequence that starts exactly where the first mora starts (`lead`) is
/// stretched back to time 0, so the articulators wait in position during the silent lead.
fn put(seq: &mut Vec<Gesture>, cursor: &mut f64, start: f64, mut g: Gesture, lead: f64, filler: impl Fn(f64) -> Gesture) {
    let end = start + g.duration_s;
    if seq.is_empty() && (start - lead).abs() < 1e-9 {
        g.duration_s += start;
    } else if start > *cursor + 1e-12 {
        seq.push(filler(start - *cursor));
    }
    *cursor = end;
    seq.push(g);
}

/// Builds the score of a sequence of morae spoken in one breath: the lung pressure and the
/// pitch stay on throughout, the vowel changes at each mora, and every consonant forms in the
/// vowel context. Returns the score, the slot of each mora and the total length.
pub fn build_sequence(items: &[MoraItem], opts: &MoraOptions) -> (GestureScore, Vec<Slot>, f64) {
    use crate::gesture::Gesture as G;
    let mut s = GestureScore::default();
    let mut slots = vec![];
    let mut t = opts.lead_s;
    for it in items {
        let (hold, vowel_hold) = if it.moraic { (opts.moraic_nasal_s, 0.0) } else { (consonant_timing(it.consonant.manner).0, opts.vowel_hold_s) };
        slots.push(Slot { start: t, consonant_end: t + hold, end: t + hold + vowel_hold });
        t += hold + vowel_hold;
    }
    let total = t + TAIL_S;

    let mut cursors = [0.0f64; gt::COUNT];
    let neutral_filler = |d: f64| G::neutral(d);
    for (it, slot) in items.iter().zip(&slots) {
        let spec = &it.consonant;
        let hold = slot.consonant_end - slot.start;
        let (_, glottis_open_until) = consonant_timing(spec.manner);

        // Vowel: a glide or a palatalised mora starts from /i/ (/u/ for w).
        let glide_vowel = spec.glide_from.or(if it.palatalised { Some("i") } else { None });
        match glide_vowel {
            Some(v) if spec.manner != Manner::None => {
                let glide_len = if spec.manner == Manner::Glide { hold } else { hold + 0.03 };
                put(&mut s.sequences[gt::VOWEL], &mut cursors[gt::VOWEL], slot.start, G::shape(glide_len, v, TAU_VOWEL), opts.lead_s, neutral_filler);
                put(&mut s.sequences[gt::VOWEL], &mut cursors[gt::VOWEL], slot.start + glide_len, G::shape(slot.end - slot.start - glide_len, &it.vowel, TAU_VOWEL), opts.lead_s, neutral_filler);
            }
            _ => put(&mut s.sequences[gt::VOWEL], &mut cursors[gt::VOWEL], slot.start, G::shape(slot.end - slot.start, &it.vowel, TAU_VOWEL), opts.lead_s, neutral_filler),
        }

        // Consonant gestures per articulator (an affricate has two in a row).
        for art in [Articulator::Lip, Articulator::TongueTip, Articulator::TongueBody] {
            let seq_type = match art {
                Articulator::Lip => gt::LIP,
                Articulator::TongueTip => gt::TONGUE_TIP,
                Articulator::TongueBody => gt::TONGUE_BODY,
            };
            let list: Vec<&(Articulator, &str)> = spec.gestures.iter().filter(|g| g.0 == art).collect();
            let mut at = slot.start;
            for (k, (_, shape)) in list.iter().enumerate() {
                let (dur, tau) = match spec.manner {
                    Manner::Affricate if k == 0 => (0.05, TAU_CLOSURE),
                    Manner::Affricate => (0.10, TAU_FRICATIVE),
                    Manner::Fricative => (hold, TAU_FRICATIVE),
                    _ => (hold, TAU_CLOSURE),
                };
                put(&mut s.sequences[seq_type], &mut cursors[seq_type], at, G::shape(dur, shape, tau), opts.lead_s, neutral_filler);
                at += dur;
            }
        }

        // Velum: open for nasals while the tract is closed.
        if spec.manner == Manner::Nasal {
            let velic = |d: f64, v: f64, neutral: bool| Gesture { duration_s: d, d_val: v, slope: 0.0, s_val: String::new(), tau_s: 0.01, neutral };
            put(&mut s.sequences[gt::VELIC], &mut cursors[gt::VELIC], slot.start, velic(hold + 0.02, 0.5, false), opts.lead_s, |d| velic(d, -0.1, true));
        }

        // Glottis: open (voiceless) up to the release for voiceless consonants, else modal.
        let (open_shape, voiced_shape) = match spec.manner {
            Manner::Plosive | Manner::Tap | Manner::Nasal => ("voiceless-plosive", "voiced-plosive"),
            Manner::Glottal => ("h", "modal"),
            _ => ("voiceless-fricative", "voiced-fricative"),
        };
        let modal_filler = |d: f64| G::shape(d, "modal", TAU_GLOTTIS);
        if spec.voiced || glottis_open_until == 0.0 {
            let first = match spec.manner {
                Manner::Fricative | Manner::Affricate | Manner::Plosive => voiced_shape,
                _ => "modal",
            };
            put(&mut s.sequences[gt::GLOTTAL_SHAPE], &mut cursors[gt::GLOTTAL_SHAPE], slot.start, G::shape(hold, first, TAU_GLOTTIS), opts.lead_s, modal_filler);
        } else {
            put(&mut s.sequences[gt::GLOTTAL_SHAPE], &mut cursors[gt::GLOTTAL_SHAPE], slot.start, G::shape(glottis_open_until, open_shape, TAU_GLOTTIS), opts.lead_s, modal_filler);
        }
    }
    // Close every sequence at the end of the file.
    for seq_type in [gt::LIP, gt::TONGUE_TIP, gt::TONGUE_BODY] {
        if !s.sequences[seq_type].is_empty() && cursors[seq_type] < total {
            s.sequences[seq_type].push(G::neutral(total - cursors[seq_type]));
        }
    }
    if !s.sequences[gt::VELIC].is_empty() && cursors[gt::VELIC] < total {
        s.sequences[gt::VELIC].push(Gesture { duration_s: total - cursors[gt::VELIC], d_val: -0.1, slope: 0.0, s_val: String::new(), tau_s: 0.01, neutral: true });
    }
    if cursors[gt::GLOTTAL_SHAPE] < total {
        s.sequences[gt::GLOTTAL_SHAPE].push(G::shape(total - cursors[gt::GLOTTAL_SHAPE], "modal", TAU_GLOTTIS));
    }
    if cursors[gt::VOWEL] < total {
        let last = items.last().map(|i| i.vowel.clone()).unwrap_or_default();
        s.sequences[gt::VOWEL].push(G::shape(total - cursors[gt::VOWEL], &last, TAU_VOWEL));
    }
    s.sequences[gt::F0] = vec![Gesture::value(total, crate::gesture::f0_st(opts.f0_hz), 0.0, 0.03)];
    s.sequences[gt::PRESSURE] = vec![
        Gesture::value(opts.lead_s, 0.0, 0.0, 0.005),
        Gesture::value(total - opts.lead_s - TAIL_S, opts.pressure_dpa, 0.0, 0.005),
        Gesture::value(TAIL_S, 0.0, 0.0, 0.005),
    ];
    (s, slots, total)
}

/// Builds the score for one mora `consonant` + `vowel` ("a", "i", "u", "e", "o").
pub fn build_mora(spec: &ConsonantSpec, vowel: &str, opts: &MoraOptions) -> (GestureScore, MoraTiming) {
    let item = MoraItem { consonant: spec.clone(), vowel: vowel.to_string(), palatalised: opts.palatalised, moraic: false };
    let (score, slots, total) = build_sequence(&[item], opts);
    (score, MoraTiming { pressure_on: opts.lead_s, consonant_end: slots[0].consonant_end, total })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_covers_the_japanese_consonants() {
        let t = consonant_table();
        for need in ["k", "g", "t", "d", "p", "b", "s", "z", "sh", "j", "ch", "ts", "h", "hy", "f", "r", "n", "m", "y", "w", ""] {
            assert!(t.iter().any(|c| c.symbol == need), "{need}");
        }
        for c in &t {
            assert!(c.manner != Manner::Plosive || c.gestures.len() == 1, "{}", c.symbol);
        }
    }

    #[test]
    fn a_plosive_score_has_one_gesture_per_articulator_and_a_voiceless_glottis() {
        let t = consonant_table();
        let k = t.iter().find(|c| c.symbol == "k").unwrap();
        let (s, timing) = build_mora(k, "a", &MoraOptions::default());
        assert!(s.sequences[gt::TONGUE_BODY].iter().any(|g| g.s_val == "tb-velar-closure"));
        assert!(s.sequences[gt::LIP].is_empty() && s.sequences[gt::TONGUE_TIP].is_empty());
        assert_eq!(s.sequences[gt::GLOTTAL_SHAPE][0].s_val, "voiceless-plosive");
        assert!((s.duration_s() - timing.total).abs() < 1e-9);
        assert!(timing.consonant_end > timing.pressure_on);
    }

    #[test]
    fn voiced_consonants_keep_the_glottis_closed_and_nasals_open_the_velum() {
        let t = consonant_table();
        let g = t.iter().find(|c| c.symbol == "g").unwrap();
        assert_eq!(build_mora(g, "a", &MoraOptions::default()).0.sequences[gt::GLOTTAL_SHAPE][0].s_val, "voiced-plosive");
        let n = t.iter().find(|c| c.symbol == "n").unwrap();
        let (s, _) = build_mora(n, "a", &MoraOptions::default());
        assert!(s.sequences[gt::VELIC].iter().any(|v| !v.neutral && v.d_val > 0.0));
    }

    #[test]
    fn a_sequence_places_the_morae_one_after_another() {
        let t = consonant_table();
        let mk = |c: &str, v: &str| MoraItem { consonant: t.iter().find(|x| x.symbol == c).unwrap().clone(), vowel: v.to_string(), palatalised: false, moraic: false };
        let items = [mk("k", "a"), mk("s", "i"), mk("", "u")];
        let opts = MoraOptions::default();
        let (s, slots, total) = build_sequence(&items, &opts);
        assert_eq!(slots.len(), 3);
        assert!((slots[0].start - opts.lead_s).abs() < 1e-12);
        for w in slots.windows(2) { assert!((w[0].end - w[1].start).abs() < 1e-12, "contiguous"); }
        assert!((s.duration_s() - total).abs() < 1e-9);
        // Vowels in order; the velar closure only in the first mora, the alveolar fricative in the second.
        let vowels: Vec<&str> = s.sequences[gt::VOWEL].iter().map(|g| g.s_val.as_str()).collect();
        assert_eq!(&vowels[..3], ["a", "i", "u"]);
        // (start, end, name) of each gesture of the tongue body.
        let tb: Vec<(f64, f64, &str)> = { let mut at = 0.0; s.sequences[gt::TONGUE_BODY].iter().map(|g| { let r = (at, at + g.duration_s, g.s_val.as_str()); at += g.duration_s; r }).filter(|x| x.2 == "tb-velar-closure").collect() };
        assert_eq!(tb.len(), 1);
        // The first mora's gesture waits in position through the silent lead, then ends with the consonant.
        assert!(tb[0].0.abs() < 1e-9 && (tb[0].1 - slots[0].consonant_end).abs() < 1e-9, "{tb:?}");
        // One sequence covers the whole sequence duration; the pressure is on throughout.
        for (i, seq) in s.sequences.iter().enumerate() {
            if seq.is_empty() { continue; }
            let d: f64 = seq.iter().map(|g| g.duration_s).sum();
            assert!((d - total).abs() < 1e-6, "sequence {i}: {d} vs {total}");
        }
    }

    #[test]
    fn affricates_have_closure_then_frication_and_palatalised_morae_start_with_i() {
        let t = consonant_table();
        let ch = t.iter().find(|c| c.symbol == "ch").unwrap();
        let (s, _) = build_mora(ch, "a", &MoraOptions::default());
        let names: Vec<&str> = s.sequences[gt::TONGUE_TIP].iter().filter(|g| !g.neutral).map(|g| g.s_val.as_str()).collect();
        assert_eq!(names, ["tt-postalveolar-closure", "tt-postalveolar-fricative"]);
        let k = t.iter().find(|c| c.symbol == "k").unwrap();
        let (s, _) = build_mora(k, "a", &MoraOptions { palatalised: true, ..MoraOptions::default() });
        assert_eq!(s.sequences[gt::VOWEL][0].s_val, "i");
        assert_eq!(s.sequences[gt::VOWEL][1].s_val, "a");
    }
}
