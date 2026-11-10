//! VocalTract2UMora core: pure-Rust, dependency-free DSP / physics.
//!
//! Module map (see docs/ARCHITECTURE.md):
//! - `lf`        LF glottal flow model, Rd mapping, editor preview (implemented)
//! - `fft`       radix-2 FFT                                     (implemented)
//! - `geometry`  speaker skeleton scalers                       (scaffold)
//! - `oto`       UTAU oto.ini entry model                       (scaffold)
//! - `tract`, `consonant`, `batch` are planned; see roadmap.

pub mod fft;
pub mod geometry;
pub mod lf;
pub mod oto;
pub mod tds;
pub mod tube;
