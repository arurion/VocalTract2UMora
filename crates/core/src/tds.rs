// SPDX-License-Identifier: GPL-3.0-or-later
//! Time-domain simulation (TDS) of vocal tract acoustics.
//!
//! A chain of tube sections is treated as an electrical network (inertance L, compliance C,
//! viscous/turbulent resistance R, soft walls). The unknowns are the branch currents (volume
//! velocities); the system is discretised with the theta method and solved every sample by a
//! sparse Cholesky factorisation. This is a port of the formulation used by VocalTractLab's
//! `TdsModel` (Birkholz et al.): VocalTractLab, (C) Peter Birkholz, GPL-3.0-or-later.
//!
//! Subset implemented here: oral chain with lip radiation and turbulence noise sources
//! (aspiration, fricatives, bursts). Not yet: nasal cavity, sinuses, piriform fossa, radiation
//! from the skin, transvelar coupling.

use crate::tube::*;
use std::f64::consts::PI;

pub const AMBIENT_DENSITY: f64 = 1.14e-3; // g/cm^3
pub const SOUND_VELOCITY: f64 = 3.5e4; // cm/s
pub const AIR_VISCOSITY: f64 = 1.86e-4; // dyne s/cm^2
/// With theta = 0.505 the TDS bandwidths match the finite-difference solution up to ~5 kHz.
const THETA: f64 = 0.505;
const THETA1: f64 = 1.0 - THETA;
const MIN_RADIUS_CM: f64 = 0.6;
/// Cutoff of the low-pass filter for the flow that drives friction noise.
const NOISE_CUTOFF_FREQ: f64 = 500.0;
const NOISE_BUF: usize = 8;

/// One lumped noise source: a pressure source with a spectrally shaped random signal.
#[derive(Clone, Default)]
struct NoiseSource {
    first_order: bool,
    cutoff: f64,
    target_amp: f64,
    current_amp: f64,
    in_buf: [f64; NOISE_BUF],
    out_buf: [f64; NOISE_BUF],
    sample: f64,
}

#[derive(Clone, Copy)]
struct Cons {
    first: usize,
    last: usize,
    narrowest: usize,
    obstacle_pos: f64,
    area: f64,
    flow: f64,
    velocity: f64,
    cutoff: f64,
    gain: f64,
    full_amp: f64,
    articulator: Articulator,
}

impl Cons {
    fn new(articulator: Articulator) -> Cons {
        Cons { first: 0, last: 0, narrowest: 0, obstacle_pos: 0.0, area: 0.0, flow: 0.0, velocity: 0.0, cutoff: 0.0, gain: 0.0, full_amp: 0.0, articulator }
    }
}

fn next_uniform(rng: &mut u64) -> f64 {
    *rng ^= *rng << 13;
    *rng ^= *rng >> 7;
    *rng ^= *rng << 17;
    ((*rng >> 11) as f64 + 0.5) / (1u64 << 53) as f64
}

/// Gaussian noise with standard deviation 1/sqrt(12), limited to [-1, 1] (as in VTL).
fn next_noise(rng: &mut u64) -> f64 {
    loop {
        let (u1, u2) = (next_uniform(rng), next_uniform(rng));
        let z = (-2.0 * u1.ln()).sqrt() * (2.0 * PI * u2).cos();
        let x = z / 12f64.sqrt();
        if (-1.0..=1.0).contains(&x) { return x; }
    }
}

