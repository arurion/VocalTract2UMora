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
use crate::splines::{Bezier3D, LineStrip2D, LineStrip3D};
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

/// Indices of the articulatory parameters.
pub mod pi {
    pub const HX: usize = 0;
    pub const HY: usize = 1;
    pub const JX: usize = 2;
    pub const JA: usize = 3;
    pub const LP: usize = 4;
    pub const LD: usize = 5;
    pub const VS: usize = 6;
    pub const VO: usize = 7;
    pub const TCX: usize = 8;
    pub const TCY: usize = 9;
    pub const TTX: usize = 10;
    pub const TTY: usize = 11;
    pub const TBX: usize = 12;
    pub const TBY: usize = 13;
    pub const TRX: usize = 14;
    pub const TRY: usize = 15;
    pub const TS1: usize = 16;
    pub const TS2: usize = 17;
    pub const TS3: usize = 18;
}
use pi::*;

/// State of one articulatory parameter: the value set by the caller (`x`, clamped to the range)
/// and the value after the biomechanical limits (`limited_x`).
#[derive(Clone, Copy, Debug, Default)]
pub struct ParamState {
    pub x: f64,
    pub limited_x: f64,
    pub min: f64,
    pub max: f64,
    pub neutral: f64,
}

pub struct VocalTract {
    pub param: [ParamState; NUM_PARAMS],
    pub lower_gums_inner_edge: [P3; NUM_JAW_RIBS],
    pub lower_gums_outer_edge: [P3; NUM_JAW_RIBS],
    pub lip_corner_path: LineStrip3D,
    pub upper_outline: LineStrip2D,
    pub lower_outline: LineStrip2D,
    pub tongue_outline: LineStrip2D,
    pub epiglottis_outline: LineStrip2D,
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
        let mut param = [ParamState::default(); NUM_PARAMS];
        for (i, p) in speaker.params.iter().enumerate() {
            param[i] = ParamState { x: p.neutral, limited_x: p.neutral, min: p.min, max: p.max, neutral: p.neutral };
        }
        let mut vt = VocalTract {
            param,
            lower_gums_inner_edge: [P3::default(); NUM_JAW_RIBS],
            lower_gums_outer_edge: [P3::default(); NUM_JAW_RIBS],
            lip_corner_path: LineStrip3D::new(),
            upper_outline: LineStrip2D::new(),
            lower_outline: LineStrip2D::new(),
            tongue_outline: LineStrip2D::new(),
            epiglottis_outline: LineStrip2D::new(),
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

    /// Sets the articulatory parameters (clamped to their ranges when the model is calculated).
    pub fn set_params(&mut self, values: &[f64; NUM_PARAMS]) {
        for i in 0..NUM_PARAMS { self.param[i].x = values[i]; }
    }

    fn restrict_param(&mut self, i: usize) {
        let p = &mut self.param[i];
        if p.x < p.min { p.x = p.min; }
        if p.x > p.max { p.x = p.max; }
    }

    /// Rotates a point of the lower jaw about the jaw fulcrum (jaw angle JA, jaw position JX).
    fn jaw_transform(&self, v: P3, cosinus: f64, sinus: f64, jx_factor: f64) -> P3 {
        let a = &self.anatomy;
        let dx = v.x + a.jaw_rest_pos[0] + jx_factor * self.param[JX].x - a.jaw_fulcrum[0];
        let dy = v.y + a.jaw_rest_pos[1] - a.jaw_fulcrum[1];
        p3(cosinus * dx - sinus * dy + a.jaw_fulcrum[0], sinus * dx + cosinus * dy + a.jaw_fulcrum[1], v.z)
    }

    /// Recomputes every surface for the current parameters.
    pub fn calc_surfaces(&mut self) {
        for i in 0..NUM_PARAMS { self.restrict_param(i); }
        let pr = self.anatomy.pharynx_rotation_angle_deg;
        let shear_coeff = (pr * PI / 180.0).cos() / (pr * PI / 180.0).sin();

        // Jaw rotation; the hyoid cannot be lower than the jaw allows.
        let angle_rad = self.param[JA].x * PI / 180.0;
        let (cosinus, sinus) = (angle_rad.cos(), angle_rad.sin());
        let vertex = self.jaw_transform(self.surface[si::MANDIBLE].get_vertex(0, NUM_LOWER_COVER_POINTS - 1), cosinus, sinus, 1.0);
        if self.param[HY].x > vertex.y { self.param[HY].limited_x = vertex.y; }

        let p = self.surface[si::NARROW_LARYNX_FRONT].get_vertex(NUM_LARYNX_RIBS - 1, NUM_LOWER_COVER_POINTS - 1);
        let q = self.surface[si::WIDE_LARYNX_FRONT].get_vertex(NUM_LARYNX_RIBS - 1, NUM_LOWER_COVER_POINTS - 1);
        let x_offset = self.get_pharynx_back_x(self.param[HY].limited_x);
        let mut t = q.x - p.x;
        if t.abs() < 0.000001 { t = 0.000001; }
        let max_hx = (vertex.x - x_offset - p.x) / t;
        if self.param[HX].x > max_hx { self.param[HX].limited_x = max_hx; }
        if self.param[HX].limited_x < self.param[HX].min { self.param[HX].limited_x = self.param[HX].min; }
        if self.param[HX].limited_x > self.param[HX].max { self.param[HX].limited_x = self.param[HX].max; }

        // Upper cover: larynx (back), pharynx, velum, palate.
        let mut rib = 0;
        let t = self.param[HX].limited_x;
        let x = self.get_pharynx_back_x(self.param[HY].limited_x);
        for i in 0..NUM_LARYNX_RIBS {
            for k in 0..NUM_UPPER_COVER_POINTS {
                let p = self.surface[si::NARROW_LARYNX_BACK].get_vertex(i, k);
                let q = self.surface[si::WIDE_LARYNX_BACK].get_vertex(i, k);
                let mut p = p * (1.0 - t) + q * t;
                let shear_x = p.y * shear_coeff;
                p.x += x + shear_x;
                p.y += self.param[HY].limited_x;
                self.surface[si::UPPER_COVER].set_vertex(rib, k, p);
            }
            rib += 1;
        }
        const PHARYNX_X: [f64; NUM_UPPER_COVER_POINTS] = [1.0, 0.69, 0.41, 0.19, 0.05, 0.0];
        const PHARYNX_Z: [f64; NUM_UPPER_COVER_POINTS] = [-1.0, -0.95, -0.81, -0.59, -0.31, 0.0];
        for i in 0..NUM_PHARYNX_RIBS {
            let t = i as f64 / (NUM_PHARYNX_RIBS - 1) as f64;
            let scale_z = 0.5 * ((1.0 - t) * self.anatomy.pharynx_lower_depth + t * self.anatomy.pharynx_upper_depth);
            let scale_x = self.anatomy.pharynx_back_width;
            let t = (i + 1) as f64 / NUM_PHARYNX_RIBS as f64;
            let y = (1.0 - t) * self.param[HY].limited_x + t * self.anatomy.pharynx_top_rib_y;
            let x = self.get_pharynx_back_x(y);
            for k in 0..NUM_UPPER_COVER_POINTS {
                self.surface[si::UPPER_COVER].set_vertex(rib, k, p3(x + scale_x * PHARYNX_X[k], y, scale_z * PHARYNX_Z[k]));
            }
            rib += 1;
        }
        let s = (self.param[VS].x - self.param[VS].min) / (self.param[VS].max - self.param[VS].min);
        let t = self.param[VO].x.clamp(0.0, 1.0);
        for i in 0..NUM_VELUM_RIBS {
            for k in 0..NUM_UPPER_COVER_POINTS {
                let p = self.surface[si::HIGH_VELUM].get_vertex(i, k);
                let q = self.surface[si::MID_VELUM].get_vertex(i, k);
                let r = self.surface[si::LOW_VELUM].get_vertex(i, k);
                let mut a = p * (1.0 - s) + q * s;
                a = a * (1.0 - t) + r * t;
                self.surface[si::UPPER_COVER].set_vertex(rib, k, a);
            }
            rib += 1;
        }
        let last_velum_rib = rib - 1;
        for i in 0..NUM_JAW_RIBS {
            let factor = self.palate_points[i].x / 1.2;
            if factor < 1.0 {
                for k in 0..NUM_UPPER_COVER_POINTS {
                    let p = self.surface[si::UPPER_COVER].get_vertex(last_velum_rib, k);
                    let q = self.surface[si::PALATE].get_vertex(i, k);
                    let mut vtx = P3::default();
                    vtx.x = q.x;
                    vtx.y = (1.0 - factor) * p.y + factor * q.y;
                    vtx.z = if q.x < 0.01 { p.z } else { q.z };
                    self.surface[si::UPPER_COVER].set_vertex(rib, k, vtx);
                }
            } else {
                for k in 0..NUM_UPPER_COVER_POINTS {
                    let v = self.surface[si::PALATE].get_vertex(i, k);
                    self.surface[si::UPPER_COVER].set_vertex(rib, k, v);
                }
            }
            rib += 1;
        }

        // Lower cover: larynx (front), throat, lower jaw.
        let mut rib = 0;
        let t = self.param[HX].limited_x;
        let x = self.get_pharynx_back_x(self.param[HY].limited_x);
        for i in 0..NUM_LARYNX_RIBS {
            for k in 0..NUM_LOWER_COVER_POINTS {
                let p = self.surface[si::NARROW_LARYNX_FRONT].get_vertex(i, k);
                let q = self.surface[si::WIDE_LARYNX_FRONT].get_vertex(i, k);
                let mut p = p * (1.0 - t) + q * t;
                let shear_x = p.y * shear_coeff;
                p.x += x + shear_x;
                p.y += self.param[HY].limited_x;
                self.surface[si::LOWER_COVER].set_vertex(rib, k, p);
            }
            rib += 1;
        }
        rib += NUM_THROAT_RIBS;
        for i in 0..NUM_JAW_RIBS {
            for k in 0..NUM_LOWER_COVER_POINTS {
                let v = self.jaw_transform(self.surface[si::MANDIBLE].get_vertex(i, k), cosinus, sinus, 1.0);
                self.surface[si::LOWER_COVER].set_vertex(rib, k, v);
            }
            rib += 1;
            self.lower_gums_outer_edge[i] = self.jaw_transform(self.lower_gums_outer_edge_orig[i], cosinus, sinus, 1.0);
            self.lower_gums_inner_edge[i] = self.jaw_transform(self.lower_gums_inner_edge_orig[i], cosinus, sinus, 1.0);
        }
        // Throat ribs of the lower cover.
        let weight = [1.0, 0.71, 1.0];
        let mut curve = Bezier3D::new();
        let r = self.surface[si::LOWER_COVER].get_vertex(NUM_LARYNX_RIBS - 1, NUM_LOWER_COVER_POINTS - 1);
        for i in 0..NUM_THROAT_RIBS {
            let q = self.surface[si::UPPER_COVER].get_vertex(NUM_LARYNX_RIBS + i, 0);
            curve.set_points_weighted(&[q, p3(r.x, r.y, q.z), r], &weight);
            for k in 0..NUM_LOWER_COVER_POINTS {
                let q = curve.get_point(k as f64 / (NUM_LOWER_COVER_POINTS - 1) as f64);
                self.surface[si::LOWER_COVER].set_vertex(NUM_LARYNX_RIBS + i, k, q);
            }
        }
        // Lower teeth follow the jaw.
        for i in 0..NUM_TEETH_RIBS {
            for k in 0..NUM_TEETH_POINTS {
                let v = self.jaw_transform(self.surface[si::LOWER_TEETH_ORIGINAL].get_vertex(i, k), cosinus, sinus, 1.0);
                self.surface[si::LOWER_TEETH].set_vertex(i, k, v);
            }
        }

        self.calc_lips();

        // Fill surfaces between the covers (left side).
        let uc = |s: &VocalTract, r: usize| s.surface[si::UPPER_COVER].get_vertex(r, 0);
        let lc = |s: &VocalTract, r: usize| s.surface[si::LOWER_COVER].get_vertex(r, 0);
        let n_l = NUM_LARYNX_RIBS;
        let n_p = NUM_PHARYNX_RIBS;
        let n_v = NUM_VELUM_RIBS;
        let fill = [
            (uc(self, n_l + n_p), lc(self, n_l + n_p - 1)),
            (uc(self, n_l + n_p + n_v - 1), lc(self, n_l + n_p)),
            (uc(self, n_l + n_p + n_v), lc(self, n_l + n_p)),
        ];
        for (rb, (up, lo)) in fill.iter().enumerate() {
            self.surface[si::LEFT_COVER].set_vertex(rb, 0, *up);
            self.surface[si::LEFT_COVER].set_vertex(rb, 1, *up);
            self.surface[si::LEFT_COVER].set_vertex(rb, 2, *lo);
            self.surface[si::LEFT_COVER].set_vertex(rb, 3, *lo);
        }
        let z_lip = [self.surface[si::UPPER_LIP].get_vertex(0, 0).z, self.surface[si::LOWER_LIP].get_vertex(0, 0).z];
        let mut inner_point = P3::default();
        let mut outer_point = P3::default();
        for k in 0..2 {
            let (inner_edge, outer_edge) = if k == 0 {
                (self.upper_gums_inner_edge, self.upper_gums_outer_edge)
            } else {
                (self.lower_gums_inner_edge, self.lower_gums_outer_edge)
            };
            for i in 0..NUM_JAW_RIBS - 1 {
                let p = inner_edge[i];
                let q = inner_edge[i + 1];
                let length = q.z - p.z;
                if p.z <= z_lip[k] {
                    inner_point = inner_edge[i];
                    outer_point = outer_edge[i];
                }
                if p.z <= z_lip[k] && q.z >= z_lip[k] && length > 0.0 {
                    let t = (z_lip[k] - p.z) / length;
                    inner_point = inner_edge[i] + (inner_edge[i + 1] - inner_edge[i]) * t;
                    outer_point = outer_edge[i] + (outer_edge[i + 1] - outer_edge[i]) * t;
                }
                if k == 0 {
                    self.surface[si::LEFT_COVER].set_vertex(3 + i, 0, inner_point);
                    self.surface[si::LEFT_COVER].set_vertex(3 + i, 1, outer_point);
                } else {
                    self.surface[si::LEFT_COVER].set_vertex(3 + i, 2, outer_point);
                    self.surface[si::LEFT_COVER].set_vertex(3 + i, 3, inner_point);
                }
            }
            if k == 0 {
                self.surface[si::LEFT_COVER].set_vertex(NUM_FILL_RIBS - 1, 0, inner_point);
                self.surface[si::LEFT_COVER].set_vertex(NUM_FILL_RIBS - 1, 1, outer_point);
            } else {
                self.surface[si::LEFT_COVER].set_vertex(NUM_FILL_RIBS - 1, 2, outer_point);
                self.surface[si::LEFT_COVER].set_vertex(NUM_FILL_RIBS - 1, 3, inner_point);
            }
        }

        // Upper and lower outlines of the sagittal contour.
        let upper_cover_point = NUM_UPPER_COVER_POINTS - 1;
        let lower_cover_point = NUM_LOWER_COVER_POINTS - 1;
        let tongue_point = NUM_TONGUE_POINTS / 4;
        let lip_rib = NUM_LIP_RIBS - 1;
        let teeth_rib = NUM_TEETH_RIBS - 1;
        self.upper_outline.reset(0);
        for i in 0..NUM_UPPER_COVER_RIBS {
            let v = self.surface[si::UPPER_COVER].get_vertex(i, upper_cover_point).to_p2();
            self.upper_outline.add_point(v);
        }
        for i in 0..NUM_TEETH_POINTS - 1 {
            let v = self.surface[si::UPPER_TEETH].get_vertex(teeth_rib, i).to_p2();
            self.upper_outline.add_point(v);
        }
        for i in 0..NUM_INNER_LIP_POINTS + 2 {
            let v = self.surface[si::UPPER_LIP].get_vertex(lip_rib, i).to_p2();
            self.upper_outline.add_point(v);
        }
        while self.upper_outline.get_control_point(self.upper_outline.num_points() as i64 - 1).x
            <= self.upper_outline.get_control_point(self.upper_outline.num_points() as i64 - 2).x {
            self.upper_outline.del_point();
        }
        self.lower_outline.reset(0);
        for i in 0..NUM_LOWER_COVER_RIBS {
            let v = self.surface[si::LOWER_COVER].get_vertex(i, lower_cover_point).to_p2();
            self.lower_outline.add_point(v);
        }
        for i in 0..NUM_TEETH_POINTS - 1 {
            let v = self.surface[si::LOWER_TEETH].get_vertex(teeth_rib, i).to_p2();
            self.lower_outline.add_point(v);
        }
        for i in 0..NUM_INNER_LIP_POINTS + 2 {
            let v = self.surface[si::LOWER_LIP].get_vertex(lip_rib, i).to_p2();
            self.lower_outline.add_point(v);
        }
        while self.lower_outline.get_control_point(self.lower_outline.num_points() as i64 - 1).x
            <= self.lower_outline.get_control_point(self.lower_outline.num_points() as i64 - 2).x {
            self.lower_outline.del_point();
        }

        self.calc_tongue();

        // Epiglottis: follows the larynx and is pushed back by the tongue root.
        let p = self.surface[si::LOWER_COVER].get_vertex(NUM_LARYNX_RIBS - 2, NUM_LOWER_COVER_POINTS - 1);
        let q = self.surface[si::LOWER_COVER].get_vertex(NUM_LARYNX_RIBS - 1, NUM_LOWER_COVER_POINTS - 1);
        let mut v = q - p;
        v.normalize();
        let a = p + v * self.anatomy.epiglottis_width;
        for k in 0..NUM_EPIGLOTTIS_POINTS {
            let p = self.surface[si::EPIGLOTTIS_ORIGINAL].get_vertex(0, k);
            let (x, y, z) = (v.x * p.x - v.y * p.y, v.y * p.x + v.x * p.y, p.z);
            self.surface[si::EPIGLOTTIS].set_vertex(0, k, p3(x + a.x, y + a.y, z + a.z));
        }
        let mut min_angle_rad = self.anatomy.epiglottis_angle_deg * PI / 180.0;
        for i in 1..10 {
            let mut p = self.surface[si::TONGUE].get_vertex(i, NUM_TONGUE_POINTS / 2) - a;
            if p.y < self.anatomy.epiglottis_height {
                if p.y < 0.01 { p.y = 0.01; }
                let angle_rad = p.y.atan2(p.x);
                if angle_rad > min_angle_rad { min_angle_rad = angle_rad; }
            }
        }
        min_angle_rad -= 0.5 * PI;
        if min_angle_rad > 0.25 * PI { min_angle_rad = 0.25 * PI; }
        let (sinus, cosinus) = (min_angle_rad.sin(), min_angle_rad.cos());
        for i in 1..NUM_EPIGLOTTIS_RIBS {
            for k in 0..NUM_EPIGLOTTIS_POINTS {
                let p = self.surface[si::EPIGLOTTIS_ORIGINAL].get_vertex(i, k);
                let (x, y, z) = (cosinus * p.x - sinus * p.y, sinus * p.x + cosinus * p.y, p.z);
                self.surface[si::EPIGLOTTIS].set_vertex(i, k, p3(x + a.x, y + a.y, z + a.z));
            }
        }
        // Uvula hangs from the velum.
        let a = self.surface[si::UPPER_COVER].get_vertex(NUM_LARYNX_RIBS + NUM_PHARYNX_RIBS + 1, NUM_UPPER_COVER_POINTS - 1);
        for i in 0..NUM_UVULA_RIBS {
            for k in 0..NUM_UVULA_POINTS {
                let p = self.surface[si::UVULA_ORIGINAL].get_vertex(i, k) + a;
                self.surface[si::UVULA].set_vertex(i, k, p);
            }
        }
        self.tongue_outline.reset(0);
        for i in 0..NUM_TONGUE_RIBS {
            let v = self.surface[si::TONGUE].get_vertex(i, tongue_point).to_p2();
            self.tongue_outline.add_point(v);
        }
        self.epiglottis_outline.reset(0);
        for i in 0..NUM_EPIGLOTTIS_RIBS {
            let v = self.surface[si::EPIGLOTTIS].get_vertex(i, NUM_EPIGLOTTIS_POINTS - 1).to_p2();
            self.epiglottis_outline.add_point(v);
        }
        for i in (0..=(NUM_EPIGLOTTIS_RIBS as i64 - 2)).rev() {
            let v = self.surface[si::EPIGLOTTIS].get_vertex(i as usize, 0).to_p2();
            self.epiglottis_outline.add_point(v);
        }

        // Mirror the half surfaces to get the two-sided versions used for the cross-sections.
        self.mirror_points(si::UPPER_COVER, si::UPPER_COVER_TWOSIDE);
        self.mirror_points(si::LOWER_COVER, si::LOWER_COVER_TWOSIDE);
        self.mirror_ribs(si::UPPER_TEETH, si::UPPER_TEETH_TWOSIDE);
        self.mirror_ribs(si::LOWER_TEETH, si::LOWER_TEETH_TWOSIDE);
        self.mirror_ribs(si::UPPER_LIP, si::UPPER_LIP_TWOSIDE);
        self.mirror_ribs(si::LOWER_LIP, si::LOWER_LIP_TWOSIDE);
        for i in 0..self.surface[si::LEFT_COVER].num_ribs {
            for k in 0..self.surface[si::LEFT_COVER].num_rib_points {
                let mut p = self.surface[si::LEFT_COVER].get_vertex(i, k);
                p.z = -p.z;
                self.surface[si::RIGHT_COVER].set_vertex(i, k, p);
            }
        }
        self.mirror_points(si::EPIGLOTTIS, si::EPIGLOTTIS_TWOSIDE);
        self.mirror_points(si::UVULA, si::UVULA_TWOSIDE);
    }

    /// Copies a half surface into a two-sided one by mirroring the rib points (z -> -z).
    fn mirror_points(&mut self, source: usize, target: usize) {
        let (nr, np_t) = (self.surface[target].num_ribs, self.surface[target].num_rib_points);
        let np = self.surface[source].num_rib_points;
        for i in 0..nr {
            for k in 0..np {
                let mut p = self.surface[source].get_vertex(i, k);
                self.surface[target].set_vertex(i, k, p);
                p.z = -p.z;
                self.surface[target].set_vertex(i, np_t - 1 - k, p);
            }
        }
    }

    /// Same, but mirroring the ribs.
    fn mirror_ribs(&mut self, source: usize, target: usize) {
        let nr_t = self.surface[target].num_ribs;
        let (nr, np) = (self.surface[source].num_ribs, self.surface[source].num_rib_points);
        for i in 0..nr {
            for k in 0..np {
                let mut p = self.surface[source].get_vertex(i, k);
                self.surface[target].set_vertex(i, k, p);
                p.z = -p.z;
                self.surface[target].set_vertex(nr_t - 1 - i, k, p);
            }
        }
    }

    /// Lip geometry: the corner path between the wide and the narrow position, the lip
    /// openings and the mouth radiation surface follow LD (distance) and LP (protrusion).
    fn get_important_lip_points(&mut self) -> (P3, P3, P3, P3, f64) {
        let n = self.narrow_lip_corner_path.num_points();
        let angle_rad = 0.5 * self.param[JA].x * PI / 180.0;
        let (cosinus, sinus) = (angle_rad.cos(), angle_rad.sin());
        self.lip_corner_path.reset(n);
        let t = (self.param[LD].x - self.param[LD].min) / (self.param[LD].max - self.param[LD].min);
        let a = self.anatomy.clone();
        for i in 0..n {
            let q = self.narrow_lip_corner_path.get_control_point(i) * (1.0 - t) + self.wide_lip_corner_path.get_control_point(i) * t;
            let dx = q.x + a.jaw_rest_pos[0] + 0.5 * self.param[JX].x - a.jaw_fulcrum[0];
            let dy = q.y + 0.5 * a.jaw_rest_pos[1] - a.jaw_fulcrum[1];
            let r = p3(cosinus * dx - sinus * dy + a.jaw_fulcrum[0], sinus * dx + cosinus * dy + a.jaw_fulcrum[1], q.z);
            self.lip_corner_path.set_point(i, r);
        }
        let mut lip_corner_separation_x = self.upper_gums_outer_edge[6].x;
        if self.lower_gums_outer_edge[6].x < lip_corner_separation_x {
            lip_corner_separation_x = self.lower_gums_outer_edge[6].x;
        }
        let t0 = self.narrow_lip_corner_path.get_intersection(p3(lip_corner_separation_x, 0.0, 0.0), p3(1.0, 0.0, 0.0));
        let t = (self.param[LP].x - self.param[LP].min) / (self.param[LP].max - self.param[LP].min);
        let corner = self.lip_corner_path.get_point(t);
        let onset = if t > t0 { self.lip_corner_path.get_point(t0) } else { corner };

        let t0v = self.upper_gums_outer_edge[8];
        let t1v = self.lower_gums_outer_edge[8];
        const X_OFFSET: f64 = 0.1;
        let y_offset = 0.35 - t * 0.3;
        const L_MAX: f64 = 1.0;
        const L_MIN: f64 = 0.3;
        let t = (self.param[LP].x - self.param[LP].min) / (self.param[LP].max - self.param[LP].min);
        let l = (1.0 - t) * L_MAX + t * L_MIN;
        let mut f0 = P3::default();
        let mut f1 = P3::default();
        f0.x = corner.x + l + 0.3;
        f1.x = corner.x + l;
        if f0.x < t0v.x + X_OFFSET { f0.x = t0v.x + X_OFFSET; }
        if f1.x < t1v.x + X_OFFSET { f1.x = t1v.x + X_OFFSET; }
        const MIN_T: f64 = -0.05;
        let t = (self.param[LP].x - self.param[LP].min) / (self.param[LP].max - self.param[LP].min);
        let y_close = 0.5 * (t0v.y + t1v.y) + (1.0 - t) * y_offset;
        let mut t = 0.5 * self.param[LD].x;
        if t < MIN_T { t = MIN_T; }
        f0.y = y_close + t;
        f1.y = y_close - t;
        if f0.y > t0v.y { f0.y = t0v.y; }
        if f1.y > t0v.y { f1.y = t0v.y; }
        if f0.y < t1v.y { f0.y = t1v.y; }
        if f1.y < t1v.y { f1.y = t1v.y; }
        let lip_tangent = self.upper_gums_outer_edge[5] - self.upper_gums_outer_edge[0];
        f0.z = corner.z + (lip_tangent.z / lip_tangent.x) * (f0.x - corner.x);
        f1.z = corner.z + (lip_tangent.z / lip_tangent.x) * (f1.x - corner.x);
        (onset, corner, f0, f1, y_close)
    }

    fn calc_lips(&mut self) {
        const EPS: f64 = 0.000001;
        let (onset, corner, f0, f1, _y_close) = self.get_important_lip_points();
        let mut lip_radius = self.anatomy.lips_width - 0.4;
        if lip_radius < 0.4 { lip_radius = 0.4; }

        let mut upper_path = LineStrip3D::new();
        let mut lower_path = LineStrip3D::new();
        let n = self.lip_corner_path.num_points();
        for i in 0..n.saturating_sub(1) {
            let p0 = self.lip_corner_path.get_control_point(i);
            let p1 = self.lip_corner_path.get_control_point(i + 1);
            if p0.x <= onset.x && p1.x >= onset.x {
                upper_path.add_point(onset);
                lower_path.add_point(onset);
            }
            if p0.x > onset.x && p0.x < corner.x {
                upper_path.add_point(p0);
                lower_path.add_point(p0);
            }
            if p0.x <= corner.x && p1.x >= corner.x {
                upper_path.add_point(corner);
                lower_path.add_point(corner);
            }
        }
        let mut curve = Bezier3D::new();
        curve.set_points_weighted(&[corner, f0, p3(f0.x, f0.y, 0.0)], &[1.0, 1.5, 1.0]);
        for i in 1..=7 { upper_path.add_point(curve.get_point(i as f64 / 7.0)); }
        curve.set_points_weighted(&[corner, f1, p3(f1.x, f1.y, 0.0)], &[1.0, 1.5, 1.0]);
        for i in 1..=7 { lower_path.add_point(curve.get_point(i as f64 / 7.0)); }

        for upper in [true, false] {
            let (outer_edge, inner_edge) = if upper {
                (self.upper_gums_outer_edge, self.upper_gums_inner_edge)
            } else {
                (self.lower_gums_outer_edge, self.lower_gums_inner_edge)
            };
            let mut outer_origin = [P3::default(); 32];
            let mut inner_origin = [P3::default(); 32];
            let mut num_ribs = 0usize;
            for i in 1..NUM_JAW_RIBS {
                let (p0, p1) = (outer_edge[i - 1], outer_edge[i]);
                let mut length = p1.x - p0.x;
                if length < EPS { length = EPS; }
                let (q0, q1) = (inner_edge[i - 1], inner_edge[i]);
                if p0.x <= onset.x && p1.x >= onset.x {
                    let t = (onset.x - p0.x) / length;
                    outer_origin[num_ribs] = p0 + (p1 - p0) * t;
                    inner_origin[num_ribs] = q0 + (q1 - q0) * t;
                    num_ribs += 1;
                }
                if p0.x <= corner.x && p1.x >= corner.x {
                    let t = (corner.x - p0.x) / length;
                    outer_origin[num_ribs] = p0 + (p1 - p0) * t;
                    inner_origin[num_ribs] = q0 + (q1 - q0) * t;
                    num_ribs += 1;
                }
                if p1.x > onset.x {
                    outer_origin[num_ribs] = p1;
                    inner_origin[num_ribs] = q1;
                    num_ribs += 1;
                }
            }
            let num_fixed = num_ribs;
            let mut num_div = [1usize; 32];
            while num_ribs < NUM_LIP_RIBS {
                let mut k = usize::MAX;
                let mut max_length = -1_000_000.0;
                for i in 0..num_fixed.saturating_sub(1) {
                    let length = (outer_origin[i + 1].z - outer_origin[i].z) / num_div[i] as f64;
                    if length > max_length { max_length = length; k = i; }
                }
                num_div[k] += 1;
                num_ribs += 1;
            }
            let mut temp_outer = [P3::default(); 32];
            let mut temp_inner = [P3::default(); 32];
            num_ribs = 0;
            for i in 0..num_fixed {
                if i < num_fixed - 1 {
                    let (p0, p1) = (outer_origin[i], outer_origin[i + 1]);
                    let (q0, q1) = (inner_origin[i], inner_origin[i + 1]);
                    for k in 0..num_div[i] {
                        let t = k as f64 / num_div[i] as f64;
                        temp_outer[num_ribs] = p0 + (p1 - p0) * t;
                        temp_inner[num_ribs] = q0 + (q1 - q0) * t;
                        num_ribs += 1;
                    }
                } else {
                    temp_outer[num_ribs] = outer_origin[i];
                    temp_inner[num_ribs] = inner_origin[i];
                    num_ribs += 1;
                }
            }
            outer_origin[..num_ribs].copy_from_slice(&temp_outer[..num_ribs]);
            inner_origin[..num_ribs].copy_from_slice(&temp_inner[..num_ribs]);

            let path = if upper { &mut upper_path } else { &mut lower_path };
            let max_x = path.get_point(1.0).x;
            let target = if upper { si::UPPER_LIP } else { si::LOWER_LIP };
            for i in 0..num_ribs {
                let p0 = outer_origin[i];
                let t = i as f64 / (num_ribs - 1) as f64;
                let p1 = path.get_point(t);
                let mut q = p1;
                q.y = p0.y;
                q = q * (1.0 - 0.7) + p1 * 0.7;
                let mut r = p0;
                r.y = p1.y;
                r = r * (1.0 - 0.0) + p1 * 0.0;
                let t = (p0.x - outer_origin[0].x) / (outer_origin[NUM_LIP_RIBS - 1].x - outer_origin[0].x);
                let t = t * t * t;
                let c1 = q * (1.0 - t) + r * t;
                curve.set_points_weighted(&[p0, c1, p1], &[1.0, 2.0, 1.0]);
                self.surface[target].set_vertex(i, 0, inner_origin[i]);
                for k in 0..NUM_INNER_LIP_POINTS - 1 {
                    let t = k as f64 / (NUM_INNER_LIP_POINTS - 2) as f64;
                    let pt = curve.get_point(t);
                    self.surface[target].set_vertex(i, k + 1, pt);
                }
                let mut q = p1;
                let mut radius = lip_radius * (q.x - corner.x) / (max_x - corner.x);
                if radius < 0.0 { radius = 0.0; }
                if upper { q.y += radius; } else { q.y -= radius; }
                for k in 0..NUM_OUTER_LIP_POINTS {
                    let t = if upper {
                        -0.5 * PI + 0.5 * PI * (k + 1) as f64 / NUM_OUTER_LIP_POINTS as f64
                    } else {
                        0.5 * PI - 0.5 * PI * (k + 1) as f64 / NUM_OUTER_LIP_POINTS as f64
                    };
                    let r = p3(q.x + radius * t.cos(), q.y + radius * t.sin(), q.z);
                    self.surface[target].set_vertex(i, NUM_INNER_LIP_POINTS + k, r);
                }
            }
        }
        self.calc_radiation(corner);
    }

    /// Surface in front of the mouth that models the radiation (a half cylinder-like shape).
    fn calc_radiation(&mut self, lip_corner: P3) {
        let lip_point = NUM_INNER_LIP_POINTS;
        let mut h = 0.0;
        let mut x = [0.0; NUM_RADIATION_POINTS];
        let mut y = [0.0; NUM_RADIATION_POINTS];
        let (mut min_z, mut new_z);
        min_z = 0.0;
        for i in 0..NUM_RADIATION_POINTS {
            let angle_rad = -0.5 * PI + PI * i as f64 / (NUM_RADIATION_POINTS - 1) as f64;
            x[i] = angle_rad.cos();
            y[i] = 0.5 * angle_rad.sin() + 0.5;
        }
        // `l`, `v` and `h` keep their values from the last lip rib for the ribs beyond it.
        let mut l = P3::default();
        let mut v = P3::default();
        let mut n;
        for i in 0..NUM_RADIATION_RIBS {
            if i < NUM_LIP_RIBS {
                n = p3(0.0, 0.0, -1.0);
                let u = self.surface[si::UPPER_LIP].get_vertex(i, lip_point);
                l = self.surface[si::LOWER_LIP].get_vertex(i, lip_point);
                let m = (u + l) * 0.5;
                v = u - l;
                h = u.y - l.y;
                if h <= 0.0 { h = 0.0; }
                if u.x <= lip_corner.x {
                    h = 0.0;
                } else {
                    new_z = m.z - h;
                    if new_z > min_z { new_z = min_z; } else { min_z = new_z; }
                    h = m.z - min_z;
                }
            } else {
                let angle_rad = 0.5 * PI - 0.5 * PI * (i - NUM_LIP_RIBS + 1) as f64 / (NUM_RADIATION_RIBS - NUM_LIP_RIBS) as f64;
                n = p3(angle_rad.cos(), 0.0, -angle_rad.sin());
            }
            for k in 0..NUM_RADIATION_POINTS {
                let p = l + v * y[k] + n * (x[k] * h);
                self.surface[si::RADIATION].set_vertex(i, k, p);
            }
        }
    }

    /// The tongue is ported in the next step.
    fn calc_tongue(&mut self) {}

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
    fn dynamic_surfaces_without_the_tongue_match_the_original_for_vowel_a() {
        let sp = Speaker::from_xml(JD3).unwrap();
        let mut vt = VocalTract::from_speaker(&sp);
        let a = sp.shape("a").unwrap();
        vt.set_params(&a.params);
        for i in 0..NUM_PARAMS { vt.restrict_param(i); vt.param[i].limited_x = vt.param[i].x; }
        vt.calc_surfaces();
        let reference = reference_surfaces(SURF_A);
        let checked = [
            ("UPPER_COVER", si::UPPER_COVER), ("LOWER_COVER", si::LOWER_COVER), ("LOWER_TEETH", si::LOWER_TEETH),
            ("UPPER_LIP", si::UPPER_LIP), ("LOWER_LIP", si::LOWER_LIP), ("RADIATION", si::RADIATION),
            ("LEFT_COVER", si::LEFT_COVER), ("RIGHT_COVER", si::RIGHT_COVER),
            ("UPPER_COVER_TWOSIDE", si::UPPER_COVER_TWOSIDE), ("LOWER_COVER_TWOSIDE", si::LOWER_COVER_TWOSIDE),
            ("UPPER_TEETH_TWOSIDE", si::UPPER_TEETH_TWOSIDE), ("LOWER_TEETH_TWOSIDE", si::LOWER_TEETH_TWOSIDE),
            ("UPPER_LIP_TWOSIDE", si::UPPER_LIP_TWOSIDE), ("LOWER_LIP_TWOSIDE", si::LOWER_LIP_TWOSIDE),
            ("UVULA", si::UVULA), ("UVULA_TWOSIDE", si::UVULA_TWOSIDE),
        ];
        for (name, k) in checked {
            let d = max_diff(&vt.surface[k], &reference[k].3);
            assert!(d < 2e-7, "{name} differs from the original by {d}");
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
