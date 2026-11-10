// SPDX-License-Identifier: GPL-3.0-or-later
//! Time-domain simulation (TDS) of vocal tract acoustics.
//!
//! A chain of tube sections is treated as an electrical network (inertance L, compliance C,
//! viscous/turbulent resistance R, soft walls). The unknowns are the branch currents (volume
//! velocities); the system is discretised with the theta method and solved every sample by a
//! sparse Cholesky factorisation. This is a port of the formulation used by VocalTractLab's
//! `TdsModel` (Birkholz et al.): VocalTractLab, (C) Peter Birkholz, GPL-3.0-or-later.
//!
//! Subset implemented here: oral chain with lip radiation. Not yet: nasal cavity, sinuses,
//! piriform fossa, noise sources, radiation from the skin, transvelar coupling.

use crate::tube::*;
use std::f64::consts::PI;

pub const AMBIENT_DENSITY: f64 = 1.14e-3; // g/cm^3
pub const SOUND_VELOCITY: f64 = 3.5e4; // cm/s
pub const AIR_VISCOSITY: f64 = 1.86e-4; // dyne s/cm^2
/// With theta = 0.505 the TDS bandwidths match the finite-difference solution up to ~5 kHz.
const THETA: f64 = 0.505;
const THETA1: f64 = 1.0 - THETA;
const MIN_RADIUS_CM: f64 = 0.6;

#[derive(Clone, Default)]
struct Sec {
    area: f64, length: f64, volume: f64,
    pressure: f64, pressure_rate: f64,
    l: f64, c: f64, r: [f64; 2],
    mw: f64, bw: f64, kw: f64,
    wall_current: f64, wall_current_rate: f64, wall_current_rate2: f64,
    alpha: f64, beta: f64,
    d: f64, e: f64,
    cur_in: Option<usize>,
    cur_out: [Option<usize>; 2],
}

#[derive(Clone, Default)]
struct Cur {
    source: Option<usize>,
    target: Option<usize>,
    mag: f64,
    rate: f64,
}

pub struct Tds {
    dt: f64,
    sec: Vec<Sec>,
    cur: Vec<Cur>,
    /// Dense storage, but only the symbolic pattern is ever read or written.
    matrix: Vec<f64>,
    rhs: Vec<f64>,
    fact: Vec<f64>,
    flow: Vec<f64>,
    row_lists: Vec<Vec<usize>>, // j < i with a structural non-zero at (i, j)
    col_lists: Vec<Vec<usize>>, // i > k with a structural non-zero at (i, k)
    flow_source_amp: f64,
    flow_source_section: Option<usize>,
    pressure_source_amp: f64,
    pressure_source_section: Option<usize>,
}

const N: usize = NUM_SECTIONS + 2; // one inflow current per section + two radiation currents
const RES_CUR: usize = NUM_SECTIONS; // current through the radiation resistance
const IND_CUR: usize = NUM_SECTIONS + 1; // current through the radiation inductance

impl Tds {
    pub fn new(tube: &Tube, sample_rate: f64) -> Tds {
        assert_eq!(tube.sections.len(), NUM_SECTIONS);
        let mut sec = vec![Sec::default(); NUM_SECTIONS];
        for (i, s) in sec.iter_mut().enumerate() {
            s.cur_in = Some(i);
            s.cur_out = [if i + 1 < NUM_SECTIONS { Some(i + 1) } else { Some(RES_CUR) }, None];
        }
        sec[NUM_SECTIONS - 1].cur_out = [Some(RES_CUR), Some(IND_CUR)];

        let mut cur = vec![Cur::default(); N];
        for (i, c) in cur.iter_mut().enumerate().take(NUM_SECTIONS) {
            c.source = if i == 0 { None } else { Some(i - 1) };
            c.target = Some(i);
        }
        cur[RES_CUR] = Cur { source: Some(NUM_SECTIONS - 1), target: None, ..Cur::default() };
        cur[IND_CUR] = Cur { source: Some(NUM_SECTIONS - 1), target: None, ..Cur::default() };

        // Symbolic pattern: all currents touching one section node form a clique.
        let mut adj = vec![vec![false; N]; N];
        for s in &sec {
            let mut ids: Vec<usize> = s.cur_in.into_iter().chain(s.cur_out.iter().flatten().copied()).collect();
            ids.sort_unstable();
            for &a in &ids { for &b in &ids { if a != b { adj[a][b] = true; } } }
        }
        // Symbolic elimination in natural order (adds fill-in; none for this tree-like network).
        for k in 0..N {
            let higher: Vec<usize> = (k + 1..N).filter(|&j| adj[k][j]).collect();
            for &a in &higher { for &b in &higher { if a != b { adj[a][b] = true; } } }
        }
        let row_lists: Vec<Vec<usize>> = (0..N).map(|i| (0..i).filter(|&j| adj[i][j]).collect()).collect();
        let col_lists: Vec<Vec<usize>> = (0..N).map(|k| (k + 1..N).filter(|&i| adj[i][k]).collect()).collect();

        let mut t = Tds {
            dt: 1.0 / sample_rate, sec, cur,
            matrix: vec![0.0; N * N], rhs: vec![0.0; N], fact: vec![0.0; N * N], flow: vec![0.0; N],
            row_lists, col_lists,
            flow_source_amp: 0.0, flow_source_section: None,
            pressure_source_amp: 0.0, pressure_source_section: None,
        };
        t.set_tube(tube);
        t
    }

