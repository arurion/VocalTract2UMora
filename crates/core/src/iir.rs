// SPDX-License-Identifier: GPL-3.0-or-later
//! Recursive (IIR) filters. The Chebyshev design is a port of VocalTractLab's `IirFilter`
//! (C) Peter Birkholz, GPL-3.0-or-later, which follows S. W. Smith, "The Scientist and
//! Engineer's Guide to DSP", with 0.5 % passband ripple.
//!
//! Difference equation: `y[n] = sum_k a[k] x[n-k] + sum_{k>=1} b[k] y[n-k]`.

use std::f64::consts::PI;

pub const MAX_ORDER: usize = 32;
const BUF_MASK: usize = 63;

#[derive(Clone)]
pub struct Iir {
    order: usize,
    a: [f64; MAX_ORDER + 1],
    b: [f64; MAX_ORDER + 1],
    in_buf: [f64; 64],
    out_buf: [f64; 64],
    pos: usize,
}

impl Iir {
    /// Chebyshev low-pass (or high-pass) filter. `cutoff_ratio` is the cutoff divided by the
    /// sampling rate (0.5 is the Nyquist frequency). `num_poles` is rounded up to an even number.
    pub fn chebyshev(cutoff_ratio: f64, highpass: bool, num_poles: usize) -> Iir {
        let percent_ripple: f64 = 0.5;
        let mut num_poles = num_poles + (num_poles & 1);
        num_poles = num_poles.min(MAX_ORDER);
        let mut a = [0.0; MAX_ORDER + 1];
        let mut b = [0.0; MAX_ORDER + 1];
        a[2] = 1.0;
        b[2] = 1.0;

        for pol in 1..=num_poles / 2 {
            let ang = PI / (2.0 * num_poles as f64) + (PI * (pol - 1) as f64) / num_poles as f64;
            let mut pol_real = -ang.cos();
            let mut pol_imag = ang.sin();
            if percent_ripple != 0.0 {
                // Warp from a circle to an ellipse.
                let temp: f64 = 100.0 / (100.0 - percent_ripple);
                let es = (temp * temp - 1.0).sqrt();
                let vx = (1.0 / num_poles as f64) * ((1.0 / es) + ((1.0 / (es * es)) + 1.0).sqrt()).ln();
                let mut kx = (1.0 / num_poles as f64) * ((1.0 / es) + ((1.0 / (es * es)) - 1.0).sqrt()).ln();
                kx = 0.5 * (kx.exp() + (-kx).exp());
                pol_real = pol_real * (0.5 * (vx.exp() - (-vx).exp())) / kx;
                pol_imag = pol_imag * (0.5 * (vx.exp() + (-vx).exp())) / kx;
            }
            // s-domain to z-domain.
            let t = 2.0 * (0.5f64).tan();
            let w = 2.0 * PI * cutoff_ratio;
            let m = pol_real * pol_real + pol_imag * pol_imag;
            let mut d = 4.0 - 4.0 * pol_real * t + m * t * t;
            let x0 = (t * t) / d;
            let x1 = (2.0 * t * t) / d;
            let x2 = (t * t) / d;
            let y1 = (8.0 - 2.0 * m * t * t) / d;
            let y2 = (-4.0 - 4.0 * pol_real * t - m * t * t) / d;
            // Low-pass to low-pass (or high-pass) transformation.
            let k = if highpass { -(0.5 * w + 0.5).cos() / (0.5 * w - 0.5).cos() } else { (0.5 - 0.5 * w).sin() / (0.5 + 0.5 * w).sin() };
            d = 1.0 + y1 * k - y2 * k * k;
            let a0 = (x0 - x1 * k + x2 * k * k) / d;
            let mut a1 = (-2.0 * x0 * k + x1 + x1 * k * k - 2.0 * x2 * k) / d;
            let a2 = (x0 * k * k - x1 * k + x2) / d;
            let mut b1 = (2.0 * k + y1 + y1 * k * k - 2.0 * y2 * k) / d;
            let b2 = (-(k * k) - y1 * k + y2) / d;
            if highpass { a1 = -a1; b1 = -b1; }
            // Add this 2-pole stage to the cascade.
            let (ta, tb) = (a, b);
            for i in 2..=MAX_ORDER {
                a[i] = a0 * ta[i] + a1 * ta[i - 1] + a2 * ta[i - 2];
                b[i] = tb[i] - b1 * tb[i - 1] - b2 * tb[i - 2];
            }
        }
        // Finish combining the coefficients.
        b[2] = 0.0;
        for i in 0..=MAX_ORDER - 2 {
            a[i] = a[i + 2];
            b[i] = -b[i + 2];
        }
        // Normalise the gain (at DC for low-pass, at Nyquist for high-pass).
        let (mut sum_a, mut sum_b) = (0.0, 0.0);
        for i in 0..=MAX_ORDER - 2 {
            if !highpass || i & 1 == 0 { sum_a += a[i]; sum_b += b[i]; } else { sum_a -= a[i]; sum_b -= b[i]; }
        }
        let gain = sum_a / (1.0 - sum_b);
        for v in a.iter_mut().take(MAX_ORDER - 1) { *v /= gain; }

        Iir { order: num_poles, a, b, in_buf: [0.0; 64], out_buf: [0.0; 64], pos: 0 }
    }

