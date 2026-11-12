// SPDX-License-Identifier: GPL-3.0-or-later
//! Articulatory vocal tract model: 19 articulatory parameters -> 3D surfaces -> centre line ->
//! cross-sections -> area function. Port of VocalTractLab's `VocalTract`,
//! (C) Peter Birkholz, GPL-3.0-or-later. Validated stage by stage against the original
//! (see `docs/PORT_PROGRESS.md` and `tests/data/`).
//!
//! Coordinates (cm): x points forward (towards the lips), y up, z to the side; the sagittal
//! plane is z = 0 and the model is mirror-symmetric, so only one half (z <= 0) is stored.

use crate::geom::*;
use crate::speaker::{Anatomy, Speaker, NUM_PARAMS};
use crate::splines::{Bezier3D, LineStrip3D};
use crate::surface::Surface;
use std::f64::consts::PI;

// Grid sizes.
pub const NUM_EPIGLOTTIS_RIBS: usize = 4;
pub const NUM_EPIGLOTTIS_POINTS: usize = 5;
pub const NUM_UVULA_RIBS: usize = 4;
pub const NUM_UVULA_POINTS: usize = 5;
pub const NUM_LARYNX_RIBS: usize = 5;
pub const NUM_PHARYNX_RIBS: usize = 3;
pub const NUM_VELUM_RIBS: usize = 6;
pub const NUM_PALATE_RIBS: usize = 9;
pub const NUM_JAW_RIBS: usize = NUM_PALATE_RIBS;
pub const NUM_THROAT_RIBS: usize = NUM_PHARYNX_RIBS;
pub const NUM_UPPER_COVER_RIBS: usize = NUM_LARYNX_RIBS + NUM_PHARYNX_RIBS + NUM_VELUM_RIBS + NUM_JAW_RIBS;
pub const NUM_UPPER_COVER_POINTS: usize = 6;
pub const NUM_LOWER_COVER_RIBS: usize = NUM_LARYNX_RIBS + NUM_THROAT_RIBS + NUM_JAW_RIBS;
pub const NUM_LOWER_COVER_POINTS: usize = 5;
pub const NUM_DYNAMIC_TONGUE_RIBS: usize = 33;
pub const NUM_STATIC_TONGUE_RIBS: usize = 4;
pub const NUM_TONGUE_RIBS: usize = NUM_DYNAMIC_TONGUE_RIBS + NUM_STATIC_TONGUE_RIBS;
pub const NUM_TONGUE_POINTS: usize = 11;
pub const NUM_TEETH_RIBS: usize = 3 * (NUM_JAW_RIBS - 1) + 1;
pub const NUM_TEETH_POINTS: usize = 5;
pub const NUM_LIP_RIBS: usize = NUM_JAW_RIBS;
pub const NUM_INNER_LIP_POINTS: usize = 5;
pub const NUM_OUTER_LIP_POINTS: usize = 5;
pub const NUM_LIP_POINTS: usize = NUM_INNER_LIP_POINTS + NUM_OUTER_LIP_POINTS;
pub const NUM_FILL_RIBS: usize = NUM_JAW_RIBS + 3;
pub const NUM_FILL_POINTS: usize = 4;
pub const NUM_RADIATION_RIBS: usize = NUM_LIP_RIBS + 4;
pub const NUM_RADIATION_POINTS: usize = 6;

/// Surface indices (same order as the original `SurfaceIndex`).
pub mod si {
    pub const UPPER_TEETH: usize = 0;
    pub const LOWER_TEETH: usize = 1;
    pub const UPPER_COVER: usize = 2;
    pub const LOWER_COVER: usize = 3;
    pub const UPPER_LIP: usize = 4;
    pub const LOWER_LIP: usize = 5;
    pub const PALATE: usize = 6;
    pub const MANDIBLE: usize = 7;
    pub const LOWER_TEETH_ORIGINAL: usize = 8;
    pub const LOW_VELUM: usize = 9;
    pub const MID_VELUM: usize = 10;
    pub const HIGH_VELUM: usize = 11;
    pub const NARROW_LARYNX_FRONT: usize = 12;
    pub const NARROW_LARYNX_BACK: usize = 13;
    pub const WIDE_LARYNX_FRONT: usize = 14;
    pub const WIDE_LARYNX_BACK: usize = 15;
    pub const TONGUE: usize = 16;
    pub const UPPER_COVER_TWOSIDE: usize = 17;
    pub const LOWER_COVER_TWOSIDE: usize = 18;
    pub const UPPER_TEETH_TWOSIDE: usize = 19;
    pub const LOWER_TEETH_TWOSIDE: usize = 20;
    pub const UPPER_LIP_TWOSIDE: usize = 21;
    pub const LOWER_LIP_TWOSIDE: usize = 22;
    pub const LEFT_COVER: usize = 23;
    pub const RIGHT_COVER: usize = 24;
    pub const UVULA_ORIGINAL: usize = 25;
    pub const UVULA: usize = 26;
    pub const UVULA_TWOSIDE: usize = 27;
    pub const EPIGLOTTIS_ORIGINAL: usize = 28;
    pub const EPIGLOTTIS: usize = 29;
    pub const EPIGLOTTIS_TWOSIDE: usize = 30;
    pub const RADIATION: usize = 31;
    pub const NUM_SURFACES: usize = 32;
}

