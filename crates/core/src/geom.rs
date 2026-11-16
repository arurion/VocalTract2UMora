// SPDX-License-Identifier: GPL-3.0-or-later
//! 2D / 3D geometry primitives. Port of the parts of VocalTractLab's `Geometry` that the vocal
//! tract model uses, (C) Peter Birkholz, GPL-3.0-or-later. Behaviour (including the in-place
//! `normalize` / `turn_left`, and the tolerances 0.01 and 0.0001) follows the original.

use std::f64::consts::PI;
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct P2 {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct P3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

pub const fn p2(x: f64, y: f64) -> P2 { P2 { x, y } }
pub const fn p3(x: f64, y: f64, z: f64) -> P3 { P3 { x, y, z } }

impl P2 {
    pub fn magnitude(&self) -> f64 { (self.x * self.x + self.y * self.y).sqrt() }
    pub fn square_magnitude(&self) -> f64 { self.x * self.x + self.y * self.y }
    /// Normalises in place (a zero vector stays zero) and returns the result.
    pub fn normalize(&mut self) -> P2 {
        let l = (self.x * self.x + self.y * self.y).sqrt();
        if l != 0.0 { self.x /= l; self.y /= l; }
        *self
    }
    /// Rotates by +90 degrees in place and returns the result.
    pub fn turn_left(&mut self) -> P2 {
        let t = self.x;
        self.x = -self.y;
        self.y = t;
        *self
    }
    pub fn turn_right(&mut self) -> P2 {
        let t = self.x;
        self.x = self.y;
        self.y = -t;
        *self
    }
    pub fn to_p3(&self) -> P3 { p3(self.x, self.y, 0.0) }
}

impl P3 {
    pub fn magnitude(&self) -> f64 { (self.x * self.x + self.y * self.y + self.z * self.z).sqrt() }
    pub fn normalize(&mut self) -> P3 {
        let l = self.magnitude();
        if l != 0.0 { self.x /= l; self.y /= l; self.z /= l; }
        *self
    }
    pub fn to_p2(&self) -> P2 { p2(self.x, self.y) }
}

macro_rules! vec_ops {
    ($t:ident, $($f:ident),+) => {
        impl Add for $t { type Output = $t; fn add(self, o: $t) -> $t { $t { $($f: self.$f + o.$f),+ } } }
        impl Sub for $t { type Output = $t; fn sub(self, o: $t) -> $t { $t { $($f: self.$f - o.$f),+ } } }
        impl Mul<f64> for $t { type Output = $t; fn mul(self, d: f64) -> $t { $t { $($f: self.$f * d),+ } } }
        impl Mul<$t> for f64 { type Output = $t; fn mul(self, p: $t) -> $t { $t { $($f: p.$f * self),+ } } }
        impl Div<f64> for $t { type Output = $t; fn div(self, d: f64) -> $t { $t { $($f: self.$f / d),+ } } }
        impl Neg for $t { type Output = $t; fn neg(self) -> $t { $t { $($f: -self.$f),+ } } }
        impl AddAssign for $t { fn add_assign(&mut self, o: $t) { $(self.$f += o.$f;)+ } }
        impl SubAssign for $t { fn sub_assign(&mut self, o: $t) { $(self.$f -= o.$f;)+ } }
        impl MulAssign<f64> for $t { fn mul_assign(&mut self, d: f64) { $(self.$f *= d;)+ } }
        impl DivAssign<f64> for $t { fn div_assign(&mut self, d: f64) { $(self.$f /= d;)+ } }
    };
}
vec_ops!(P2, x, y);
vec_ops!(P3, x, y, z);

pub fn scalar_product2(a: P2, b: P2) -> f64 { a.x * b.x + a.y * b.y }
pub fn scalar_product(a: P3, b: P3) -> f64 { a.x * b.x + a.y * b.y + a.z * b.z }
pub fn cross_product(a: P3, b: P3) -> P3 {
    p3(a.y * b.z - a.z * b.y, a.z * b.x - a.x * b.z, a.x * b.y - a.y * b.x)
}

/// A ray / infinite line: point `p` and direction `v`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vector2D {
    pub p: P2,
    pub v: P2,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Line2D {
    pub p: [P2; 2],
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Line3D {
    pub p: [P3; 2],
}

impl Vector2D {
    pub fn new(p: P2, v: P2) -> Self { Vector2D { p, v } }
    pub fn get_point(&self, t: f64) -> P2 { self.p + self.v * t }
    pub fn normalize(&mut self) {
        let arg = self.v.x * self.v.x + self.v.y * self.v.y;
        if arg != 1.0 && arg != 0.0 {
            let a = arg.sqrt();
            self.v.x /= a;
            self.v.y /= a;
        }
    }
    /// Intersection with another ray; returns (point, t) where t is the parameter on `self`.
    pub fn intersect_vector(&self, o: &Vector2D) -> (P2, f64) {
        let (b, w) = (o.p, o.v);
        let mut den = self.v.x * w.y - self.v.y * w.x;
        if den == 0.0 { den = 0.0001; }
        let t = (w.x * (self.p.y - b.y) - w.y * (self.p.x - b.x)) / den;
        (self.get_point(t), t)
    }
    /// Intersection with a 2D line segment; `ok` is false if it lies outside the segment.
    pub fn intersect_line2(&self, l: &Line2D) -> (P2, f64, bool) {
        let b = l.p[0];
        let w = l.p[1] - l.p[0];
        let mut den = self.v.x * w.y - self.v.y * w.x;
        if den == 0.0 { den = 0.0001; }
        let m = (self.v.x * (self.p.y - b.y) - self.v.y * (self.p.x - b.x)) / den;
        if m < -0.01 || m > 1.01 || l.p[0] == l.p[1] {
            return (self.p, 0.0, false);
        }
        let t = (w.x * (self.p.y - b.y) - w.y * (self.p.x - b.x)) / den;
        (self.get_point(t), t, true)
    }
    /// Intersection with a 3D line segment (only x and y are considered); the returned point
    /// is on the 3D line.
    pub fn intersect_line3(&self, l: &Line3D) -> (P3, f64, bool) {
        let b = l.p[0];
        let w = l.p[1] - l.p[0];
        let mut den = self.v.x * w.y - self.v.y * w.x;
        if den == 0.0 { den = 0.0001; }
        let m = (self.v.x * (self.p.y - b.y) - self.v.y * (self.p.x - b.x)) / den;
        if m < -0.01 || m > 1.01 || (w.x == 0.0 && w.y == 0.0) {
            return (p3(self.p.x, self.p.y, 0.0), 0.0, false);
        }
        let t = (w.x * (self.p.y - b.y) - w.y * (self.p.x - b.x)) / den;
        (l.get_point(m), t, true)
    }
}

impl Line2D {
    pub fn new(p0: P2, p1: P2) -> Self { Line2D { p: [p0, p1] } }
    pub fn get_point(&self, t: f64) -> P2 { self.p[0] + (self.p[1] - self.p[0]) * t }
    /// Intersection with a ray; `t` is the parameter on this segment.
    pub fn intersect_vector(&self, v: &Vector2D) -> (P2, f64, bool) {
        let a = self.p[0];
        let vv = self.p[1] - self.p[0];
        let (b, w) = (v.p, v.v);
        let orig = vv.x * w.y - vv.y * w.x;
        let den = if orig == 0.0 { 0.0001 } else { orig };
        let t = (w.x * (a.y - b.y) - w.y * (a.x - b.x)) / den;
        let ok = t > -0.01 && t < 1.01 && orig != 0.0;
        (self.get_point(t), t, ok)
    }
    pub fn intersect_line(&self, l: &Line2D) -> (P2, f64, bool) {
        let a = self.p[0];
        let v = self.p[1] - self.p[0];
        let b = l.p[0];
        let w = l.p[1] - l.p[0];
        let orig = v.x * w.y - v.y * w.x;
        let den = if orig == 0.0 { 0.0001 } else { orig };
        let t = (w.x * (a.y - b.y) - w.y * (a.x - b.x)) / den;
        let m = (v.x * (a.y - b.y) - v.y * (a.x - b.x)) / den;
        let ok = t > -0.01 && t < 1.01 && m > -0.01 && m < 1.01 && orig != 0.0;
        (self.get_point(t), t, ok)
    }
}

impl Line3D {
    pub fn new(p0: P3, p1: P3) -> Self { Line3D { p: [p0, p1] } }
    pub fn get_point(&self, t: f64) -> P3 { self.p[0] + (self.p[1] - self.p[0]) * t }
    /// Intersection (in the xy plane) with a ray; returns the 3D point, `t` and `ok`.
    pub fn intersect_vector(&self, v: &Vector2D) -> (P3, f64, bool) {
        let l = Line2D::new(self.p[0].to_p2(), self.p[1].to_p2());
        let (_q, t, ok) = l.intersect_vector(v);
        (self.get_point(t), t, ok)
    }
}

/// A circle with an optional valid arc (`arc_angle[0] == arc_angle[1]` means the full circle).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Circle {
    pub m: P2,
    pub r: f64,
    pub arc_angle: [f64; 2],
}

impl Circle {
    pub fn new(c: P2, radius: f64) -> Self { Circle { m: c, r: radius, arc_angle: [0.0, 0.0] } }

    pub fn set_valid_arc(&mut self, mut a0: f64, mut a1: f64) {
        while a0 < 0.0 { a0 += 2.0 * PI; }
        while a0 > 2.0 * PI { a0 -= 2.0 * PI; }
        self.arc_angle[0] = a0;
        while a1 < 0.0 { a1 += 2.0 * PI; }
        while a1 > 2.0 * PI { a1 -= 2.0 * PI; }
        self.arc_angle[1] = a1;
    }

    pub fn get_point(&self, angle: f64) -> P2 {
        p2(self.m.x + self.r * angle.cos(), self.m.y + self.r * angle.sin())
    }

    pub fn get_normal(&self, angle: f64) -> Vector2D {
        let (c, s) = (angle.cos(), angle.sin());
        Vector2D::new(p2(self.m.x + self.r * c, self.m.y + self.r * s), p2(c, s))
    }

    fn in_arc(&self, angle: f64) -> bool {
        let a = self.arc_angle;
        if a[0] == a[1] {
            true
        } else if a[1] > a[0] {
            angle >= a[0] && angle <= a[1]
        } else {
            angle <= a[1] || angle >= a[0]
        }
    }

    /// Shared root finding for a line `p + n v` against the circle: the (up to two) points.
    fn roots(&self, p: P2, v: P2) -> Option<([f64; 2], [P2; 2], [f64; 2])> {
        let mut den = v.x * v.x + v.y * v.y;
        if den == 0.0 { den = 0.0001; }
        let pp = (2.0 * v.x * (p.x - self.m.x) + 2.0 * v.y * (p.y - self.m.y)) / den;
        let q = ((p.x - self.m.x) * (p.x - self.m.x) + (p.y - self.m.y) * (p.y - self.m.y) - self.r * self.r) / den;
        let rad = 0.25 * pp * pp - q;
        if rad < 0.0 { return None; }
        let root = rad.sqrt();
        let n = [-0.5 * pp + root, -0.5 * pp - root];
        let s = [p + v * n[0], p + v * n[1]];
        let mut ang = [0.0; 2];
        for i in 0..2 {
            ang[i] = (s[i].y - self.m.y).atan2(s[i].x - self.m.x);
            if ang[i] < 0.0 { ang[i] += 2.0 * PI; }
        }
        Some((n, s, ang))
    }

    fn pick(p: P2, s: [P2; 2], ang: [f64; 2], ok: [bool; 2]) -> (P2, f64, bool) {
        let d = |q: P2| (q.x - p.x) * (q.x - p.x) + (q.y - p.y) * (q.y - p.y);
        if ok[0] && ok[1] {
            if d(s[0]) < d(s[1]) { (s[0], ang[0], true) } else { (s[1], ang[1], true) }
        } else if ok[0] {
            (s[0], ang[0], true)
        } else if ok[1] {
            (s[1], ang[1], true)
        } else {
            (p2(0.0, 0.0), 0.0, false)
        }
    }

    /// Intersection with a ray (the intersection nearest to the ray origin within the arc).
    /// Returns (point, angle, ok).
    pub fn intersect_vector(&self, v: &Vector2D) -> (P2, f64, bool) {
        let Some((_n, s, ang)) = self.roots(v.p, v.v) else { return (p2(0.0, 0.0), 0.0, false) };
        Self::pick(v.p, s, ang, [self.in_arc(ang[0]), self.in_arc(ang[1])])
    }

    /// Intersection with a line segment (tolerance 0.01 at both ends) within the arc.
    pub fn intersect_line(&self, l: &Line2D) -> (P2, f64, bool) {
        let p = l.p[0];
        let v = l.p[1] - l.p[0];
        let Some((n, s, ang)) = self.roots(p, v) else { return (p2(0.0, 0.0), 0.0, false) };
        let in_line = |t: f64| t >= -0.01 && t <= 1.01;
        let ok = [in_line(n[0]) && self.in_arc(ang[0]), in_line(n[1]) && self.in_arc(ang[1])];
        Self::pick(p, s, ang, ok)
    }
}

/// Angle of the tangent from the point `h` to the circle (`c`, `r`); `clockwise` selects which
/// of the two tangents. Returns 0 if `h` is inside the circle.
pub fn get_circle_tangent(h: P2, c: P2, r: f64, clockwise: bool) -> f64 {
    let p = h.x - c.x;
    let q = h.y - c.y;
    let radikant = p * p + q * q - r * r;
    if radikant < 0.0 { return 0.0; }
    let mut n = radikant.sqrt();
    if clockwise { n = -n; }
    (p * n + q * r).atan2(p * r - q * n)
}

/// Angle (on the ellipse parameter) of the tangent from the point `h` to the ellipse centred
/// on `c` with semi-axes `a` and `b`.
pub fn get_ellipse_tangent(h: P2, c: P2, a: f64, b: f64, clockwise: bool) -> f64 {
    let r = c - h;
    let mut radikant = r.x * r.x * b * b - b * b * a * a + r.y * r.y * a * a;
    if radikant < 0.0 { radikant = 0.0; }
    let root = radikant.sqrt();
    let mut deno = r.x * r.x * b * b + r.y * r.y * a * a;
    const EPS: f64 = 0.000001;
    if deno.abs() < EPS { deno = EPS; }
    let alpha0 = (-b * (r.y * a * a + r.x * root) / deno).atan2(-a * (r.x * b * b - r.y * root) / deno);
    let alpha1 = (-b * (r.y * a * a - r.x * root) / deno).atan2(-a * (r.x * b * b + r.y * root) / deno);
    let (cos, sin) = (alpha0.cos(), alpha0.sin());
    let v1 = c + p2(a * cos, b * sin) - h;
    let v2 = p2(-a * sin, b * cos);
    let test = v1.x * v2.x + v1.y * v2.y;
    if (test >= 0.0 && clockwise) || (test < 0.0 && !clockwise) { alpha1 } else { alpha0 }
}

pub const NUM_ELLIPSE_PROPS: usize = 33;

/// An ellipse centred on `m`. Note: `get_point` returns points relative to the origin
/// (the centre is not added), as in the original.
#[derive(Clone, Debug)]
pub struct Ellipse2D {
    pub m: P2,
    pub half_width: f64,
    pub half_height: f64,
    prop_angle: [f64; NUM_ELLIPSE_PROPS],
    prop_position: [f64; NUM_ELLIPSE_PROPS],
    perimeter: f64,
}

impl Ellipse2D {
    pub fn new(m: P2, half_width: f64, half_height: f64) -> Self {
        let mut e = Ellipse2D { m, half_width, half_height, prop_angle: [0.0; NUM_ELLIPSE_PROPS], prop_position: [0.0; NUM_ELLIPSE_PROPS], perimeter: 0.0 };
        e.set(m, half_width, half_height);
        e
    }

    pub fn set(&mut self, m: P2, half_width: f64, half_height: f64) {
        self.m = m;
        self.half_width = half_width;
        self.half_height = half_height;
        let mut prop_p = [P2::default(); NUM_ELLIPSE_PROPS];
        for i in 0..NUM_ELLIPSE_PROPS {
            self.prop_angle[i] = 2.0 * PI * i as f64 / (NUM_ELLIPSE_PROPS - 1) as f64;
            prop_p[i] = self.get_point(self.prop_angle[i]);
        }
        self.prop_position[0] = 0.0;
        for i in 1..NUM_ELLIPSE_PROPS {
            self.prop_position[i] = self.prop_position[i - 1] + (prop_p[i] - prop_p[i - 1]).magnitude();
        }
        self.perimeter = self.prop_position[NUM_ELLIPSE_PROPS - 1];
    }

    pub fn get_point(&self, angle: f64) -> P2 {
        p2(angle.cos() * self.half_width, angle.sin() * self.half_height)
    }

    /// Angle belonging to the relative arc length position `t` (0..1) along the perimeter.
    pub fn get_angle(&self, t: f64) -> f64 {
        let u = (t * self.perimeter).clamp(0.0, self.perimeter);
        let mut i = 0;
        while i < NUM_ELLIPSE_PROPS - 2 && u > self.prop_position[i + 1] { i += 1; }
        let mut a = self.prop_position[i + 1] - self.prop_position[i];
        if a == 0.0 { a = 0.0001; }
        self.prop_angle[i] + ((self.prop_angle[i + 1] - self.prop_angle[i]) * (u - self.prop_position[i])) / a
    }

    pub fn perimeter(&self) -> f64 { self.perimeter }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_is_in_place_and_zero_safe() {
        let mut a = p2(3.0, 4.0);
        let r = a.normalize();
        assert_eq!(a, r);
        assert!((a.magnitude() - 1.0).abs() < 1e-12);
        let mut z = p2(0.0, 0.0);
        assert_eq!(z.normalize(), p2(0.0, 0.0));
    }

    #[test]
    fn ray_hits_segment_only_inside() {
        let ray = Vector2D::new(p2(0.0, 0.0), p2(1.0, 0.0));
        let seg = Line2D::new(p2(2.0, -1.0), p2(2.0, 1.0));
        let (pt, t, ok) = ray.intersect_line2(&seg);
        assert!(ok && (t - 2.0).abs() < 1e-9 && (pt.x - 2.0).abs() < 1e-9);
        let far = Line2D::new(p2(2.0, 3.0), p2(2.0, 5.0));
        assert!(!ray.intersect_line2(&far).2);
    }

    #[test]
    fn circle_intersections_respect_the_arc() {
        let mut c = Circle::new(p2(0.0, 0.0), 1.0);
        let ray = Vector2D::new(p2(-3.0, 0.0), p2(1.0, 0.0));
        let (pt, _a, ok) = c.intersect_vector(&ray);
        assert!(ok && (pt.x + 1.0).abs() < 1e-9, "nearest hit first");
        c.set_valid_arc(-0.5, 0.5); // only the right side is valid
        let (pt, _a, ok) = c.intersect_vector(&ray);
        assert!(ok && (pt.x - 1.0).abs() < 1e-9);
        let miss = Vector2D::new(p2(-3.0, 5.0), p2(1.0, 0.0));
        assert!(!c.intersect_vector(&miss).2);
    }

    #[test]
    fn ellipse_angle_follows_arc_length() {
        let e = Ellipse2D::new(p2(0.0, 0.0), 2.0, 1.0);
        assert!(e.get_angle(0.0).abs() < 1e-12);
        assert!((e.get_angle(1.0) - 2.0 * PI).abs() < 1e-9);
        assert!((e.get_angle(0.5) - PI).abs() < 1e-6);
        // 32-segment polyline approximation of the exact perimeter (about 9.688).
        assert!((e.perimeter() - 9.672892).abs() < 1e-5, "{}", e.perimeter());
    }
}
