// SPDX-License-Identifier: GPL-3.0-or-later
//! Curves used by the vocal tract model: rational Bezier curves, 2D / 3D line strips. Port of
//! VocalTractLab's `Splines`, (C) Peter Birkholz, GPL-3.0-or-later. Quirks of the original are
//! kept (e.g. loops that keep the *last* matching segment, `del_point` not invalidating cached
//! curve parameters), because the tongue and cross-section code depends on them.

use crate::geom::*;

pub const MAX_SPLINE_POINTS: usize = 256;

/// Re-parametrises a curve by arc length: returns the curve parameter `u` for which the point
/// at `u` lies at the relative arc length `t` (sampled with 100 points).
pub fn uniform_param(mut point_at: impl FnMut(f64) -> P3, t: f64) -> f64 {
    const EPSILON: f64 = 0.000001;
    const N: usize = 100;
    let mut pts = [P3::default(); N];
    let mut s = [0.0; N];
    let mut pos = [0.0; N];
    for i in 0..N {
        s[i] = i as f64 / (N - 1) as f64;
        pts[i] = point_at(s[i]);
        pos[i] = if i == 0 { 0.0 } else { pos[i - 1] + (pts[i] - pts[i - 1]).magnitude() };
    }
    let mut l = pos[N - 1];
    if l < EPSILON { l = EPSILON; }
    for p in pos.iter_mut() { *p /= l; }
    let t = t.clamp(0.0, 1.0);
    let mut k: i64 = -1;
    let mut ratio = 0.0;
    for i in 0..N - 1 {
        if t >= pos[i] && t <= pos[i + 1] {
            k = i as i64;
            let mut l = pos[i + 1] - pos[i];
            if l < EPSILON { l = EPSILON; }
            ratio = (t - pos[i]) / l;
        }
    }
    if k == -1 {
        if t < 0.5 { k = 0; ratio = 0.0; } else { k = N as i64 - 2; ratio = 1.0; }
    }
    let k = k as usize;
    s[k] + ratio * (s[k + 1] - s[k])
}

/// Rational Bezier curve in 3D.
#[derive(Clone, Debug, Default)]
pub struct Bezier3D {
    p: Vec<P3>,
    w: Vec<f64>,
    a: Vec<P3>,
    b: Vec<f64>,
    changed: bool,
}

impl Bezier3D {
    pub fn new() -> Self { Bezier3D { changed: true, ..Default::default() } }

    pub fn from_points(points: &[P3]) -> Self {
        let mut c = Bezier3D::new();
        c.set_points(points);
        c
    }

    pub fn set_points(&mut self, points: &[P3]) {
        let n = points.len().min(MAX_SPLINE_POINTS);
        self.p = points[..n].to_vec();
        self.w = vec![1.0; n];
        self.changed = true;
    }

    pub fn set_points_weighted(&mut self, points: &[P3], weights: &[f64]) {
        let n = points.len().min(weights.len()).min(MAX_SPLINE_POINTS);
        self.p = points[..n].to_vec();
        self.w = weights[..n].to_vec();
        self.changed = true;
    }

    pub fn add_point(&mut self, pt: P3, weight: f64) {
        if self.p.len() >= MAX_SPLINE_POINTS { return; }
        self.p.push(pt);
        self.w.push(weight);
        self.changed = true;
    }

    pub fn num_points(&self) -> usize { self.p.len() }

    pub fn get_control_point(&self, i: usize) -> P3 { self.p.get(i).copied().unwrap_or_default() }

    fn bernstein(k: usize, n: usize) -> Vec<f64> {
        let mut coeff = vec![0.0; n + 1];
        coeff[k] = 1.0;
        let mut new_coeff = vec![0.0; n + 2];
        for _ in 1..=(n - k) {
            for v in new_coeff.iter_mut() { *v = 0.0; }
            for j in 0..=n {
                new_coeff[j] += coeff[j];
                new_coeff[j + 1] -= coeff[j];
            }
            coeff[..=n].copy_from_slice(&new_coeff[..=n]);
        }
        let mut numerator = 1.0;
        let mut denominator = 1.0;
        for j in 2..=n { numerator *= j as f64; }
        for j in 2..=k { denominator *= j as f64; }
        for j in 2..=(n - k) { denominator *= j as f64; }
        let f = numerator / denominator;
        for c in coeff.iter_mut() { *c *= f; }
        coeff
    }