    /// Copies the geometry and wall properties of the tube into the simulation state.
    pub fn set_tube(&mut self, tube: &Tube) {
        for (s, t) in self.sec.iter_mut().zip(&tube.sections) {
            s.area = t.area_cm2.max(MIN_AREA_CM2);
            s.length = t.length_cm;
            s.volume = s.area * s.length;
            s.mw = t.wall_mass;
            s.bw = t.wall_resistance;
            s.kw = t.wall_stiffness;
        }
    }

    pub fn reset(&mut self) {
        for s in &mut self.sec {
            s.pressure = 0.0; s.pressure_rate = 0.0;
            s.wall_current = 0.0; s.wall_current_rate = 0.0; s.wall_current_rate2 = 0.0;
        }
        for c in &mut self.cur { c.mag = 0.0; c.rate = 0.0; }
    }

    /// Volume velocity (cm^3/s) injected into `section` (added to its continuity equation).
    pub fn set_flow_source(&mut self, flow_cm3_s: f64, section: usize) {
        self.flow_source_amp = flow_cm3_s;
        self.flow_source_section = Some(section);
    }

    /// Static pressure source (dPa) at the inlet of `section`, e.g. the lung pressure at the
    /// first tracheal section. 1 cmH2O = 980.665 dPa.
    pub fn set_pressure_source(&mut self, pressure_dpa: f64, section: usize) {
        self.pressure_source_amp = pressure_dpa;
        self.pressure_source_section = Some(section);
    }

    /// Advances one sample and returns the radiated volume velocity at the lips (cm^3/s).
    pub fn step(&mut self) -> f64 {
        self.prepare();
        self.calc_matrix();
        self.solve();
        self.update();
        self.cur[RES_CUR].mag + self.cur[IND_CUR].mag
    }

    pub fn section_pressure(&self, i: usize) -> f64 { self.sec[i].pressure }
    pub fn section_flow_in(&self, i: usize) -> f64 { self.cur[i].mag }

    // ----------------------------------------------------------------------------------