pub struct VocalTract {
    pub anatomy: Anatomy,
    pub palate_points: Vec<P3>,
    pub jaw_points: Vec<P3>,
    pub surface: Vec<Surface>,
    pub upper_gums_inner_edge: [P3; NUM_JAW_RIBS],
    pub upper_gums_outer_edge: [P3; NUM_JAW_RIBS],
    pub lower_gums_inner_edge_orig: [P3; NUM_JAW_RIBS],
    pub lower_gums_outer_edge_orig: [P3; NUM_JAW_RIBS],
    pub wide_lip_corner_path: LineStrip3D,
    pub narrow_lip_corner_path: LineStrip3D,
}

impl VocalTract {
    /// Builds the static (articulation-independent) geometry of a speaker.
    pub fn from_speaker(speaker: &Speaker) -> VocalTract {
        let anatomy = speaker.anatomy.clone();
        let palate_points = anatomy.palate.iter().map(|r| p3(r.x, 0.0, r.z)).collect();
        let jaw_points = anatomy.jaw.iter().map(|r| p3(r.x, 0.0, r.z)).collect();
        let mut vt = VocalTract {
            anatomy,
            palate_points,
            jaw_points,
            surface: Vec::new(),
            upper_gums_inner_edge: [P3::default(); NUM_JAW_RIBS],
            upper_gums_outer_edge: [P3::default(); NUM_JAW_RIBS],
            lower_gums_inner_edge_orig: [P3::default(); NUM_JAW_RIBS],
            lower_gums_outer_edge_orig: [P3::default(); NUM_JAW_RIBS],
            wide_lip_corner_path: LineStrip3D::new(),
            narrow_lip_corner_path: LineStrip3D::new(),
        };
        vt.init_surface_grids();
        vt.init_larynx();
        vt.init_jaws();
        vt.init_velum();
        vt
    }

