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
use crate::tube::Articulator;
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

pub const NUM_PROFILE_SAMPLES: usize = 96;
pub const PROFILE_LENGTH: f64 = 5.5;
pub const PROFILE_SAMPLE_LENGTH: f64 = PROFILE_LENGTH / NUM_PROFILE_SAMPLES as f64;
pub const INVALID_PROFILE_SAMPLE: f64 = 1_000_000.0;
pub const MIN_PROFILE_VALUE: f64 = -2.75;
pub const MAX_PROFILE_VALUE: f64 = 10.0;
pub const EXTREME_PROFILE_VALUE: f64 = 1_000_000.0;
pub const NUM_CENTERLINE_POINTS_EXPONENT: usize = 7;
pub const NUM_CENTERLINE_POINTS: usize = (1 << NUM_CENTERLINE_POINTS_EXPONENT) + 1;
pub const NUM_TUBE_SECTIONS: usize = 40;
pub const NUM_TUBE_PHARYNX_SECTIONS: usize = 16;

/// A point of the centre line of the vocal tract with the normal and the extent of the cut.
#[derive(Clone, Copy, Debug, Default)]
pub struct CenterLinePoint {
    pub point: P2,
    pub normal: P2,
    pub min: f64,
    pub max: f64,
    pub pos: f64,
    pub reserved: f64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CrossSection {
    pub area: f64,
    pub circ: f64,
    pub pos: f64,
    pub articulator: Articulator,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TubeSection {
    pub area: f64,
    pub circ: f64,
    pub pos: f64,
    pub length: f64,
    pub articulator: Articulator,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TongueRib {
    pub point: P2,
    pub left_side_height: f64,
    pub right_side_height: f64,
    pub left: P2,
    pub right: P2,
    pub normal: P2,
    pub min_x: f64,
    pub max_x: f64,
    pub min_y: f64,
    pub max_y: f64,
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
    pub tongue_rib: Vec<TongueRib>,
    pub rough_center_line: Vec<CenterLinePoint>,
    pub center_line: Vec<CenterLinePoint>,
    pub center_line_length: f64,
    pub cross_section: Vec<CrossSection>,
    pub tube_section: Vec<TubeSection>,
    pub nasal_port_area_cm2: f64,
    pub nasal_port_pos_cm: f64,
    pub incisor_pos_cm: f64,
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
            tongue_rib: vec![TongueRib::default(); NUM_TONGUE_RIBS],
            rough_center_line: vec![CenterLinePoint::default(); NUM_CENTERLINE_POINTS],
            center_line: vec![CenterLinePoint::default(); NUM_CENTERLINE_POINTS],
            center_line_length: 0.0,
            cross_section: vec![CrossSection::default(); NUM_CENTERLINE_POINTS],
            tube_section: vec![TubeSection::default(); NUM_TUBE_SECTIONS],
            nasal_port_area_cm2: 0.0,
            nasal_port_pos_cm: 0.0,
            incisor_pos_cm: 0.0,
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

    // ------------------------------------------------------------------------------------
    // Cross-section profiles
    // ------------------------------------------------------------------------------------

    fn insert_upper_profile_line(mut p0: P2, mut p1: P2, surface_index: usize, upper: &mut [f64; NUM_PROFILE_SAMPLES], upper_surf: &mut [i32; NUM_PROFILE_SAMPLES]) {
        if p0.x == p1.x { return; }
        p0.x += 0.5 * PROFILE_LENGTH;
        p1.x += 0.5 * PROFILE_LENGTH;
        if p0.x > p1.x { std::mem::swap(&mut p0, &mut p1); }
        const LENGTH_INC: f64 = 0.01;
        let mut v = p1 - p0;
        v.normalize();
        p0 -= v * LENGTH_INC;
        p1 += v * LENGTH_INC;
        let first = (p0.x / PROFILE_SAMPLE_LENGTH) as i64;
        let last = (p1.x / PROFILE_SAMPLE_LENGTH) as i64;
        if first == last { return; }
        let dx = PROFILE_SAMPLE_LENGTH;
        let dy = (p1.y - p0.y) * dx / (p1.x - p0.x);
        let mut y = p0.y + ((first as f64 + 1.0) * dx - p0.x) * dy / dx;
        for i in first + 1..=last {
            if i >= 0 && (i as usize) < NUM_PROFILE_SAMPLES {
                let i = i as usize;
                if y <= upper[i] && y >= MIN_PROFILE_VALUE && y <= MAX_PROFILE_VALUE {
                    upper[i] = y;
                    upper_surf[i] = surface_index as i32;
                }
            }
            y += dy;
        }
    }

    fn insert_lower_profile_line(mut p0: P2, mut p1: P2, surface_index: usize, lower: &mut [f64; NUM_PROFILE_SAMPLES], lower_surf: &mut [i32; NUM_PROFILE_SAMPLES]) {
        if p0.x == p1.x { return; }
        p0.x += 0.5 * PROFILE_LENGTH;
        p1.x += 0.5 * PROFILE_LENGTH;
        if p0.x > p1.x { std::mem::swap(&mut p0, &mut p1); }
        const LENGTH_INC: f64 = 0.01;
        let mut v = p1 - p0;
        v.normalize();
        p0 -= v * LENGTH_INC;
        p1 += v * LENGTH_INC;
        let first = (p0.x / PROFILE_SAMPLE_LENGTH) as i64;
        let last = (p1.x / PROFILE_SAMPLE_LENGTH) as i64;
        if first == last { return; }
        let dx = PROFILE_SAMPLE_LENGTH;
        let dy = (p1.y - p0.y) * dx / (p1.x - p0.x);
        let mut y = p0.y + ((first as f64 + 1.0) * dx - p0.x) * dy / dx;
        for i in first + 1..=last {
            if i >= 0 && (i as usize) < NUM_PROFILE_SAMPLES {
                let i = i as usize;
                if y >= lower[i] && y >= MIN_PROFILE_VALUE && y <= MAX_PROFILE_VALUE {
                    lower[i] = y;
                    lower_surf[i] = surface_index as i32;
                }
            }
            y += dy;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn insert_lower_cover_profile_line(mut p0: P2, mut p1: P2, surface_index: usize, upper: &mut [f64; NUM_PROFILE_SAMPLES], upper_surf: &mut [i32; NUM_PROFILE_SAMPLES], lower: &mut [f64; NUM_PROFILE_SAMPLES], lower_surf: &mut [i32; NUM_PROFILE_SAMPLES]) {
        if p0.x == p1.x { return; }
        p0.x += 0.5 * PROFILE_LENGTH;
        p1.x += 0.5 * PROFILE_LENGTH;
        if p0.x > p1.x { std::mem::swap(&mut p0, &mut p1); }
        const LENGTH_INC: f64 = 0.01;
        let mut v = p1 - p0;
        v.normalize();
        p0 -= v * LENGTH_INC;
        p1 += v * LENGTH_INC;
        let first = (p0.x / PROFILE_SAMPLE_LENGTH) as i64;
        let last = (p1.x / PROFILE_SAMPLE_LENGTH) as i64;
        if first == last { return; }
        let dx = PROFILE_SAMPLE_LENGTH;
        let dy = (p1.y - p0.y) * dx / (p1.x - p0.x);
        let mut y = p0.y + ((first as f64 + 1.0) * dx - p0.x) * dy / dx;
        for i in first + 1..=last {
            if i >= 0 && (i as usize) < NUM_PROFILE_SAMPLES {
                let i = i as usize;
                if y >= MIN_PROFILE_VALUE && y <= MAX_PROFILE_VALUE {
                    if y >= lower[i] {
                        lower[i] = y;
                        lower_surf[i] = surface_index as i32;
                    }
                    if upper[i] == EXTREME_PROFILE_VALUE && y <= upper[i] {
                        upper[i] = y;
                        upper_surf[i] = surface_index as i32;
                    }
                }
            }
            y += dy;
        }
    }

    /// Upper and lower profile of the cross-section cut by the vertical plane through `p`
    /// along `v` (profile value = distance along `v`, sample index = lateral position).
    /// Returns the articulator that forms the lower boundary near the midline.
    pub fn get_cross_profiles(&mut self, p: P2, v: P2, upper: &mut [f64; NUM_PROFILE_SAMPLES], lower: &mut [f64; NUM_PROFILE_SAMPLES], consider_tongue: bool) -> Articulator {
        const MIN_SQUARED_NORMAL_LENGTH: f64 = 0.0000001;
        const INVALID: f64 = INVALID_PROFILE_SAMPLE;
        const N: usize = NUM_PROFILE_SAMPLES;
        const N2: usize = NUM_PROFILE_SAMPLES / 2;
        const MAX_CUTS: usize = 2048;
        let profile_surfaces = [
            si::UPPER_COVER, si::UPPER_TEETH, si::UPPER_LIP, si::UVULA,
            si::LOWER_COVER, si::LOWER_TEETH, si::LOWER_LIP, si::EPIGLOTTIS,
            si::LEFT_COVER, si::RADIATION,
        ];
        struct Cut { p0: P2, p1: P2, n: P2, global: usize }

        let mut upper_surf = [-1i32; N];
        let mut lower_surf = [-1i32; N];
        for i in 0..N {
            upper[i] = EXTREME_PROFILE_VALUE;
            lower[i] = -EXTREME_PROFILE_VALUE;
        }
        let mut cuts: Vec<Cut> = Vec::new();
        for &g in profile_surfaces.iter() {
            if consider_tongue || (g != si::UVULA && g != si::EPIGLOTTIS) {
                let s = &mut self.surface[g];
                s.prepare_intersection(p, v);
                for (p0, p1, n) in s.cut_triangles() {
                    if cuts.len() < MAX_CUTS && p0.y < MAX_PROFILE_VALUE && p1.y < MAX_PROFILE_VALUE && p1.y > MIN_PROFILE_VALUE {
                        cuts.push(Cut { p0, p1, n, global: g });
                    }
                }
            }
        }

        let mut upper_teeth_max_y = -EXTREME_PROFILE_VALUE;
        let mut lower_teeth_min_y = EXTREME_PROFILE_VALUE;
        let mut upper_lip_min_y = EXTREME_PROFILE_VALUE;
        let mut lower_lip_max_y = -EXTREME_PROFILE_VALUE;
        for c in &cuts {
            let (p0, p1) = (c.p0, c.p1);
            if c.global == si::UPPER_TEETH {
                if p0.y > upper_teeth_max_y { upper_teeth_max_y = p0.y; }
                if p1.y > upper_teeth_max_y { upper_teeth_max_y = p1.y; }
            }
            if c.global == si::LOWER_TEETH {
                if p0.y < lower_teeth_min_y { lower_teeth_min_y = p0.y; }
                if p1.y < lower_teeth_min_y { lower_teeth_min_y = p1.y; }
            }
            if c.global == si::UPPER_LIP {
                if p0.y < upper_lip_min_y { upper_lip_min_y = p0.y; }
                if p1.y < upper_lip_min_y { upper_lip_min_y = p1.y; }
            }
            if c.global == si::LOWER_LIP {
                if p0.y > lower_lip_max_y { lower_lip_max_y = p0.y; }
                if p1.y > lower_lip_max_y { lower_lip_max_y = p1.y; }
            }
        }
        let both_teeth_cut = upper_teeth_max_y > lower_teeth_min_y;
        let both_lips_cut = upper_lip_min_y != EXTREME_PROFILE_VALUE && lower_lip_max_y != -EXTREME_PROFILE_VALUE;

        for c in &cuts {
            let (p0, p1, n, g) = (c.p0, c.p1, c.n, c.global);
            match g {
                si::UPPER_COVER | si::UPPER_TEETH | si::UPPER_LIP | si::UVULA => {
                    if n.y < 0.0 { Self::insert_upper_profile_line(p0, p1, g, upper, &mut upper_surf); }
                }
                si::LOWER_COVER => {
                    if n.y > 0.0 { Self::insert_lower_cover_profile_line(p0, p1, g, upper, &mut upper_surf, lower, &mut lower_surf); }
                }
                si::LOWER_TEETH | si::LOWER_LIP | si::EPIGLOTTIS => {
                    if n.y > 0.0 { Self::insert_lower_profile_line(p0, p1, g, lower, &mut lower_surf); }
                }
                _ => {
                    let right_orientation = !(v.x < 0.0 && v.y < 0.0);
                    if n.x * n.x + n.y * n.y > MIN_SQUARED_NORMAL_LENGTH && right_orientation {
                        if n.y <= 0.0 {
                            Self::insert_upper_profile_line(p0, p1, g, upper, &mut upper_surf);
                        } else if n.y >= 0.0 {
                            Self::insert_lower_profile_line(p0, p1, g, lower, &mut lower_surf);
                        }
                    }
                }
            }
        }

        for i in 0..N2 {
            if upper[i] == EXTREME_PROFILE_VALUE { upper[i] = INVALID; }
            if lower[i] == -EXTREME_PROFILE_VALUE { lower[i] = INVALID; }
        }
        // The teeth limit the profile towards the midline.
        let mut upper_limit = 1_000_000.0;
        let mut lower_limit = -1_000_000.0;
        for i in (0..N2).rev() {
            if upper[i] != INVALID {
                if upper_surf[i] == si::UPPER_TEETH as i32 && upper[i] < upper_limit { upper_limit = upper[i]; }
                if upper[i] > upper_limit { upper[i] = upper_limit; }
            }
            if lower[i] != INVALID {
                if lower_surf[i] == si::LOWER_TEETH as i32 && lower[i] > lower_limit { lower_limit = lower[i]; }
                if lower[i] < lower_limit { lower[i] = lower_limit; }
            }
        }
        let mut last_value = INVALID;
        for i in 0..N2 {
            if upper[i] == INVALID { upper[i] = last_value; } else { last_value = upper[i]; }
        }
        let mut first_upper_valid: i64 = -1;
        let mut first_lower_valid: i64 = -1;
        for i in 0..N2 {
            if first_lower_valid == -1 && lower[i] != INVALID { first_lower_valid = i as i64; }
            if first_upper_valid == -1 && upper[i] != INVALID { first_upper_valid = i as i64; }
        }
        if first_lower_valid == -1 {
            if first_upper_valid == -1 { first_upper_valid = 0; }
            for i in first_upper_valid as usize..N2 { lower[i] = MIN_PROFILE_VALUE; }
        } else {
            let mut i = N2 - 1;
            while i > 0 && lower[i] == INVALID {
                lower[i] = MIN_PROFILE_VALUE;
                i -= 1;
            }
        }
        // Interpolate gaps of the lower profile.
        let (mut left, mut right) = (0usize, 0usize);
        for i in 0..N2 {
            if lower[i] == INVALID {
                if i <= left || i >= right {
                    left = i;
                    right = i;
                    while lower[left] == INVALID && left > 0 { left -= 1; }
                    while lower[right] == INVALID && right < N2 - 1 { right += 1; }
                }
                if i > left && i < right {
                    if lower[left] != INVALID && lower[right] != INVALID {
                        lower[i] = lower[left] + (i - left) as f64 * (lower[right] - lower[left]) / (right - left) as f64;
                    } else if lower[left] != INVALID {
                        lower[i] = lower[left];
                    }
                }
            }
        }
        for i in 0..N2 {
            if lower[i] != INVALID && upper[i] != INVALID && lower[i] > upper[i] { lower[i] = upper[i]; }
        }
        let mut upper_left = 0usize;
        while upper[upper_left] == INVALID && upper_left < N2 - 1 { upper_left += 1; }
        let mut lower_left = 0usize;
        while lower[lower_left] == INVALID && lower_left < N2 - 1 { lower_left += 1; }
        let mut leftmost = N2 - 1;
        if upper[upper_left] != INVALID && upper_left < leftmost { leftmost = upper_left; }
        if lower[lower_left] != INVALID && lower_left < leftmost { leftmost = lower_left; }
        if upper[upper_left] == INVALID { upper[upper_left] = MAX_PROFILE_VALUE; }
        if lower[lower_left] == INVALID { lower[lower_left] = MIN_PROFILE_VALUE; }
        if upper_left < lower_left {
            for i in upper_left..lower_left { lower[i] = lower[lower_left]; }
        } else {
            for i in lower_left..upper_left { upper[i] = upper[upper_left]; }
        }
        if both_lips_cut && !both_teeth_cut && upper[N2 - 1] == lower[N2 - 1] {
            for i in 0..N2 {
                if upper[i] != INVALID && lower[i] != INVALID {
                    let d = 0.5 * (lower[i] + upper[i]);
                    lower[i] = d;
                    upper[i] = d;
                }
            }
        }
        // Mirror the left half to the right half.
        for i in 0..=N2 {
            upper[N - 1 - i] = upper[i];
            lower[N - 1 - i] = lower[i];
        }
        let mut rightmost = N - 1 - leftmost;

        if consider_tongue {
            let mut tongue_profile = [-EXTREME_PROFILE_VALUE; N];
            let mut tongue_surf = [-1i32; N];
            let s = &mut self.surface[si::TONGUE];
            s.prepare_intersection(p, v);
            for (p0, p1, n) in s.cut_triangles() {
                if n.y >= 0.0 {
                    Self::insert_lower_profile_line(p0, p1, si::TONGUE, &mut tongue_profile, &mut tongue_surf);
                }
            }
            for i in 0..N {
                if tongue_profile[i] == -EXTREME_PROFILE_VALUE { tongue_profile[i] = INVALID; }
            }
            let (mut left, mut right) = (0usize, 0usize);
            for i in 0..N {
                if tongue_profile[i] == INVALID {
                    if i <= left || i >= right {
                        left = i;
                        right = i;
                        while tongue_profile[left] == INVALID && left > 0 { left -= 1; }
                        while tongue_profile[right] == INVALID && right < N - 1 { right += 1; }
                    }
                    if right > left {
                        if tongue_profile[left] != INVALID && tongue_profile[right] != INVALID {
                            tongue_profile[i] = tongue_profile[left] + (i - left) as f64 * (tongue_profile[right] - tongue_profile[left]) / (right - left) as f64;
                        } else if tongue_profile[left] != INVALID {
                            tongue_profile[i] = tongue_profile[left];
                        } else if tongue_profile[right] != INVALID {
                            tongue_profile[i] = tongue_profile[right];
                        }
                    }
                }
            }
            for i in leftmost..=rightmost {
                if tongue_profile[i] != INVALID && lower[i] != INVALID && tongue_profile[i] > lower[i] {
                    lower[i] = tongue_profile[i];
                    lower_surf[i] = tongue_surf[i];
                }
            }
            while leftmost < N2 && lower[leftmost] > upper[leftmost] - 0.1 { leftmost += 1; }
            while rightmost > N2 && lower[rightmost] > upper[rightmost] - 0.1 { rightmost -= 1; }
            for i in 0..N {
                if i >= leftmost && i <= rightmost {
                    if lower[i] > upper[i] { lower[i] = upper[i]; }
                } else {
                    lower[i] = INVALID;
                    upper[i] = INVALID;
                }
            }
        }

        // Cut off everything that is not part of the connected opening around the midline.
        const OPEN_THRESHOLD: f64 = 0.1;
        const CUTOFF_THRESHOLD: f64 = 0.01;
        let mut i = N2 - 1;
        while upper[i] - lower[i] < OPEN_THRESHOLD && i > 0 { i -= 1; }
        while upper[i] - lower[i] >= CUTOFF_THRESHOLD && i > 0 { i -= 1; }
        loop {
            upper[i] = INVALID;
            lower[i] = INVALID;
            if i == 0 { break; }
            i -= 1;
        }
        let mut i = N2 - 1;
        while upper[i] - lower[i] < OPEN_THRESHOLD && i < N - 1 { i += 1; }
        while upper[i] - lower[i] >= CUTOFF_THRESHOLD && i < N - 1 { i += 1; }
        while i <= N - 1 {
            upper[i] = INVALID;
            lower[i] = INVALID;
            i += 1;
        }
        let mut leftmost = 0usize;
        while (upper[leftmost] == INVALID || lower[leftmost] == INVALID) && leftmost < N2 - 1 { leftmost += 1; }
        if upper[leftmost] != INVALID && lower[leftmost] != INVALID {
            let m = 0.5 * (upper[leftmost] + lower[leftmost]);
            upper[leftmost] = m;
            lower[leftmost] = m;
        }
        let mut rightmost = N - 1;
        while (upper[rightmost] == INVALID || lower[rightmost] == INVALID) && rightmost > N2 { rightmost -= 1; }
        if upper[rightmost] != INVALID && lower[rightmost] != INVALID {
            let m = 0.5 * (upper[rightmost] + lower[rightmost]);
            upper[rightmost] = m;
            lower[rightmost] = m;
        }

        // Which articulator forms the lower boundary near the midline?
        let mut articulator = Articulator::Other;
        let (mut has_tongue, mut has_lower_lip, mut has_lower_teeth) = (false, false, false);
        const CHECK_RANGE_CM: f64 = 0.5;
        let num_check = (CHECK_RANGE_CM / PROFILE_SAMPLE_LENGTH) as usize;
        for i in 1..num_check {
            let sfc = lower_surf[N2 - i];
            if sfc == si::TONGUE as i32 {
                has_tongue = true;
            } else if sfc == si::LOWER_LIP as i32 || sfc == si::RADIATION as i32 {
                has_lower_lip = true;
            } else if sfc == si::LOWER_TEETH as i32 {
                has_lower_teeth = true;
            }
        }
        if has_lower_teeth {
            articulator = Articulator::LowerIncisors;
        } else if has_lower_lip {
            articulator = Articulator::LowerLip;
        } else if has_tongue {
            articulator = Articulator::Tongue;
        }
        articulator
    }

    /// Area and circumference of the opening between the upper and lower profile.
    pub fn get_cross_section(upper: &[f64; NUM_PROFILE_SAMPLES], lower: &[f64; NUM_PROFILE_SAMPLES]) -> (f64, f64) {
        const INVALID: f64 = INVALID_PROFILE_SAMPLE;
        let d2 = PROFILE_SAMPLE_LENGTH * PROFILE_SAMPLE_LENGTH;
        let (mut area, mut circ) = (0.0, 0.0);
        for i in 0..NUM_PROFILE_SAMPLES - 1 {
            if upper[i] != INVALID && upper[i + 1] != INVALID && lower[i] != INVALID && lower[i + 1] != INVALID {
                let a = upper[i] - lower[i];
                let b = upper[i + 1] - lower[i + 1];
                area += 0.5 * (a + b) * PROFILE_SAMPLE_LENGTH;
                let a = upper[i + 1] - upper[i];
                let b = lower[i + 1] - lower[i];
                circ += (a * a + d2).sqrt() + (b * b + d2).sqrt();
            }
        }
        (area, circ)
    }

    // ------------------------------------------------------------------------------------
    // Centre line, cross-sections and tube
    // ------------------------------------------------------------------------------------

    /// Result of an intersection query in the original's convention: `t = 1e6` and the point
    /// (0, 0) when there is no intersection.
    fn unpack(r: Option<(f64, P2)>) -> (bool, f64, P2) {
        match r {
            Some((t, p)) => (true, t, p),
            None => (false, 1_000_000.0, p2(0.0, 0.0)),
        }
    }

    fn outline_cuts(&self, p: P2, n: P2) -> [(bool, f64, P2); 3] {
        [
            Self::unpack(self.upper_outline.get_special_intersection(p, n)),
            Self::unpack(self.lower_outline.get_closest_intersection(p, n)),
            Self::unpack(self.tongue_outline.get_closest_intersection(p, n)),
        ]
    }

    fn verify_center_line_normal(&mut self, l: usize, m: usize, r: usize) {
        let p = self.center_line[m].point;
        let v = self.center_line[m].normal;
        for side in [l, r] {
            let rr = self.center_line[side].point + self.center_line[side].normal * self.center_line[side].min;
            let w = self.center_line[side].point + self.center_line[side].normal * self.center_line[side].max - rr;
            let q = p - rr;
            let denominator = v.x * w.y - v.y * w.x;
            if denominator != 0.0 {
                let s = (v.x * q.y - q.x * v.y) / denominator;
                if s >= 0.0 && s <= 1.0 {
                    let t = (w.x * q.y - q.x * w.y) / denominator;
                    if t <= 0.0 && t >= self.center_line[m].min {
                        let mut nn = self.center_line[m].point - rr;
                        nn.normalize();
                        self.center_line[m].normal = nn;
                        self.center_line[m].reserved = 1.0;
                    } else if t >= 0.0 && t <= self.center_line[m].max {
                        let mut nn = rr + w - self.center_line[m].point;
                        nn.normalize();
                        self.center_line[m].normal = nn;
                        self.center_line[m].reserved = 1.0;
                    }
                }
            }
        }
    }

    pub fn calc_center_line(&mut self) {
        const SKIPPED_LIP_POINTS: i64 = 1;
        const NCP: usize = NUM_CENTERLINE_POINTS;
        let tongue_center = p3(self.param[TCX].limited_x, self.param[TCY].limited_x, 0.0);
        let rx = self.anatomy.tongue_center_radius_x;
        let ry = self.anatomy.tongue_center_radius_y;
        let mut min_y = self.upper_outline.get_control_point(0).y;
        let r = self.lower_outline.get_control_point(0);
        if r.y > min_y { min_y = r.y; }
        let mut max_x = self.upper_outline.get_control_point(self.upper_outline.num_points() as i64 - 1 - SKIPPED_LIP_POINTS).x;
        let r = self.lower_outline.get_control_point(self.lower_outline.num_points() as i64 - 1 - SKIPPED_LIP_POINTS);
        if r.x < max_x { max_x = r.x; }
        let mu_len = [tongue_center.y - min_y, 0.5 * PI * 0.5 * (rx + ry), max_x - tongue_center.x];
        let mu_length = mu_len[0] + mu_len[1] + mu_len[2];

        let mut nu_line = vec![P2::default(); NCP + 2];
        for i in 0..NCP {
            let pos = mu_length * i as f64 / (NCP - 1) as f64;
            let (pt, v);
            if pos <= mu_len[0] {
                pt = p2(tongue_center.x - rx, tongue_center.y - mu_len[0] + pos);
                v = p2(-1.0, 0.0);
            } else if pos <= mu_len[0] + mu_len[1] {
                let angle = PI - 0.5 * PI * (pos - mu_len[0]) / mu_len[1];
                let (cosinus, sinus) = (angle.cos(), angle.sin());
                pt = p2(tongue_center.x + rx * cosinus, tongue_center.y + ry * sinus);
                let mut vv = p2(rx * cosinus, ry * sinus);
                vv.normalize();
                v = vv;
            } else {
                pt = p2(tongue_center.x + pos - mu_len[0] - mu_len[1], tongue_center.y + ry);
                v = p2(0.0, 1.0);
            }
            let ok0 = Self::unpack(self.upper_outline.get_special_intersection(pt, v));
            let ok1 = Self::unpack(self.lower_outline.get_closest_intersection(pt, v));
            let ok2 = Self::unpack(self.tongue_outline.get_closest_intersection(pt, v));
            let ok3 = Self::unpack(self.epiglottis_outline.get_first_intersection(pt, v));
            let (okk, t, sp) = ([ok0.0, ok1.0, ok2.0, ok3.0], [ok0.1, ok1.1, ok2.1, ok3.1], [ok0.2, ok1.2, ok2.2, ok3.2]);
            let top = 0;
            let mut bottom = 1;
            if okk[0] {
                let mut t_max = -1_000_000.0;
                if okk[1] && t[1] > t_max { t_max = t[1]; bottom = 1; }
                if okk[2] && t[2] > t_max { t_max = t[2]; bottom = 2; }
                if okk[3] && t[3] > t_max { bottom = 3; }
            }
            let center_point = (sp[top] + sp[bottom]) * 0.5;
            let rc = &mut self.rough_center_line[i];
            rc.point = center_point;
            rc.normal = v;
            rc.max = (t[top] - t[bottom]) / 2.0;
            rc.min = -rc.max;
            rc.pos = 0.0;
            nu_line[i + 1] = center_point;
        }
        let mut nu_line_length = 0.0;
        let mut nu = vec![0.0; NCP + 2];
        for i in 1..NCP {
            nu[i] = nu_line_length;
            nu_line_length += (nu_line[i + 1] - nu_line[i]).magnitude();
        }
        nu[NCP] = nu_line_length;
        const RANGE_LENGTH: f64 = 2.0;
        const HALF_RANGE_LENGTH: f64 = 0.5 * RANGE_LENGTH;
        nu_line[0] = nu_line[1] + p2(0.0, -HALF_RANGE_LENGTH);
        nu_line[NCP + 1] = nu_line[NCP] + p2(HALF_RANGE_LENGTH, 0.0);
        nu[0] = -HALF_RANGE_LENGTH;
        nu[NCP + 1] = nu[NCP] + HALF_RANGE_LENGTH;
        let mut range_index = [0usize; 2];
        for i in 0..NCP {
            let mut range_pos = [0.0; 2];
            range_pos[0] = nu_line_length * i as f64 / (NCP - 1) as f64 - HALF_RANGE_LENGTH;
            range_pos[1] = range_pos[0] + RANGE_LENGTH;
            let mut range_point = [P2::default(); 2];
            for k in 0..2 {
                while range_index[k] < NCP && range_pos[k] > nu[range_index[k] + 1] { range_index[k] += 1; }
                let ri = range_index[k];
                let d = (range_pos[k] - nu[ri]) / (nu[ri + 1] - nu[ri]);
                range_point[k] = nu_line[ri] + (nu_line[ri + 1] - nu_line[ri]) * d;
            }
            let mut center_point = p2(0.0, 0.0);
            if range_index[1] > range_index[0] {
                let mut length = (nu_line[range_index[0] + 1] - range_point[0]).magnitude();
                center_point += (range_point[0] + nu_line[range_index[0] + 1]) * (length * 0.5);
                for k in range_index[0] + 1..range_index[1] {
                    length = (nu_line[k + 1] - nu_line[k]).magnitude();
                    center_point += (nu_line[k] + nu_line[k + 1]) * (length * 0.5);
                }
                length = (range_point[1] - nu_line[range_index[1]]).magnitude();
                center_point += (range_point[1] + nu_line[range_index[1]]) * (length * 0.5);
            } else {
                let length = (range_point[1] - range_point[0]).magnitude();
                center_point += (range_point[0] + range_point[1]) * (length * 0.5);
            }
            self.center_line[i].point = center_point / RANGE_LENGTH;
        }
        let q0 = self.upper_outline.get_control_point(0);
        let q1 = self.lower_outline.get_control_point(0);
        let x = self.center_line[0].point.x;
        let y = q0.y + (q1.y - q0.y) * (x - q0.x) / (q1.x - q0.x);
        self.center_line[0].point.y = y;
        for i in 0..4 {
            if self.center_line[i].point.y < y { self.center_line[i].point.y = y; }
        }
        let mut nn = q0 - q1;
        nn.normalize();
        self.center_line[0].normal = nn;
        self.center_line[NCP - 1].normal = p2(0.0, 1.0);
        let x = self.center_line[NCP - 1].point.x;
        for i in NCP - 4..NCP {
            if self.center_line[i].point.x > x { self.center_line[i].point.x = x; }
        }
        self.center_line_length = 0.0;
        for i in 0..NCP - 1 {
            self.center_line[i].pos = self.center_line_length;
            self.center_line_length += (self.center_line[i + 1].point - self.center_line[i].point).magnitude();
        }
        self.center_line[NCP - 1].pos = self.center_line_length;
        for i in 1..NCP - 1 {
            let q0 = self.center_line[i].point - self.center_line[i - 1].point;
            let q1 = self.center_line[i + 1].point - self.center_line[i].point;
            let mut nn = p2(-q0.y - q1.y, q0.x + q1.x);
            nn.normalize();
            self.center_line[i].normal = nn;
        }
        for i in 0..NCP {
            self.center_line[i].reserved = 0.0;
            let (pt, n) = (self.center_line[i].point, self.center_line[i].normal);
            let c = self.outline_cuts(pt, n);
            if c[0].0 { self.center_line[i].max = c[0].1; } else { self.center_line[i].max = 3.0; self.center_line[i].reserved = 1.0; }
            if c[1].0 && c[2].0 {
                self.center_line[i].min = if c[1].1 > c[2].1 { c[1].1 } else { c[2].1 };
            } else if c[2].0 {
                self.center_line[i].min = c[2].1;
            } else if c[1].0 {
                self.center_line[i].min = c[1].1;
            } else {
                self.center_line[i].min = -3.0;
                self.center_line[i].reserved = 1.0;
            }
        }
        for i in 0..NUM_CENTERLINE_POINTS_EXPONENT {
            let num_to_check = 1usize << i;
            let distance = 1usize << (NUM_CENTERLINE_POINTS_EXPONENT - i);
            let mut index = distance / 2;
            for _ in 0..num_to_check {
                self.verify_center_line_normal(index - distance / 2, index, index + distance / 2);
                index += distance;
            }
        }
        for i in 0..NCP {
            if self.center_line[i].reserved != 0.0 {
                let (pt, n) = (self.center_line[i].point, self.center_line[i].normal);
                let c = self.outline_cuts(pt, n);
                self.center_line[i].max = if c[0].0 { c[0].1 } else { 3.0 };
                if c[1].0 && c[2].0 {
                    self.center_line[i].min = if c[1].1 > c[2].1 { c[1].1 } else { c[2].1 };
                } else if c[2].0 {
                    self.center_line[i].min = c[2].1;
                } else if c[1].0 {
                    self.center_line[i].min = c[1].1;
                } else {
                    self.center_line[i].min = -3.0;
                }
            }
        }
    }

    /// Position along the centre line of the point on it that is closest to `q`.
    pub fn get_center_line_pos(&self, q: P2) -> f64 {
        const EPS: f64 = 0.000001;
        let mut min_dist = 1_000_000.0;
        let mut best_index: i64 = -1;
        let mut best_t = 0.0;
        for i in 0..NUM_CENTERLINE_POINTS - 1 {
            let mut p0 = self.center_line[i].point;
            let n0 = self.center_line[i].normal;
            let mut p1 = self.center_line[i + 1].point;
            let n1 = self.center_line[i + 1].normal;
            let mut v = (p1 - p0) * EPS;
            p0 -= v;
            p1 += v;
            if (q.x - p0.x) * n0.y - n0.x * (q.y - p0.y) >= 0.0 && (q.x - p1.x) * n1.y - n1.x * (q.y - p1.y) <= 0.0 {
                v = p1 - p0;
                let r = q - p0;
                let m = n1 - n0;
                let mut denominator = v.y * m.x - v.x * m.y;
                if denominator == 0.0 { denominator = EPS; }
                let p = (r.x * m.y - r.y * m.x + v.y * n0.x - v.x * n0.y) / denominator;
                let mut qq = (r.x * n0.y - r.y * n0.x) / denominator;
                qq = 0.25 * p * p - qq;
                if qq < 0.0 { qq = 0.0; }
                qq = qq.sqrt();
                let (t0, t1) = (-0.5 * p + qq, -0.5 * p - qq);
                let t = if t0 > -EPS && t0 < 1.0 + EPS { t0 } else { t1 };
                let rr = p0 + v * t;
                let dist = (q - rr).magnitude();
                if dist < min_dist {
                    min_dist = dist;
                    best_index = i as i64;
                    best_t = t;
                }
            }
        }
        if best_index == -1 {
            best_index = 0;
            for i in 0..NUM_CENTERLINE_POINTS {
                let dist = (self.center_line[i].point - q).magnitude();
                if dist < min_dist {
                    min_dist = dist;
                    best_index = i as i64;
                    best_t = 0.0;
                }
            }
        }
        if best_index == NUM_CENTERLINE_POINTS as i64 - 1 {
            best_index = NUM_CENTERLINE_POINTS as i64 - 2;
            best_t = 1.0;
        }
        let b = best_index as usize;
        self.center_line[b].pos + best_t * (self.center_line[b + 1].pos - self.center_line[b].pos)
    }

    /// The tongue-side parameter as a minimum area (cm^2) of the cross-sections.
    pub fn tongue_side_param_to_min_area_cm2(v: f64) -> f64 {
        let mut min_area = 0.0;
        if v > 0.2 {
            const MAX_AREA: f64 = 0.15;
            min_area = MAX_AREA * (v - 0.2) / 0.2;
            if min_area > MAX_AREA { min_area = MAX_AREA; }
        } else if v < -0.05 {
            const MAX_AREA: f64 = 0.25;
            min_area = MAX_AREA * (v + 0.05) / (-0.2);
            if min_area > MAX_AREA { min_area = MAX_AREA; }
        }
        min_area
    }

    pub fn calc_cross_sections(&mut self) {
        let tongue_tip_radius = self.anatomy.tongue_tip_radius;
        for i in 0..NUM_CENTERLINE_POINTS {
            let mut upper = [0.0; NUM_PROFILE_SAMPLES];
            let mut lower = [0.0; NUM_PROFILE_SAMPLES];
            let (pt, n) = (self.center_line[i].point, self.center_line[i].normal);
            let articulator = self.get_cross_profiles(pt, n, &mut upper, &mut lower, true);
            let (area, circ) = Self::get_cross_section(&upper, &lower);
            self.cross_section[i] = CrossSection { area, circ, pos: self.center_line[i].pos, articulator };
        }
        let port_rib = NUM_LARYNX_RIBS + NUM_PHARYNX_RIBS;
        let np = self.surface[si::UPPER_COVER].num_rib_points;
        self.nasal_port_area_cm2 = self.anatomy.max_nasal_port_area * self.param[VO].x;
        if self.nasal_port_area_cm2 < 0.0 { self.nasal_port_area_cm2 = 0.0; }
        let q = self.surface[si::UPPER_COVER].get_vertex(port_rib, np / 2).to_p2();
        self.nasal_port_pos_cm = self.get_center_line_pos(q);

        const EPS: f64 = 0.000001;
        let teeth_x = self.surface[si::UPPER_TEETH].get_vertex(NUM_TEETH_RIBS - 1, 2).x;
        for i in 0..NUM_CENTERLINE_POINTS - 1 {
            let p = self.center_line[i].point;
            let q = self.center_line[i + 1].point;
            if p.x < teeth_x && q.x >= teeth_x {
                let mut t = q.x - p.x;
                if t < EPS { t = EPS; }
                self.incisor_pos_cm = self.center_line[i].pos + ((teeth_x - p.x) / t) * (q - p).magnitude();
            }
        }
        for i in 0..NUM_CENTERLINE_POINTS {
            let p = self.center_line[i].point;
            let w = self.center_line[i].normal;
            let q = p2(self.param[TTX].limited_x, self.param[TTY].limited_x);
            let r = q - p;
            if r.y * w.x - r.x * w.y > tongue_tip_radius && p.x > self.param[TCX].limited_x && self.cross_section[i].articulator == Articulator::Tongue {
                self.cross_section[i].articulator = Articulator::Other;
            }
        }
        const LEFT_INCISOR_MARGIN_CM: f64 = 0.5;
        const RIGHT_INCISOR_MARGIN_CM: f64 = 0.3;
        const MIN_INCISOR_AREA_CM2: f64 = 0.15;
        for i in 0..NUM_CENTERLINE_POINTS {
            let cs = &mut self.cross_section[i];
            if cs.pos >= self.incisor_pos_cm - LEFT_INCISOR_MARGIN_CM && cs.pos <= self.incisor_pos_cm + RIGHT_INCISOR_MARGIN_CM && cs.area < MIN_INCISOR_AREA_CM2 {
                cs.area = MIN_INCISOR_AREA_CM2;
                cs.circ = 2.0 * (MIN_INCISOR_AREA_CM2 * PI).sqrt();
            }
        }
        let mut min_area = [0.0f64; 2];
        min_area[0] = Self::tongue_side_param_to_min_area_cm2(self.param[TS2].x);
        if self.param[TS2].x < 0.0 { min_area[0] = 0.0; }
        min_area[1] = Self::tongue_side_param_to_min_area_cm2(self.param[TS3].x);
        let min_circ = [2.0 * (min_area[0] * PI).sqrt(), 2.0 * (min_area[1] * PI).sqrt()];
        let mut tongue_tip_right_cm = 0.0;
        let mut lower_lip_left_cm = 1_000_000.0;
        for i in 0..NUM_CENTERLINE_POINTS {
            let pos_cm = self.cross_section[i].pos;
            if self.cross_section[i].articulator == Articulator::Tongue { tongue_tip_right_cm = pos_cm; }
            if self.cross_section[i].articulator == Articulator::LowerLip && pos_cm < lower_lip_left_cm { lower_lip_left_cm = pos_cm; }
        }
        let tongue_tip_left_cm = tongue_tip_right_cm - 2.0;
        for i in 0..NUM_CENTERLINE_POINTS {
            let cs = &mut self.cross_section[i];
            if cs.pos <= tongue_tip_left_cm {
                if cs.area < min_area[0] { cs.area = min_area[0]; }
                if cs.circ < min_circ[0] { cs.circ = min_circ[0]; }
            }
            if cs.pos >= tongue_tip_left_cm && cs.pos <= lower_lip_left_cm {
                if cs.area < min_area[1] { cs.area = min_area[1]; }
                if cs.circ < min_circ[1] { cs.circ = min_circ[1]; }
            }
        }
    }

    /// Resamples the cross-sections into the 40 tube sections of the acoustic model.
    pub fn cross_sections_to_tube_sections(&mut self) {
        const EPS: f64 = 0.000001;
        for i in 0..NUM_TUBE_PHARYNX_SECTIONS {
            self.tube_section[i].pos = self.nasal_port_pos_cm * i as f64 / NUM_TUBE_PHARYNX_SECTIONS as f64;
        }
        let num_mouth = NUM_TUBE_SECTIONS - NUM_TUBE_PHARYNX_SECTIONS;
        let mouth_length = self.center_line_length - self.nasal_port_pos_cm;
        let first_section_length = mouth_length / 16.0;
        let last_section_length = mouth_length / 32.0;
        let im = num_mouth as f64;
        let im2 = im * num_mouth as f64;
        let im3 = im2 * num_mouth as f64;
        let im4 = im3 * num_mouth as f64;
        let a1 = first_section_length;
        let a2 = ((mouth_length - a1 * im) * 3.0 * im2 - im3 * (last_section_length - a1)) / im4;
        let a3 = (im2 * (last_section_length - a1) - 2.0 * im * (mouth_length - a1 * im)) / im4;
        for i in NUM_TUBE_PHARYNX_SECTIONS..NUM_TUBE_SECTIONS {
            let k = (i - NUM_TUBE_PHARYNX_SECTIONS) as f64;
            self.tube_section[i].pos = self.nasal_port_pos_cm + a1 * k + a2 * k * k + a3 * k * k * k;
        }
        for i in 0..NUM_TUBE_SECTIONS {
            self.tube_section[i].length = if i < NUM_TUBE_SECTIONS - 1 {
                self.tube_section[i + 1].pos - self.tube_section[i].pos
            } else {
                self.center_line_length - self.tube_section[i].pos
            };
        }
        const EXTREME: f64 = 1_000_000.0;
        let cs = self.cross_section.clone();
        let mut left_index = 0usize;
        let mut right_index = 0usize;
        for i in 0..NUM_TUBE_SECTIONS {
            let left_x = self.tube_section[i].pos;
            let right_x = left_x + self.tube_section[i].length;
            while cs[left_index].pos > left_x && left_index > 0 { left_index -= 1; }
            while cs[left_index + 1].pos < left_x && left_index < NUM_CENTERLINE_POINTS - 2 { left_index += 1; }
            while cs[right_index].pos > right_x && right_index > 0 { right_index -= 1; }
            while cs[right_index + 1].pos < right_x && right_index < NUM_CENTERLINE_POINTS - 2 { right_index += 1; }
            if left_index == right_index {
                self.tube_section[i].articulator = cs[left_index].articulator;
            } else {
                let mut m = left_index + 1;
                for k in left_index + 1..=right_index {
                    if cs[k].area < cs[m].area { m = k; }
                }
                self.tube_section[i].articulator = cs[m].articulator;
            }
            let (mut area, mut circ) = (EXTREME, EXTREME);
            for k in left_index..=right_index {
                if k == left_index && k == right_index {
                    let mut length = cs[left_index + 1].pos - cs[left_index].pos;
                    if length < EPS { length = EPS; }
                    let dy = cs[left_index + 1].area - cs[left_index].area;
                    let mut y = cs[left_index].area + dy * (left_x - cs[left_index].pos) / length;
                    if y < area { area = y; }
                    y = cs[left_index].area + dy * (right_x - cs[left_index].pos) / length;
                    if y < area { area = y; }
                    let dy = cs[left_index + 1].circ - cs[left_index].circ;
                    y = cs[left_index].circ + dy * (left_x - cs[left_index].pos) / length;
                    if y < circ { circ = y; }
                    y = cs[left_index].circ + dy * (right_x - cs[left_index].pos) / length;
                    if y < circ { circ = y; }
                } else if k == left_index {
                    let mut length = cs[left_index + 1].pos - cs[left_index].pos;
                    if length < EPS { length = EPS; }
                    let dy = cs[left_index + 1].area - cs[left_index].area;
                    let mut y = cs[left_index].area + dy * (left_x - cs[left_index].pos) / length;
                    if y < area { area = y; }
                    y = cs[left_index + 1].area;
                    if y < area { area = y; }
                    let dy = cs[left_index + 1].circ - cs[left_index].circ;
                    y = cs[left_index].circ + dy * (left_x - cs[left_index].pos) / length;
                    if y < circ { circ = y; }
                    y = cs[left_index + 1].circ;
                    if y < circ { circ = y; }
                } else if k == right_index {
                    let mut length = cs[right_index + 1].pos - cs[right_index].pos;
                    if length < EPS { length = EPS; }
                    let dy = cs[right_index + 1].area - cs[right_index].area;
                    let mut y = cs[right_index].area;
                    if y < area { area = y; }
                    y = cs[right_index].area + dy * (right_x - cs[right_index].pos) / length;
                    if y < area { area = y; }
                    let dy = cs[right_index + 1].circ - cs[right_index].circ;
                    y = cs[right_index].circ;
                    if y < circ { circ = y; }
                    y = cs[right_index].circ + dy * (right_x - cs[right_index].pos) / length;
                    if y < circ { circ = y; }
                } else {
                    if cs[k].area < area { area = cs[k].area; }
                    if cs[k + 1].area < area { area = cs[k + 1].area; }
                    if cs[k].circ < circ { circ = cs[k].circ; }
                    if cs[k + 1].circ < circ { circ = cs[k + 1].circ; }
                }
            }
            self.tube_section[i].area = area;
            self.tube_section[i].circ = circ;
        }
    }

    /// Calculates everything from the articulatory parameters (clamps them first).
    pub fn calculate_all(&mut self) {
        for i in 0..NUM_PARAMS {
            self.restrict_param(i);
            self.param[i].limited_x = self.param[i].x;
        }
        self.calc_surfaces();
        self.calc_center_line();
        self.calc_cross_sections();
        self.cross_sections_to_tube_sections();
    }

    /// The acoustic tube for the current articulation (oral part; trachea and glottis keep
    /// their defaults).
    pub fn get_tube(&self) -> crate::tube::Tube {
        let mut tube = crate::tube::Tube::default();
        let lengths: Vec<f64> = self.tube_section.iter().map(|t| t.length).collect();
        let areas: Vec<f64> = self.tube_section.iter().map(|t| t.area).collect();
        let tags: Vec<Articulator> = self.tube_section.iter().map(|t| t.articulator).collect();
        tube.set_supraglottal(&lengths, &areas);
        tube.set_articulators(&tags);
        tube.teeth_position_cm = self.incisor_pos_cm;
        tube.tongue_tip_side_elevation = self.param[TS3].x;
        tube
    }

    // ------------------------------------------------------------------------------------
    // Tongue
    // ------------------------------------------------------------------------------------

    /// Elevation of the tongue sides (cm) belonging to a tongue-side parameter value.
    pub fn tongue_side_param_to_elevation_cm(v: f64) -> f64 {
        if v >= 0.0 {
            const MAX_ELEVATION_CM: f64 = 1.0;
            let e = MAX_ELEVATION_CM * (v / 0.3);
            if e > MAX_ELEVATION_CM { MAX_ELEVATION_CM } else { e }
        } else {
            const MIN_ELEVATION_CM: f64 = -1.0;
            let e = MIN_ELEVATION_CM * (v / -0.3);
            if e < MIN_ELEVATION_CM { MIN_ELEVATION_CM } else { e }
        }
    }

    fn verify_tongue_rib_normal(&mut self, rigid: usize, flexible: usize) {
        let r = self.tongue_rib[rigid].point + self.tongue_rib[rigid].normal * self.tongue_rib[rigid].min_y;
        let w = self.tongue_rib[rigid].point + self.tongue_rib[rigid].normal * self.tongue_rib[rigid].max_y - r;
        let p = self.tongue_rib[flexible].point;
        let v = self.tongue_rib[flexible].normal;
        let q = p - r;
        let denominator = v.x * w.y - v.y * w.x;
        if denominator != 0.0 {
            let s = (v.x * q.y - q.x * v.y) / denominator;
            if s >= 0.0 && s <= 1.0 {
                let t = (w.x * q.y - q.x * w.y) / denominator;
                if t <= 0.0 && t >= self.tongue_rib[flexible].min_y {
                    let mut n = self.tongue_rib[flexible].point - r;
                    n.normalize();
                    self.tongue_rib[flexible].normal = n;
                } else if t >= 0.0 && t <= self.tongue_rib[flexible].max_y {
                    let mut n = r + w - self.tongue_rib[flexible].point;
                    n.normalize();
                    self.tongue_rib[flexible].normal = n;
                }
            }
        }
    }

    /// Largest displacement from `a` towards `c` of an ellipse (radii rx, ry) that does not
    /// cross the border polyline; returns the limited centre position.
    fn limit_ellipse_pos(c: P2, rx: f64, ry: f64, border: &LineStrip2D, a: P2) -> P2 {
        const EPS: f64 = 0.000001;
        let n = border.num_points();
        let (rx2, ry2) = (rx * rx, ry * ry);
        let mut final_t = (c - a).magnitude();
        let mut v = c - a;
        v.normalize();
        for i in 0..n {
            let p0 = border.get_control_point(i as i64);
            let mut q = a - p0;
            let mut denominator = v.x * v.x * ry2 + v.y * v.y * rx2;
            if denominator.abs() > EPS {
                let p = 2.0 * (q.x * v.x * ry2 + q.y * v.y * rx2) / denominator;
                let qq = (q.x * q.x * ry2 + q.y * q.y * rx2 - rx2 * ry2) / denominator;
                let mut root = 0.25 * p * p - qq;
                if root >= 0.0 {
                    root = root.sqrt();
                    let t = -0.5 * p - root;
                    if t >= 0.0 && t < final_t { final_t = t; }
                }
            }
            if i < n - 1 {
                let p1 = border.get_control_point(i as i64 + 1);
                let w = p1 - p0;
                let alpha_rad = (-ry * w.x).atan2(rx * w.y);
                let u = p2(rx * alpha_rad.cos(), ry * alpha_rad.sin());
                denominator = v.x * w.y - v.y * w.x;
                if denominator.abs() > EPS {
                    q = a - p0 + u;
                    let s = (q.y * v.x - q.x * v.y) / denominator;
                    if s >= 0.0 && s <= 1.0 {
                        let t = (q.y * w.x - q.x * w.y) / denominator;
                        if t >= 0.0 && t < final_t { final_t = t; }
                    }
                    q = a - p0 - u;
                    let s = (q.y * v.x - q.x * v.y) / denominator;
                    if s >= 0.0 && s <= 1.0 {
                        let t = (q.y * w.x - q.x * w.y) / denominator;
                        if t >= 0.0 && t < final_t { final_t = t; }
                    }
                }
            }
        }
        a + v * final_t
    }

    /// Keeps the tongue parameters in a range where the tongue stays inside the vocal tract.
    /// Like the original this leaves the restricted values in `param[].x` (except for the
    /// tongue body and tip positions, which are restored); the limited values go to `limited_x`.
    fn restrict_tongue_params(&mut self) {
        const EPS: f64 = 0.000001;
        const DELTA: f64 = 0.3;
        let tongue_tip_radius = self.anatomy.tongue_tip_radius;
        let (orig_ttx, orig_tty, orig_tcx, orig_tcy) = (self.param[TTX].x, self.param[TTY].x, self.param[TCX].x, self.param[TCY].x);
        for i in [TCX, TCY, TTX, TTY] { self.restrict_param(i); }

        let q0 = self.surface[si::UPPER_COVER].get_vertex(NUM_UPPER_COVER_RIBS - NUM_PALATE_RIBS + 1, NUM_UPPER_COVER_POINTS - 1).to_p2();
        let q1 = self.surface[si::UPPER_COVER].get_vertex(NUM_UPPER_COVER_RIBS - 1, NUM_UPPER_COVER_POINTS - 1).to_p2();
        let mut a = p2(q0.x, q1.y - (q1.x - q0.x));
        let mut upper_border = LineStrip2D::new();
        for i in NUM_LARYNX_RIBS..NUM_UPPER_COVER_RIBS {
            upper_border.add_point(self.surface[si::UPPER_COVER].get_vertex(i, NUM_UPPER_COVER_POINTS - 1).to_p2());
        }
        for i in 0..2 {
            upper_border.add_point(self.surface[si::UPPER_TEETH].get_vertex(NUM_TEETH_RIBS - 1, i).to_p2());
        }
        let p0 = upper_border.get_control_point(upper_border.num_points() as i64 - 1);
        upper_border.add_point(p2(p0.x, p0.y - 10.0));
        let n = upper_border.num_points();
        for i in 0..n {
            let mut c = upper_border.get_control_point(i as i64);
            let mut d = c - a;
            d.normalize();
            c += d * DELTA;
            if c.x > p0.x { c.x = p0.x; }
            upper_border.set_point(i, c);
        }
        let mut lower_border_tt = LineStrip2D::new();
        lower_border_tt.reset(1);
        for i in NUM_LARYNX_RIBS - 1..NUM_LOWER_COVER_RIBS {
            lower_border_tt.add_point(self.surface[si::LOWER_COVER].get_vertex(i, NUM_LOWER_COVER_POINTS - 1).to_p2());
        }
        for i in 0..3 {
            lower_border_tt.add_point(self.surface[si::LOWER_TEETH].get_vertex(NUM_TEETH_RIBS - 1, i).to_p2());
        }
        let mut c = lower_border_tt.get_control_point(1);
        c.x -= 10.0;
        lower_border_tt.set_point(0, c);

        let mut lower_border_tb = LineStrip2D::new();
        lower_border_tb.reset(1);
        let p0 = self.surface[si::LOWER_COVER].get_vertex(NUM_LARYNX_RIBS - 1, NUM_LOWER_COVER_POINTS - 1).to_p2();
        let pp2 = self.surface[si::LOWER_COVER].get_vertex(NUM_LOWER_COVER_RIBS - 1, NUM_LOWER_COVER_POINTS - 1).to_p2();
        let mut p1 = p2(pp2.x - 1.0, p0.y);
        if p1.x < p0.x { p1.x = p0.x; }
        lower_border_tb.add_point(p0);
        lower_border_tb.add_point(p1);
        lower_border_tb.add_point(pp2);
        for i in 0..3 {
            lower_border_tb.add_point(self.surface[si::LOWER_TEETH].get_vertex(NUM_TEETH_RIBS - 1, i).to_p2());
        }
        let mut c = lower_border_tb.get_control_point(1);
        c.x -= 10.0;
        lower_border_tb.set_point(0, c);

        let rx = self.anatomy.tongue_center_radius_x;
        let ry = self.anatomy.tongue_center_radius_y;
        if self.param[TTX].x < self.param[TCX].x + rx + tongue_tip_radius {
            self.param[TTX].x = self.param[TCX].x + rx + tongue_tip_radius;
        }
        let mut c = Self::limit_ellipse_pos(p2(self.param[TTX].x, self.param[TTY].x), tongue_tip_radius, tongue_tip_radius, &upper_border, a);
        self.param[TTX].x = c.x;
        self.param[TTY].x = c.y;
        if self.param[TCX].x > self.param[TTX].x - rx - tongue_tip_radius {
            self.param[TCX].x = self.param[TTX].x - rx - tongue_tip_radius;
        }
        c = Self::limit_ellipse_pos(p2(self.param[TCX].x, self.param[TCY].x), rx, ry, &upper_border, a);
        self.param[TCX].x = c.x;
        self.param[TCY].x = c.y;
        if self.param[TTX].x < self.param[TCX].x + rx + tongue_tip_radius {
            self.param[TTX].x = self.param[TCX].x + rx + tongue_tip_radius;
        }
        a.y = 1000.0;
        c = Self::limit_ellipse_pos(p2(self.param[TTX].x, self.param[TTY].x), tongue_tip_radius, tongue_tip_radius, &lower_border_tt, a);
        self.param[TTX].x = c.x;
        self.param[TTY].x = c.y;
        c = Self::limit_ellipse_pos(p2(self.param[TCX].x, self.param[TCY].x), rx, ry, &lower_border_tb, a);
        self.param[TCX].x = c.x;
        self.param[TCY].x = c.y;

        self.restrict_param(TRX);
        self.restrict_param(TRY);
        let mut r = rx;
        if ry > r { r = ry; }
        let mut alpha_rad = ry.atan2(rx);
        if alpha_rad <= PI && alpha_rad >= -PI { alpha_rad += PI; }
        c.x = self.param[TCX].x + alpha_rad.cos() * (rx + EPS);
        c.y = self.param[TCY].x + alpha_rad.sin() * (ry + EPS);
        let h = self.surface[si::LOWER_COVER].get_vertex(NUM_LARYNX_RIBS - 1, NUM_LOWER_COVER_POINTS - 1).to_p2();
        if self.param[TRY].x < h.y { self.param[TRY].x = h.y; }
        if self.param[TRY].x > self.param[TCY].x { self.param[TRY].x = self.param[TCY].x; }
        if self.param[TRX].x - c.x > c.y - self.param[TRY].x {
            self.param[TRX].x = c.x + c.y - self.param[TRY].x;
        }
        let min_x = self.param[TCX].x - rx - (self.param[TCY].x - h.y);
        if self.param[TRX].x < min_x { self.param[TRX].x = min_x; }
        let mut max_x = self.param[TCX].x - r;
        if h.x + 0.5 > max_x { max_x = h.x + 0.5; }
        if self.param[TRX].x > max_x { self.param[TRX].x = max_x; }

        self.restrict_param(TBX);
        self.restrict_param(TBY);
        if self.param[TBX].x < self.param[TCX].x { self.param[TBX].x = self.param[TCX].x; }
        if self.param[TBX].x > self.param[TTX].x { self.param[TBX].x = self.param[TTX].x; }
        r = rx;
        if ry > r { r = ry; }
        r += EPS;
        c.x = self.param[TCX].x;
        c.y = self.param[TCY].x + 1.415 * r;
        if self.param[TBY].x - c.y > self.param[TBX].x - c.x { self.param[TBY].x = c.y + self.param[TBX].x - c.x; }
        if self.param[TBY].x - c.y < -(self.param[TBX].x - c.x) { self.param[TBY].x = c.y - (self.param[TBX].x - c.x); }
        r = tongue_tip_radius + EPS;
        c.x = self.param[TTX].x;
        c.y = self.param[TTY].x + 1.415 * r;
        if self.param[TBY].x - c.y < self.param[TBX].x - c.x { self.param[TBY].x = c.y + self.param[TBX].x - c.x; }

        self.restrict_param(TS1);
        self.restrict_param(TS2);
        self.restrict_param(TS3);
        for i in [TCX, TCY, TTX, TTY, TBX, TBY, TRX, TRY, TS1, TS2, TS3] {
            self.param[i].limited_x = self.param[i].x;
        }
        self.param[TTX].x = orig_ttx;
        self.param[TTY].x = orig_tty;
        self.param[TCX].x = orig_tcx;
        self.param[TCY].x = orig_tcy;
    }

    /// Builds the tongue mesh: each rib is a curve across the tongue whose width follows the
    /// free space between the palate and the tongue, with raised sides.
    fn calc_tongue_ribs(&mut self) {
        const INVALID: f64 = INVALID_PROFILE_SAMPLE;
        let width_factor = -2.0 * self.palate_points[0].z / 4.6;
        let half_width = 1.75 * width_factor;
        for i in 0..NUM_TONGUE_RIBS {
            self.tongue_rib[i].left = p2(-half_width, self.tongue_rib[i].left_side_height);
            self.tongue_rib[i].right = p2(half_width, self.tongue_rib[i].right_side_height);
        }
        let target_rib = NUM_LARYNX_RIBS - 1;
        for k in 0..NUM_TONGUE_POINTS {
            let target_point = 2 + k * (NUM_LOWER_COVER_POINTS * 2 - 1 - 1 - 4) / (NUM_TONGUE_POINTS - 1);
            let q = if target_point < NUM_LOWER_COVER_POINTS {
                self.surface[si::LOWER_COVER].get_vertex(target_rib, target_point)
            } else {
                let mut q = self.surface[si::LOWER_COVER].get_vertex(target_rib, 2 * NUM_LOWER_COVER_POINTS - 1 - target_point);
                q.z = -q.z;
                q
            };
            self.surface[si::TONGUE].set_vertex(0, k, q);
        }
        self.tongue_rib[0].min_x = self.surface[si::TONGUE].get_vertex(0, 0).z;
        self.tongue_rib[0].max_x = self.surface[si::TONGUE].get_vertex(0, NUM_TONGUE_POINTS - 1).z;

        const N2: usize = NUM_PROFILE_SAMPLES / 2;
        let mut a2_left = [0.0; NUM_TONGUE_RIBS];
        let mut a3_left = [0.0; NUM_TONGUE_RIBS];
        let mut a2_right = [0.0; NUM_TONGUE_RIBS];
        let mut a3_right = [0.0; NUM_TONGUE_RIBS];
        let mut upper_profile = vec![[0.0f64; NUM_PROFILE_SAMPLES]; NUM_TONGUE_RIBS];
        let mut lower_profile = vec![[0.0f64; NUM_PROFILE_SAMPLES]; NUM_TONGUE_RIBS];
        for i in 1..NUM_TONGUE_RIBS {
            let (pt, nrm) = (self.tongue_rib[i].point, self.tongue_rib[i].normal);
            let (mut up, mut lo) = ([0.0; NUM_PROFILE_SAMPLES], [0.0; NUM_PROFILE_SAMPLES]);
            self.get_cross_profiles(pt, nrm, &mut up, &mut lo, false);
            upper_profile[i] = up;
            lower_profile[i] = lo;

            let left_edge = self.tongue_rib[i].left;
            a2_left[i] = 3.0 * left_edge.y / (left_edge.x * left_edge.x);
            a3_left[i] = -2.0 * left_edge.y / (left_edge.x * left_edge.x * left_edge.x);
            self.tongue_rib[i].min_x = left_edge.x;
            let mut last_inside = false;
            let mut ok = false;
            for k in 0..N2 {
                let x = -0.5 * PROFILE_LENGTH + k as f64 * PROFILE_SAMPLE_LENGTH;
                let mut is_inside = false;
                if upper_profile[i][k] != INVALID && lower_profile[i][k] != INVALID && x >= left_edge.x {
                    let y = a2_left[i] * x * x + a3_left[i] * x * x * x;
                    if y >= lower_profile[i][k] && y <= upper_profile[i][k] { is_inside = true; }
                }
                if !last_inside && is_inside {
                    self.tongue_rib[i].min_x = x;
                    ok = true;
                }
                last_inside = is_inside;
            }
            if !ok { self.tongue_rib[i].min_x = -0.1; }

            let right_edge = self.tongue_rib[i].right;
            a2_right[i] = 3.0 * right_edge.y / (right_edge.x * right_edge.x);
            a3_right[i] = -2.0 * right_edge.y / (right_edge.x * right_edge.x * right_edge.x);
            self.tongue_rib[i].max_x = right_edge.x;
            let mut last_inside = false;
            let mut ok = false;
            for k in (N2 + 1..NUM_PROFILE_SAMPLES).rev() {
                let x = k as f64 * PROFILE_SAMPLE_LENGTH - 0.5 * PROFILE_LENGTH;
                let mut is_inside = false;
                if upper_profile[i][k] != INVALID && lower_profile[i][k] != INVALID && x <= right_edge.x {
                    let y = a2_right[i] * x * x + a3_right[i] * x * x * x;
                    if y >= lower_profile[i][k] && y <= upper_profile[i][k] { is_inside = true; }
                }
                if !last_inside && is_inside {
                    self.tongue_rib[i].max_x = x;
                    ok = true;
                }
                last_inside = is_inside;
            }
            if !ok { self.tongue_rib[i].max_x = 0.1; }

            let mut min_width = if i < 5 {
                1.0 + 0.3 * i as f64 / 5.0
            } else if i >= NUM_TONGUE_RIBS - 10 {
                let k = i - (NUM_TONGUE_RIBS - 10);
                1.3 - 0.5 * k as f64 / 10.0
            } else {
                1.3
            };
            min_width *= width_factor;
            if self.tongue_rib[i].max_x < min_width { self.tongue_rib[i].max_x = min_width; }
            if self.tongue_rib[i].min_x > -min_width { self.tongue_rib[i].min_x = -min_width; }
        }
        const LOWPASS_COEFF: f64 = 0.25;
        for i in 1..NUM_TONGUE_RIBS {
            let mut z = self.tongue_rib[i - 1].max_x + LOWPASS_COEFF * (self.tongue_rib[i].max_x - self.tongue_rib[i - 1].max_x);
            if z <= self.tongue_rib[i].max_x { self.tongue_rib[i].max_x = z; }
            z = self.tongue_rib[i - 1].min_x + LOWPASS_COEFF * (self.tongue_rib[i].min_x - self.tongue_rib[i - 1].min_x);
            if z >= self.tongue_rib[i].min_x { self.tongue_rib[i].min_x = z; }
        }
        for i in (0..=NUM_TONGUE_RIBS - 2).rev() {
            let mut z = self.tongue_rib[i + 1].max_x + LOWPASS_COEFF * (self.tongue_rib[i].max_x - self.tongue_rib[i + 1].max_x);
            if z <= self.tongue_rib[i].max_x { self.tongue_rib[i].max_x = z; }
            z = self.tongue_rib[i + 1].min_x + LOWPASS_COEFF * (self.tongue_rib[i].min_x - self.tongue_rib[i + 1].min_x);
            if z >= self.tongue_rib[i].min_x { self.tongue_rib[i].min_x = z; }
        }
        for i in 1..NUM_TONGUE_RIBS {
            let rib = self.tongue_rib[i];
            for k in 0..NUM_TONGUE_POINTS {
                let z = rib.min_x + (rib.max_x - rib.min_x) * k as f64 / (NUM_TONGUE_POINTS - 1) as f64;
                let mut t = if k <= NUM_TONGUE_POINTS / 2 {
                    a2_left[i] * z * z + a3_left[i] * z * z * z
                } else {
                    a2_right[i] * z * z + a3_right[i] * z * z * z
                };
                let mut index = ((z + 0.5 * PROFILE_LENGTH) / PROFILE_SAMPLE_LENGTH) as i64;
                let mut ratio = (z + 0.5 * PROFILE_LENGTH - index as f64 * PROFILE_SAMPLE_LENGTH) / PROFILE_SAMPLE_LENGTH;
                if index < 0 {
                    index = 0;
                    ratio = 0.0;
                }
                if index > NUM_PROFILE_SAMPLES as i64 - 2 {
                    index = NUM_PROFILE_SAMPLES as i64 - 2;
                    ratio = 1.0;
                }
                let idx = index as usize;
                if upper_profile[i][idx] != INVALID && upper_profile[i][idx + 1] != INVALID {
                    let y = upper_profile[i][idx] + ratio * (upper_profile[i][idx + 1] - upper_profile[i][idx]) - 0.05;
                    if t > y { t = y; }
                }
                self.surface[si::TONGUE].set_vertex(i, k, p3(rib.point.x + rib.normal.x * t, rib.point.y + rib.normal.y * t, z));
            }
        }
    }

    fn calc_tongue(&mut self) {
        const EPS: f64 = 0.000001;
        if self.anatomy.automatic_tongue_root_calc {
            let c = p2(self.param[TCX].limited_x, self.param[TCY].limited_x);
            let h = self.surface[si::LOWER_COVER].get_vertex(NUM_LARYNX_RIBS - 1, NUM_LOWER_COVER_POINTS - 1).to_p2();
            let distance = (c - h).magnitude();
            self.param[TRX].x = self.anatomy.trx_slope * distance + self.anatomy.trx_intercept;
            self.param[TRY].x = self.anatomy.try_slope * self.param[TCX].x + self.anatomy.try_intercept;
        }
        self.restrict_tongue_params();

        let c0 = p2(self.param[TCX].limited_x, self.param[TCY].limited_x);
        let c1 = p2(self.param[TTX].limited_x, self.param[TTY].limited_x);
        let (r0x, r0y) = (self.anatomy.tongue_center_radius_x, self.anatomy.tongue_center_radius_y);
        let r1 = self.anatomy.tongue_tip_radius;
        let weight = [1.0, 2.0, 1.0];
        let mut q = [P3::default(); 3];
        let mut alpha = [0.0f64; 4];

        q[0] = self.surface[si::LOWER_COVER].get_vertex(NUM_LARYNX_RIBS - 1, NUM_LOWER_COVER_POINTS - 1);
        alpha[0] = get_ellipse_tangent(q[0].to_p2(), c0, r0x, r0y, true);
        q[2] = p3(c0.x + r0x * alpha[0].cos(), c0.y + r0y * alpha[0].sin(), 0.0);
        q[1] = p3(self.param[TRX].x, self.param[TRY].x, 0.0);
        alpha[0] = get_ellipse_tangent(q[1].to_p2(), c0, r0x, r0y, true);
        q[2] = p3(c0.x + r0x * alpha[0].cos(), c0.y + r0y * alpha[0].sin(), 0.0);
        let mut root_curve = Bezier3D::new();
        root_curve.set_points_weighted(&q, &weight);

        q[1] = p3(self.param[TBX].x, self.param[TBY].x, 0.0);
        alpha[1] = get_ellipse_tangent(q[1].to_p2(), c0, r0x, r0y, false);
        if alpha[1] > alpha[0] && alpha[1] - alpha[0] < 0.25 * PI { alpha[1] = alpha[0]; }
        q[0] = p3(c0.x + r0x * alpha[1].cos(), c0.y + r0y * alpha[1].sin(), 0.0);
        alpha[2] = get_circle_tangent(q[1].to_p2(), c1, r1, true);
        q[2] = p3(c1.x + r1 * alpha[2].cos(), c1.y + r1 * alpha[2].sin(), 0.0);
        let mut blade_curve = Bezier3D::new();
        blade_curve.set_points_weighted(&q, &weight);

        let mut target_curve = LineStrip2D::new();
        target_curve.reset(0);
        let mut tongue_side_index = [0usize; 5];
        for i in 0..32 {
            let pt = root_curve.get_point(i as f64 / 32.0).to_p2();
            target_curve.add_point(pt);
        }
        tongue_side_index[1] = target_curve.num_points() - 8;
        if alpha[0] < 0.0 { alpha[0] += 2.0 * PI; }
        let mut delta = alpha[1] - alpha[0];
        if delta > -EPS { delta = -EPS; }
        for i in 0..32 {
            let angle = alpha[0] + delta * i as f64 / 32.0;
            target_curve.add_point(p2(c0.x + r0x * angle.cos(), c0.y + r0y * angle.sin()));
        }
        tongue_side_index[2] = target_curve.num_points() - 8;
        for i in 0..32 {
            let pt = blade_curve.get_point(i as f64 / 32.0).to_p2();
            target_curve.add_point(pt);
        }
        tongue_side_index[3] = target_curve.num_points() - 1;
        alpha[3] = 0.0;
        if alpha[2] < 0.0 { alpha[2] += 2.0 * PI; }
        delta = alpha[3] - alpha[2];
        if delta > -EPS { delta = -EPS; }
        for i in 0..8 {
            let angle = alpha[2] + delta * i as f64 / 7.0;
            target_curve.add_point(p2(c1.x + r1 * angle.cos(), c1.y + r1 * angle.sin()));
        }
        tongue_side_index[4] = target_curve.num_points() - 1;

        let mut elevation = [0.0; 5];
        elevation[0] = 1.0;
        elevation[1] = Self::tongue_side_param_to_elevation_cm(self.param[TS1].x);
        elevation[2] = Self::tongue_side_param_to_elevation_cm(self.param[TS2].x);
        elevation[3] = Self::tongue_side_param_to_elevation_cm(self.param[TS3].x);
        elevation[4] = elevation[3];
        if elevation[4] > 0.0 { elevation[4] = 0.0; }
        let mut left_side_height = LineStrip2D::new();
        let mut right_side_height = LineStrip2D::new();
        left_side_height.reset(0);
        for i in 0..5 {
            let t = target_curve.get_curve_param(tongue_side_index[i] as i64);
            left_side_height.add_point(p2(t, elevation[i]));
        }
        right_side_height.reset(0);
        for i in 0..5 {
            let t = target_curve.get_curve_param(tongue_side_index[i] as i64);
            right_side_height.add_point(p2(t, elevation[i]));
        }

        // The target curve must not leave the vocal tract: push points inside the upper outline.
        let center = p2(0.0, -1.5);
        let n_points = target_curve.num_points();
        for i in 0..n_points {
            let mut n = p2(0.0, 0.0);
            let c = target_curve.get_control_point(i as i64);
            if c.x <= center.x && c.y <= center.y {
                n = p2(1.0, 0.0);
            } else if c.x >= center.x && c.y >= center.y {
                n = p2(0.0, -1.0);
            } else if c.x <= center.x && c.y >= center.y {
                n = center - c;
            }
            if n.x != 0.0 || n.y != 0.0 {
                n.normalize();
                if let Some((t, intersection)) = self.upper_outline.get_special_intersection(c, n) {
                    if t > 0.0 { target_curve.set_point(i, intersection); }
                }
            }
        }

        for i in 0..NUM_DYNAMIC_TONGUE_RIBS {
            let t = i as f64 / (NUM_DYNAMIC_TONGUE_RIBS - 1) as f64;
            self.tongue_rib[i].point = target_curve.get_point(t);
            self.tongue_rib[i].left_side_height = left_side_height.get_function_value(t);
            self.tongue_rib[i].right_side_height = right_side_height.get_function_value(t);
        }
        const FILTER_COEFF: f64 = 0.5;
        for i in 1..NUM_DYNAMIC_TONGUE_RIBS {
            let (pl, pr) = (self.tongue_rib[i - 1].left_side_height, self.tongue_rib[i - 1].right_side_height);
            self.tongue_rib[i].left_side_height += FILTER_COEFF * (pl - self.tongue_rib[i].left_side_height);
            self.tongue_rib[i].right_side_height += FILTER_COEFF * (pr - self.tongue_rib[i].right_side_height);
        }
        const NN: usize = 4;
        for i in NN..NUM_DYNAMIC_TONGUE_RIBS - NN {
            let mut d = self.tongue_rib[i + 1].point - self.tongue_rib[i - 1].point;
            d.turn_left();
            d.normalize();
            self.tongue_rib[i].normal = d;
        }
        let n0 = p2(-1.0, 0.0);
        let n1 = self.tongue_rib[NN].normal;
        for i in 0..NN {
            let t = i as f64 / NN as f64;
            self.tongue_rib[i].normal = (1.0 - t) * n0 + t * n1;
        }
        let n0 = self.tongue_rib[NUM_DYNAMIC_TONGUE_RIBS - NN - 1].normal;
        let n1 = p2(0.0, 1.0);
        for i in 0..NN {
            let t = (i + 1) as f64 / NN as f64;
            self.tongue_rib[NUM_DYNAMIC_TONGUE_RIBS - NN + i].normal = (1.0 - t) * n0 + t * n1;
        }
        for i in 0..NUM_DYNAMIC_TONGUE_RIBS {
            self.tongue_rib[i].min_y = self.param[TS1].min - 0.5;
            self.tongue_rib[i].max_y = self.param[TS1].max + 0.5;
        }
        // Make the rib normals consistent (no crossing ribs), forwards then backwards, and average.
        let mut orig_normal = [P2::default(); NUM_TONGUE_RIBS];
        let mut new_normal = [P2::default(); NUM_TONGUE_RIBS];
        for i in 0..NUM_DYNAMIC_TONGUE_RIBS { orig_normal[i] = self.tongue_rib[i].normal; }
        for i in 0..NUM_DYNAMIC_TONGUE_RIBS - 1 { self.verify_tongue_rib_normal(i, i + 1); }
        for i in 0..NUM_DYNAMIC_TONGUE_RIBS {
            new_normal[i] = self.tongue_rib[i].normal;
            self.tongue_rib[i].normal = orig_normal[i];
        }
        for i in (1..NUM_DYNAMIC_TONGUE_RIBS).rev() { self.verify_tongue_rib_normal(i, i - 1); }
        for i in 0..NUM_DYNAMIC_TONGUE_RIBS {
            new_normal[i] += self.tongue_rib[i].normal;
            self.tongue_rib[i].normal = new_normal[i].normalize();
        }

        // Lower boundary of the tongue tip region (lower cover, teeth, a far point).
        let mut lower_boundary = LineStrip2D::new();
        lower_boundary.reset(0);
        let mut c = self.surface[si::LOWER_COVER].get_vertex(NUM_LARYNX_RIBS + NUM_THROAT_RIBS, NUM_LOWER_COVER_POINTS - 1).to_p2();
        c.y += 0.5;
        lower_boundary.add_point(c);
        match self.lower_outline.get_closest_intersection(c, p2(1.0, 0.0)) {
            Some((_t, intersection)) => lower_boundary.add_point(intersection),
            None => lower_boundary.add_point(c),
        }
        let min_y = lower_boundary.get_control_point(1).y;
        for i in NUM_LARYNX_RIBS + NUM_THROAT_RIBS..NUM_LOWER_COVER_RIBS {
            let c = self.surface[si::LOWER_COVER].get_vertex(i, NUM_LOWER_COVER_POINTS - 1).to_p2();
            if c.y > min_y { lower_boundary.add_point(c); }
        }
        for i in 0..3 {
            let v = self.surface[si::LOWER_TEETH].get_vertex(NUM_TEETH_RIBS - 1, i).to_p2();
            lower_boundary.add_point(v);
        }
        let mut c = lower_boundary.get_point(lower_boundary.num_points() as f64 - 1.0);
        c.x += 10.0;
        lower_boundary.add_point(c);

        let tongue_tip_radius = self.anatomy.tongue_tip_radius;
        let i = NUM_DYNAMIC_TONGUE_RIBS - 1;
        let t = if self.tongue_rib[i].left_side_height < self.tongue_rib[i].right_side_height {
            self.tongue_rib[i].left_side_height
        } else {
            self.tongue_rib[i].right_side_height
        };
        let mut final_point = self.tongue_rib[i].point + self.tongue_rib[i].normal * t;
        if self.param[TTY].limited_x - tongue_tip_radius < final_point.y {
            final_point.y = self.param[TTY].limited_x - tongue_tip_radius;
        }
        let i = NUM_DYNAMIC_TONGUE_RIBS;
        self.tongue_rib[i].normal = p2(0.0, 1.0);
        self.tongue_rib[i].left_side_height = 0.0;
        self.tongue_rib[i].right_side_height = 0.0;
        self.tongue_rib[i].point = final_point;

        let n = p2(0.0, 1.0);
        for i in NUM_TONGUE_RIBS / 2..=NUM_DYNAMIC_TONGUE_RIBS {
            if let Some((t, intersection)) = lower_boundary.get_closest_intersection(self.tongue_rib[i].point, n) {
                if t > 0.0 { self.tongue_rib[i].point = intersection; }
            }
            if let Some((t, _)) = lower_boundary.get_closest_intersection(self.tongue_rib[i].point, self.tongue_rib[i].normal) {
                if self.tongue_rib[i].left_side_height < t { self.tongue_rib[i].left_side_height = t; }
                if self.tongue_rib[i].right_side_height < t { self.tongue_rib[i].right_side_height = t; }
            }
        }
        let c = self.tongue_rib[NUM_TONGUE_RIBS - 4].point;
        let mut d = c;
        d.x -= 2.0 * tongue_tip_radius;
        d.y -= 2.0 * tongue_tip_radius;
        if let Some((t, intersection)) = lower_boundary.get_closest_intersection(c, d - c) {
            if t < 1.0 { d = intersection; }
        }
        let i = NUM_TONGUE_RIBS - 3;
        self.tongue_rib[i].point = d;
        self.tongue_rib[i].normal = p2(0.0, 1.0);
        self.tongue_rib[i].left_side_height = 0.0;
        self.tongue_rib[i].right_side_height = 0.0;
        let c = self.tongue_rib[NUM_TONGUE_RIBS - 3].point;
        self.tongue_rib[NUM_TONGUE_RIBS - 2].point = match lower_boundary.get_closest_intersection(c, p2(0.0, -1.0)) {
            Some((_t, intersection)) => intersection,
            None => c,
        };
        if self.tongue_rib[NUM_TONGUE_RIBS - 2].point.x < lower_boundary.get_control_point(1).x {
            self.tongue_rib[NUM_TONGUE_RIBS - 1].point = lower_boundary.get_control_point(1);
        } else {
            self.tongue_rib[NUM_TONGUE_RIBS - 1].point = self.tongue_rib[NUM_TONGUE_RIBS - 2].point;
        }
        for i in [NUM_TONGUE_RIBS - 2, NUM_TONGUE_RIBS - 1] {
            self.tongue_rib[i].normal = p2(0.0, 1.0);
            self.tongue_rib[i].left_side_height = 0.0;
            self.tongue_rib[i].right_side_height = 0.0;
        }

        self.calc_tongue_ribs();

        // A bulge on the first ribs (the tongue root region).
        const NUM_BULGE_RIBS: usize = 8;
        const BULGE_HEIGHT_CM: f64 = 0.5;
        let nrp = self.surface[si::TONGUE].num_rib_points;
        for i in 0..NUM_BULGE_RIBS {
            let n = self.tongue_rib[i].normal;
            for k in 0..nrp {
                let b = BULGE_HEIGHT_CM
                    * (0.5 - 0.5 * (2.0 * PI * i as f64 / (NUM_BULGE_RIBS - 1) as f64).cos())
                    * (0.5 - 0.5 * (2.0 * PI * k as f64 / (nrp - 1) as f64).cos());
                let mut pt = self.surface[si::TONGUE].get_vertex(i, k);
                pt.x += n.x * b;
                pt.y += n.y * b;
                self.surface[si::TONGUE].set_vertex(i, k, pt);
            }
        }
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
    fn all_surfaces_including_the_tongue_match_the_original_for_vowel_a() {
        let sp = Speaker::from_xml(JD3).unwrap();
        let mut vt = VocalTract::from_speaker(&sp);
        vt.set_params(&sp.shape("a").unwrap().params);
        for i in 0..NUM_PARAMS { vt.restrict_param(i); vt.param[i].limited_x = vt.param[i].x; }
        vt.calc_surfaces();
        let reference = reference_surfaces(SURF_A);
        for k in 0..si::NUM_SURFACES {
            let d = max_diff(&vt.surface[k], &reference[k].3);
            assert!(d < 5e-7, "surface {k} differs from the original by {d}");
        }
    }


    struct RefShape {
        name: String,
        incisor: f64,
        tip_elevation: f64,
        sections: Vec<(f64, f64, i32)>, // area, length, articulator
    }

    fn reference_tubes(text: &str) -> Vec<RefShape> {
        let mut out: Vec<RefShape> = vec![];
        for line in text.lines() {
            let f: Vec<&str> = line.split_whitespace().collect();
            match f.first().copied() {
                Some("SHAPE") => out.push(RefShape { name: f[1].to_string(), incisor: 0.0, tip_elevation: 0.0, sections: vec![] }),
                Some("META") => {
                    let r = out.last_mut().unwrap();
                    r.incisor = f[2].parse().unwrap();
                    r.tip_elevation = f[4].parse().unwrap();
                }
                Some("S") => out.last_mut().unwrap().sections.push((f[2].parse().unwrap(), f[3].parse().unwrap(), f[4].parse().unwrap())),
                _ => {}
            }
        }
        out
    }

    #[test]
    fn area_functions_of_all_shapes_match_the_original() {
        let sp = Speaker::from_xml(JD3).unwrap();
        let reference = reference_tubes(include_str!("../tests/data/vtl_tubes.txt"));
        assert_eq!(reference.len(), 69);
        let mut worst: (f64, String) = (0.0, String::new());
        let mut failures: Vec<String> = vec![];
        for r in &reference {
            let mut vt = VocalTract::from_speaker(&sp);
            vt.set_params(&sp.shape(&r.name).unwrap().params);
            vt.calculate_all();
            let tube = vt.get_tube(); // applies the minimum area, as the original Tube does
            let mut err: f64 = 0.0;
            for (k, (area, length, art)) in r.sections.iter().enumerate() {
                let t = tube.sections[crate::tube::FIRST_PHARYNX + k];
                err = err.max((t.area_cm2 - area).abs()).max((t.length_cm - length).abs());
                if t.articulator as i32 != *art { err = err.max(1.0); }
            }
            err = err.max((vt.incisor_pos_cm - r.incisor).abs()).max((vt.param[TS3].x - r.tip_elevation).abs());
            if err > worst.0 { worst = (err, r.name.clone()); }
            if err > 1e-5 { failures.push(format!("{} ({err:.2e})", r.name)); }
        }
        println!("worst: {} {:.3e}; failures: {}", worst.1, worst.0, failures.len());
        assert!(failures.is_empty(), "shapes that differ from the original: {failures:?}");
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
