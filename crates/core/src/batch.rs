// SPDX-License-Identifier: GPL-3.0-or-later
//! Batch generation of a UTAU voicebank: a recording list is read line by line, every line is
//! synthesised as one breath, the oto.ini entries follow from the physical events of the
//! simulation, and everything is bundled into a ZIP archive.
//!
//! List format: one recording per line; a line is a run of kana (hiragana or katakana, spaces
//! allowed) spoken in one breath. Lines starting with `#` and empty lines are ignored.
//! A line with a single mora gives the usual one-file-per-mora voicebank; a line with many
//! morae gives a long continuous recording with one oto entry per mora.

use crate::consonant::ConsonantEditor;
use crate::kana::{romaji, split_morae, to_shift_jis, Mora};
use crate::mora::{build_sequence, consonant_table, moraic_nasal_symbol, Manner, MoraItem, MoraOptions, MoraTiming};
use crate::oto::{compute_oto, compute_oto_steady, detect_events, detect_moraic_nasal, OtoEntry};
use crate::render::{render_score, wav_bytes, RenderOptions};
use crate::speaker::Speaker;
use crate::zip::ZipWriter;

#[derive(Debug, Clone)]
pub struct BatchOptions {
    pub mora: MoraOptions,
    pub sample_rate: f64,
    /// Target RMS of the steady vowel (full scale = 1), so that all files have the same level.
    pub target_vowel_rms: f64,
    pub noise_seed: u64,
}