    fn init_surface_grids(&mut self) {
        use si::*;
        let mut s = vec![Surface::default(); NUM_SURFACES];
        let mut init = |i: usize, r: usize, p: usize| s[i] = Surface::new(r, p);
        init(UPPER_COVER, NUM_UPPER_COVER_RIBS, NUM_UPPER_COVER_POINTS);
        init(LOWER_COVER, NUM_LOWER_COVER_RIBS, NUM_LOWER_COVER_POINTS);
        init(UPPER_TEETH, NUM_TEETH_RIBS, NUM_TEETH_POINTS);
        init(LOWER_TEETH, NUM_TEETH_RIBS, NUM_TEETH_POINTS);
        init(UPPER_LIP, NUM_LIP_RIBS, NUM_LIP_POINTS);
        init(LOWER_LIP, NUM_LIP_RIBS, NUM_LIP_POINTS);
        init(TONGUE, NUM_TONGUE_RIBS, NUM_TONGUE_POINTS);
        init(LEFT_COVER, NUM_FILL_RIBS, NUM_FILL_POINTS);
        init(RIGHT_COVER, NUM_FILL_RIBS, NUM_FILL_POINTS);
        init(EPIGLOTTIS, NUM_EPIGLOTTIS_RIBS, NUM_EPIGLOTTIS_POINTS);
        init(UVULA, NUM_UVULA_RIBS, NUM_UVULA_POINTS);
        init(RADIATION, NUM_RADIATION_RIBS, NUM_RADIATION_POINTS);
        init(UPPER_COVER_TWOSIDE, NUM_UPPER_COVER_RIBS, NUM_UPPER_COVER_POINTS * 2 - 1);
        init(LOWER_COVER_TWOSIDE, NUM_LOWER_COVER_RIBS, NUM_LOWER_COVER_POINTS * 2 - 1);
        init(UPPER_TEETH_TWOSIDE, NUM_TEETH_RIBS * 2 - 1, NUM_TEETH_POINTS);
        init(LOWER_TEETH_TWOSIDE, NUM_TEETH_RIBS * 2 - 1, NUM_TEETH_POINTS);
        init(UPPER_LIP_TWOSIDE, NUM_LIP_RIBS * 2 - 1, NUM_LIP_POINTS);
        init(LOWER_LIP_TWOSIDE, NUM_LIP_RIBS * 2 - 1, NUM_LIP_POINTS);
        init(EPIGLOTTIS_TWOSIDE, NUM_EPIGLOTTIS_RIBS, NUM_EPIGLOTTIS_POINTS * 2 - 1);
        init(UVULA_TWOSIDE, NUM_UVULA_RIBS, NUM_UVULA_POINTS * 2 - 1);
        init(NARROW_LARYNX_FRONT, NUM_LARYNX_RIBS, NUM_LOWER_COVER_POINTS);
        init(NARROW_LARYNX_BACK, NUM_LARYNX_RIBS, NUM_UPPER_COVER_POINTS);
        init(WIDE_LARYNX_FRONT, NUM_LARYNX_RIBS, NUM_LOWER_COVER_POINTS);
        init(WIDE_LARYNX_BACK, NUM_LARYNX_RIBS, NUM_UPPER_COVER_POINTS);
        init(LOWER_TEETH_ORIGINAL, NUM_TEETH_RIBS, NUM_TEETH_POINTS);
        init(LOW_VELUM, NUM_VELUM_RIBS, NUM_UPPER_COVER_POINTS);
        init(MID_VELUM, NUM_VELUM_RIBS, NUM_UPPER_COVER_POINTS);
        init(HIGH_VELUM, NUM_VELUM_RIBS, NUM_UPPER_COVER_POINTS);
        init(PALATE, NUM_JAW_RIBS, NUM_UPPER_COVER_POINTS);
        init(MANDIBLE, NUM_JAW_RIBS, NUM_LOWER_COVER_POINTS);
        init(EPIGLOTTIS_ORIGINAL, NUM_EPIGLOTTIS_RIBS, NUM_EPIGLOTTIS_POINTS);
        init(UVULA_ORIGINAL, NUM_UVULA_RIBS, NUM_UVULA_POINTS);
        for i in [LOWER_COVER, UPPER_TEETH, UPPER_LIP, TONGUE, LEFT_COVER, EPIGLOTTIS, LOWER_COVER_TWOSIDE,
                  UPPER_TEETH_TWOSIDE, UPPER_LIP_TWOSIDE, EPIGLOTTIS_TWOSIDE] {
            s[i].swap_triangle_orientation();
        }
        self.surface = s;
    }

    /// x position of the back wall of the pharynx at height `y`.
    pub fn get_pharynx_back_x(&self, y: f64) -> f64 {
        const MIN_ANGLE_DEG: f64 = -135.0;
        const MAX_ANGLE_DEG: f64 = -45.0;
        let a = self.anatomy.pharynx_fulcrum;
        let mut angle_deg = self.anatomy.pharynx_rotation_angle_deg;
        if angle_deg > 0.0 { angle_deg -= 2.0 * PI; } // (sic) as in the original
        let angle_deg = angle_deg.clamp(MIN_ANGLE_DEG, MAX_ANGLE_DEG);
        let angle_rad = angle_deg * PI / 180.0;
        a[0] + (y - a[1]) * angle_rad.cos() / angle_rad.sin()
    }