    fn calculate_coeff(&mut self) {
        let n = self.p.len().saturating_sub(1);
        self.a = vec![P3::default(); n + 1];
        self.b = vec![0.0; n + 1];
        if !self.p.is_empty() {
            for i in 0..=n {
                let coeff = Self::bernstein(i, n);
                for j in 0..=n {
                    self.a[j] += self.p[i] * (self.w[i] * coeff[j]);
                    self.b[j] += self.w[i] * coeff[j];
                }
            }
        }
        self.changed = false;
    }

    pub fn get_point(&mut self, t: f64) -> P3 {
        if self.changed { self.calculate_coeff(); }
        if self.p.len() < 2 { return P3::default(); }
        let n = self.p.len() - 1;
        let mut q = P3::default();
        let mut f = 1.0;
        let mut den = 0.0;
        for i in 0..=n {
            q += self.a[i] * f;
            den += self.b[i] * f;
            f *= t;
        }
        q / den
    }

    pub fn uniform_param(&mut self, t: f64) -> f64 {
        uniform_param(|u| self.get_point(u), t)
    }
}

/// Polyline in 3D parametrised by relative arc length.
#[derive(Clone, Debug, Default)]
pub struct LineStrip3D {
    p: Vec<P3>,
    pos: Vec<f64>,
    changed: bool,
}

impl LineStrip3D {
    pub fn new() -> Self { LineStrip3D { changed: true, ..Default::default() } }

    pub fn set_points(&mut self, points: &[P3]) {
        let n = points.len().min(MAX_SPLINE_POINTS);
        self.p = points[..n].to_vec();
        self.changed = true;
    }

    pub fn add_point(&mut self, pt: P3) {
        if self.p.len() >= MAX_SPLINE_POINTS { return; }
        self.p.push(pt);
        self.changed = true;
    }

    pub fn num_points(&self) -> usize { self.p.len() }
    pub fn get_control_point(&self, i: usize) -> P3 { self.p.get(i).copied().unwrap_or_default() }

    fn calculate_params(&mut self) {
        let n = self.p.len();
        self.pos = vec![0.0; n];
        if n < 1 { return; }
        for i in 1..n { self.pos[i] = self.pos[i - 1] + (self.p[i] - self.p[i - 1]).magnitude(); }
        let length = self.pos[n - 1];
        if length > 0.0 { for i in 1..n { self.pos[i] /= length; } }
    }

    pub fn get_point(&mut self, t: f64) -> P3 {
        if self.changed { self.calculate_params(); self.changed = false; }
        let n = self.p.len();
        if n < 1 { return P3::default(); }
        if n == 1 { return self.p[0]; }
        let t = t.clamp(0.0, 1.0);
        const EPS: f64 = 0.000001;
        let mut k: i64 = -1;
        let mut ratio = 0.0;
        for i in 0..n - 1 {
            if t >= self.pos[i] - EPS && t <= self.pos[i + 1] + EPS {
                k = i as i64;
                let mut length = self.pos[i + 1] - self.pos[i];
                if length < EPS { length = EPS; }
                ratio = (t - self.pos[i]) / length;
            }
        }
        if k == -1 { return P3::default(); }
        let k = k as usize;
        self.p[k] + (self.p[k + 1] - self.p[k]) * ratio
    }

    pub fn get_curve_param(&mut self, index: i64) -> f64 {
        if self.changed { self.calculate_params(); self.changed = false; }
        let n = self.p.len() as i64;
        let i = index.clamp(0, (n - 1).max(0)) as usize;
        self.pos.get(i).copied().unwrap_or(0.0)
    }

