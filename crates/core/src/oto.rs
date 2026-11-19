//! UTAU oto.ini entry. Times are in milliseconds.

#[derive(Debug, Clone, PartialEq)]
pub struct OtoEntry {
    pub wav: String,
    pub alias: String,
    /// Left blank (offset).
    pub offset: f64,
    /// Fixed (consonant) region.
    pub consonant: f64,
    /// Right blank; negative values are measured from the end of the file.
    pub cutoff: f64,
    /// Preutterance.
    pub preutterance: f64,
    pub overlap: f64,
}

impl OtoEntry {
    /// `wav=alias,offset,consonant,cutoff,preutterance,overlap`
    pub fn to_line(&self) -> String {
        format!(
            "{}={},{:.1},{:.1},{:.1},{:.1},{:.1}",
            self.wav, self.alias, self.offset, self.consonant, self.cutoff, self.preutterance, self.overlap
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_line() {
        let e = OtoEntry {
            wav: "ka.wav".into(), alias: "か".into(), offset: 10.0,
            consonant: 90.0, cutoff: -200.0, preutterance: 60.0, overlap: 20.0,
        };
        assert_eq!(e.to_line(), "ka.wav=か,10.0,90.0,-200.0,60.0,20.0");
    }
}

// ---------------------------------------------------------------------------------------
// Oto parameters from the physical events of a rendered mora
// ---------------------------------------------------------------------------------------

use crate::mora::{Manner, MoraTiming};
use crate::render::Rendered;

/// Physical moments of a rendered mora (seconds from the start of the file).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OtoEvents {
    /// First audible sound of the consonant (burst, frication, murmur ...).
    pub sound_s: f64,
    /// End of the closure (plosives, affricates, taps, nasals).
    pub release_s: Option<f64>,
    /// The vocal folds start to vibrate (strongly) after the consonant.
    pub voicing_onset_s: Option<f64>,
    /// The vowel has begun: the point where the consonant ends and the note starts.
    pub vowel_onset_s: f64,
    /// Level of the steady vowel (RMS), the reference for the thresholds.
    pub vowel_rms: f64,
}

/// UTAU oto.ini times in milliseconds (`cutoff` is negative: measured from the end of the file).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OtoTimes {
    pub offset: f64,
    pub consonant: f64,
    pub cutoff: f64,
    pub preutterance: f64,
    pub overlap: f64,
}

/// Short-time RMS envelope with a 5 ms window every 1 ms.
fn envelope(audio: &[f64], fs: f64) -> Vec<f64> {
    let win = (0.005 * fs) as usize;
    let hop = (0.001 * fs) as usize;
    let mut env = vec![];
    let mut i = 0;
    while i + win <= audio.len() {
        env.push((audio[i..i + win].iter().map(|v| v * v).sum::<f64>() / win as f64).sqrt());
        i += hop;
    }
    env
}

const CLOSURE_AREA_CM2: f64 = 0.05;

/// Finds the events of a rendered mora from the traces and the audio.
pub fn detect_events(r: &Rendered, timing: &MoraTiming, manner: Manner, voiced: bool) -> OtoEvents {
    let fs = r.sample_rate;
    let env = envelope(&r.audio, fs);
    let time_of = |k: usize| k as f64 * 0.001 + 0.0025; // centre of the window
    // Level of the steady vowel: middle of the vowel hold.
    let (v0, v1) = (timing.consonant_end + 0.15, timing.total - 0.12);
    let vowel: Vec<f64> = env.iter().enumerate().filter(|(k, _)| time_of(*k) >= v0 && time_of(*k) <= v1).map(|(_, e)| *e).collect();
    let vowel_rms = if vowel.is_empty() { 1e-9 } else { vowel.iter().sum::<f64>() / vowel.len() as f64 };

    // First audible sound after the lung pressure starts.
    let sound_threshold = 0.08 * vowel_rms;
    let audible = env.iter().enumerate().find(|(k, e)| time_of(*k) >= timing.pressure_on && time_of(*k) <= timing.total && **e >= sound_threshold).map(|(k, _)| time_of(k));
    let audible = audible.unwrap_or(timing.consonant_end);

    // Closure: the first time the narrowest section is below the threshold, until it opens.
    let mut release_s = None;
    let mut closed_seen = false;
    for t in &r.traces {
        if t.t_s < timing.pressure_on - 0.05 { continue; }
        if t.t_s > timing.total { break; } // stay inside this mora's window (it may be one of many)
        if t.min_area_cm2 < CLOSURE_AREA_CM2 { closed_seen = true; }
        else if closed_seen { release_s = Some(t.t_s); break; }
    }

    // Voicing: strong vibration of the vocal folds after the consonant.
    let steady_amp = {
        let a: Vec<f64> = r.traces.iter().filter(|t| t.t_s >= v0 && t.t_s <= v1).map(|t| t.glottal_amplitude_cm).collect();
        if a.is_empty() { 1.0 } else { a.iter().sum::<f64>() / a.len() as f64 }
    };
    let from = release_s.unwrap_or(timing.pressure_on);
    let voicing_onset_s = r.traces.iter()
        .find(|t| t.t_s >= from && t.t_s <= timing.total && t.glottal_amplitude_cm >= 0.5 * steady_amp && t.lung_pressure_dpa > 1000.0)
        .map(|t| t.t_s);

    // The moment of the note start (preutterance) and of the first sound, by manner.
    let rel = release_s.unwrap_or(timing.consonant_end);
    let voice = voicing_onset_s.unwrap_or(timing.consonant_end);
    let (sound_s, vowel_onset_s) = match manner {
        Manner::Plosive if !voiced => (rel - 0.005, voice.max(rel)),
        Manner::Plosive => (rel - 0.010, rel + 0.020),
        Manner::Affricate => (rel - 0.005, voice.max(rel)),
        Manner::Tap => (rel - 0.015, rel + 0.010),
        Manner::Nasal => (audible, rel + 0.010),
        Manner::Fricative if !voiced => (audible, voice),
        Manner::Fricative => (audible, timing.consonant_end + 0.01),
        Manner::Glottal => (audible, voice),
        Manner::Glide => (audible, timing.consonant_end + 0.01),
        Manner::None => (audible, audible + 0.02),
    };
    let sound_s = sound_s.max(0.0);
    let vowel_onset_s = vowel_onset_s.max(sound_s + 0.010);
    OtoEvents { sound_s, release_s, voicing_onset_s, vowel_onset_s, vowel_rms }
}