    fn init_larynx(&mut self) {
        const EPS: f64 = 0.000001;
        // Enforce the conditions on the larynx outline (as the original does at run time).
        for k in 0..2 {
            let l = if k == 0 { &mut self.anatomy.larynx_narrow } else { &mut self.anatomy.larynx_wide };
            if l[0][1] != l[7][1] { l[7][1] = l[0][1]; }
            if l[2][1] != l[5][1] { l[5][1] = l[2][1]; }
            if l[3][1] != l[4][1] { l[4][1] = l[3][1]; }
            if l[2][1] < l[3][1] { l[2][1] = l[3][1]; l[5][1] = l[3][1]; }
            if l[6][1] < l[5][1] { l[6][1] = l[5][1]; }
            if l[1][1] < l[6][1] { l[1][1] = l[6][1]; }
            if l[0][1] < l[1][1] { l[0][1] = l[1][1]; l[7][1] = l[1][1]; }
            if l[7][0] > l[0][0] { l[7][0] = l[0][0]; }
            if l[5][0] > l[2][0] { l[5][0] = l[2][0]; }
            if l[4][0] > l[3][0] { l[4][0] = l[3][0]; }
        }
        let w = [1.0, 0.7, 1.0];
        let mut curve = Bezier3D::new();
        for k in 0..2 {
            let l = if k == 0 { self.anatomy.larynx_narrow.clone() } else { self.anatomy.larynx_wide.clone() };
            let mut y = [0.0; NUM_LARYNX_RIBS];
            let mut xl = [0.0; NUM_LARYNX_RIBS];
            let mut xr = [0.0; NUM_LARYNX_RIBS];
            let mut xm = [0.0; NUM_LARYNX_RIBS];
            let mut zm = [0.0; NUM_LARYNX_RIBS];
            let a = &self.anatomy;
            y[0] = l[3][1]; xl[0] = l[4][0]; xr[0] = l[3][0]; xm[0] = 0.5 * (xl[0] + xr[0]); zm[0] = -0.5 * a.larynx_lower_depth;
            y[1] = l[2][1]; xl[1] = l[5][0]; xr[1] = l[2][0]; xm[1] = 0.5 * (xl[1] + xr[1]); zm[1] = -0.5 * a.larynx_upper_depth;
            y[2] = l[6][1]; xl[2] = l[6][0];
            let mut d = l[1][1] - l[2][1];
            if d < EPS { d = EPS; }
            xr[2] = l[2][0] + (l[1][0] - l[2][0]) * (l[6][1] - l[2][1]) / d;
            xm[2] = 0.25 * xl[2] + 0.75 * xr[2];
            zm[2] = -0.5 * a.pharynx_lower_depth;
            y[3] = l[1][1];
            d = l[7][1] - l[6][1];
            if d < EPS { d = EPS; }
            xl[3] = l[6][0] + (l[7][0] - l[6][0]) * (l[1][1] - l[6][1]) / d;
            xr[3] = l[1][0];
            xm[3] = 0.2 * xl[3] + 0.8 * xr[3];
            zm[3] = -0.5 * a.pharynx_lower_depth;
            y[4] = l[0][1]; xl[4] = l[7][0]; xr[4] = l[0][0]; xm[4] = 0.5 * (xl[4] + xr[4]); zm[4] = -0.5 * a.pharynx_lower_depth;

            let (back, front) = if k == 0 { (si::NARROW_LARYNX_BACK, si::NARROW_LARYNX_FRONT) } else { (si::WIDE_LARYNX_BACK, si::WIDE_LARYNX_FRONT) };
            for i in 0..NUM_LARYNX_RIBS {
                let c = [p3(xm[i], y[i], zm[i]), p3(xl[i], y[i], zm[i]), p3(xl[i], y[i], 0.0)];
                curve.set_points_weighted(&c, &w);
                for j in 0..NUM_UPPER_COVER_POINTS {
                    let d = curve.uniform_param(j as f64 / (NUM_UPPER_COVER_POINTS - 1) as f64);
                    let q = curve.get_point(d);
                    self.surface[back].set_vertex(i, j, q);
                }
                let c = [p3(xm[i], y[i], zm[i]), p3(xr[i], y[i], zm[i]), p3(xr[i], y[i], 0.0)];
                curve.set_points_weighted(&c, &w);
                for j in 0..NUM_LOWER_COVER_POINTS {
                    let d = curve.uniform_param(j as f64 / (NUM_LOWER_COVER_POINTS - 1) as f64);
                    let q = curve.get_point(d);
                    self.surface[front].set_vertex(i, j, q);
                }
            }
        }
        // Epiglottis (original shape, before it is moved with the larynx).
        let (width, height, depth) = (self.anatomy.epiglottis_width, self.anatomy.epiglottis_height, self.anatomy.epiglottis_depth);
        let mut pp = [
            p3(0.0, 0.0, 0.0),
            p3(-0.25 * width, 0.0, -0.75 * 0.5 * depth),
            p3(-0.5 * width, 0.0, -0.5 * depth),
            p3(-width, 0.0, -0.75 * 0.5 * depth),
            p3(-width, 0.0, 0.0),
        ];
        for q in pp.iter_mut() {
            q.y = 0.0;
        }
        for (i, q) in pp.iter_mut().enumerate() {
            q.y = 0.0;
            self.surface[si::EPIGLOTTIS_ORIGINAL].set_vertex(0, i, *q);
            q.y = 0.25 * height;
            self.surface[si::EPIGLOTTIS_ORIGINAL].set_vertex(1, i, *q);
            q.y = 0.75 * height;
            self.surface[si::EPIGLOTTIS_ORIGINAL].set_vertex(2, i, *q);
        }
        let d = height;
        let top = [
            p3(-0.5 * width, d, 0.0),
            p3(-0.5 * width, d, -0.75 * 0.5 * depth),
            p3(-0.5 * width, d, -0.75 * 0.5 * depth),
            p3(-0.5 * width, d, -0.75 * 0.5 * depth),
            p3(-0.5 * width, d, 0.0),
        ];
        for (i, q) in top.iter().enumerate() {
            self.surface[si::EPIGLOTTIS_ORIGINAL].set_vertex(3, i, *q);
        }
    }