    /// Curve parameter where the strip crosses a plane (the crossing nearest to the plane
    /// point wins); 0 if there is none.
    pub fn get_intersection(&mut self, plane_point: P3, plane_normal: P3) -> f64 {
        if self.changed { self.calculate_params(); self.changed = false; }
        let n = self.p.len();
        if n < 2 { return 0.0; }
        const EPS: f64 = 0.000001;
        let mut result = 0.0;
        let mut min_dist = 1_000_000.0;
        for i in 0..n - 1 {
            let mut p0 = self.p[i];
            if i == 0 {
                let mut v = self.p[i + 1] - self.p[i];
                let length = v.magnitude();
                if length > 0.0 { v /= length; p0 -= v * EPS; }
            }
            let mut p1 = self.p[i + 1];
            if i + 1 == n - 1 {
                let mut v = self.p[i + 1] - self.p[i];
                let length = v.magnitude();
                if length > 0.0 { v /= length; p1 += v * EPS; }
            }
            let d0 = scalar_product(p0 - plane_point, plane_normal);
            let d1 = scalar_product(p1 - plane_point, plane_normal);
            if (d0 <= 0.0 && d1 >= 0.0) || (d1 <= 0.0 && d0 >= 0.0) {
                let v = p1 - p0;
                let den = scalar_product(v, plane_normal);
                if den != 0.0 {
                    let t = -scalar_product(p0 - plane_point, plane_normal) / den;
                    let q = p0 + v * t;
                    let dist = (q - plane_point).magnitude();
                    if dist < min_dist {
                        result = self.pos[i] + t * (self.pos[i + 1] - self.pos[i]);
                        min_dist = dist;
                    }
                }
            }
        }
        result
    }
}

/// Polyline in 2D.
#[derive(Clone, Debug, Default)]
pub struct LineStrip2D {
    p: Vec<P2>,
    pos: Vec<f64>,
    changed: bool,
}

impl LineStrip2D {
    pub fn new() -> Self { LineStrip2D { changed: true, ..Default::default() } }

    pub fn reset(&mut self, n: usize) {
        let n = n.min(MAX_SPLINE_POINTS);
        self.p = vec![P2::default(); n];
        self.changed = true;
    }

    pub fn set_points(&mut self, points: &[P2]) {
        let n = points.len().min(MAX_SPLINE_POINTS);
        self.p = points[..n].to_vec();
        self.changed = true;
    }

    pub fn set_point(&mut self, i: usize, pt: P2) {
        if i < self.p.len() { self.p[i] = pt; self.changed = true; }
    }

    pub fn add_point(&mut self, pt: P2) {
        if self.p.len() >= MAX_SPLINE_POINTS { return; }
        self.p.push(pt);
        self.changed = true;
    }

    /// Removes the last point. Like the original, this does NOT invalidate the cached curve
    /// parameters.
    pub fn del_point(&mut self) {
        self.p.pop();
        self.pos.truncate(self.p.len());
    }

    pub fn num_points(&self) -> usize { self.p.len() }
    pub fn get_control_point(&self, i: i64) -> P2 {
        if i < 0 { return P2::default(); }
        self.p.get(i as usize).copied().unwrap_or_default()
    }

    fn calculate_params(&mut self) {
        let n = self.p.len();
        self.pos = vec![0.0; n];
        if n < 1 { return; }
        for i in 1..n { self.pos[i] = self.pos[i - 1] + (self.p[i] - self.p[i - 1]).magnitude(); }
        let length = self.pos[n - 1];
        if length > 0.0 { for i in 1..n { self.pos[i] /= length; } }
    }

    fn ensure(&mut self) {
        if self.changed { self.calculate_params(); self.changed = false; }
    }

