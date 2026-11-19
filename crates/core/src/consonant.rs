// SPDX-License-Identifier: GPL-3.0-or-later
//! Vowel-dependent consonant targets by interpolation between three corner shapes.
//!
//! A consonant has three prototype shapes, one in the context of each corner vowel /a/, /i/,
//! /u/ (VTL: `tt-alveolar-closure(a)`, `(i)`, `(u)`). The shape for any other vowel context is
//! `x = x_a + alpha (x_i - x_a) + beta (x_u - x_a)`, where (alpha, beta) are the coordinates of
//! the context vowel in the a-i-u triangle. Tongue and lips are mapped independently so that lip
//! rounding does not depend on the tongue position. Port of VocalTractLab's `GesturalScore`
//! (`mapToVowelSubspace`, `limitVowelSubspaceCoord`, `getContextDependentConsonant`),
//! (C) Peter Birkholz, GPL-3.0-or-later.
//!
//! On top of that, hand-made overrides can be stored per (consonant, vowel) pair, e.g. for the
//! palatalised Japanese /ki/, /ke/ and the contracted syllables, which the interpolation cannot
//! reproduce.

use crate::speaker::{Speaker, NUM_PARAMS};
use crate::vocal_tract::pi::*;

pub type Params = [f64; NUM_PARAMS];

/// Parameters that describe the tongue shape (everything except the lips).
pub const TONGUE_PARAMS: [usize; 17] = [HX, HY, JX, JA, VS, VO, TCX, TCY, TTX, TTY, TBX, TBY, TRX, TRY, TS1, TS2, TS3];

/// Coordinates of a shape in the triangle spanned by /a/ (0, 0), /i/ (1, 0) and /u/ (0, 1).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct VowelCoords {
    pub alpha_tongue: f64,
    pub beta_tongue: f64,
    pub alpha_lips: f64,
    pub beta_lips: f64,
}

/// Pseudo-inverse `(M^T M)^-1 M^T` of an N x 2 matrix, returned as 2 rows of N values.
pub fn pseudo_inverse_nx2(m: &[[f64; 2]]) -> [Vec<f64>; 2] {
    let n = m.len();
    let mut a = [[0.0; 2]; 2];
    for x in 0..2 {
        for y in 0..2 {
            let mut sum = 0.0;
            for k in 0..n { sum += m[k][y] * m[k][x]; }
            a[x][y] = sum;
        }
    }
    const EPS: f64 = 0.000000001;
    let mut det = a[0][0] * a[1][1] - a[0][1] * a[1][0];
    if det.abs() < EPS { det = EPS; }
    let b = [[a[1][1] / det, -a[0][1] / det], [-a[1][0] / det, a[0][0] / det]];
    let mut inv = [vec![0.0; n], vec![0.0; n]];
    for x in 0..n {
        for y in 0..2 {
            inv[y][x] = b[y][0] * m[x][0] + b[y][1] * m[x][1];
        }
    }
    inv
}

/// Maps a vocal tract shape into the subspace of the corner vowels `a`, `i` and `u`
/// (least squares for the tongue, an exact 2 x 2 solution for the two lip parameters).
pub fn map_to_vowel_subspace(a: &Params, i: &Params, u: &Params, params: &Params) -> VowelCoords {
    let mut m = [[0.0; 2]; 17];
    let mut v = [0.0; 17];
    for (r, &p) in TONGUE_PARAMS.iter().enumerate() {
        m[r][0] = i[p] - a[p];
        m[r][1] = u[p] - a[p];
        v[r] = params[p] - a[p];
    }
    let inv = pseudo_inverse_nx2(&m);
    let (mut alpha_tongue, mut beta_tongue) = (0.0, 0.0);
    for r in 0..17 {
        alpha_tongue += inv[0][r] * v[r];
        beta_tongue += inv[1][r] * v[r];
    }
    let (ca, cb) = (i[LP] - a[LP], u[LP] - a[LP]);
    let (cc, cd) = (i[LD] - a[LD], u[LD] - a[LD]);
    let (x, y) = (params[LP] - a[LP], params[LD] - a[LD]);
    const EPS: f64 = 0.000000001;
    let mut det = ca * cd - cb * cc;
    if det.abs() < EPS { det = EPS; }
    VowelCoords {
        alpha_tongue,
        beta_tongue,
        alpha_lips: (x * cd - cb * y) / det,
        beta_lips: (ca * y - x * cc) / det,
    }
}