    fn init_jaws(&mut self) {
        const MIN_ANGLE_DEG: f64 = 0.000001;
        const MAX_ANGLE_DEG: f64 = 89.999999;
        const GROOVE_DEPTH: f64 = 0.15;
        let mut curve = Bezier3D::new();
        let w = [1.0, 1.0, 1.0];

        // Palate and mandible profiles.
        for i in 0..NUM_JAW_RIBS {
            let a = &self.anatomy;
            let c0 = self.palate_points[i];
            let height = a.palate[i].palate_height;
            let angle = a.palate[i].palate_angle_deg.clamp(MIN_ANGLE_DEG, MAX_ANGLE_DEG);
            let mut c1 = p3(c0.x, height, c0.z + height / (angle * PI / 180.0).tan());
            if c1.z > 0.0 { c1.z = 0.0; }
            let c2 = p3(c0.x, height, 0.0);
            curve.set_points_weighted(&[c0, c1, c2], &w);
            for j in 0..NUM_UPPER_COVER_POINTS {
                let u = curve.uniform_param(j as f64 / (NUM_UPPER_COVER_POINTS - 1) as f64);
                let q = curve.get_point(u);
                self.surface[si::PALATE].set_vertex(i, j, q);
            }

            let c0 = self.jaw_points[i];
            let height = a.jaw[i].jaw_height;
            let angle = a.jaw[i].jaw_angle_deg.clamp(MIN_ANGLE_DEG, MAX_ANGLE_DEG);
            let delta = (self.jaw_points[8].x - self.jaw_points[0].x) * 1.0 / 4.5;
            let bottom_x = if c0.x - self.jaw_points[0].x < delta { self.jaw_points[0].x + delta } else { c0.x };
            let mut c1 = p3(bottom_x, -height, c0.z + height / (angle * PI / 180.0).tan());
            if c1.z > 0.0 { c1.z = 0.0; }
            let c2 = p3(bottom_x, -height, 0.0);
            curve.set_points_weighted(&[c0, c1, c2], &w);
            for j in 0..NUM_LOWER_COVER_POINTS {
                let u = curve.uniform_param(j as f64 / (NUM_LOWER_COVER_POINTS - 1) as f64);
                let q = curve.get_point(u);
                self.surface[si::MANDIBLE].set_vertex(i, j, q);
            }
        }

        // Teeth: ribs at the palate / jaw points and two ribs between each pair.
        for upper in [true, false] {
            let pts = if upper { self.palate_points.clone() } else { self.jaw_points.clone() };
            let (h, ttop, tbot): (Vec<f64>, Vec<f64>, Vec<f64>) = if upper {
                (self.anatomy.palate.iter().map(|r| r.teeth_height).collect(),
                 self.anatomy.palate.iter().map(|r| r.teeth_width_top).collect(),
                 self.anatomy.palate.iter().map(|r| r.teeth_width_bottom).collect())
            } else {
                (self.anatomy.jaw.iter().map(|r| r.teeth_height).collect(),
                 self.anatomy.jaw.iter().map(|r| r.teeth_width_top).collect(),
                 self.anatomy.jaw.iter().map(|r| r.teeth_width_bottom).collect())
            };
            let mut th = [0.0; NUM_TEETH_RIBS];
            let mut ttw = [0.0; NUM_TEETH_RIBS];
            let mut tbw = [0.0; NUM_TEETH_RIBS];
            let mut tp = [P3::default(); NUM_TEETH_RIBS];
            let mut tn = [P3::default(); NUM_TEETH_RIBS];
            tp[0] = pts[0];
            tn[0] = p3(pts[1].z - pts[0].z, 0.0, -(pts[1].x - pts[0].x));
            tn[0].normalize();
            tp[NUM_TEETH_RIBS - 1] = pts[NUM_JAW_RIBS - 1];
            tn[NUM_TEETH_RIBS - 1] = p3(1.0, 0.0, 0.0);
            for i in 1..NUM_JAW_RIBS - 1 {
                let j = i * 3;
                tp[j] = pts[i];
                tn[j] = p3(pts[i + 1].z - pts[i - 1].z, 0.0, -(pts[i + 1].x - pts[i - 1].x));
                tn[j].normalize();
            }
            for i in 0..NUM_JAW_RIBS - 1 {
                let t = 0.75;
                th[i * 3 + 1] = h[i];
                ttw[i * 3 + 1] = t * ttop[i] + (1.0 - t) * ttop[i + 1];
                tbw[i * 3 + 1] = t * tbot[i] + (1.0 - t) * tbot[i + 1];
                tp[i * 3 + 1] = tp[i * 3] * t + tp[i * 3 + 3] * (1.0 - t);
                tn[i * 3 + 1] = tn[i * 3] * t + tn[i * 3 + 3] * (1.0 - t);
                tn[i * 3 + 1].normalize();
                th[i * 3 + 2] = h[i];
                ttw[i * 3 + 2] = (1.0 - t) * ttop[i] + t * ttop[i + 1];
                tbw[i * 3 + 2] = (1.0 - t) * tbot[i] + t * tbot[i + 1];
                tp[i * 3 + 2] = tp[i * 3] * (1.0 - t) + tp[i * 3 + 3] * t;
                tn[i * 3 + 2] = tn[i * 3] * (1.0 - t) + tn[i * 3 + 3] * t;
                tn[i * 3 + 2].normalize();
                if h[i] == 0.0 {
                    ttw[i * 3 + 1] = 0.0; tbw[i * 3 + 1] = 0.0;
                    ttw[i * 3 + 2] = 0.0; tbw[i * 3 + 2] = 0.0;
                }
            }
            let t = 0.8;
            th[0] = th[1] - GROOVE_DEPTH;
            ttw[0] = 0.0;
            tbw[0] = 0.0;
            th[NUM_TEETH_RIBS - 1] = t * th[NUM_TEETH_RIBS - 2];
            ttw[NUM_TEETH_RIBS - 1] = t * ttw[NUM_TEETH_RIBS - 2];
            tbw[NUM_TEETH_RIBS - 1] = t * tbw[NUM_TEETH_RIBS - 2];
            for i in 1..NUM_JAW_RIBS - 1 {
                let j = i * 3;
                th[j] = if th[j - 1] < th[j + 1] { th[j - 1] - GROOVE_DEPTH } else { th[j + 1] - GROOVE_DEPTH };
                ttw[j] = t * 0.5 * (ttw[j - 1] + ttw[j + 1]);
                tbw[j] = t * 0.5 * (tbw[j - 1] + tbw[j + 1]);
            }
            let target = if upper { si::UPPER_TEETH } else { si::LOWER_TEETH_ORIGINAL };
            for i in 0..NUM_TEETH_RIBS {
                if ttw[i] < 0.0 { ttw[i] = 0.0; }
                if tbw[i] < 0.0 { tbw[i] = 0.0; }
                if th[i] < 0.0 { th[i] = 0.0; }
                let s = &mut self.surface[target];
                let mut q = tp[i];
                s.set_vertex(i, 0, q);
                s.set_vertex(i, 4, q);
                if upper {
                    q = tp[i] + tn[i] * (ttw[i] - tbw[i]);
                    q.y = -th[i];
                    s.set_vertex(i, 1, q);
                    q = tp[i] + tn[i] * ttw[i];
                    q.y = -th[i];
                    s.set_vertex(i, 2, q);
                } else {
                    q = tp[i] + tn[i] * (tbw[i] - ttw[i]);
                    q.y = th[i];
                    s.set_vertex(i, 1, q);
                    q = tp[i] + tn[i] * tbw[i];
                    q.y = th[i];
                    s.set_vertex(i, 2, q);
                }
                q.y = 0.0;
                s.set_vertex(i, 3, q);
            }
            for i in 0..NUM_JAW_RIBS {
                if upper {
                    self.upper_gums_inner_edge[i] = self.palate_points[i];
                    self.upper_gums_outer_edge[i] = self.palate_points[i] + tn[3 * i] * ttop[i];
                } else {
                    self.lower_gums_inner_edge_orig[i] = self.jaw_points[i];
                    self.lower_gums_outer_edge_orig[i] = self.jaw_points[i] + tn[3 * i] * tbot[i];
                }
            }
        }

        // Paths of the lip corners (wide = spread lips, narrow = rounded lips).
        self.wide_lip_corner_path = LineStrip3D::new();
        self.narrow_lip_corner_path = LineStrip3D::new();
        for i in 4..=6 {
            let mut q = self.upper_gums_outer_edge[i];
            q.y = 0.0;
            self.wide_lip_corner_path.add_point(q);
            self.narrow_lip_corner_path.add_point(q);
        }
        let palate_length = self.palate_points[8].x - self.palate_points[0].x;
        let palate_depth = -2.0 * self.palate_points[0].z;
        let lip_protrusion = 1.0 * palate_length / 4.5;
        let lip_front_min_z = -1.0 * palate_depth / 4.6;
        let lip_front_max_z = -0.4 * palate_depth / 4.6;
        let max_lip_corner_x = self.upper_gums_outer_edge[8].x + lip_protrusion;
        self.wide_lip_corner_path.add_point(p3(max_lip_corner_x, 0.0, lip_front_min_z));
        self.narrow_lip_corner_path.add_point(p3(max_lip_corner_x, 0.0, lip_front_max_z));
    }

