// SPDX-License-Identifier: GPL-3.0-or-later
//! Minimal in-place radix-2 FFT (no dependencies).

use std::f64::consts::PI;

/// In-place complex FFT. `re.len()` must equal `im.len()` and be a power of two.
pub fn fft(re: &mut [f64], im: &mut [f64]) {
    let n = re.len();
    assert!(n.is_power_of_two() && im.len() == n, "fft size must be a power of two");
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -2.0 * PI / len as f64;
        let (wr, wi) = (ang.cos(), ang.sin());
        for start in (0..n).step_by(len) {
            let (mut cr, mut ci) = (1.0, 0.0);
            for k in 0..len / 2 {
                let (a, b) = (start + k, start + k + len / 2);
                let tr = re[b] * cr - im[b] * ci;
                let ti = re[b] * ci + im[b] * cr;
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
                let ncr = cr * wr - ci * wi;
                ci = cr * wi + ci * wr;
                cr = ncr;
            }
        }
        len <<= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sine_lands_in_one_bin() {
        let n = 256;
        let k = 10;
        let mut re: Vec<f64> = (0..n).map(|i| (2.0 * PI * k as f64 * i as f64 / n as f64).sin()).collect();
        let mut im = vec![0.0; n];
        fft(&mut re, &mut im);
        let mag = |i: usize| (re[i] * re[i] + im[i] * im[i]).sqrt();
        assert!((mag(k) - n as f64 / 2.0).abs() < 1e-9);
        assert!(mag(k + 3) < 1e-9);
    }
}
