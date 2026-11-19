// SPDX-License-Identifier: GPL-3.0-or-later
//! Triangulated surface on a grid of ribs x rib points, and its intersection with a vertical
//! plane. Port of the parts of VocalTractLab's `Surface` that the vocal tract model needs,
//! (C) Peter Birkholz, GPL-3.0-or-later.
//!
//! The vertical cutting plane contains the line `Q + t v` in the xy plane (the sagittal plane)
//! and the z axis. Intersection points are returned in plane coordinates
//! `(x = height z, y = position along the line)`, as in the original.
//!
//! The original accelerates the search with a tile grid; here every triangle is tested
//! (the set of intersected triangles is identical, only the order differs).

use crate::geom::*;

#[derive(Clone, Debug, Default)]
pub struct Surface {
    pub num_ribs: usize,
    pub num_rib_points: usize,
    pub vertices: Vec<P3>,
    /// Vertex indices of each triangle.
    tri_vertex: Vec<[usize; 3]>,
    /// Edge indices of each triangle.
    tri_edge: Vec<[usize; 3]>,
    edge_vertex: Vec<[usize; 2]>,
    // intersection state
    v_side: Vec<i8>,
    v_tested: Vec<bool>,
    e_tested: Vec<bool>,
    e_hit: Vec<bool>,
    e_point: Vec<P2>,
    line_point: P2,
    left_line_point: P2,
    right_line_point: P2,
    line_vector: P2,
    /// For each pair of neighbouring ribs: whether the cutting line may cross it.
    rib_pair_mixed: Vec<bool>,
    /// Bounds of the vertices in the xy plane, recomputed lazily after vertices changed.
    bounds: (f64, f64, f64, f64),
    bounds_dirty: bool,
}

impl Surface {
    pub fn new(ribs: usize, rib_points: usize) -> Self {
        let mut s = Surface::default();
        s.init(ribs, rib_points);
        s
    }

    pub fn num_triangles(&self) -> usize { self.tri_vertex.len() }
    pub fn num_vertices(&self) -> usize { self.vertices.len() }

    /// (Re)builds the grid topology; all vertices are reset to the origin.
    pub fn init(&mut self, ribs: usize, rib_points: usize) {
        *self = Surface::default();
        self.num_ribs = ribs;
        self.num_rib_points = rib_points;
        if ribs == 0 || rib_points == 0 { return; }
        let (nr, np) = (ribs, rib_points);
        let num_triangles = 2 * (nr - 1) * (np - 1);
        let num_vertices = nr * np;
        let num_edges = 3 * nr * np - 2 * nr - 2 * np + 1;
        self.vertices = vec![P3::default(); num_vertices];

        let mut edges = Vec::with_capacity(num_edges);
        for j in 0..np {
            for i in 0..nr - 1 { edges.push([i * np + j, (i + 1) * np + j]); }
        }
        for i in 0..nr {
            for j in 0..np - 1 { edges.push([i * np + j, i * np + j + 1]); }
        }
        for j in 0..np - 1 {
            for i in 0..nr - 1 { edges.push([i * np + j, (i + 1) * np + j + 1]); }
        }
        debug_assert_eq!(edges.len(), num_edges);

        let a = np * (nr - 1);
        let b = a + (np - 1) * nr;
        let mut tv = Vec::with_capacity(num_triangles);
        let mut te = Vec::with_capacity(num_triangles);
        for i in 0..nr - 1 {
            for j in 0..np - 1 {
                tv.push([i * np + j, (i + 1) * np + j + 1, i * np + j + 1]);
                te.push([b + j * (nr - 1) + i, (j + 1) * (nr - 1) + i, a + i * (np - 1) + j]);
                tv.push([i * np + j, (i + 1) * np + j, (i + 1) * np + j + 1]);
                te.push([b + j * (nr - 1) + i, j * (nr - 1) + i, a + (i + 1) * (np - 1) + j]);
            }
        }
        self.tri_vertex = tv;
        self.tri_edge = te;
        self.edge_vertex = edges;
        self.v_side = vec![0; num_vertices];
        self.v_tested = vec![false; num_vertices];
        self.e_tested = vec![false; num_edges];
        self.e_hit = vec![false; num_edges];
        self.e_point = vec![P2::default(); num_edges];
    }

    #[inline]
    pub fn set_vertex(&mut self, rib: usize, rib_point: usize, p: P3) {
        self.vertices[rib * self.num_rib_points + rib_point] = p;
        self.bounds_dirty = true;
    }

    #[inline]
    pub fn get_vertex(&self, rib: usize, rib_point: usize) -> P3 {
        self.vertices[rib * self.num_rib_points + rib_point]
    }

    /// Reverses the orientation of every triangle (flips the sign of the triangle normals).
    pub fn swap_triangle_orientation(&mut self) {
        for t in self.tri_vertex.iter_mut() { t.swap(0, 2); }
    }