/// Advances one noise source by one sample.
fn noise_sample(s: &mut NoiseSource, threshold: f64, position: u64, dt: f64, rng: &mut u64) {
    if s.target_amp > s.current_amp {
        // Rises slowly (3 ms time constant on sqrt(amp)), falls instantly: avoids frication
        // noise just before the implosion of plosives.
        let f = (-1.0 / (0.003 / dt)).exp();
        let (st, sc) = (s.target_amp.sqrt(), s.current_amp.sqrt());
        let sc = (1.0 - f) * st + f * sc;
        s.current_amp = sc * sc;
    } else {
        s.current_amp = s.target_amp;
    }
    if s.current_amp < threshold {
        // Gate the output only. VTL also resets `current_amp` to 0 here, which keeps weak
        // sources (target below ~18 at 44.1 kHz) from ever starting, because the first step of
        // the smoothed rise is already below the threshold. We let the rise continue instead.
        s.in_buf = [0.0; NOISE_BUF];
        s.out_buf = [0.0; NOISE_BUF];
        s.sample = 0.0;
        return;
    }
    let ratio = s.cutoff.max(1.0) * dt;
    let (a, b, order) = if s.first_order {
        let x = (-2.0 * PI * ratio).exp();
        ([1.0 - x, 0.0, 0.0], [0.0, x, 0.0], 1)
    } else {
        // Critically damped second-order low-pass (Q = 1/sqrt(2)).
        let q = 1.0 / 2f64.sqrt();
        let k = (PI * ratio).tan();
        let den = k * k + k / q + 1.0;
        let a0 = k * k / den;
        ([a0, 2.0 * a0, a0], [1.0, -2.0 * (k * k - 1.0) / den, -(k * k - k / q + 1.0) / den], 2)
    };
    // Constant gain-bandwidth product: louder for low cutoffs, quieter for high ones.
    let factor = 1000.0 / s.cutoff.max(1.0);
    let gain = s.current_amp * factor * factor.sqrt();
    let x = next_noise(rng);
    let idx = (position as usize) & (NOISE_BUF - 1);
    s.in_buf[idx] = x;
    let mut sum = a[0] * x;
    for k in 1..=order {
        let j = (position as usize).wrapping_sub(k) & (NOISE_BUF - 1);
        sum += a[k] * s.in_buf[j] + b[k] * s.out_buf[j];
    }
    s.out_buf[idx] = sum;
    s.sample = sum * gain;
}

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
    pos: f64,
    articulator: Articulator,
    dipole: NoiseSource,
}