    fn prepare(&mut self) {
        let dt = self.dt;
        for s in &mut self.sec {
            if s.area < MIN_AREA_CM2 { s.area = MIN_AREA_CM2; }
            let circ = 2.0 * (s.area * PI).sqrt();
            let (mut a, mut b) = ((s.area / PI).sqrt(), (s.area / PI).sqrt());
            if a < MIN_RADIUS_CM {
                // Elliptical cross-section: much higher viscous resistance for narrow slits.
                a = MIN_RADIUS_CM;
                b = s.area / (PI * a);
            }
            s.l = AMBIENT_DENSITY * 0.5 * s.length / s.area;
            s.c = s.volume / (AMBIENT_DENSITY * SOUND_VELOCITY * SOUND_VELOCITY);
            s.r[0] = (2.0 * AIR_VISCOSITY * s.length * (a * a + b * b)) / (PI * a * a * a * b * b * b);
            s.r[1] = s.r[0];
            s.alpha = 0.0;
            s.beta = 0.0;
            let _ = circ;
        }
        // Soft walls (not for the glottal sections).
        for i in 0..NUM_SECTIONS {
            if i == LOWER_GLOTTIS || i == UPPER_GLOTTIS { continue; }
            let s = &mut self.sec[i];
            let surface = (2.0 * (s.area * PI).sqrt() * s.length).max(MIN_AREA_CM2);
            let rw = s.bw / surface;
            let lw = s.mw / surface;
            let cw = surface / s.kw;
            let k1 = lw / (dt * dt * THETA * THETA) + rw / (dt * THETA);
            s.alpha = 1.0 / (k1 + 1.0 / cw);
            s.beta = s.alpha
                * (s.wall_current * k1
                    + s.wall_current_rate * (lw * (THETA1 / THETA + 1.0) / (dt * THETA) + rw * (THETA1 / THETA))
                    + s.wall_current_rate2 * lw * (THETA1 / THETA));
        }
        // Turbulence losses: Bernoulli resistance when the flow goes from a wide into a narrow
        // section; the kinetic pressure is lost completely on expansion.
        for i in (FIRST_PHARYNX + 1)..=LAST_MOUTH {
            let u = self.cur[i].mag;
            let (a_prev, a_cur) = (self.sec[i - 1].area, self.sec[i].area);
            if (a_cur < a_prev && u > 0.0) || (a_cur > a_prev && u < 0.0) {
                self.sec[i - 1].r[1] -= u * 0.5 * AMBIENT_DENSITY / (a_prev * a_prev);
                self.sec[i].r[0] += u * 0.5 * AMBIENT_DENSITY / (a_cur * a_cur);
            }
        }
        // Nonlinear resistances at the glottis: entrance (Bernoulli) and between glottal sections.
        let k_ent = 1.0;
        let u = self.cur[LOWER_GLOTTIS].mag;
        if u > 0.0 {
            let (sa, ta) = (self.sec[LAST_TRACHEA].area, self.sec[LOWER_GLOTTIS].area);
            self.sec[LOWER_GLOTTIS].r[0] += k_ent * 0.5 * AMBIENT_DENSITY * u.abs() * (1.0 / (ta * ta) - 1.0 / (sa * sa));
        }
        let u = self.cur[UPPER_GLOTTIS].mag;
        let (sa, ta) = (self.sec[LOWER_GLOTTIS].area, self.sec[UPPER_GLOTTIS].area);
        if u > 0.0 && ta < sa {
            self.sec[LOWER_GLOTTIS].r[1] += u.abs() * 0.5 * AMBIENT_DENSITY * (1.0 / (ta * ta) - 1.0 / (sa * sa));
        }
        // D and E of every section: P = D + E * (inflow - outflow).
        for (i, s) in self.sec.iter_mut().enumerate() {
            let mut src = 0.0;
            if Some(i) == self.flow_source_section { src += self.flow_source_amp; }
            let d = dt * THETA / (s.c + s.alpha);
            s.e = d;
            s.d = s.pressure + dt * THETA1 * s.pressure_rate - d * (s.beta - src);
        }
    }

    fn calc_matrix(&mut self) {
        let dt = self.dt;
        let dtt = dt * THETA;
        for i in 0..N {
            let (src, tgt) = (self.cur[i].source, self.cur[i].target);
            match tgt {
                None => {
                    // Radiation into free space: a resistance and an inductance in parallel.
                    let a = &self.sec[src.unwrap()];
                    let (u_r, u_l) = (self.cur[RES_CUR].mag, self.cur[IND_CUR].mag);
                    let (ur_rate, ul_rate) = (self.cur[RES_CUR].rate, self.cur[IND_CUR].rate);
                    let (l_a, r_a) = (a.l, a.r[1]);
                    let r_rad = (128.0 * AMBIENT_DENSITY * SOUND_VELOCITY) / (9.0 * PI * PI * a.area);
                    let l_rad = (8.0 * AMBIENT_DENSITY) / (3.0 * PI * (a.area * PI).sqrt());
                    let (f, g, h);
                    if i == RES_CUR {
                        f = l_a / dtt + r_a + r_rad;
                        g = l_a / dtt + r_a;
                        h = -(l_a / dtt) * (u_r + u_l) - l_a * (THETA1 / THETA) * (ur_rate + ul_rate);
                    } else {
                        let l_ab = l_a + l_rad;
                        f = l_a / dtt + r_a;
                        g = l_ab / dtt + r_a;
                        h = -(1.0 / dtt) * (l_a * u_r + l_ab * u_l) - (THETA1 / THETA) * (l_a * ur_rate + l_ab * ul_rate);
                    }
                    let (e, d, cin) = (a.e, a.d, a.cur_in);
                    if let Some(ci) = cin { self.matrix[i * N + ci] = e; }
                    self.matrix[i * N + RES_CUR] = -e - f;
                    self.matrix[i * N + IND_CUR] = -e - g;
                    self.rhs[i] = h - d;
                }
                Some(t) => {
                    let b = &self.sec[t];
                    let (l_b, r_b) = (b.l, b.r[0]);
                    let (mut l_a, mut r_a, mut e_a, mut d_a) = (0.0, 0.0, 0.0, 0.0);
                    let mut a_in = None;
                    if let Some(s) = src {
                        let a = &self.sec[s];
                        l_a = a.l; r_a = a.r[1]; e_a = a.e; d_a = a.d; a_in = a.cur_in;
                    }
                    let (l_ab, r_ab) = (l_a + l_b, r_a + r_b);
                    let mut s_amp = 0.0;
                    if Some(t) == self.pressure_source_section { s_amp -= self.pressure_source_amp; }
                    let (u, u_rate) = (self.cur[i].mag, self.cur[i].rate);
                    let g = l_ab / dtt + r_ab;
                    let h = -u_rate * l_ab * (THETA1 / THETA) - (l_ab * u) / dtt + s_amp;
                    if let Some(ci) = a_in { self.matrix[i * N + ci] = e_a; }
                    self.matrix[i * N + i] = -b.e - g - e_a;
                    for o in b.cur_out.iter().flatten() { self.matrix[i * N + *o] = b.e; }
                    self.rhs[i] = h + b.d - d_a;
                }
            }
        }
    }