    pub fn get_point(&mut self, t: f64) -> P2 {
        self.ensure();
        let n = self.p.len();
        if n < 1 { return P2::default(); }
        if n == 1 { return self.p[0]; }
        let t = t.clamp(0.0, 1.0);
        const EPS: f64 = 0.000001;
        let mut k: i64 = -1;
        let mut ratio = 0.0;
        for i in 0..n - 1 {
            if t >= self.pos[i] - EPS && t <= self.pos[i + 1] + EPS {
                k = i as i64;
                let mut length = self.pos[i + 1] - self.pos[i];
                if length < EPS { length = EPS; }
                ratio = (t - self.pos[i]) / length;
            }
        }
        if k == -1 { return P2::default(); }
        let k = k as usize;
        self.p[k] + (self.p[k + 1] - self.p[k]) * ratio
    }

    /// y as a function of x (the strip should be monotonic in x); the last matching segment wins.
    pub fn get_function_value(&self, x: f64) -> f64 {
        let n = self.p.len();
        if n < 1 { return 0.0; }
        if n == 1 { return self.p[0].y; }
        const EPS: f64 = 0.000001;
        let mut result = 0.0;
        for i in 0..n - 1 {
            if x >= self.p[i].x - EPS && x <= self.p[i + 1].x + EPS {
                let mut length = self.p[i + 1].x - self.p[i].x;
                if length < EPS { length = EPS; }
                result = self.p[i].y + (self.p[i + 1].y - self.p[i].y) * (x - self.p[i].x) / length;
            }
        }
        result
    }

    pub fn get_curve_param(&mut self, index: i64) -> f64 {
        self.ensure();
        let n = self.p.len() as i64;
        let i = index.clamp(0, (n - 1).max(0)) as usize;
        self.pos.get(i).copied().unwrap_or(0.0)
    }

    /// Shared scan: calls `visit(i, s_ok, denominator, r, w)` for each segment that the
    /// line `Q + t v` crosses (tolerance EPSILON); stops when `visit` returns true.
    fn scan(&self, q: P2, v: P2, only_rising: bool, mut visit: impl FnMut(f64) -> bool) {
        const EPS: f64 = 0.000001;
        let mut n = p2(-v.y, v.x);
        n.normalize();
        let p_left = q + n * EPS;
        let p_right = q - n * EPS;
        let mut old_section = 0;
        for i in 0..self.p.len() {
            let mut section = 0;
            let mut w = self.p[i] - p_left;
            if w.x * v.y - w.y * v.x < 0.0 { section = -1; }
            w = self.p[i] - p_right;
            if w.x * v.y - w.y * v.x > 0.0 { section = 1; }
            let crossing = if only_rising {
                i > 0 && section >= 0 && old_section <= 0
            } else {
                i > 0 && ((section >= 0 && old_section <= 0) || (section <= 0 && old_section >= 0))
            };
            if crossing {
                let r = q - self.p[i - 1];
                let w = self.p[i] - self.p[i - 1];
                let den = v.x * w.y - v.y * w.x;
                if den != 0.0 {
                    let s = (v.x * r.y - r.x * v.y) / den;
                    if s >= -EPS && s <= 1.0 + EPS {
                        let d = (w.x * r.y - r.x * w.y) / den;
                        if visit(d) { return; }
                    }
                }
            }
            old_section = section;
        }
    }

    /// Intersection of the line `Q + t v` with the strip that is closest to `Q`
    /// (smallest |t|). Returns (t, point) on success.
    pub fn get_closest_intersection(&self, q: P2, v: P2) -> Option<(f64, P2)> {
        let mut t = 1_000_000.0;
        let mut ok = false;
        self.scan(q, v, false, |d| {
            if d.abs() < f64::abs(t) { t = d; ok = true; }
            false
        });
        if ok { Some((t, q + v * t)) } else { None }
    }

    /// First crossing of the strip (in point order).
    pub fn get_first_intersection(&self, q: P2, v: P2) -> Option<(f64, P2)> {
        let mut found = None;
        self.scan(q, v, false, |d| { found = Some((d, q + v * d)); true });
        found
    }