#[derive(Clone, Default)]
struct Cur {
    source: Option<usize>,
    target: Option<usize>,
    mag: f64,
    rate: f64,
    /// The flow low-pass filtered at 500 Hz; drives the noise sources.
    noise_mag: f64,
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
    /// Generate turbulence noise sources (default on).
    pub generate_noise: bool,
    lips_dipole: NoiseSource,
    teeth_position: f64,
    aspiration_db: f64,
    tongue_tip_elevation: f64,
    position: u64,
    rng: u64,
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
            generate_noise: true, lips_dipole: NoiseSource::default(),
            teeth_position: 15.0, aspiration_db: -40.0, tongue_tip_elevation: 0.0,
            position: 0, rng: 0x9E37_79B9_7F4A_7C15,
        };
        t.set_tube(tube);
        t
    }

    /// Copies the geometry and wall properties of the tube into the simulation state.
    pub fn set_tube(&mut self, tube: &Tube) {
        let pos = tube.positions();
        self.teeth_position = tube.teeth_position_cm;
        self.aspiration_db = tube.aspiration_strength_db;
        self.tongue_tip_elevation = tube.tongue_tip_side_elevation;
        for (i, (s, t)) in self.sec.iter_mut().zip(&tube.sections).enumerate() {
            s.pos = pos[i];
            s.articulator = t.articulator;
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
        for c in &mut self.cur { c.mag = 0.0; c.rate = 0.0; c.noise_mag = 0.0; }
        for s in &mut self.sec { s.dipole = NoiseSource::default(); }
        self.lips_dipole = NoiseSource::default();
    }

    /// Seeds the noise generator (renders are reproducible for a given seed).
    pub fn set_noise_seed(&mut self, seed: u64) {
        self.rng = seed | 1;
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
        self.position += 1;
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
        if self.generate_noise { self.calc_noise_sources(); }
        // D and E of every section: P = D + E * (inflow - outflow).
        for (i, s) in self.sec.iter_mut().enumerate() {
            let mut src = 0.0;
            if Some(i) == self.flow_source_section { src += self.flow_source_amp; }
            let d = dt * THETA / (s.c + s.alpha);
            s.e = d;
            s.d = s.pressure + dt * THETA1 * s.pressure_rate - d * (s.beta - src);
        }
    }

    /// Finds the constrictions (glottis, tongue, lower lip) and places the dipole noise sources.
    fn calc_noise_sources(&mut self) {
        const MAX_CONSTRICTION_AREA: f64 = 1.0;
        const MAX_DELTA_AREA: f64 = 0.2;
        const MAX_TEETH_DISTANCE: f64 = 2.0;
        for s in &mut self.sec { s.dipole.target_amp = 0.0; }
        self.lips_dipole.target_amp = 0.0;

        let teeth = self.teeth_position;
        let area = |sec: &Vec<Sec>, i: usize| sec[i].area;
        let mut cons: Vec<Cons> = Vec::with_capacity(4);

        // The aspiration source right above the glottis is always present.
        let mut c0 = Cons::new(Articulator::VocalFolds);
        c0.first = LOWER_GLOTTIS;
        c0.last = UPPER_GLOTTIS;
        c0.narrowest = UPPER_GLOTTIS;
        c0.obstacle_pos = 1.5;
        cons.push(c0);

        // Extends [narrowest, narrowest] while the area stays within MAX_DELTA_AREA and the
        // articulator matches, then trims by one section at each end (as VTL does).
        let grow = |sec: &Vec<Sec>, narrowest: usize, art: Articulator, lo: usize, hi: usize| -> (usize, usize) {
            let max_area = sec[narrowest].area + MAX_DELTA_AREA;
            let (mut f, mut l) = (narrowest, narrowest);
            while sec[f].area < max_area && sec[f].articulator == art && f > lo { f -= 1; }
            while sec[l].area < max_area && sec[l].articulator == art && l < hi { l += 1; }
            (f, l)
        };
        let obstacle_for = |sec: &Vec<Sec>, c: &Cons, min_for_teeth: &mut f64| -> f64 {
            let jet_pos = sec[c.last].pos + sec[c.last].length;
            if teeth - jet_pos < MAX_TEETH_DISTANCE {
                *min_for_teeth = sec[c.narrowest].area;
                teeth // /s, sh/
            } else {
                let n = (c.last + 1).min(NUM_SECTIONS - 1);
                sec[n].pos + 0.5 * sec[n].length // /x, ch/
            }
        };

        let mut min_tongue_for_teeth = 1.0e6;
        // Most constricted tongue section.
        let mut min_area = 1.0e6;
        let mut min_sec = None;
        for i in FIRST_PHARYNX..=LAST_MOUTH {
            if self.sec[i].articulator == Articulator::Tongue && self.sec[i].area < min_area {
                min_area = self.sec[i].area;
                min_sec = Some(i);
            }
        }
        if let (true, Some(ms)) = (min_area < MAX_CONSTRICTION_AREA, min_sec) {
            let mut c = Cons::new(Articulator::Tongue);
            c.narrowest = ms;
            let (f, l) = grow(&self.sec, ms, Articulator::Tongue, FIRST_PHARYNX, LAST_MOUTH);
            c.first = f + 1;
            c.last = l.saturating_sub(1);
            c.obstacle_pos = obstacle_for(&self.sec, &c, &mut min_tongue_for_teeth);
            cons.push(c);
        }
        // A possible second constriction in the tongue region.
        if cons.last().map(|c| c.articulator) == Some(Articulator::Tongue) {
            let prev = *cons.last().unwrap();
            let mut min_area = 1.0e6;
            let mut min_sec = None;
            for i in FIRST_PHARYNX..=LAST_MOUTH {
                if self.sec[i].articulator == Articulator::Tongue && self.sec[i].area < min_area && (i < prev.first || i > prev.last) {
                    min_area = self.sec[i].area;
                    min_sec = Some(i);
                }
            }
            if let (true, Some(ms)) = (min_area < MAX_CONSTRICTION_AREA, min_sec) {
                let mut c = Cons::new(Articulator::Tongue);
                c.narrowest = ms;
                let (f, l) = grow(&self.sec, ms, Articulator::Tongue, FIRST_PHARYNX, LAST_MOUTH);
                c.first = f + 1;
                c.last = l.saturating_sub(1);
                if c.first > prev.last + 1 || c.last + 1 < prev.first {
                    c.obstacle_pos = obstacle_for(&self.sec, &c, &mut min_tongue_for_teeth);
                    cons.push(c);
                }
            }
        }
        // Most constricted section at the lower lip.
        let mut min_lip = 1.0e6;
        let mut lip_sec = None;
        for i in FIRST_MOUTH..=LAST_MOUTH {
            if self.sec[i].articulator == Articulator::LowerLip && self.sec[i].area < min_lip {
                min_lip = self.sec[i].area;
                lip_sec = Some(i);
            }
        }
        if let (true, Some(ls)) = (min_lip < MAX_CONSTRICTION_AREA && min_lip < min_tongue_for_teeth, lip_sec) {
            let mut c = Cons::new(Articulator::LowerLip);
            c.narrowest = ls;
            let max_area = min_lip + MAX_DELTA_AREA;
            let (f, mut l) = grow(&self.sec, ls, Articulator::LowerLip, FIRST_MOUTH, LAST_MOUTH);
            c.first = f + 1;
            if self.sec[l].area >= max_area { l -= 1; }
            c.last = l;
            c.obstacle_pos = 0.5 * (self.sec[c.first].pos + self.sec[c.last].pos + self.sec[c.last].length);
            cons.push(c);
        }

        // Strength, spectrum and position of the dipole sources of every constriction.
        for c in cons.iter_mut() {
            let ts = &self.sec[c.narrowest];
            c.area = ts.area.max(MIN_AREA_CM2);
            let mut flow = 0.0;
            for o in ts.cur_out.iter().flatten() { flow += self.cur[*o].noise_mag; }
            c.flow = flow.max(0.0); // only outgoing flow makes noise (avoids clicks)
            c.velocity = c.flow / c.area;
            c.cutoff = 1000.0;
            c.gain = 0.0;
            match c.articulator {
                Articulator::LowerLip => {
                    let (gain_f, cutoff_f) = (2.0e-6, 8000.0); // /f/: flat spectrum
                    let (gain_p, cutoff_p) = (0.5e-6, 500.0); // /p, b/ burst: low-frequency
                    let mut area_at_teeth = 1.0e6;
                    for i in FIRST_MOUTH..=LAST_MOUTH {
                        if teeth >= self.sec[i].pos && teeth <= self.sec[i].pos + self.sec[i].length {
                            area_at_teeth = area(&self.sec, i);
                        }
                    }
                    let d = (-(area_at_teeth - 0.15) / 0.10).exp().min(1.0);
                    c.gain = d * gain_f + (1.0 - d) * gain_p;
                    c.cutoff = d * cutoff_f + (1.0 - d) * cutoff_p;
                }
                Articulator::VocalFolds => {
                    c.gain = 1.0e-6 * 10f64.powf(self.aspiration_db / 20.0);
                    c.cutoff = 10000.0; // flat aspiration spectrum
                }
                _ => {
                    if (c.obstacle_pos - teeth).abs() < 0.0001 {
                        // /s, sh, c, d, t/: fricatives are high-frequency, bursts lower.
                        let (gain_fr, cutoff_fr) = (1.0e-6, 5000.0);
                        let (gain_pl, cutoff_pl) = (0.8e-6, 1500.0);
                        let d = self.tongue_tip_elevation.clamp(0.0, 1.0);
                        c.gain = d * gain_fr + (1.0 - d) * gain_pl;
                        c.cutoff = d * cutoff_fr + (1.0 - d) * cutoff_pl;
                        if self.tongue_tip_elevation < 0.0 { c.gain = 0.0; } // lateral passage
                    } else {
                        c.gain = 0.4e-6; // wall source for /x, k/
                        c.cutoff = 800.0;
                    }
                }
            }
            // Taper to zero between 0.5 and 1.0 cm^2 and below 0.15 cm^2 (prevents clicks).
            if c.area >= 0.5 { c.gain *= 1.0 - (c.area - 0.5) / (MAX_CONSTRICTION_AREA - 0.5); }
            if c.area < 0.15 { c.gain *= c.area / 0.15; }
            c.full_amp = c.gain * c.velocity.abs() * c.velocity * c.velocity * c.area.sqrt();
            c.cutoff = c.cutoff.clamp(1.0, 10000.0);

            // Put the source at the obstacle: split between the two ends of its section.
            let last = &self.sec[LAST_MOUTH];
            if c.obstacle_pos >= last.pos + last.length { c.obstacle_pos = last.pos + 0.99 * last.length; }
            let obstacle = (FIRST_PHARYNX..=LAST_MOUTH)
                .find(|&i| self.sec[i].pos <= c.obstacle_pos && self.sec[i].pos + self.sec[i].length >= c.obstacle_pos);
            if let Some(o) = obstacle {
                let down = (c.obstacle_pos - self.sec[o].pos) / self.sec[o].length;
                let up = 1.0 - down;
                let (cutoff, full) = (c.cutoff, c.full_amp);
                let s = &mut self.sec[o].dipole;
                s.target_amp = up * full; s.first_order = false; s.cutoff = cutoff;
                let s = if o < LAST_MOUTH { &mut self.sec[o + 1].dipole } else { &mut self.lips_dipole };
                s.target_amp = down * full; s.first_order = false; s.cutoff = cutoff;
            }
        }

        // New noise samples at the source positions.
        let (pos, dt) = (self.position, self.dt);
        for i in FIRST_PHARYNX..=LAST_MOUTH {
            noise_sample(&mut self.sec[i].dipole, 0.001, pos, dt, &mut self.rng);
        }
        noise_sample(&mut self.lips_dipole, 0.001, pos, dt, &mut self.rng);
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
                    self.rhs[i] = h - self.lips_dipole.sample - d;
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
                    s_amp -= b.dipole.sample;
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
        let nf = (-2.0 * PI * NOISE_CUTOFF_FREQ * self.dt).exp();
        for i in 0..N {
            let old = self.cur[i].mag;
            self.cur[i].mag = self.flow[i];
            self.cur[i].noise_mag = (1.0 - nf) * self.cur[i].mag + nf * self.cur[i].noise_mag;
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

    /// Energy of the (differentiated, i.e. pressure-like) radiated flow in a frequency band.
    fn band_energy(y: &[f64], f_lo: f64, f_hi: f64, fs: f64) -> f64 {
        let n = 16384;
        let seg = &y[y.len() - n - 1..];
        let mut re: Vec<f64> = (0..n)
            .map(|i| (seg[i + 1] - seg[i]) * (0.5 - 0.5 * (2.0 * PI * i as f64 / n as f64).cos()))
            .collect();
        let mut im = vec![0.0; n];
        fft(&mut re, &mut im);
        let bin = fs / n as f64;
        (0..n / 2).filter(|&k| (k as f64 * bin) >= f_lo && (k as f64 * bin) <= f_hi).map(|k| re[k] * re[k] + im[k] * im[k]).sum()
    }

    fn run_noise(tube: &Tube, pressure_cmh2o: f64, seed: u64, samples: usize) -> Vec<f64> {
        let mut tds = Tds::new(tube, 44100.0);
        tds.set_noise_seed(seed);
        tds.set_pressure_source(pressure_cmh2o * 980.665, FIRST_TRACHEA);
        (0..samples).map(|_| tds.step()).collect()
    }

    fn sibilant_tube(narrow: f64) -> Tube {
        let mut t = Tube::default();
        t.set_glottis_area(0.3);
        t.set_uniform_supraglottal(16.9, 4.0);
        let mut tags = vec![Articulator::Other; NUM_SUPRAGLOTTAL];
        let mut areas = vec![4.0; NUM_SUPRAGLOTTAL];
        for (k, a) in [(32, narrow + 0.08), (33, narrow), (34, narrow + 0.08)] {
            tags[k] = Articulator::Tongue;
            areas[k] = a;
        }
        let lens = vec![16.9 / NUM_SUPRAGLOTTAL as f64; NUM_SUPRAGLOTTAL];
        t.set_supraglottal(&lens, &areas);
        t.set_articulators(&tags);
        t.tongue_tip_side_elevation = 1.0; // fricative
        t
    }

    #[test]
    fn tongue_constriction_at_the_teeth_makes_high_frequency_noise() {
        let with = run_noise(&sibilant_tube(0.12), 8.0, 7, 30000);
        let mut open = Tube::default();
        open.set_glottis_area(0.3);
        open.set_uniform_supraglottal(16.9, 4.0); // fully open tube: aspiration only
        let without = run_noise(&open, 8.0, 7, 30000);
        let e_with = band_energy(&with, 3000.0, 8000.0, 44100.0);
        let e_without = band_energy(&without, 3000.0, 8000.0, 44100.0);
        println!("3-8 kHz energy: with constriction {e_with:.3e}, without {e_without:.3e}");
        assert!(e_with > 10.0 * e_without, "{e_with} vs {e_without}");
    }

    #[test]
    fn aspiration_noise_follows_aspiration_strength() {
        let mk = |db: f64| {
            let mut t = Tube::default();
            t.set_glottis_area(0.3);
            t.set_uniform_supraglottal(16.9, 4.0);
            t.aspiration_strength_db = db;
            band_energy(&run_noise(&t, 8.0, 3, 30000), 500.0, 6000.0, 44100.0)
        };
        let (quiet, loud) = (mk(-40.0), mk(-20.0));
        println!("aspiration energy -40 dB: {quiet:.3e}, -20 dB: {loud:.3e}");
        assert!(loud > 20.0 * quiet, "{loud} vs {quiet}");
    }

    #[test]
    fn weak_aspiration_still_starts() {
        // -60 dB: the target amplitude is below the level at which VTL's gate would never open.
        let mut t = Tube::default();
        t.set_glottis_area(0.3);
        t.set_uniform_supraglottal(16.9, 4.0);
        t.aspiration_strength_db = -60.0;
        let e = band_energy(&run_noise(&t, 8.0, 3, 30000), 500.0, 6000.0, 44100.0);
        println!("aspiration energy at -60 dB: {e:.3e}");
        assert!(e > 100.0, "{e}");
    }

    #[test]
    fn no_flow_means_no_noise() {
        let y = run_noise(&sibilant_tube(0.12), 0.0, 5, 5000);
        assert!(y.iter().all(|v| v.abs() < 1e-9));
    }

    #[test]
    fn noise_is_reproducible_for_a_seed() {
        let t = sibilant_tube(0.12);
        let a = run_noise(&t, 8.0, 11, 6000);
        let b = run_noise(&t, 8.0, 11, 6000);
        let c = run_noise(&t, 8.0, 12, 6000);
        assert_eq!(a, b);
        assert_ne!(a, c);
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