fn limit_pair(alpha: &mut f64, beta: &mut f64) {
    *alpha = alpha.clamp(0.0, 1.0);
    *beta = beta.clamp(0.0, 1.0);
    if *alpha + *beta > 1.0 {
        let d = *alpha + *beta - 1.0;
        *alpha -= 0.5 * d;
        *beta -= 0.5 * d;
    }
}

/// Limits the coordinates to the triangle: 0 <= alpha, beta and alpha + beta <= 1.
pub fn limit_vowel_subspace_coord(c: &mut VowelCoords) {
    limit_pair(&mut c.alpha_tongue, &mut c.beta_tongue);
    limit_pair(&mut c.alpha_lips, &mut c.beta_lips);
}

/// The consonant shape for a context given by vowel-triangle coordinates.
pub fn interpolate_consonant(in_a: &Params, in_i: &Params, in_u: &Params, c: VowelCoords) -> Params {
    let mut out = [0.0; NUM_PARAMS];
    for p in 0..NUM_PARAMS {
        out[p] = in_a[p] + c.alpha_tongue * (in_i[p] - in_a[p]) + c.beta_tongue * (in_u[p] - in_a[p]);
    }
    for p in [LP, LD] {
        out[p] = in_a[p] + c.alpha_lips * (in_i[p] - in_a[p]) + c.beta_lips * (in_u[p] - in_a[p]);
    }
    out
}

/// A consonant with its prototypes in the contexts of the corner vowels /a/, /i/, /u/.
#[derive(Debug, Clone, PartialEq)]
pub struct ConsonantDef {
    pub name: String,
    pub in_a: Params,
    pub in_i: Params,
    pub in_u: Params,
}