    /// Copies the vertex positions of another surface with identical topology.
    pub fn copy_vertices_from(&mut self, other: &Surface) {
        self.vertices.clone_from(&other.vertices);
    }

    /// Prepares the intersection with the vertical plane through `q` along direction `v`.
    pub fn prepare_intersection(&mut self, q: P2, mut v: P2) {
        const EPS: f64 = 0.000001;
        for t in self.v_tested.iter_mut() { *t = false; }
        for t in self.e_tested.iter_mut() { *t = false; }
        v.normalize();
        self.line_vector = v;
        self.line_point = q;
        let n = p2(-v.y, v.x);
        self.left_line_point = q + n * EPS;
        self.right_line_point = q - n * EPS;
        let (nr, np) = (self.num_ribs, self.num_rib_points);
        self.rib_pair_mixed.clear();
        // A line that misses the bounding rectangle of the whole surface cannot cut it.
        if self.bounds_dirty {
            self.bounds = self.bounds_xy();
            self.bounds_dirty = false;
        }
        let (x0, x1, y0, y1) = self.bounds;
        const MARGIN: f64 = 0.0001;
        let side = |x: f64, y: f64| (x - q.x) * v.y - (y - q.y) * v.x;
        let corners = [side(x0 - MARGIN, y0 - MARGIN), side(x1 + MARGIN, y0 - MARGIN), side(x0 - MARGIN, y1 + MARGIN), side(x1 + MARGIN, y1 + MARGIN)];
        if corners.iter().all(|&c| c > 0.0) || corners.iter().all(|&c| c < 0.0) {
            self.rib_pair_mixed.resize(nr.saturating_sub(1), false);
            return;
        }
        // Side of every vertex, then which rib pairs can be cut at all.
        for vi in 0..self.vertices.len() { self.side_of(vi); }
        for i in 0..nr.saturating_sub(1) {
            let sides = &self.v_side[i * np..(i + 2) * np];
            let all_pos = sides.iter().all(|&s| s > 0);
            let all_neg = sides.iter().all(|&s| s < 0);
            self.rib_pair_mixed.push(!(all_pos || all_neg));
        }
    }

    /// All triangles cut by the prepared plane, in triangle order: (end points of the cut, normal).
    pub fn cut_triangles(&mut self) -> Vec<(P2, P2, P2)> {
        let mut out = Vec::new();
        let per_pair = 2 * self.num_rib_points.saturating_sub(1);
        for i in 0..self.rib_pair_mixed.len() {
            if !self.rib_pair_mixed[i] { continue; }
            for t in i * per_pair..(i + 1) * per_pair {
                if let Some(r) = self.triangle_intersection(t) { out.push(r); }
            }
        }
        out
    }

    fn side_of(&mut self, vi: usize) {
        if self.v_tested[vi] { return; }
        self.v_tested[vi] = true;
        self.v_side[vi] = 0;
        let c = self.vertices[vi];
        let lv = self.line_vector;
        let dl = (c.x - self.left_line_point.x) * lv.y - (c.y - self.left_line_point.y) * lv.x;
        if dl < 0.0 { self.v_side[vi] = -1; }
        let dr = (c.x - self.right_line_point.x) * lv.y - (c.y - self.right_line_point.y) * lv.x;
        if dr > 0.0 { self.v_side[vi] = 1; }
    }

    fn edge_intersection(&mut self, ei: usize) -> bool {
        if self.e_tested[ei] { return self.e_hit[ei]; }
        self.e_tested[ei] = true;
        let [v0, v1] = self.edge_vertex[ei];
        self.side_of(v0);
        self.side_of(v1);
        self.e_hit[ei] = false;
        let (s0, s1) = (self.v_side[v0], self.v_side[v1]);
        if (s0 >= 0 && s1 <= 0) || (s0 <= 0 && s1 >= 0) {
            const EPS: f64 = 0.000001;
            let lv = self.line_vector;
            let p = self.vertices[v0];
            let u = self.vertices[v1] - p;
            let r = p3(p.x - self.line_point.x, p.y - self.line_point.y, p.z);
            let den = -u.x * lv.y + u.y * lv.x;
            if den != 0.0 {
                let d = (-lv.x * r.y + lv.y * r.x) / den;
                if d >= -EPS && d < 1.0 + EPS {
                    self.e_hit[ei] = true;
                    self.e_point[ei] = p2(
                        (lv.x * (u.y * r.z - u.z * r.y) + lv.y * (u.z * r.x - u.x * r.z)) / den,
                        (-u.x * r.y + u.y * r.x) / den,
                    );
                }
            }
        }
        self.e_hit[ei]
    }