    pub fn reset(&mut self) {
        self.in_buf = [0.0; 64];
        self.out_buf = [0.0; 64];
        self.pos = 0;
    }

    pub fn process(&mut self, x: f64) -> f64 {
        self.in_buf[self.pos & BUF_MASK] = x;
        let mut sum = self.a[0] * x;
        for i in 1..=self.order {
            let j = self.pos.wrapping_sub(i) & BUF_MASK;
            sum += self.a[i] * self.in_buf[j] + self.b[i] * self.out_buf[j];
        }
        self.out_buf[self.pos & BUF_MASK] = sum;
        self.pos = self.pos.wrapping_add(1);
        sum
    }

    /// Magnitude of the frequency response at `freq_ratio` (frequency / sampling rate).
    pub fn magnitude(&self, freq_ratio: f64) -> f64 {
        let (w, mut nr, mut ni, mut dr, mut di) = (2.0 * PI * freq_ratio, self.a[0], 0.0, 1.0, 0.0);
        for i in 1..=self.order {
            let (c, s) = ((w * i as f64).cos(), -(w * i as f64).sin());
            nr += self.a[i] * c;
            ni += self.a[i] * s;
            dr -= self.b[i] * c;
            di -= self.b[i] * s;
        }
        ((nr * nr + ni * ni) / (dr * dr + di * di)).sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chebyshev_lowpass_passes_low_and_blocks_high() {
        let f = Iir::chebyshev(25.0 / 44100.0, false, 4);
        assert!((f.magnitude(1e-6) - 1.0).abs() < 1e-3, "dc gain {}", f.magnitude(1e-6));
        assert!(f.magnitude(10.0 / 44100.0) > 0.9);
        assert!(f.magnitude(100.0 / 44100.0) < 0.01, "100 Hz: {}", f.magnitude(100.0 / 44100.0));
        let g = Iir::chebyshev(12000.0 / 44100.0, false, 8);
        assert!(g.magnitude(5000.0 / 44100.0) > 0.95);
        assert!(g.magnitude(20000.0 / 44100.0) < 0.01);
    }

    #[test]
    fn step_response_settles_to_one() {
        let mut f = Iir::chebyshev(25.0 / 44100.0, false, 4);
        let mut y = 0.0;
        for _ in 0..44100 { y = f.process(1.0); }
        assert!((y - 1.0).abs() < 1e-3, "{y}");
    }

    #[test]
    fn highpass_blocks_dc() {
        let f = Iir::chebyshev(1000.0 / 44100.0, true, 4);
        assert!(f.magnitude(1e-6) < 1e-3);
        assert!((f.magnitude(0.4) - 1.0).abs() < 0.05);
    }
}
