//! VocalTract2UMora core: pure-Rust, dependency-free DSP / physics.
//!
//! Module map (see docs/ARCHITECTURE.md):
//! - `lf`        LF glottal flow model + Rd parameter mapping   (implemented: Rd -> Ra/Rk/Rg)
//! - `geometry`  speaker skeleton scalers                       (scaffold)
//! - `oto`       UTAU oto.ini entry model                       (scaffold)
//! - `tract`, `consonant`, `batch` are planned; see roadmap.

pub mod geometry;
pub mod lf;
pub mod oto;