    /// Intersection of triangle `index` with the prepared plane: the end points of the cut
    /// segment and the triangle normal in plane coordinates, or `None` if it is not cut.
    pub fn triangle_intersection(&mut self, index: usize) -> Option<(P2, P2, P2)> {
        // Quick reject: if all three vertices lie strictly on the same side of the cutting line,
        // none of the edges crosses it (the edge test needs `s0 >= 0 && s1 <= 0` or the reverse).
        let [a, b, c] = self.tri_vertex[index];
        self.side_of(a);
        self.side_of(b);
        self.side_of(c);
        let (sa, sb, sc) = (self.v_side[a], self.v_side[b], self.v_side[c]);
        if (sa > 0 && sb > 0 && sc > 0) || (sa < 0 && sb < 0 && sc < 0) { return None; }
        let [e0, e1, e2] = self.tri_edge[index];
        let mut q = [P2::default(); 3];
        let mut n = 0;
        for e in [e0, e1, e2] {
            if self.edge_intersection(e) { q[n] = self.e_point[e]; n += 1; }
        }
        if n < 2 { return None; }
        let [v0, v1, v2] = self.tri_vertex[index];
        let normal = cross_product(self.vertices[v1] - self.vertices[v0], self.vertices[v2] - self.vertices[v0]);
        let lv = self.line_vector;
        let nrm = p2(normal.z, normal.x * lv.x + normal.y * lv.y);
        if n == 2 { return Some((q[0], q[1], nrm)); }
        let l = [(q[0] - q[1]).square_magnitude(), (q[1] - q[2]).square_magnitude(), (q[2] - q[0]).square_magnitude()];
        let (p0, p1) = if l[0] >= l[1] && l[0] >= l[2] {
            (q[0], q[1])
        } else if l[1] >= l[2] {
            (q[1], q[2])
        } else {
            (q[2], q[0])
        };
        Some((p0, p1, nrm))
    }

    /// Axis-aligned bounds of the vertices in the xy plane: (min_x, max_x, min_y, max_y).
    pub fn bounds_xy(&self) -> (f64, f64, f64, f64) {
        let mut b = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for v in &self.vertices {
            b.0 = b.0.min(v.x); b.1 = b.1.max(v.x);
            b.2 = b.2.min(v.y); b.3 = b.3.max(v.y);
        }
        b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A flat 4 x 3 grid in the plane y = 0 spanning x in 0..3 and z in 0..2.
    fn flat() -> Surface {
        let mut s = Surface::new(4, 3);
        for r in 0..4 { for p in 0..3 { s.set_vertex(r, p, p3(r as f64, 0.0, p as f64)); } }
        s
    }

    #[test]
    fn topology_counts_match_the_original_formulas() {
        let s = flat();
        assert_eq!(s.num_vertices(), 12);
        assert_eq!(s.num_triangles(), 2 * 3 * 2);
        assert_eq!(s.edge_vertex.len(), 3 * 12 - 8 - 6 + 1);
    }

    #[test]
    fn plane_cuts_the_surface_along_its_width() {
        let mut s = flat();
        // Line through (1.5, -1) pointing in +y: crosses the flat surface at x = 1.5 for z in 0..2.
        s.prepare_intersection(p2(1.5, -1.0), p2(0.0, 1.0));
        let mut total = 0.0;
        for t in 0..s.num_triangles() {
            if let Some((a, b, _)) = s.triangle_intersection(t) {
                // Plane coordinates: x = z (height), y = position along the line (1.0 at y = 0).
                assert!((a.y - 1.0).abs() < 1e-9 && (b.y - 1.0).abs() < 1e-9);
                total += (a.x - b.x).abs();
            }
        }
        assert!((total - 2.0).abs() < 1e-9, "cut length {total}");
    }

    #[test]
    fn cut_triangles_equals_testing_every_triangle() {
        let mut s = flat();
        for (q, v) in [(p2(1.5, -1.0), p2(0.0, 1.0)), (p2(-1.0, 0.5), p2(1.0, 0.2)), (p2(0.5, 5.0), p2(1.0, -1.0))] {
            s.prepare_intersection(q, v);
            let all: Vec<_> = (0..s.num_triangles()).filter_map(|t| s.triangle_intersection(t)).collect();
            s.prepare_intersection(q, v);
            assert_eq!(s.cut_triangles(), all);
        }
    }

    #[test]
    fn plane_missing_the_surface_gives_nothing() {
        let mut s = flat();
        s.prepare_intersection(p2(10.0, -1.0), p2(0.0, 1.0));
        assert!((0..s.num_triangles()).all(|t| s.triangle_intersection(t).is_none()));
    }

    #[test]
    fn swapping_orientation_flips_the_normal() {
        let mut s = flat();
        s.prepare_intersection(p2(1.5, -1.0), p2(0.0, 1.0));
        let n1 = (0..s.num_triangles()).find_map(|t| s.triangle_intersection(t)).unwrap().2;
        s.swap_triangle_orientation();
        s.prepare_intersection(p2(1.5, -1.0), p2(0.0, 1.0));
        let n2 = (0..s.num_triangles()).find_map(|t| s.triangle_intersection(t)).unwrap().2;
        assert!((n1.x + n2.x).abs() < 1e-12 && (n1.y + n2.y).abs() < 1e-12);
    }
}