/// Steady part of the vowel that is kept after the note start (ms).
pub const STEADY_VOWEL_MS: f64 = 250.0;
/// Left blank before the first sound (ms).
pub const LEFT_MARGIN_MS: f64 = 20.0;

/// oto.ini times from the events. `total_s` is the length of the file.
pub fn compute_oto(e: &OtoEvents, total_s: f64) -> OtoTimes {
    let total_ms = total_s * 1000.0;
    let offset = (e.sound_s * 1000.0 - LEFT_MARGIN_MS).max(0.0);
    let preutterance = (e.vowel_onset_s * 1000.0 - offset).max(1.0);
    let overlap = (preutterance / 3.0).clamp(5.0, 40.0).min(preutterance - 1.0).max(0.0);
    // The fixed (not stretched) region reaches a little into the vowel.
    let mut consonant = preutterance + 30.0;
    // The right blank keeps a steady vowel of STEADY_VOWEL_MS after the note start.
    let keep_end = (offset + preutterance + STEADY_VOWEL_MS).min(total_ms - 60.0).max(offset + consonant + 50.0);
    let cutoff = -(total_ms - keep_end).max(0.0);
    consonant = consonant.min(keep_end - offset - 20.0);
    OtoTimes { offset, consonant, cutoff, preutterance, overlap }
}

impl OtoTimes {
    pub fn to_entry(&self, wav: &str, alias: &str) -> OtoEntry {
        OtoEntry { wav: wav.to_string(), alias: alias.to_string(), offset: self.offset, consonant: self.consonant, cutoff: self.cutoff, preutterance: self.preutterance, overlap: self.overlap }
    }
}

#[cfg(test)]
mod oto_event_tests {
    use super::*;
    use crate::consonant::ConsonantEditor;
    use crate::mora::{build_mora, consonant_table, MoraOptions};
    use crate::render::{render_score, RenderOptions};
    use crate::speaker::Speaker;

    fn mora_events(c: &str, v: &str) -> (OtoEvents, OtoTimes, f64) {
        let sp = Speaker::from_xml(include_str!("../../../resources/JD3.speaker")).unwrap();
        let ed = ConsonantEditor::from_speaker(&sp).unwrap();
        let t = consonant_table();
        let spec = t.iter().find(|x| x.symbol == c).unwrap();
        let (score, timing) = build_mora(spec, v, &MoraOptions::default());
        let r = render_score(&sp, &ed, &score, &RenderOptions::default());
        let ev = detect_events(&r, &timing, spec.manner, spec.voiced);
        let oto = compute_oto(&ev, timing.total);
        (ev, oto, timing.total)
    }

    #[test]
    fn a_voiceless_stop_has_burst_before_voicing_and_consistent_oto() {
        let (ev, oto, total) = mora_events("k", "a");
        let rel = ev.release_s.expect("closure release");
        let voice = ev.voicing_onset_s.expect("voicing onset");
        let vot_ms = (voice - rel) * 1000.0;
        println!("k: sound {:.3} release {rel:.3} voicing {voice:.3} (VOT {vot_ms:.0} ms) vowel {:.3}; {oto:?}", ev.sound_s, ev.vowel_onset_s);
        assert!(ev.sound_s <= rel && rel <= ev.vowel_onset_s, "ordering");
        assert!(vot_ms > 15.0 && vot_ms < 120.0, "VOT {vot_ms}");
        // The oto fields are ordered like UTAU expects them.
        assert!(oto.offset >= 0.0 && oto.offset < ev.sound_s * 1000.0 + 1e-9);
        assert!(oto.overlap < oto.preutterance && oto.preutterance < oto.consonant, "{oto:?}");
        assert!(oto.cutoff < 0.0 && oto.offset + oto.consonant < total * 1000.0 + oto.cutoff + 1e-9, "{oto:?}");
        // The preutterance lands on the voicing onset (the note start).
        assert!((oto.offset + oto.preutterance - ev.vowel_onset_s * 1000.0).abs() < 1.0);
    }

    #[test]
    fn voiceless_consonants_have_a_longer_consonant_part_than_voiced_ones() {
        let k = mora_events("k", "a").1;
        let g = mora_events("g", "a").1;
        println!("k pre {:.0} ms, g pre {:.0} ms", k.preutterance, g.preutterance);
        assert!(k.preutterance > g.preutterance, "aspiration makes /k/ longer than /g/");
    }

    #[test]
    fn a_fricative_starts_with_noise_long_before_voicing() {
        let (ev, oto, _) = mora_events("s", "a");
        assert!(ev.release_s.is_none(), "no closure for /s/");
        assert!(ev.vowel_onset_s - ev.sound_s > 0.05, "frication lasts {:.0} ms", (ev.vowel_onset_s - ev.sound_s) * 1000.0);
        assert!(oto.preutterance > 60.0, "{oto:?}");
    }
}