    fn init_velum(&mut self) {
        let w = [1.0, 1.0, 1.0];
        let mut curve = Bezier3D::new();
        let a = self.anatomy.clone();
        let source_z = -0.5 * a.pharynx_upper_depth;
        let t = a.palate[0].palate_angle_deg.clamp(0.00001, 89.99999);
        let target_z = self.palate_points[0].z + a.palate[1].palate_height / (t * PI / 180.0).tan();
        let posterior = p3(
            self.get_pharynx_back_x(a.pharynx_top_rib_y) + a.pharynx_back_width,
            a.pharynx_top_rib_y,
            -0.5 * a.pharynx_upper_depth,
        );
        let anterior = p3(0.0, 0.0, self.palate_points[0].z);
        for i in 0..NUM_VELUM_RIBS {
            let c0 = if i == 0 {
                posterior
            } else {
                let t = (i - 1) as f64 / (NUM_VELUM_RIBS - 2) as f64;
                posterior * (1.0 - t) + anterior * t
            };
            let t = i as f64 / (NUM_VELUM_RIBS - 1) as f64;
            for (pts, target) in [(&a.velum_low, si::LOW_VELUM), (&a.velum_mid, si::MID_VELUM), (&a.velum_high, si::HIGH_VELUM)] {
                let mut c2 = if i == 0 {
                    p3(self.get_pharynx_back_x(pts[0][1]), pts[0][1], 0.0)
                } else {
                    p3(pts[i - 1][0], pts[i - 1][1], 0.0)
                };
                c2.z = 0.0;
                let c1 = p3(c2.x, c2.y, (1.0 - t) * source_z + t * target_z);
                curve.set_points_weighted(&[c0, c1, c2], &w);
                for j in 0..NUM_UPPER_COVER_POINTS {
                    let u = j as f64 / (NUM_UPPER_COVER_POINTS - 1) as f64;
                    let u = curve.uniform_param(u);
                    let q = curve.get_point(u);
                    self.surface[target].set_vertex(i, j, q);
                }
            }
        }
        // Uvula (original shape).
        let (width, height, depth) = (a.uvula_width, a.uvula_height, a.uvula_depth);
        for i in 0..NUM_UVULA_POINTS {
            let t = PI * i as f64 / (NUM_UVULA_POINTS - 1) as f64;
            let mut q = p3(0.5 * width * t.cos() - 0.5 * width, 0.0, -0.5 * depth * t.sin());
            self.surface[si::UVULA_ORIGINAL].set_vertex(0, i, q);
            q.y = -0.5 * height;
            self.surface[si::UVULA_ORIGINAL].set_vertex(1, i, q);
        }
        for i in 0..NUM_UVULA_POINTS {
            let t = PI * i as f64 / (NUM_UVULA_POINTS - 1) as f64;
            let q = p3(0.75 * 0.5 * width * t.cos() - 0.5 * width, -0.75 * height, -0.75 * 0.5 * depth * t.sin());
            self.surface[si::UVULA_ORIGINAL].set_vertex(2, i, q);
        }
        for i in 0..NUM_UVULA_POINTS {
            self.surface[si::UVULA_ORIGINAL].set_vertex(3, i, p3(-0.5 * width, -height, 0.0));
        }
    }
}

