use wasm_bindgen::prelude::*;

/// Crate version, used by the UI to confirm the Wasm module loaded.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Returns `[ra, rk, rg]` for a given Rd.
#[wasm_bindgen]
pub fn lf_shape_from_rd(rd: f64) -> Vec<f64> {
    let s = vt2umora_core::lf::shape_from_rd(rd);
    vec![s.ra, s.rk, s.rg]
}