impl Default for BatchOptions {
    fn default() -> Self {
        BatchOptions { mora: MoraOptions::default(), sample_rate: 44100.0, target_vowel_rms: 0.10, noise_seed: 1 }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ListLine {
    /// 1-based line number in the list text.
    pub line_no: usize,
    pub text: String,
    pub morae: Result<Vec<Mora>, String>,
}

/// Reads a recording list (see the module documentation).
pub fn parse_list(text: &str) -> Vec<ListLine> {
    text.lines().enumerate().filter_map(|(i, raw)| {
        let t = raw.trim();
        if t.is_empty() || t.starts_with('#') { return None; }
        Some(ListLine { line_no: i + 1, text: t.to_string(), morae: split_morae(t).map_err(|e| e.to_string()) })
    }).collect()
}

#[derive(Debug, Clone)]
pub struct LineResult {
    pub wav_name: String,
    pub wav: Vec<u8>,
    pub entries: Vec<OtoEntry>,
}

/// File name for a recording: running number and the romaji of the morae (ASCII).
pub fn wav_file_name(index: usize, morae: &[Mora]) -> String {
    let mut r: String = morae.iter().map(romaji).collect::<Vec<_>>().join("");
    r.truncate(40);
    format!("{:03}_{}.wav", index + 1, r)
}

/// Renders one recording (a run of morae spoken in one breath) and derives its oto entries.
pub fn render_line(sp: &Speaker, editor: &ConsonantEditor, index: usize, morae: &[Mora], opts: &BatchOptions) -> Result<LineResult, String> {
    let table = consonant_table();
    let mut items = vec![];
    for (k, m) in morae.iter().enumerate() {
        // A moraic nasal ん takes its articulation from the next mora.
        let moraic = m.consonant == "N";
        let symbol = if moraic { moraic_nasal_symbol(morae.get(k + 1).map(|n| n.consonant)) } else { m.consonant };
        let spec = table.iter().find(|c| c.symbol == symbol).ok_or_else(|| format!("子音 {} は未対応です", m.consonant))?;
        items.push(MoraItem { consonant: spec.clone(), vowel: m.vowel.to_string(), palatalised: m.palatalised, moraic });
    }
    if items.is_empty() { return Err("モーラがありません".to_string()); }
    let (score, slots, total) = build_sequence(&items, &opts.mora);
    let ropts = RenderOptions { sample_rate: opts.sample_rate, noise_seed: opts.noise_seed + index as u64, ..RenderOptions::default() };
    let r = render_score(sp, editor, &score, &ropts);

    // Level: the mean steady-vowel RMS of the line is brought to the target.
    let mut entries = vec![];
    let mut vowel_rms_sum = 0.0;
    let mut n = 0.0;
    let name = wav_file_name(index, morae);
    for (k, (m, slot)) in morae.iter().zip(&slots).enumerate() {
        let spec = &items[k].consonant;
        if spec.manner == Manner::Rest { continue; }
        if items[k].moraic {
            // ん (n m ng) or ン (nasalised vowel N): a segment of its own, ending with the nasal
            // so that the next mora is not part of it. Not used for the level of the line.
            let closing = spec.symbol != "N";
            let ev = detect_moraic_nasal(&r, slot.start, slot.end, closing);
            let steady_ms = (slot.end - ev.vowel_onset_s) * 1000.0 - 10.0;
            entries.push(compute_oto_steady(&ev, total, steady_ms).to_entry(&name, if closing { "ん" } else { "ン" }));
            continue;
        }
        let timing = MoraTiming { pressure_on: (slot.start - 0.03).max(0.0), consonant_end: slot.consonant_end, total: slot.end };
        let ev = detect_events(&r, &timing, spec.manner, spec.voiced);
        vowel_rms_sum += ev.vowel_rms;
        n += 1.0;
        if m.kana != "ー" {
            entries.push(compute_oto(&ev, total).to_entry(&name, &m.kana));
        }
    }
    let mean_rms = (vowel_rms_sum / n).max(1e-9);
    let peak = r.audio.iter().fold(0.0f64, |a, v| a.max(v.abs())).max(1e-9);
    let gain = (opts.target_vowel_rms / mean_rms).min(0.95 / peak);
    let samples: Vec<f64> = r.audio.iter().map(|v| (v * gain).clamp(-1.0, 1.0)).collect();
    Ok(LineResult { wav_name: name, wav: wav_bytes(&samples, opts.sample_rate as u32), entries })
}

/// oto.ini text (Shift_JIS bytes). `Err(char)` if an alias has a character that Shift_JIS
/// (as supported here) cannot represent.
pub fn oto_ini_bytes(entries: &[OtoEntry]) -> Result<Vec<u8>, char> {
    let text: String = entries.iter().map(|e| e.to_line() + "\r\n").collect();
    to_shift_jis(&text)
}

/// Bundles WAV files, `oto.ini` and `character.txt` into a ZIP archive.
pub fn voicebank_zip(files: &[(String, Vec<u8>)], oto_ini: &[u8], character_name: &str) -> Vec<u8> {
    let mut z = ZipWriter::new();
    for (name, data) in files { z.add(name, data); }
    z.add("oto.ini", oto_ini);
    z.add("character.txt", format!("name={character_name}\r\n").as_bytes());
    z.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    const JD3: &str = include_str!("../../../resources/JD3.speaker");

    #[test]
    fn parses_lines_comments_and_errors() {
        let l = parse_list("# comment\n\nかきくけこ\nさ し\nxyz\nきゃ\n");
        assert_eq!(l.len(), 4);
        assert_eq!(l[0].line_no, 3);
        assert_eq!(l[0].morae.as_ref().unwrap().len(), 5);
        assert_eq!(l[1].morae.as_ref().unwrap().len(), 2);
        assert!(l[2].morae.is_err());
        assert_eq!(l[3].morae.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn file_names_are_ascii_and_numbered() {
        let m = split_morae("かしゃ").unwrap();
        assert_eq!(wav_file_name(0, &m), "001_kasha.wav");
        assert_eq!(wav_file_name(11, &m), "012_kasha.wav");
    }

    #[test]
    fn a_continuous_line_gives_one_wav_and_one_oto_entry_per_mora() {
        let sp = Speaker::from_xml(JD3).unwrap();
        let ed = ConsonantEditor::from_speaker(&sp).unwrap();
        let morae = split_morae("かさた").unwrap();
        let r = render_line(&sp, &ed, 0, &morae, &BatchOptions::default()).unwrap();
        assert_eq!(&r.wav[0..4], b"RIFF");
        assert_eq!(r.entries.len(), 3);
        let total_ms = (r.wav.len() as f64 - 44.0) / 2.0 / 44.1;
        // The three morae follow one another in time, each with consistent oto values.
        let starts: Vec<f64> = r.entries.iter().map(|e| e.offset + e.preutterance).collect();
        assert!(starts.windows(2).all(|w| w[1] > w[0] + 200.0), "note starts {starts:?}");
        for e in &r.entries {
            assert!(e.overlap < e.preutterance && e.preutterance < e.consonant, "{e:?}");
            assert!(e.cutoff < 0.0 && e.offset + e.consonant < total_ms + e.cutoff + 1.0, "{e:?} of {total_ms} ms");
            assert_eq!(e.wav, "001_kasata.wav");
        }
        assert_eq!(r.entries[1].alias, "さ");
    }

    #[test]
    fn levels_are_normalised_and_nasals_render() {
        let sp = Speaker::from_xml(JD3).unwrap();
        let ed = ConsonantEditor::from_speaker(&sp).unwrap();
        let o = BatchOptions::default();
        let loud = render_line(&sp, &ed, 0, &split_morae("あ").unwrap(), &o).unwrap();
        let peak = |w: &[u8]| w[44..].chunks(2).map(|c| i16::from_le_bytes([c[0], c[1]]).unsigned_abs()).max().unwrap();
        assert!(peak(&loud.wav) < 32767);
        // Nasals need the nasal cavity: the murmur makes the closure clearly audible.
        let nasal = render_line(&sp, &ed, 0, &split_morae("なま").unwrap(), &o).unwrap();
        println!("{:#?}", nasal.entries);
        assert_eq!(nasal.entries.len(), 2);
        let samples: Vec<f64> = nasal.wav[44..].chunks(2).map(|c| i16::from_le_bytes([c[0], c[1]]) as f64).collect();
        let rms = |a: usize, b: usize| (samples[a..b].iter().map(|v| v * v).sum::<f64>() / (b - a) as f64).sqrt();
        let e = &nasal.entries[0];
        let sr = o.sample_rate;
        let at = |ms: f64| ((ms / 1000.0 * sr) as usize).min(samples.len() - 1);
        // From the left blank to the end of the consonant part (closure + release) of "な".
        let closure = rms(at(e.offset + 0.3 * e.preutterance), at(e.offset + e.preutterance - 5.0).max(at(e.offset + 0.3 * e.preutterance) + 10));
        let vowel = rms(at(e.offset + e.consonant + 40.0), at(e.offset + e.consonant + 80.0));
        println!("closure rms {closure:.0}, vowel rms {vowel:.0}");
        assert!(closure > 0.03 * vowel, "no murmur: closure {closure} vs vowel {vowel}");
    }

    #[test]
    fn moraic_nasals_assimilate_and_get_their_own_segments() {
        use crate::mora::moraic_nasal_symbol as sym;
        assert_eq!(sym(Some("m")), "m");
        assert_eq!(sym(Some("b")), "m");
        assert_eq!(sym(Some("k")), "ng");
        assert_eq!(sym(Some("t")), "n");
        assert_eq!(sym(Some("")), "N");
        assert_eq!(sym(Some("s")), "N");
        assert_eq!(sym(None), "N");

        let sp = Speaker::from_xml(JD3).unwrap();
        let ed = ConsonantEditor::from_speaker(&sp).unwrap();
        let o = BatchOptions::default();
        // ん before か (ng), before ま (m), before あ (N) and at the end (N).
        let line = render_line(&sp, &ed, 0, &split_morae("かんかあんまあんあん").unwrap(), &o).unwrap();
        println!("{:#?}", line.entries.iter().map(|e| (e.alias.clone(), e.offset.round(), e.preutterance.round(), e.overlap.round(), e.consonant.round(), e.cutoff.round())).collect::<Vec<_>>());
        let aliases: Vec<&str> = line.entries.iter().map(|e| e.alias.as_str()).collect();
        assert_eq!(aliases, ["か", "ん", "か", "あ", "ん", "ま", "あ", "ン", "あ", "ン"]);
        let total_ms = (line.wav.len() as f64 - 44.0) / 2.0 / 44.1;
        for e in &line.entries {
            assert!(e.offset >= 0.0 && e.preutterance >= 1.0 && e.overlap >= 0.0 && e.overlap < e.preutterance, "{e:?}");
            assert!(e.cutoff < 0.0 && e.offset + e.consonant < total_ms + e.cutoff + 1.0, "{e:?} of {total_ms} ms");
        }
        // Every ん / ン is audible: its steady part has a level comparable to the vowels.
        let samples: Vec<f64> = line.wav[44..].chunks(2).map(|c| i16::from_le_bytes([c[0], c[1]]) as f64).collect();
        let rms = |from_ms: f64, to_ms: f64| {
            let (a, b) = ((from_ms * 44.1) as usize, ((to_ms * 44.1) as usize).min(samples.len()));
            (samples[a..b].iter().map(|v| v * v).sum::<f64>() / (b - a) as f64).sqrt()
        };
        let vowel_level = rms(line.entries[0].offset + line.entries[0].preutterance + 100.0, line.entries[0].offset + line.entries[0].preutterance + 250.0);
        for e in line.entries.iter().filter(|e| e.alias == "ん" || e.alias == "ン") {
            let on = e.offset + e.preutterance;
            let level = rms(on + 50.0, on + 140.0);
            println!("{} at {on:.0} ms: level {level:.0} (vowel {vowel_level:.0})", e.alias);
            assert!(level > 0.15 * vowel_level, "{} is nearly silent: {level} vs {vowel_level}", e.alias);
        }
        // The nasal segments end with the nasal: the right blank does not reach the next mora.
        let nasal = &line.entries[1];
        let next = &line.entries[2];
        assert!(total_ms + nasal.cutoff <= next.offset + next.preutterance, "{nasal:?} {next:?}");
    }

    /// A line of the Kurosu 8-mora structure (one consonant per section, vowels rotated; a
    /// moraic nasal in the third place), written down after his published description.
    fn kurosu8_line(c: &str, shift: usize) -> String {
        let pattern: [Option<usize>; 8] = [Some(0), Some(0), None, Some(1), Some(0), Some(2), Some(0), Some(1)];
        let row: [[&str; 5]; 4] = [["か", "き", "く", "け", "こ"], ["さ", "すぃ", "す", "せ", "そ"], ["ふぁ", "ふぃ", "ふ", "ふぇ", "ふぉ"], ["ヴぁ", "ヴぃ", "ヴ", "ヴぇ", "ヴぉ"]];
        let r = match c { "k" => 0, "s" => 1, "f" => 2, _ => 3 };
        let mut out = String::from("_");
        for p in pattern { match p { None => out.push('ん'), Some(x) => out.push_str(row[r][(x + shift) % 5]) } }
        out
    }

    #[test]
    fn kurosu_style_lines_with_rests_and_special_kana_render() {
        let sp = Speaker::from_xml(JD3).unwrap();
        let ed = ConsonantEditor::from_speaker(&sp).unwrap();
        let o = BatchOptions::default();
        for (c, shift) in [("k", 0), ("s", 2), ("f", 0), ("v", 1)] {
            let text = kurosu8_line(c, shift);
            let morae = split_morae(&text).unwrap();
            let t0 = std::time::Instant::now();
            let r = render_line(&sp, &ed, 0, &morae, &o).unwrap();
            let secs = (r.wav.len() as f64 - 44.0) / 2.0 / o.sample_rate;
            println!("{text}: {} morae, {} entries, {secs:.1} s of audio in {:.2} s", morae.len(), r.entries.len(), t0.elapsed().as_secs_f64());
            assert_eq!(r.entries.len(), morae.len());
            assert!(r.entries.iter().any(|e| e.alias == "ん" || e.alias == "ン"));
        }
        // A rest is silent and is not an oto entry.
        let line = render_line(&sp, &ed, 0, &split_morae("あ_い").unwrap(), &o).unwrap();
        assert_eq!(line.entries.iter().map(|e| e.alias.as_str()).collect::<Vec<_>>(), ["あ", "い"]);
        let samples: Vec<f64> = line.wav[44..].chunks(2).map(|c| i16::from_le_bytes([c[0], c[1]]) as f64).collect();
        let rms = |a_s: f64, b_s: f64| { let (a, b) = ((a_s * 44100.0) as usize, (b_s * 44100.0) as usize); (samples[a..b].iter().map(|v| v * v).sum::<f64>() / (b - a) as f64).sqrt() };
        // lead 0.05 + あ (0.02 + 0.40) = 0.47 s; the rest lasts 0.30 s from there.
        let (vowel, rest) = (rms(0.30, 0.45), rms(0.55, 0.75));
        println!("vowel rms {vowel:.0}, rest rms {rest:.0}");
        assert!(rest < 0.05 * vowel, "the rest is not silent: {rest} vs {vowel}");
    }

    #[test]
    fn oto_ini_is_shift_jis_and_the_zip_bundles_everything() {
        let e = OtoEntry { wav: "001_ka.wav".into(), alias: "か".into(), offset: 10.0, consonant: 90.0, cutoff: -100.0, preutterance: 60.0, overlap: 20.0 };
        let ini = oto_ini_bytes(&[e]).unwrap();
        assert_eq!(&ini[..11], b"001_ka.wav=");
        assert_eq!(&ini[11..13], &[0x82, 0xA9]); // か in Shift_JIS
        let z = voicebank_zip(&[("001_ka.wav".to_string(), vec![1, 2, 3])], &ini, "test");
        assert_eq!(&z[0..4], b"PK\x03\x04");
        assert!(z.windows(7).any(|w| w == b"oto.ini"));
        assert!(z.windows(13).any(|w| w == b"character.txt"));
    }
}