pub const _ASSERT_PARAMS: usize = NUM_PARAMS;

#[cfg(test)]
pub(crate) mod testutil {
    use super::*;

    /// Loads the reference surfaces dumped from the original (`tools/vtl_reference/dump_surf.cpp`).
    pub fn reference_surfaces(text: &str) -> Vec<(usize, usize, usize, Vec<P3>)> {
        let mut out: Vec<(usize, usize, usize, Vec<P3>)> = vec![];
        for line in text.lines() {
            let f: Vec<&str> = line.split_whitespace().collect();
            if f.first() == Some(&"SURF") {
                out.push((f[1].parse().unwrap(), f[2].parse().unwrap(), f[3].parse().unwrap(), vec![]));
            } else if f.first() == Some(&"V") {
                out.last_mut().unwrap().3.push(p3(f[3].parse().unwrap(), f[4].parse().unwrap(), f[5].parse().unwrap()));
            }
        }
        out
    }

    /// Maximum coordinate difference between a surface and its reference.
    pub fn max_diff(s: &Surface, reference: &[P3]) -> f64 {
        assert_eq!(s.vertices.len(), reference.len());
        s.vertices.iter().zip(reference).map(|(a, b)| (a.x - b.x).abs().max((a.y - b.y).abs()).max((a.z - b.z).abs())).fold(0.0, f64::max)
    }
}