    /// Solves A x = b with A = -matrix (symmetric positive definite) by sparse Cholesky.
    fn solve(&mut self) {
        let n = N;
        for i in 0..n {
            self.fact[i * n + i] = -self.matrix[i * n + i];
            for &j in &self.row_lists[i] { self.fact[i * n + j] = -self.matrix[i * n + j]; }
        }
        for k in 0..n {
            let mut diag = self.fact[k * n + k];
            for &j in &self.row_lists[k] { diag -= self.fact[k * n + j] * self.fact[k * n + j]; }
            let diag = diag.max(1e-300).sqrt();
            self.fact[k * n + k] = diag;
            for idx in 0..self.col_lists[k].len() {
                let i = self.col_lists[k][idx];
                let mut v = self.fact[i * n + k];
                for &j in &self.row_lists[k] { v -= self.fact[i * n + j] * self.fact[k * n + j]; }
                self.fact[i * n + k] = v / diag;
            }
        }
        // forward: L y = -rhs
        let mut y = [0.0f64; N];
        for k in 0..n {
            let mut v = -self.rhs[k];
            for &j in &self.row_lists[k] { v -= self.fact[k * n + j] * y[j]; }
            y[k] = v / self.fact[k * n + k];
        }
        // backward: L^T x = y
        for k in (0..n).rev() {
            let mut v = y[k];
            for &i in &self.col_lists[k] { v -= self.fact[i * n + k] * self.flow[i]; }
            self.flow[k] = v / self.fact[k * n + k];
        }
    }