    /// Nearest crossing in the direction of `v` for segments that cross from the right to the
    /// left side only (used for the upper outline).
    pub fn get_special_intersection(&self, q: P2, v: P2) -> Option<(f64, P2)> {
        let mut t = 1_000_000.0;
        let mut ok = false;
        self.scan(q, v, true, |d| {
            if !ok || (d >= 0.0 && d < t) || (d <= 0.0 && d > t) { t = d; ok = true; }
            false
        });
        if ok { Some((t, q + v * t)) } else { None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bezier_interpolates_end_points_and_arc_length_param_is_uniform() {
        let pts = [p3(0.0, 0.0, 0.0), p3(1.0, 2.0, 0.0), p3(3.0, 2.0, 0.0), p3(4.0, 0.0, 0.0)];
        let mut c = Bezier3D::from_points(&pts);
        let (a, b) = (c.get_point(0.0), c.get_point(1.0));
        assert!((a - pts[0]).magnitude() < 1e-12 && (b - pts[3]).magnitude() < 1e-12);
        // Uniform parameters give equally spaced points (within the 100-sample resolution).
        let u: Vec<P3> = (0..=4).map(|k| { let t = c.uniform_param(k as f64 / 4.0); c.get_point(t) }).collect();
        let d: Vec<f64> = u.windows(2).map(|w| (w[1] - w[0]).magnitude()).collect();
        for w in d.windows(2) { assert!((w[0] - w[1]).abs() < 0.05 * w[0], "{d:?}"); }
    }

    #[test]
    fn bezier_weights_pull_the_curve() {
        let pts = [p3(0.0, 0.0, 0.0), p3(1.0, 1.0, 0.0), p3(2.0, 0.0, 0.0)];
        let mut plain = Bezier3D::from_points(&pts);
        let mut heavy = Bezier3D::new();
        heavy.set_points_weighted(&pts, &[1.0, 4.0, 1.0]);
        assert!(heavy.get_point(0.5).y > plain.get_point(0.5).y);
    }

    #[test]
    fn line_strip_3d_plane_intersection() {
        let mut l = LineStrip3D::new();
        for x in 0..=4 { l.add_point(p3(x as f64, 0.0, 0.0)); }
        let t = l.get_intersection(p3(1.0, 0.0, 0.0), p3(1.0, 0.0, 0.0));
        assert!((t - 0.25).abs() < 1e-9, "{t}");
        assert!((l.get_point(0.5) - p3(2.0, 0.0, 0.0)).magnitude() < 1e-9);
    }

    #[test]
    fn line_strip_2d_intersections() {
        // A polyline going up from (0,0) to (4,4) in steps.
        let mut s = LineStrip2D::new();
        for i in 0..=4 { s.add_point(p2(i as f64, i as f64)); }
        let (t, pt) = s.get_closest_intersection(p2(0.0, 2.0), p2(1.0, 0.0)).expect("hit");
        assert!((t - 2.0).abs() < 1e-6 && (pt.x - 2.0).abs() < 1e-6);
        assert!(s.get_closest_intersection(p2(0.0, 9.0), p2(1.0, 0.0)).is_none());
        let (t, _) = s.get_first_intersection(p2(0.0, 3.0), p2(1.0, 0.0)).expect("hit");
        assert!((t - 3.0).abs() < 1e-6);
        assert!((s.get_function_value(2.5) - 2.5).abs() < 1e-9);
        assert!((s.get_curve_param(2) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn del_point_keeps_cached_params_like_the_original() {
        let mut s = LineStrip2D::new();
        for i in 0..4 { s.add_point(p2(i as f64, 0.0)); }
        assert!((s.get_curve_param(3) - 1.0).abs() < 1e-12);
        s.del_point();
        assert_eq!(s.num_points(), 3);
        assert!((s.get_curve_param(2) - 2.0 / 3.0).abs() < 1e-12, "cached parameters are kept");
    }
}
