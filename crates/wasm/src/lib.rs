// SPDX-License-Identifier: GPL-3.0-or-later
use vt2umora_core::lf::{self, LfError, LfShape, VoiceParams};
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
