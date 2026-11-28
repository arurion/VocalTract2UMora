// SPDX-License-Identifier: GPL-3.0-or-later
//! Builds a UTAU voicebank (ZIP) from a recording list, natively, for checking and scripting.
//!
//!   cargo run --release --example build_bank -- <single|vcv|cvvc> <list.txt|builtin> <out.zip> [max_lines]
//!
//! `builtin` is the Kurosu 8-mora structure (see `lists.rs`). The speaker is the bundled JD3.

use vt2umora_core::batch::{build_voicebank, BatchOptions, OtoStyle};
use vt2umora_core::consonant::ConsonantEditor;
use vt2umora_core::speaker::Speaker;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("usage: build_bank <single|vcv|cvvc> <list.txt|builtin> <out.zip> [max_lines]");
        std::process::exit(2);
    }
    let style = match args[1].as_str() { "single" => OtoStyle::Single, "vcv" => OtoStyle::Vcv, "cvvc" => OtoStyle::Cvvc, s => { eprintln!("unknown style {s}"); std::process::exit(2) } };
    let mut text = if args[2] == "builtin" { vt2umora_core::lists::kurosu8_list() } else { std::fs::read_to_string(&args[2]).expect("cannot read the list") };
    if let Some(n) = args.get(4).and_then(|s| s.parse::<usize>().ok()) {
        text = text.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')).take(n).collect::<Vec<_>>().join("\n");
    }
    let sp = Speaker::from_xml(include_str!("../../../resources/JD3.speaker")).expect("speaker");
    let ed = ConsonantEditor::from_speaker(&sp).expect("consonants");
    let opts = BatchOptions { style, ..BatchOptions::default() };
    let t0 = std::time::Instant::now();
    let rep = build_voicebank(&sp, &ed, &text, &opts, "VT2UMora").expect("build");
    std::fs::write(&args[3], &rep.zip).expect("write");
    for e in &rep.errors { eprintln!("{e}"); }
    eprintln!("{} entries, {} files, {} bytes in {:.1} s", rep.entries, rep.files, rep.zip.len(), t0.elapsed().as_secs_f64());
}
