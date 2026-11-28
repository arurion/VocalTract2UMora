// SPDX-License-Identifier: GPL-3.0-or-later
use vt2umora_core::batch::{self, Assembled, BatchOptions, LineResult, OtoStyle};
use vt2umora_core::consonant::ConsonantEditor;
use vt2umora_core::geometry::SpeakerGeometry;
use vt2umora_core::lf::{self, LfError, LfShape, VoiceParams};
use vt2umora_core::oto::OtoEntry;
use vt2umora_core::speaker::Speaker;
use wasm_bindgen::prelude::*;

/// Crate version, used by the UI to confirm the Wasm module loaded.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Returns `[ra, rk, rg]` for a given Rd (Fant 2004 regression).
#[wasm_bindgen]
pub fn lf_shape_from_rd(rd: f64) -> Vec<f64> {
    let s = lf::shape_from_rd(rd);
    vec![s.ra, s.rk, s.rg]
}

fn err(e: LfError) -> JsError {
    match e {
        LfError::InvalidParams => JsError::new("invalid-params"),
        LfError::NoSolution => JsError::new("no-solution"),
    }
}

/// Result of an LF preview. Getters copy out typed arrays.
#[wasm_bindgen]
pub struct LfPreview {
    inner: lf::Preview,
}

#[wasm_bindgen]
impl LfPreview {
    /// One period of dUg/dt.
    pub fn dflow(&self) -> Vec<f64> { self.inner.dflow.clone() }
    /// One period of Ug.
    pub fn flow(&self) -> Vec<f64> { self.inner.flow.clone() }
    /// Spectrum (dB) of dUg/dt plus aspiration noise.
    pub fn spectrum_db(&self) -> Vec<f64> { self.inner.spectrum_db.clone() }
    #[wasm_bindgen(getter)]
    pub fn bin_hz(&self) -> f64 { self.inner.bin_hz }
    #[wasm_bindgen(getter)]
    pub fn tilt_db_per_oct(&self) -> f64 { self.inner.tilt_db_per_oct }
    #[wasm_bindgen(getter)]
    pub fn tilt_level_1k_db(&self) -> f64 { self.inner.tilt_level_1k_db }
    #[wasm_bindgen(getter)]
    pub fn h1_h2_db(&self) -> f64 { self.inner.h1_h2_db }
    #[wasm_bindgen(getter)]
    pub fn voiced(&self) -> bool { self.inner.voiced }
    /// Normalised Te (maximum closing instant), Tp (flow peak) and Ta (return time).
    #[wasm_bindgen(getter)]
    pub fn te(&self) -> f64 { self.inner.pulse.te }
    #[wasm_bindgen(getter)]
    pub fn tp(&self) -> f64 { self.inner.pulse.tp }
    #[wasm_bindgen(getter)]
    pub fn ta(&self) -> f64 { self.inner.pulse.ta }
}

/// LF preview. Errors are the strings `"invalid-params"` or `"no-solution"`.
#[wasm_bindgen]
pub fn lf_preview(
    f0: f64,
    ra: f64,
    rk: f64,
    rg: f64,
    breath_cmh2o: f64,
    aspiration: f64,
    sample_rate: f64,
    wave_points: u32,
) -> Result<LfPreview, JsError> {
    let params = VoiceParams {
        f0,
        shape: LfShape { ra, rk, rg },
        breath_cmh2o,
        aspiration,
    };
    lf::preview(&params, sample_rate, wave_points as usize)
        .map(|inner| LfPreview { inner })
        .map_err(err)
}

/// Clamps a speaker geometry and derives lengths and neutral-tract formants.
/// Returns `[tract_length_cm, oral_ratio, nasal_volume_scale, oral_cm, pharyngeal_cm, f1, f2, f3]`.
#[wasm_bindgen]
pub fn geometry_info(tract_length_cm: f64, oral_ratio: f64, nasal_volume_scale: f64) -> Vec<f64> {
    let g = SpeakerGeometry { tract_length_cm, oral_ratio, nasal_volume_scale }.clamped();
    let f = g.neutral_formants(3);
    vec![
        g.tract_length_cm,
        g.oral_ratio,
        g.nasal_volume_scale,
        g.oral_length_cm(),
        g.pharyngeal_length_cm(),
        f[0],
        f[1],
        f[2],
    ]
}

// ---------------------------------------------------------------------------------------------
// Batch voicebank generation. Every Web Worker owns an `Engine` and renders whole lines;
// the main thread collects the results in an `Assembler` and gets the ZIP from it.
// ---------------------------------------------------------------------------------------------

