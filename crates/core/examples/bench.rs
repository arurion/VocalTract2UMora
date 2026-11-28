// SPDX-License-Identifier: GPL-3.0-or-later
//! Where does the rendering time go? Times the geometry (per curve step) and the synthesizer
//! (per sample) separately.
use std::time::Instant;
use vt2umora_core::speaker::Speaker;
use vt2umora_core::synth::Synthesizer;
use vt2umora_core::tube::Tube;
use vt2umora_core::vocal_tract::VocalTract;

fn main() {
    let sp = Speaker::from_xml(include_str!("../../../resources/JD3.speaker")).unwrap();
    let mut vt = VocalTract::from_speaker(&sp);
    let n = 400;
    let t0 = Instant::now();
    let mut tube = Tube::default();
    for i in 0..n {
        let mut p = [0.0; vt2umora_core::speaker::NUM_PARAMS];
        p[0] = (i % 7) as f64 * 0.1;
        vt.set_params(&p);
        vt.calculate_all();
        tube = vt.get_tube();
    }
    let geo = t0.elapsed().as_secs_f64() / n as f64;
    // Split of calculate_all.
    let mut parts = [0.0f64; 4];
    for i in 0..n {
        let mut p = [0.0; vt2umora_core::speaker::NUM_PARAMS];
        p[0] = (i % 7) as f64 * 0.1;
        vt.set_params(&p);
        let t = Instant::now(); vt.calc_surfaces(); parts[0] += t.elapsed().as_secs_f64();
        let t = Instant::now(); vt.calc_center_line(); parts[1] += t.elapsed().as_secs_f64();
        let t = Instant::now(); vt.calc_cross_sections(); parts[2] += t.elapsed().as_secs_f64();
        let t = Instant::now(); vt.cross_sections_to_tube_sections(); parts[3] += t.elapsed().as_secs_f64();
    }
    println!("surfaces {:.3} ms, center line {:.3} ms, cross sections {:.3} ms, tube sections {:.3} ms", parts[0] / n as f64 * 1e3, parts[1] / n as f64 * 1e3, parts[2] / n as f64 * 1e3, parts[3] / n as f64 * 1e3);
    println!("geometry per curve step: {:.3} ms (x {} steps/s = {:.2} s per audio second)", geo * 1e3, vt2umora_core::gesture::CURVE_SAMPLING_RATE, geo * vt2umora_core::gesture::CURVE_SAMPLING_RATE as f64);

    let mut synth = Synthesizer::new(&tube, 44100.0);
    let t0 = Instant::now();
    let m = 44100;
    let mut acc = 0.0;
    for _ in 0..m { synth.set_tube(&tube); acc += synth.step(); }
    let per_sample = t0.elapsed().as_secs_f64() / m as f64;
    println!("set_tube + step per sample: {:.2} us = {:.2} s per audio second (acc {acc:.3})", per_sample * 1e6, per_sample * 44100.0);
}