/// A hand-made target for one (consonant, context vowel) pair.
#[derive(Debug, Clone, PartialEq)]
pub struct Override {
    pub consonant: String,
    pub vowel: String,
    pub params: Params,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetSource {
    Interpolated,
    Overridden,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Target {
    pub params: Params,
    pub source: TargetSource,
    /// Coordinates of the context vowel (after limiting), for display.
    pub coords: VowelCoords,
}

/// The consonant phoneme editor's model: corner vowels, consonant prototypes, overrides.
#[derive(Debug, Clone, PartialEq)]
pub struct ConsonantEditor {
    pub vowel_a: Params,
    pub vowel_i: Params,
    pub vowel_u: Params,
    pub consonants: Vec<ConsonantDef>,
    pub overrides: Vec<Override>,
}

impl ConsonantEditor {
    /// Takes the corner vowels and every consonant that has `(a)`, `(i)` and `(u)` variants from a
    /// speaker file.
    pub fn from_speaker(sp: &Speaker) -> Option<ConsonantEditor> {
        let corner = |n: &str| sp.shape(n).map(|s| s.params);
        let (vowel_a, vowel_i, vowel_u) = (corner("a")?, corner("i")?, corner("u")?);
        let mut consonants: Vec<ConsonantDef> = vec![];
        for s in &sp.shapes {
            let Some(base) = s.name.strip_suffix("(a)") else { continue };
            let (Some(i), Some(u)) = (corner(&format!("{base}(i)")), corner(&format!("{base}(u)"))) else { continue };
            consonants.push(ConsonantDef { name: base.to_string(), in_a: s.params, in_i: i, in_u: u });
        }
        // Not part of the VTL speaker file: a bilabial fricative (Japanese ふ) made from the labial
        // closure by opening the lips a little (LD + 0.14 cm leaves about 0.1-0.2 cm^2).
        if let Some(base) = consonants.iter().find(|c| c.name == "ll-labial-closure").cloned() {
            let open = |mut p: Params| { p[LD] += 0.14; p };
            consonants.push(ConsonantDef { name: "ll-labial-fricative".to_string(), in_a: open(base.in_a), in_i: open(base.in_i), in_u: open(base.in_u) });
        }
        Some(ConsonantEditor { vowel_a, vowel_i, vowel_u, consonants, overrides: vec![] })
    }

    pub fn consonant(&self, name: &str) -> Option<&ConsonantDef> {
        self.consonants.iter().find(|c| c.name == name)
    }

    /// Coordinates of a vowel shape in the triangle, limited to the triangle.
    pub fn vowel_coords(&self, vowel: &Params) -> VowelCoords {
        let mut c = map_to_vowel_subspace(&self.vowel_a, &self.vowel_i, &self.vowel_u, vowel);
        limit_vowel_subspace_coord(&mut c);
        c
    }

    pub fn override_for(&self, consonant: &str, vowel: &str) -> Option<&Override> {
        self.overrides.iter().find(|o| o.consonant == consonant && o.vowel == vowel)
    }

    /// Stores (or replaces) a hand-made target for a (consonant, vowel) pair.
    pub fn set_override(&mut self, consonant: &str, vowel: &str, params: Params) {
        match self.overrides.iter_mut().find(|o| o.consonant == consonant && o.vowel == vowel) {
            Some(o) => o.params = params,
            None => self.overrides.push(Override { consonant: consonant.to_string(), vowel: vowel.to_string(), params }),
        }
    }

    /// Removes the override; returns whether there was one.
    pub fn clear_override(&mut self, consonant: &str, vowel: &str) -> bool {
        let n = self.overrides.len();
        self.overrides.retain(|o| !(o.consonant == consonant && o.vowel == vowel));
        self.overrides.len() != n
    }

    /// The target of `consonant` in the context of a vowel. `vowel` is the name used for
    /// overrides, `vowel_params` the vowel's shape (any vowel, not only the corner vowels).
    /// An override wins over the interpolation. `None` if the consonant is unknown.
    pub fn target(&self, consonant: &str, vowel: &str, vowel_params: &Params) -> Option<Target> {
        let def = self.consonant(consonant)?;
        let coords = self.vowel_coords(vowel_params);
        if let Some(o) = self.override_for(consonant, vowel) {
            return Some(Target { params: o.params, source: TargetSource::Overridden, coords });
        }
        Some(Target { params: interpolate_consonant(&def.in_a, &def.in_i, &def.in_u, coords), source: TargetSource::Interpolated, coords })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const JD3: &str = include_str!("../../../resources/JD3.speaker");
    const CTX: &str = include_str!("../tests/data/vtl_consonant_ctx.txt");

    fn editor() -> (Speaker, ConsonantEditor) {
        let sp = Speaker::from_xml(JD3).unwrap();
        let ed = ConsonantEditor::from_speaker(&sp).unwrap();
        (sp, ed)
    }

    fn row(f: &[&str], skip: usize) -> Vec<f64> { f[skip..].iter().map(|x| x.parse().unwrap()).collect() }

    #[test]
    fn vowel_coordinates_match_the_original() {
        let (sp, ed) = editor();
        let (mut checked, mut limited_checked) = (0, 0);
        for line in CTX.lines() {
            let f: Vec<&str> = line.split_whitespace().collect();
            match f.first().copied() {
                Some("COORD") => {
                    let r = row(&f, 2);
                    let c = map_to_vowel_subspace(&ed.vowel_a, &ed.vowel_i, &ed.vowel_u, &sp.shape(f[1]).unwrap().params);
                    for (got, want) in [c.alpha_tongue, c.beta_tongue, c.alpha_lips, c.beta_lips].iter().zip(&r) {
                        assert!((got - want).abs() < 1e-7, "/{}/ coord {got} vs {want}", f[1]);
                    }
                    checked += 1;
                }
                Some("LIMITED") => {
                    let r = row(&f, 2);
                    let c = ed.vowel_coords(&sp.shape(f[1]).unwrap().params);
                    for (got, want) in [c.alpha_tongue, c.beta_tongue, c.alpha_lips, c.beta_lips].iter().zip(&r) {
                        assert!((got - want).abs() < 1e-7, "/{}/ limited coord {got} vs {want}", f[1]);
                    }
                    limited_checked += 1;
                }
                _ => {}
            }
        }
        assert!(checked >= 8 && limited_checked >= 8, "{checked} {limited_checked}");
    }

    #[test]
    fn context_dependent_consonants_match_the_original() {
        let (sp, ed) = editor();
        let mut n = 0;
        for line in CTX.lines() {
            let f: Vec<&str> = line.split_whitespace().collect();
            if f.first() != Some(&"CTX") { continue; }
            let want = row(&f, 4);
            let vowel = sp.shape(f[2]).unwrap().params;
            let t = ed.target(f[1], f[2], &vowel).unwrap();
            assert_eq!(t.source, TargetSource::Interpolated);
            for p in 0..NUM_PARAMS {
                assert!((t.params[p] - want[p]).abs() < 1e-7, "{} in /{}/ param {p}: {} vs {}", f[1], f[2], t.params[p], want[p]);
            }
            n += 1;
        }
        assert!(n >= 56, "{n} cases");
    }

    #[test]
    fn corner_vowels_map_to_the_corners_and_give_back_the_prototypes() {
        let (sp, ed) = editor();
        let c = ed.vowel_coords(&sp.shape("i").unwrap().params);
        assert!((c.alpha_tongue - 1.0).abs() < 1e-9 && c.beta_tongue.abs() < 1e-9);
        let c = ed.vowel_coords(&sp.shape("u").unwrap().params);
        assert!(c.alpha_tongue.abs() < 1e-9 && (c.beta_tongue - 1.0).abs() < 1e-9);
        let velar = ed.consonant("tb-velar-closure").unwrap();
        for (v, proto) in [("a", &velar.in_a), ("i", &velar.in_i), ("u", &velar.in_u)] {
            let t = ed.target("tb-velar-closure", v, &sp.shape(v).unwrap().params).unwrap();
            for p in 0..NUM_PARAMS { assert!((t.params[p] - proto[p]).abs() < 1e-9, "/{v}/ param {p}"); }
        }
    }

    #[test]
    fn mid_vowels_lie_between_their_corners() {
        let (sp, ed) = editor();
        let e = ed.vowel_coords(&sp.shape("e").unwrap().params);
        assert!(e.alpha_tongue > 0.5 && e.alpha_tongue < 1.0, "/e/ is between /a/ and /i/: {e:?}");
        let o = ed.vowel_coords(&sp.shape("o").unwrap().params);
        assert!(o.beta_tongue > 0.5, "/o/ is towards /u/: {o:?}");
        assert!(o.alpha_lips + o.beta_lips <= 1.0 + 1e-12 && o.alpha_lips >= 0.0 && o.beta_lips >= 0.0);
    }

    #[test]
    fn velar_closure_is_fronted_before_i() {
        let (sp, ed) = editor();
        let tcx = |v: &str| ed.target("tb-velar-closure", v, &sp.shape(v).unwrap().params).unwrap().params[TCX];
        assert!(tcx("i") > tcx("e") && tcx("e") > tcx("a"), "{} {} {}", tcx("a"), tcx("e"), tcx("i"));
        assert!(tcx("i") - tcx("a") > 0.4);
    }

    #[test]
    fn overrides_win_and_can_be_removed_independently() {
        let (sp, mut ed) = editor();
        let vowel = sp.shape("i").unwrap().params;
        let before = ed.target("tb-velar-closure", "i", &vowel).unwrap();
        let mut custom = before.params;
        custom[TCX] += 0.3;
        ed.set_override("tb-velar-closure", "i", custom);
        let t = ed.target("tb-velar-closure", "i", &vowel).unwrap();
        assert_eq!(t.source, TargetSource::Overridden);
        assert!((t.params[TCX] - custom[TCX]).abs() < 1e-12);
        // Other vowel contexts and other consonants are untouched.
        let other = ed.target("tb-velar-closure", "u", &sp.shape("u").unwrap().params).unwrap();
        assert_eq!(other.source, TargetSource::Interpolated);
        assert_eq!(ed.target("tt-alveolar-closure", "i", &vowel).unwrap().source, TargetSource::Interpolated);
        // Replacing keeps a single entry; clearing restores the interpolation.
        ed.set_override("tb-velar-closure", "i", before.params);
        assert_eq!(ed.overrides.len(), 1);
        assert!(ed.clear_override("tb-velar-closure", "i"));
        assert!(!ed.clear_override("tb-velar-closure", "i"));
        assert_eq!(ed.target("tb-velar-closure", "i", &vowel).unwrap(), before);
    }

    #[test]
    fn unknown_consonant_is_none_and_lists_all_prototypes() {
        let (_sp, ed) = editor();
        assert!(ed.target("no-such-consonant", "a", &ed.vowel_a).is_none());
        assert!(ed.consonants.len() >= 12, "{} consonants", ed.consonants.len());
        for need in ["ll-labial-closure", "tt-alveolar-closure", "tb-velar-closure", "tt-alveolar-fricative", "tt-postalveolar-fricative", "tb-palatal-fricative"] {
            assert!(ed.consonant(need).is_some(), "{need}");
        }
    }

    #[test]
    fn pseudo_inverse_solves_least_squares() {
        // M = [[1,0],[0,1],[1,1]]; b = M * (2, 3)
        let m = [[1.0, 0.0], [0.0, 1.0], [1.0, 1.0]];
        let inv = pseudo_inverse_nx2(&m);
        let b = [2.0, 3.0, 5.0];
        let (x, y): (f64, f64) = (inv[0].iter().zip(&b).map(|(a, b)| a * b).sum(), inv[1].iter().zip(&b).map(|(a, b)| a * b).sum());
        assert!((x - 2.0).abs() < 1e-12 && (y - 3.0).abs() < 1e-12);
    }
}