const BUNDLED_SPEAKER: &str = include_str!("../../../resources/JD3.speaker");

fn parse_style(style: &str) -> Result<OtoStyle, JsError> {
    match style {
        "single" => Ok(OtoStyle::Single),
        "vcv" => Ok(OtoStyle::Vcv),
        "cvvc" => Ok(OtoStyle::Cvvc),
        s => Err(JsError::new(&format!("unknown style {s} (single, vcv or cvvc)"))),
    }
}

/// The built-in recording list (Kurosu 8-mora structure, 145 lines).
#[wasm_bindgen]
pub fn builtin_list() -> String {
    vt2umora_core::lists::kurosu8_list()
}

/// A recording list read line by line: which lines are usable and why the others are not.
#[wasm_bindgen]
pub struct ListPlan {
    lines: Vec<batch::ListLine>,
}

/// Reads a recording list (comments and empty lines dropped).
#[wasm_bindgen]
pub fn plan_list(text: &str) -> ListPlan {
    ListPlan { lines: batch::parse_list(text) }
}

#[wasm_bindgen]
impl ListPlan {
    #[wasm_bindgen(getter)]
    pub fn len(&self) -> usize { self.lines.len() }
    /// 1-based line number in the list text.
    pub fn line_no(&self, i: usize) -> usize { self.lines[i].line_no }
    pub fn text(&self, i: usize) -> String { self.lines[i].text.clone() }
    /// Number of morae of the line, 0 if it cannot be read.
    pub fn mora_count(&self, i: usize) -> usize { self.lines[i].morae.as_ref().map(|m| m.len()).unwrap_or(0) }
    /// Why the line cannot be read ("" if it can).
    pub fn error(&self, i: usize) -> String { self.lines[i].morae.as_ref().err().cloned().unwrap_or_default() }
}

/// Settings of a batch run.
#[wasm_bindgen]
pub struct BatchConfig {
    style: OtoStyle,
    inner: BatchOptions,
}

#[wasm_bindgen]
impl BatchConfig {
    /// `style`: "single", "vcv" or "cvvc".
    #[wasm_bindgen(constructor)]
    pub fn new(style: &str) -> Result<BatchConfig, JsError> {
        let style = parse_style(style)?;
        Ok(BatchConfig { style, inner: BatchOptions { style, ..BatchOptions::default() } })
    }
    pub fn set_f0_hz(&mut self, v: f64) { self.inner.mora.f0_hz = v.clamp(40.0, 600.0); }
    pub fn set_pressure_dpa(&mut self, v: f64) { self.inner.mora.pressure_dpa = v.clamp(500.0, 20000.0); }
    pub fn set_vowel_hold_s(&mut self, v: f64) { self.inner.mora.vowel_hold_s = v.clamp(0.15, 1.5); }
    pub fn set_moraic_nasal_s(&mut self, v: f64) { self.inner.mora.moraic_nasal_s = v.clamp(0.08, 0.6); }
    pub fn set_rest_s(&mut self, v: f64) { self.inner.mora.rest_s = v.clamp(0.1, 1.5); }
    pub fn set_sample_rate(&mut self, v: f64) { self.inner.sample_rate = v.clamp(16000.0, 96000.0); }
    pub fn set_target_vowel_rms(&mut self, v: f64) { self.inner.target_vowel_rms = v.clamp(0.01, 0.4); }
    /// Geometry step of the renderer: 1 is exact, 2 (default) and 4 are faster.
    pub fn set_geometry_stride(&mut self, v: u32) { self.inner.geometry_stride = (v as usize).clamp(1, 8); }
    pub fn set_noise_seed(&mut self, v: u32) { self.inner.noise_seed = v as u64; }
    #[wasm_bindgen(getter)]
    pub fn style(&self) -> String {
        match self.style { OtoStyle::Single => "single", OtoStyle::Vcv => "vcv", OtoStyle::Cvvc => "cvvc" }.to_string()
    }
}

/// The synthesiser of one worker: a speaker and its consonant targets.
#[wasm_bindgen]
pub struct Engine {
    speaker: Speaker,
    editor: ConsonantEditor,
}

#[wasm_bindgen]
impl Engine {
    /// With `speaker_xml` empty the bundled speaker (JD3) is used.
    #[wasm_bindgen(constructor)]
    pub fn new(speaker_xml: &str) -> Result<Engine, JsError> {
        let xml = if speaker_xml.trim().is_empty() { BUNDLED_SPEAKER } else { speaker_xml };
        let speaker = Speaker::from_xml(xml).map_err(|e| JsError::new(&format!("speaker file: {e}")))?;
        let editor = ConsonantEditor::from_speaker(&speaker).ok_or_else(|| JsError::new("speaker file has no consonant shapes"))?;
        Ok(Engine { speaker, editor })
    }