    fn update(&mut self) {
        let dtt = self.dt * THETA;
        for i in 0..N {
            let old = self.cur[i].mag;
            self.cur[i].mag = self.flow[i];
            self.cur[i].rate = (self.cur[i].mag - old) / dtt - (THETA1 / THETA) * self.cur[i].rate;
        }
        for i in 0..NUM_SECTIONS {
            let net = self.cur[i].mag
                - self.sec[i].cur_out.iter().flatten().map(|&o| self.cur[o].mag).sum::<f64>();
            let s = &mut self.sec[i];
            let old_p = s.pressure;
            s.pressure = s.d + s.e * net;
            s.pressure_rate = (s.pressure - old_p) / dtt - s.pressure_rate * (THETA1 / THETA);
            let (old_w, old_wr) = (s.wall_current, s.wall_current_rate);
            s.wall_current = s.pressure_rate * s.alpha + s.beta;
            s.wall_current_rate = (s.wall_current - old_w) / dtt - old_wr * (THETA1 / THETA);
            s.wall_current_rate2 = (s.wall_current_rate - old_wr) / dtt - s.wall_current_rate2 * (THETA1 / THETA);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fft::fft;

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> f64 {
            self.0 ^= self.0 << 13; self.0 ^= self.0 >> 7; self.0 ^= self.0 << 17;
            (self.0 >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
        }
    }

    /// Welch estimate of |Y/X| between a white-noise flow source and the radiated flow.
    fn transfer_peaks(tube: &Tube) -> Vec<f64> {
        let fs = 44100.0;
        let mut tds = Tds::new(tube, fs);
        let mut rng = Rng(0x1234_5678_9ABC_DEF1);
        let total = 1 << 16;
        let (mut x, mut y) = (Vec::with_capacity(total), Vec::with_capacity(total));
        for _ in 0..total {
            let s = 100.0 * rng.next();
            tds.set_flow_source(s, FIRST_PHARYNX);
            y.push(tds.step());
            x.push(s);
        }
        let nfft = 4096;
        let mut pxx = vec![0.0; nfft / 2];
        let mut pxy = vec![0.0; nfft / 2];
        let mut start = 4096; // skip start-up
        while start + nfft <= total {
            let win = |i: usize| 0.5 - 0.5 * (2.0 * PI * i as f64 / nfft as f64).cos();
            let (mut xr, mut xi) = ((0..nfft).map(|i| x[start + i] * win(i)).collect::<Vec<_>>(), vec![0.0; nfft]);
            let (mut yr, mut yi) = ((0..nfft).map(|i| y[start + i] * win(i)).collect::<Vec<_>>(), vec![0.0; nfft]);
            fft(&mut xr, &mut xi);
            fft(&mut yr, &mut yi);
            for k in 0..nfft / 2 {
                pxx[k] += xr[k] * xr[k] + xi[k] * xi[k];
                pxy[k] += (yr[k] * yr[k] + yi[k] * yi[k]).sqrt() * (xr[k] * xr[k] + xi[k] * xi[k]).sqrt();
            }
            start += nfft / 2;
        }
        let h: Vec<f64> = (0..nfft / 2).map(|k| pxy[k] / pxx[k].max(1e-30)).collect();
        let bin = fs / nfft as f64;
        let mut peaks = vec![];
        for k in 3..nfft / 2 - 1 {
            if h[k] > h[k - 1] && h[k] >= h[k + 1] && k as f64 * bin > 200.0 && k as f64 * bin < 4500.0 {
                peaks.push(k as f64 * bin);
            }
        }
        peaks
    }

    #[test]
    fn uniform_tube_resonates_near_quarter_wave_formants() {
        let mut tube = Tube::default();
        tube.set_glottis_area(0.05);
        tube.set_uniform_supraglottal(16.9, 4.0); // + 0.6 cm of glottis ~ 17.5 cm
        let p = transfer_peaks(&tube);
        assert!(p.len() >= 3, "peaks: {p:?}");
        println!("peaks: {p:?}");
        // Closed-open tube of ~17.5 cm: 500 / 1500 / 2500 Hz, lowered by the lip end correction.
        assert!((430.0..=560.0).contains(&p[0]), "F1 {}", p[0]);
        assert!((1350.0..=1650.0).contains(&p[1]), "F2 {}", p[1]);
        assert!((2250.0..=2700.0).contains(&p[2]), "F3 {}", p[2]);
    }

    #[test]
    fn longer_tract_has_lower_formants() {
        let f1 = |len: f64| {
            let mut t = Tube::default();
            t.set_glottis_area(0.05);
            t.set_uniform_supraglottal(len, 4.0);
            transfer_peaks(&t)[0]
        };
        assert!(f1(14.0) > f1(16.9) && f1(16.9) > f1(20.0));
    }

    #[test]
    fn lung_pressure_drives_flow_through_open_glottis() {
        let mut tube = Tube::default();
        tube.set_glottis_area(0.1);
        tube.set_uniform_supraglottal(16.9, 4.0);
        let mut tds = Tds::new(&tube, 44100.0);
        tds.set_pressure_source(8.0 * 980.665, FIRST_TRACHEA);
        let mut flow = 0.0;
        for _ in 0..20000 { flow = tds.step(); }
        // Steady flow of a few hundred cm^3/s, as for a 0.1 cm^2 glottis at 8 cmH2O.
        assert!(flow > 50.0 && flow < 2000.0, "flow {flow}");
        // The flow is in the correct direction (outwards) and finite.
        assert!(tds.section_pressure(FIRST_PHARYNX).is_finite());
    }

    #[test]
    fn closed_glottis_blocks_flow() {
        let mut tube = Tube::default();
        tube.set_glottis_area(MIN_AREA_CM2);
        tube.set_uniform_supraglottal(16.9, 4.0);
        let mut tds = Tds::new(&tube, 44100.0);
        tds.set_pressure_source(8.0 * 980.665, FIRST_TRACHEA);
        let mut flow = 0.0;
        for _ in 0..20000 { flow = tds.step(); }
        assert!(flow.abs() < 1.0, "flow through closed glottis {flow}");
    }
}