#[cfg(test)]
mod tests {
    use super::testutil::*;
    use super::*;

    const JD3: &str = include_str!("../../../resources/JD3.speaker");
    const SURF_A: &str = include_str!("../tests/data/vtl_surfaces_a.txt");

    #[test]
    fn static_reference_surfaces_match_the_original() {
        let vt = VocalTract::from_speaker(&Speaker::from_xml(JD3).unwrap());
        let reference = reference_surfaces(SURF_A);
        let static_surfaces = [
            si::PALATE, si::MANDIBLE, si::UPPER_TEETH, si::LOWER_TEETH_ORIGINAL, si::LOW_VELUM, si::MID_VELUM,
            si::HIGH_VELUM, si::NARROW_LARYNX_FRONT, si::NARROW_LARYNX_BACK, si::WIDE_LARYNX_FRONT,
            si::WIDE_LARYNX_BACK, si::EPIGLOTTIS_ORIGINAL, si::UVULA_ORIGINAL,
        ];
        for k in static_surfaces {
            let (idx, nr, np, v) = &reference[k];
            assert_eq!(*idx, k);
            assert_eq!((vt.surface[k].num_ribs, vt.surface[k].num_rib_points), (*nr, *np), "grid of surface {k}");
            let d = max_diff(&vt.surface[k], v);
            assert!(d < 2e-8, "surface {k} differs from the original by {d}");
        }
    }

    #[test]
    fn lip_corner_paths_and_gums() {
        let vt = VocalTract::from_speaker(&Speaker::from_xml(JD3).unwrap());
        assert_eq!(vt.wide_lip_corner_path.num_points(), 4);
        assert_eq!(vt.narrow_lip_corner_path.num_points(), 4);
        let last_wide = vt.wide_lip_corner_path.get_control_point(3);
        let last_narrow = vt.narrow_lip_corner_path.get_control_point(3);
        assert!(last_wide.z < last_narrow.z && last_narrow.z < 0.0, "spread lip corner is further out");
        assert!(vt.upper_gums_outer_edge[8].x > vt.upper_gums_inner_edge[8].x - 1e-9);
    }
}