    /// Renders one line of a list (text of the line, `index` = position among the readable
    /// lines, which also numbers the WAV file).
    pub fn render_line(&self, line_text: &str, index: usize, config: &BatchConfig) -> Result<RenderedLine, JsError> {
        let morae = vt2umora_core::kana::split_morae(line_text).map_err(|e| JsError::new(&e.to_string()))?;
        let r = batch::render_line(&self.speaker, &self.editor, index, &morae, &config.inner).map_err(|e| JsError::new(&e))?;
        Ok(RenderedLine { inner: r })
    }
}

/// One rendered line: the WAV file and its oto entries.
#[wasm_bindgen]
pub struct RenderedLine {
    inner: LineResult,
}

#[wasm_bindgen]
impl RenderedLine {
    #[wasm_bindgen(getter)]
    pub fn wav_name(&self) -> String { self.inner.wav_name.clone() }
    pub fn wav(&self) -> Vec<u8> { self.inner.wav.clone() }
    #[wasm_bindgen(getter)]
    pub fn entry_count(&self) -> usize { self.inner.entries.len() }
    pub fn entry_alias(&self, k: usize) -> String { self.inner.entries[k].alias.clone() }
    /// `[offset, consonant, cutoff, preutterance, overlap]` in ms.
    pub fn entry_times(&self, k: usize) -> Vec<f64> {
        let e: &OtoEntry = &self.inner.entries[k];
        vec![e.offset, e.consonant, e.cutoff, e.preutterance, e.overlap]
    }
}

/// Collects the rendered lines (in list order) and builds the voicebank.
#[wasm_bindgen]
pub struct Assembler {
    results: Vec<LineResult>,
    errors: Vec<String>,
    built: Option<Assembled>,
}

#[wasm_bindgen]
pub fn new_assembler() -> Assembler {
    Assembler { results: vec![], errors: vec![], built: None }
}

#[wasm_bindgen]
impl Assembler {
    /// Adds a line rendered by a worker. Lines must be added in list order.
    pub fn add_line(&mut self, line: &RenderedLine) {
        self.results.push(line.inner.clone());
    }
    /// Adds a line from its parts (when the result came over `postMessage`): returns its index.
    pub fn add_wav(&mut self, wav_name: &str, wav: Vec<u8>) -> usize {
        self.results.push(LineResult { wav_name: wav_name.to_string(), wav, entries: vec![] });
        self.results.len() - 1
    }
    /// `times` = `[offset, consonant, cutoff, preutterance, overlap]`.
    pub fn add_entry(&mut self, line: usize, alias: &str, times: &[f64]) {
        if times.len() != 5 { return; }
        let wav = self.results[line].wav_name.clone();
        self.results[line].entries.push(OtoEntry { wav, alias: alias.to_string(), offset: times[0], consonant: times[1], cutoff: times[2], preutterance: times[3], overlap: times[4] });
    }
    /// A line that could not be rendered ("N行目「...」: reason").
    pub fn add_error(&mut self, message: &str) { self.errors.push(message.to_string()); }
    /// Builds the voicebank; afterwards `zip`, `entries`, `files` and `oto_text` can be read.
    pub fn finish(&mut self, character_name: &str) -> Result<(), JsError> {
        let r = batch::assemble_voicebank_with_text(std::mem::take(&mut self.results), std::mem::take(&mut self.errors), character_name).map_err(|e| JsError::new(&e))?;
        self.built = Some(r);
        Ok(())
    }
    pub fn zip(&self) -> Vec<u8> { self.built.as_ref().map(|b| b.report.zip.clone()).unwrap_or_default() }
    #[wasm_bindgen(getter)]
    pub fn entries(&self) -> usize { self.built.as_ref().map(|b| b.report.entries).unwrap_or(0) }
    #[wasm_bindgen(getter)]
    pub fn files(&self) -> usize { self.built.as_ref().map(|b| b.report.files).unwrap_or(0) }
    /// The oto.ini as text (UTF-8, for the preview; the ZIP has it in Shift_JIS).
    pub fn oto_text(&self) -> String { self.built.as_ref().map(|b| b.oto_text.clone()).unwrap_or_default() }
    /// Lines that could not be rendered, one per entry.
    pub fn errors(&self) -> Vec<String> { self.built.as_ref().map(|b| b.report.errors.clone()).unwrap_or_default() }
}
