use glam::Vec2;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransformMode {
    Affine,
    PerspectiveQuad,
    MeshGrid,
    ThinPlateSpline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InterpolationMode {
    Nearest,
    Bilinear,
    Bicubic,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AffineTransform2D {
    pub translation: Vec2,
    pub pivot: Vec2,
    pub scale: Vec2,
    pub rotation_rad: f32,
    pub skew: Vec2, // Skew (X, Y) in radians
    pub flip_h: bool,
    pub flip_v: bool,
}

impl Default for AffineTransform2D {
    fn default() -> Self {
        Self {
            translation: Vec2::ZERO,
            pivot: Vec2::ZERO,
            scale: Vec2::ONE,
            rotation_rad: 0.0,
            skew: Vec2::ZERO,
            flip_h: false,
            flip_v: false,
        }
    }
}

impl AffineTransform2D {
    pub fn new(pivot: Vec2) -> Self {
        Self {
            translation: Vec2::ZERO,
            pivot,
            scale: Vec2::ONE,
            rotation_rad: 0.0,
            skew: Vec2::ZERO,
            flip_h: false,
            flip_v: false,
        }
    }

    /// Maps a point from source local patch coordinates to transformed canvas coordinates
    pub fn forward(&self, pt: Vec2) -> Vec2 {
        // 1. Shift relative to pivot
        let mut p = pt - self.pivot;

        // 2. Flip
        if self.flip_h {
            p.x = -p.x;
        }
        if self.flip_v {
            p.y = -p.y;
        }

        // 3. Skew
        if self.skew.x.abs() > 1e-6 || self.skew.y.abs() > 1e-6 {
            let sx = p.x + p.y * self.skew.x.tan();
            let sy = p.y + p.x * self.skew.y.tan();
            p.x = sx;
            p.y = sy;
        }

        // 4. Scale
        p *= self.scale;

        // 5. Rotate
        let cos_r = self.rotation_rad.cos();
        let sin_r = self.rotation_rad.sin();
        let rot_p = Vec2::new(
            p.x * cos_r - p.y * sin_r,
            p.x * sin_r + p.y * cos_r,
        );

        // 6. Restore pivot and apply translation
        rot_p + self.pivot + self.translation
    }

    /// Maps a canvas coordinate backward to source local patch coordinates
    pub fn inverse(&self, pt: Vec2) -> Vec2 {
        // 1. Shift back translation and pivot
        let p = pt - self.translation - self.pivot;

        // 2. Inverse Rotate
        let cos_r = (-self.rotation_rad).cos();
        let sin_r = (-self.rotation_rad).sin();
        let mut rot_p = Vec2::new(
            p.x * cos_r - p.y * sin_r,
            p.x * sin_r + p.y * cos_r,
        );

        // 3. Inverse Scale
        let sx = if self.scale.x.abs() > 1e-6 { self.scale.x } else if self.scale.x < 0.0 { -1e-6 } else { 1e-6 };
        let sy = if self.scale.y.abs() > 1e-6 { self.scale.y } else if self.scale.y < 0.0 { -1e-6 } else { 1e-6 };
        rot_p.x /= sx;
        rot_p.y /= sy;

        // 4. Inverse Skew
        if self.skew.x.abs() > 1e-6 || self.skew.y.abs() > 1e-6 {
            let tan_x = self.skew.x.tan();
            let tan_y = self.skew.y.tan();
            let denom = 1.0 - tan_x * tan_y;
            if denom.abs() > 1e-6 {
                let orig_x = (rot_p.x - rot_p.y * tan_x) / denom;
                let orig_y = (rot_p.y - rot_p.x * tan_y) / denom;
                rot_p.x = orig_x;
                rot_p.y = orig_y;
            }
        }

        // 5. Inverse Flip
        if self.flip_h {
            rot_p.x = -rot_p.x;
        }
        if self.flip_v {
            rot_p.y = -rot_p.y;
        }

        // 6. Restore pivot
        rot_p + self.pivot
    }
}

// =========================================================================
// Perspective Quad Homography Transform
// =========================================================================

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PerspectiveQuadTransform {
    pub src_corners: [Vec2; 4], // [TopLeft, TopRight, BottomRight, BottomLeft]
    pub dst_corners: [Vec2; 4],
    inv_h: [f32; 9],           // 3x3 inverse homography matrix (row-major)
    pub is_valid: bool,
}

impl Default for PerspectiveQuadTransform {
    fn default() -> Self {
        Self {
            src_corners: [
                Vec2::new(0.0, 0.0),
                Vec2::new(100.0, 0.0),
                Vec2::new(100.0, 100.0),
                Vec2::new(0.0, 100.0),
            ],
            dst_corners: [
                Vec2::new(0.0, 0.0),
                Vec2::new(100.0, 0.0),
                Vec2::new(100.0, 100.0),
                Vec2::new(0.0, 100.0),
            ],
            inv_h: [
                1.0, 0.0, 0.0,
                0.0, 1.0, 0.0,
                0.0, 0.0, 1.0,
            ],
            is_valid: true,
        }
    }
}

impl PerspectiveQuadTransform {
    pub fn new(src_corners: [Vec2; 4], dst_corners: [Vec2; 4]) -> Self {
        let mut transform = Self {
            src_corners,
            dst_corners,
            inv_h: [0.0; 9],
            is_valid: false,
        };
        transform.recompute();
        transform
    }

    /// Solves the 3x3 Projective Homography mapping dst_corners -> src_corners for backward sampling
    pub fn recompute(&mut self) {
        // We want H_inv that maps (dst_x, dst_y) -> (src_x, src_y)
        // Using Direct Linear Transformation (DLT) 8x8 system
        if let Some(h) = solve_homography(&self.dst_corners, &self.src_corners) {
            self.inv_h = h;
            self.is_valid = true;
        } else {
            self.is_valid = false;
        }
    }

    /// Backward maps a canvas position (x, y) to local patch position (u, v)
    #[inline]
    pub fn inverse(&self, pt: Vec2) -> Option<Vec2> {
        if !self.is_valid {
            return None;
        }
        let w = self.inv_h[6] * pt.x + self.inv_h[7] * pt.y + self.inv_h[8];
        if w.abs() < 1e-7 {
            return None;
        }
        let inv_w = 1.0 / w;
        let u = (self.inv_h[0] * pt.x + self.inv_h[1] * pt.y + self.inv_h[2]) * inv_w;
        let v = (self.inv_h[3] * pt.x + self.inv_h[4] * pt.y + self.inv_h[5]) * inv_w;
        Some(Vec2::new(u, v))
    }
}

/// Solves 3x3 Homography H such that H * from[i] = to[i] for 4 points
fn solve_homography(from: &[Vec2; 4], to: &[Vec2; 4]) -> Option<[f32; 9]> {
    let mut a = [[0.0_f32; 9]; 8];

    for i in 0..4 {
        let x = from[i].x;
        let y = from[i].y;
        let u = to[i].x;
        let v = to[i].y;

        // Equation 1 for x-component
        a[i * 2][0] = -x;
        a[i * 2][1] = -y;
        a[i * 2][2] = -1.0;
        a[i * 2][3] = 0.0;
        a[i * 2][4] = 0.0;
        a[i * 2][5] = 0.0;
        a[i * 2][6] = x * u;
        a[i * 2][7] = y * u;
        a[i * 2][8] = u;

        // Equation 2 for y-component
        a[i * 2 + 1][0] = 0.0;
        a[i * 2 + 1][1] = 0.0;
        a[i * 2 + 1][2] = 0.0;
        a[i * 2 + 1][3] = -x;
        a[i * 2 + 1][4] = -y;
        a[i * 2 + 1][5] = -1.0;
        a[i * 2 + 1][6] = x * v;
        a[i * 2 + 1][7] = y * v;
        a[i * 2 + 1][8] = v;
    }

    // Solve 8x8 system via Gaussian elimination with partial pivoting (setting h8 = 1.0)
    let mut m = [[0.0_f32; 9]; 8];
    for r in 0..8 {
        for c in 0..8 {
            m[r][c] = a[r][c];
        }
        m[r][8] = -a[r][8]; // RHS
    }

    for i in 0..8 {
        let mut pivot = i;
        let mut max_val = m[i][i].abs();
        for r in (i + 1)..8 {
            if m[r][i].abs() > max_val {
                max_val = m[r][i].abs();
                pivot = r;
            }
        }

        if max_val < 1e-8 {
            return None;
        }

        if pivot != i {
            m.swap(i, pivot);
        }

        let diag = m[i][i];
        for c in i..=8 {
            m[i][c] /= diag;
        }

        for r in 0..8 {
            if r != i {
                let factor = m[r][i];
                for c in i..=8 {
                    m[r][c] -= factor * m[i][c];
                }
            }
        }
    }

    Some([
        m[0][8], m[1][8], m[2][8],
        m[3][8], m[4][8], m[5][8],
        m[6][8], m[7][8], 1.0,
    ])
}

// =========================================================================
// Mesh Warp Grid (N x M Control Vertices)
// =========================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeshWarpGrid {
    pub rows: usize, // e.g. 4 (creates 3 cell rows)
    pub cols: usize, // e.g. 4 (creates 3 cell cols)
    pub src_origin: Vec2,
    pub patch_w: f32,
    pub patch_h: f32,
    pub vertices: Vec<Vec2>, // Length = rows * cols (canvas-space coordinates)
}

impl MeshWarpGrid {
    pub fn new(rows: usize, cols: usize, src_origin: Vec2, patch_w: f32, patch_h: f32) -> Self {
        let rows = rows.clamp(2, 16);
        let cols = cols.clamp(2, 16);
        let mut vertices = Vec::with_capacity(rows * cols);

        for r in 0..rows {
            let v = if rows > 1 { r as f32 / (rows - 1) as f32 } else { 0.0 };
            for c in 0..cols {
                let u = if cols > 1 { c as f32 / (cols - 1) as f32 } else { 0.0 };
                let pt = src_origin + Vec2::new(u * patch_w, v * patch_h);
                vertices.push(pt);
            }
        }

        Self {
            rows,
            cols,
            src_origin,
            patch_w,
            patch_h,
            vertices,
        }
    }

    pub fn reset_grid(&mut self) {
        let rows = self.rows;
        let cols = self.cols;
        self.vertices.clear();
        for r in 0..rows {
            let v = if rows > 1 { r as f32 / (rows - 1) as f32 } else { 0.0 };
            for c in 0..cols {
                let u = if cols > 1 { c as f32 / (cols - 1) as f32 } else { 0.0 };
                let pt = self.src_origin + Vec2::new(u * self.patch_w, v * self.patch_h);
                self.vertices.push(pt);
            }
        }
    }

    #[inline]
    pub fn vertex_idx(&self, row: usize, col: usize) -> usize {
        row * self.cols + col
    }

    #[inline]
    pub fn get_vertex(&self, row: usize, col: usize) -> Vec2 {
        self.vertices[self.vertex_idx(row, col)]
    }

    #[inline]
    pub fn set_vertex(&mut self, row: usize, col: usize, pos: Vec2) {
        let idx = self.vertex_idx(row, col);
        self.vertices[idx] = pos;
    }

    /// Resize the grid density while preserving warped vertex positions via bicubic resampling
    pub fn resize_density(&mut self, new_rows: usize, new_cols: usize) {
        let new_rows = new_rows.clamp(2, 16);
        let new_cols = new_cols.clamp(2, 16);
        if new_rows == self.rows && new_cols == self.cols {
            return;
        }

        let mut new_vertices = Vec::with_capacity(new_rows * new_cols);
        for r in 0..new_rows {
            let v = if new_rows > 1 { r as f32 / (new_rows - 1) as f32 } else { 0.0 };
            for c in 0..new_cols {
                let u = if new_cols > 1 { c as f32 / (new_cols - 1) as f32 } else { 0.0 };
                let sample_pos = self.sample_surface(u, v);
                new_vertices.push(sample_pos);
            }
        }

        self.rows = new_rows;
        self.cols = new_cols;
        self.vertices = new_vertices;
    }

    /// Evaluates forward position on the warped mesh surface for normalized UV in [0, 1]
    pub fn sample_surface(&self, u: f32, v: f32) -> Vec2 {
        let u = u.clamp(0.0, 1.0);
        let v = v.clamp(0.0, 1.0);

        let cell_c_f = u * (self.cols - 1) as f32;
        let cell_r_f = v * (self.rows - 1) as f32;

        let c0 = (cell_c_f.floor() as usize).min(self.cols - 2);
        let r0 = (cell_r_f.floor() as usize).min(self.rows - 2);
        let c1 = c0 + 1;
        let r1 = r0 + 1;

        let fu = cell_c_f - c0 as f32;
        let fv = cell_r_f - r0 as f32;

        let p00 = self.get_vertex(r0, c0);
        let p10 = self.get_vertex(r0, c1);
        let p01 = self.get_vertex(r1, c0);
        let p11 = self.get_vertex(r1, c1);

        let top = p00.lerp(p10, fu);
        let bot = p01.lerp(p11, fu);
        top.lerp(bot, fv)
    }

    /// Backward maps a canvas coordinate to patch local UV coordinates (0..patch_w, 0..patch_h)
    pub fn inverse_sample(&self, canvas_pt: Vec2) -> Option<Vec2> {
        // Test each quad cell in the mesh
        for r in 0..(self.rows - 1) {
            for c in 0..(self.cols - 1) {
                let p00 = self.get_vertex(r, c);
                let p10 = self.get_vertex(r, c + 1);
                let p11 = self.get_vertex(r + 1, c + 1);
                let p01 = self.get_vertex(r + 1, c);

                // Check bounding box of cell first
                let min_x = p00.x.min(p10.x).min(p11.x).min(p01.x) - 1.0;
                let max_x = p00.x.max(p10.x).max(p11.x).max(p01.x) + 1.0;
                let min_y = p00.y.min(p10.y).min(p11.y).min(p01.y) - 1.0;
                let max_y = p00.y.max(p10.y).max(p11.y).max(p01.y) + 1.0;

                if canvas_pt.x < min_x || canvas_pt.x > max_x || canvas_pt.y < min_y || canvas_pt.y > max_y {
                    continue;
                }

                if let Some((cell_u, cell_v)) = inverse_bilinear_quad(p00, p10, p11, p01, canvas_pt) {
                    if (0.0..=1.0).contains(&cell_u) && (0.0..=1.0).contains(&cell_v) {
                        let global_u = (c as f32 + cell_u) / (self.cols - 1) as f32;
                        let global_v = (r as f32 + cell_v) / (self.rows - 1) as f32;
                        return Some(Vec2::new(global_u * self.patch_w, global_v * self.patch_h));
                    }
                }
            }
        }
        None
    }
}

/// Solves inverse bilinear coordinates (u, v) on a quad ABCD such that
/// P = (1-u)(1-v)A + u(1-v)B + uvC + (1-u)vD
fn inverse_bilinear_quad(a: Vec2, b: Vec2, c: Vec2, d: Vec2, p: Vec2) -> Option<(f32, f32)> {
    let e = b - a;
    let f = d - a;
    let g = a - b + c - d;
    let h = p - a;

    let cross_2d = |v1: Vec2, v2: Vec2| -> f32 { v1.x * v2.y - v1.y * v2.x };

    let k2 = cross_2d(g, f);
    let k1 = cross_2d(e, f) + cross_2d(h, g);
    let k0 = cross_2d(h, e);

    // If quad is a parallelogram (g ~ 0)
    if k2.abs() < 1e-7 {
        if k1.abs() < 1e-7 {
            return None;
        }
        let v = -k0 / k1;
        let u_denom = e.x + g.x * v;
        let u = if u_denom.abs() > 1e-6 {
            (h.x - f.x * v) / u_denom
        } else {
            let u_denom_y = e.y + g.y * v;
            if u_denom_y.abs() > 1e-6 {
                (h.y - f.y * v) / u_denom_y
            } else {
                return None;
            }
        };
        return Some((u, v));
    }

    // Solve quadratic equation: k2 * v^2 + k1 * v + k0 = 0
    let discriminant = k1 * k1 - 4.0 * k2 * k0;
    if discriminant < 0.0 {
        return None;
    }

    let sqrt_d = discriminant.sqrt();
    let v_candidates = [
        (-k1 + sqrt_d) / (2.0 * k2),
        (-k1 - sqrt_d) / (2.0 * k2),
    ];

    for &v in &v_candidates {
        if (-0.05..=1.05).contains(&v) {
            let u_denom = e.x + g.x * v;
            let u = if u_denom.abs() > 1e-6 {
                (h.x - f.x * v) / u_denom
            } else {
                let u_denom_y = e.y + g.y * v;
                if u_denom_y.abs() > 1e-6 {
                    (h.y - f.y * v) / u_denom_y
                } else {
                    continue;
                }
            };

            if (-0.05..=1.05).contains(&u) {
                return Some((u.clamp(0.0, 1.0), v.clamp(0.0, 1.0)));
            }
        }
    }

    None
}

// =========================================================================
// Thin Plate Spline (TPS) Landmark Warping Engine
// =========================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThinPlateSpline2D {
    pub src_points: Vec<Vec2>, // Target pin positions on canvas (backward mapping independent variables)
    pub dst_points: Vec<Vec2>, // Source patch positions (dependent variables)
    weights_x: Vec<f32>,       // TPS weights for x-coordinate
    weights_y: Vec<f32>,       // TPS weights for y-coordinate
    affine_x: [f32; 3],        // [a0, a1, a2] for x
    affine_y: [f32; 3],        // [a0, a1, a2] for y
    pub is_valid: bool,
}

impl Default for ThinPlateSpline2D {
    fn default() -> Self {
        Self {
            src_points: Vec::new(),
            dst_points: Vec::new(),
            weights_x: Vec::new(),
            weights_y: Vec::new(),
            affine_x: [0.0; 3],
            affine_y: [0.0; 3],
            is_valid: false,
        }
    }
}

impl ThinPlateSpline2D {
    /// TPS Radial Basis Function Kernel: U(r) = r^2 * ln(r + 1e-6)
    #[inline]
    fn rbf(r: f32) -> f32 {
        if r <= 1e-6 {
            0.0
        } else {
            let r2 = r * r;
            r2 * (r + 1e-6).ln()
        }
    }

    /// Creates and solves a 2D Thin Plate Spline mapping canvas points -> source patch points
    pub fn solve(canvas_pins: &[Vec2], patch_pins: &[Vec2]) -> Self {
        let k = canvas_pins.len();
        if k < 3 || canvas_pins.len() != patch_pins.len() {
            return Self::default();
        }

        let n = k + 3;
        let mut m = vec![vec![0.0_f32; n + 2]; n]; // n rows, n+2 cols (last 2 cols are RHS for X and Y)

        // Fill K matrix: K[i][j] = U(||p_i - p_j||)
        let lambda = 0.001_f32; // Regularization smoothing
        for i in 0..k {
            for j in 0..k {
                if i == j {
                    m[i][j] = lambda;
                } else {
                    let dist = canvas_pins[i].distance(canvas_pins[j]);
                    m[i][j] = Self::rbf(dist);
                }
            }
        }

        // Fill P matrix: [1, x_i, y_i]
        for i in 0..k {
            m[i][k] = 1.0;
            m[i][k + 1] = canvas_pins[i].x;
            m[i][k + 2] = canvas_pins[i].y;

            // Fill P^T
            m[k][i] = 1.0;
            m[k + 1][i] = canvas_pins[i].x;
            m[k + 2][i] = canvas_pins[i].y;

            // Fill RHS
            m[i][n] = patch_pins[i].x;
            m[i][n + 1] = patch_pins[i].y;
        }

        // Solve system via Gauss-Jordan elimination with partial pivoting
        for i in 0..n {
            let mut pivot = i;
            let mut max_val = m[i][i].abs();
            for r in (i + 1)..n {
                if m[r][i].abs() > max_val {
                    max_val = m[r][i].abs();
                    pivot = r;
                }
            }

            if max_val < 1e-9 {
                return Self::default();
            }

            if pivot != i {
                m.swap(i, pivot);
            }

            let diag = m[i][i];
            for c in i..(n + 2) {
                m[i][c] /= diag;
            }

            for r in 0..n {
                if r != i {
                    let factor = m[r][i];
                    for c in i..(n + 2) {
                        m[r][c] -= factor * m[i][c];
                    }
                }
            }
        }

        let mut weights_x = Vec::with_capacity(k);
        let mut weights_y = Vec::with_capacity(k);
        for i in 0..k {
            weights_x.push(m[i][n]);
            weights_y.push(m[i][n + 1]);
        }

        let affine_x = [m[k][n], m[k + 1][n], m[k + 2][n]];
        let affine_y = [m[k][n + 1], m[k + 1][n + 1], m[k + 2][n + 1]];

        Self {
            src_points: canvas_pins.to_vec(),
            dst_points: patch_pins.to_vec(),
            weights_x,
            weights_y,
            affine_x,
            affine_y,
            is_valid: true,
        }
    }

    /// Evaluates backward mapped source coordinate for a canvas point
    #[inline]
    pub fn evaluate(&self, pt: Vec2) -> Vec2 {
        if !self.is_valid {
            return pt;
        }

        let mut out_x = self.affine_x[0] + self.affine_x[1] * pt.x + self.affine_x[2] * pt.y;
        let mut out_y = self.affine_y[0] + self.affine_y[1] * pt.x + self.affine_y[2] * pt.y;

        for i in 0..self.src_points.len() {
            let dist = pt.distance(self.src_points[i]);
            let u = Self::rbf(dist);
            out_x += self.weights_x[i] * u;
            out_y += self.weights_y[i] * u;
        }

        Vec2::new(out_x, out_y)
    }
}

// =========================================================================
// High-Quality Sub-Pixel Image Sampling Filters
// =========================================================================

/// Nearest-neighbor sampling helper for crisp pixel art
#[inline]
pub fn sample_nearest(src: &[u8], w: u32, h: u32, x: f32, y: f32) -> (u8, u8, u8, u8) {
    let ix = x.floor() as isize;
    let iy = y.floor() as isize;
    if ix >= 0 && ix < w as isize && iy >= 0 && iy < h as isize {
        let idx = (iy as usize * w as usize + ix as usize) * 4;
        if idx + 3 < src.len() {
            return (src[idx], src[idx + 1], src[idx + 2], src[idx + 3]);
        }
    }
    (0, 0, 0, 0)
}

/// Bilinear sub-pixel sampling helper
#[inline]
pub fn sample_bilinear(src: &[u8], w: u32, h: u32, x: f32, y: f32) -> (u8, u8, u8, u8) {
    if x < 0.0 || y < 0.0 || x >= w as f32 || y >= h as f32 {
        let ix = x.round() as isize;
        let iy = y.round() as isize;
        if ix >= 0 && ix < w as isize && iy >= 0 && iy < h as isize {
            let idx = (iy as usize * w as usize + ix as usize) * 4;
            if idx + 3 < src.len() {
                return (src[idx], src[idx + 1], src[idx + 2], src[idx + 3]);
            }
        }
        return (0, 0, 0, 0);
    }

    let x0 = x.floor() as usize;
    let y0 = y.floor() as usize;
    let x1 = (x0 + 1).min(w as usize - 1);
    let y1 = (y0 + 1).min(h as usize - 1);

    let fx = x - x0 as f32;
    let fy = y - y0 as f32;
    let inv_fx = 1.0 - fx;
    let inv_fy = 1.0 - fy;

    let w00 = inv_fx * inv_fy;
    let w10 = fx * inv_fy;
    let w01 = inv_fx * fy;
    let w11 = fx * fy;

    let get_px = |px: usize, py: usize| -> (f32, f32, f32, f32) {
        let idx = (py * w as usize + px) * 4;
        (
            src[idx] as f32,
            src[idx + 1] as f32,
            src[idx + 2] as f32,
            src[idx + 3] as f32,
        )
    };

    let p00 = get_px(x0, y0);
    let p10 = get_px(x1, y0);
    let p01 = get_px(x0, y1);
    let p11 = get_px(x1, y1);

    let r = (p00.0 * w00 + p10.0 * w10 + p01.0 * w01 + p11.0 * w11).round() as u8;
    let g = (p00.1 * w00 + p10.1 * w10 + p01.1 * w01 + p11.1 * w11).round() as u8;
    let b = (p00.2 * w00 + p10.2 * w10 + p01.2 * w01 + p11.2 * w11).round() as u8;
    let a = (p00.3 * w00 + p10.3 * w10 + p01.3 * w01 + p11.3 * w11).round() as u8;

    (r, g, b, a)
}

/// Catmull-Rom Cubic Spline 1D weight evaluation
#[inline]
fn cubic_hermite(a: f32, b: f32, c: f32, d: f32, t: f32) -> f32 {
    let a_val = -0.5 * a + 1.5 * b - 1.5 * c + 0.5 * d;
    let b_val = a - 2.5 * b + 2.0 * c - 0.5 * d;
    let c_val = -0.5 * a + 0.5 * c;
    let d_val = b;
    a_val * t * t * t + b_val * t * t + c_val * t + d_val
}

/// Bicubic (16-sample Catmull-Rom) sub-pixel sampling helper
#[inline]
pub fn sample_bicubic(src: &[u8], w: u32, h: u32, x: f32, y: f32) -> (u8, u8, u8, u8) {
    if x < 0.0 || y < 0.0 || x >= w as f32 || y >= h as f32 {
        return (0, 0, 0, 0);
    }

    let x_int = x.floor() as i32;
    let y_int = y.floor() as i32;
    let tx = x - x_int as f32;
    let ty = y - y_int as f32;

    let get_px = |ix: i32, iy: i32| -> [f32; 4] {
        let cx = ix.clamp(0, w as i32 - 1) as usize;
        let cy = iy.clamp(0, h as i32 - 1) as usize;
        let idx = (cy * w as usize + cx) * 4;
        [
            src[idx] as f32,
            src[idx + 1] as f32,
            src[idx + 2] as f32,
            src[idx + 3] as f32,
        ]
    };

    let mut col_results = [[0.0_f32; 4]; 4];
    for m in 0..4 {
        let sample_y = y_int - 1 + m;
        let p0 = get_px(x_int - 1, sample_y);
        let p1 = get_px(x_int, sample_y);
        let p2 = get_px(x_int + 1, sample_y);
        let p3 = get_px(x_int + 2, sample_y);

        for ch in 0..4 {
            col_results[m as usize][ch] = cubic_hermite(p0[ch], p1[ch], p2[ch], p3[ch], tx);
        }
    }

    let mut final_rgba = [0u8; 4];
    for ch in 0..4 {
        let v = cubic_hermite(
            col_results[0][ch],
            col_results[1][ch],
            col_results[2][ch],
            col_results[3][ch],
            ty,
        );
        final_rgba[ch] = v.round().clamp(0.0, 255.0) as u8;
    }

    (final_rgba[0], final_rgba[1], final_rgba[2], final_rgba[3])
}

#[inline]
pub fn sample_interpolated(
    src: &[u8],
    w: u32,
    h: u32,
    x: f32,
    y: f32,
    mode: InterpolationMode,
) -> (u8, u8, u8, u8) {
    match mode {
        InterpolationMode::Nearest => sample_nearest(src, w, h, x, y),
        InterpolationMode::Bilinear => sample_bilinear(src, w, h, x, y),
        InterpolationMode::Bicubic => sample_bicubic(src, w, h, x, y),
    }
}

// =========================================================================
// Zero-Allocation High-Performance Patch Rasterizers
// =========================================================================

/// Alpha-composites a source pixel onto destination buffer
#[inline]
fn composite_pixel_in_place(dst: &mut [u8], dst_idx: usize, r: u8, g: u8, b: u8, a: u8) {
    if a == 0 || dst_idx + 3 >= dst.len() {
        return;
    }

    let cur_a = dst[dst_idx + 3] as u32;
    let new_a = a as u32;

    if new_a == 255 || cur_a == 0 {
        dst[dst_idx] = r;
        dst[dst_idx + 1] = g;
        dst[dst_idx + 2] = b;
        dst[dst_idx + 3] = a;
    } else {
        let inv_a = 255 - new_a;
        let out_a = new_a + cur_a * inv_a / 255;
        if out_a > 0 {
            let out_r = ((r as u32 * new_a + dst[dst_idx] as u32 * cur_a * inv_a / 255) / out_a) as u8;
            let out_g = ((g as u32 * new_a + dst[dst_idx + 1] as u32 * cur_a * inv_a / 255) / out_a) as u8;
            let out_b = ((b as u32 * new_a + dst[dst_idx + 2] as u32 * cur_a * inv_a / 255) / out_a) as u8;
            dst[dst_idx] = out_r;
            dst[dst_idx + 1] = out_g;
            dst[dst_idx + 2] = out_b;
            dst[dst_idx + 3] = out_a.min(255) as u8;
        }
    }
}

/// Transforms an Affine patch and composites it onto a target layer canvas
pub fn render_transformed_patch(
    src_patch: &[u8],
    patch_w: u32,
    patch_h: u32,
    patch_origin: Vec2,
    transform: &AffineTransform2D,
    interp: InterpolationMode,
    dst_layer_pixels: &mut [u8],
    doc_w: u32,
    doc_h: u32,
) {
    if patch_w == 0 || patch_h == 0 || src_patch.is_empty() {
        return;
    }

    let corners = [
        patch_origin,
        patch_origin + Vec2::new(patch_w as f32, 0.0),
        patch_origin + Vec2::new(patch_w as f32, patch_h as f32),
        patch_origin + Vec2::new(0.0, patch_h as f32),
    ];

    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;
    let mut min_y = f32::MAX;
    let mut max_y = f32::MIN;

    for &c in &corners {
        let tc = transform.forward(c);
        min_x = min_x.min(tc.x);
        max_x = max_x.max(tc.x);
        min_y = min_y.min(tc.y);
        max_y = max_y.max(tc.y);
    }

    let start_x = (min_x.floor() as isize).max(0).min(doc_w as isize) as usize;
    let end_x = (max_x.ceil() as isize + 1).max(0).min(doc_w as isize) as usize;
    let start_y = (min_y.floor() as isize).max(0).min(doc_h as isize) as usize;
    let end_y = (max_y.ceil() as isize + 1).max(0).min(doc_h as isize) as usize;

    for y in start_y..end_y {
        for x in start_x..end_x {
            let canvas_pt = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
            let src_canvas_pt = transform.inverse(canvas_pt);
            let local_x = src_canvas_pt.x - patch_origin.x;
            let local_y = src_canvas_pt.y - patch_origin.y;

            let (r, g, b, a) = sample_interpolated(src_patch, patch_w, patch_h, local_x, local_y, interp);
            if a > 0 {
                let dst_idx = (y * doc_w as usize + x) * 4;
                composite_pixel_in_place(dst_layer_pixels, dst_idx, r, g, b, a);
            }
        }
    }
}

/// Renders a Perspective Quad warped patch onto target layer canvas
pub fn render_quad_transformed_patch(
    src_patch: &[u8],
    patch_w: u32,
    patch_h: u32,
    quad: &PerspectiveQuadTransform,
    interp: InterpolationMode,
    dst_layer_pixels: &mut [u8],
    doc_w: u32,
    doc_h: u32,
) {
    if patch_w == 0 || patch_h == 0 || src_patch.is_empty() || !quad.is_valid {
        return;
    }

    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;
    let mut min_y = f32::MAX;
    let mut max_y = f32::MIN;

    for &c in &quad.dst_corners {
        min_x = min_x.min(c.x);
        max_x = max_x.max(c.x);
        min_y = min_y.min(c.y);
        max_y = max_y.max(c.y);
    }

    let start_x = (min_x.floor() as isize).max(0).min(doc_w as isize) as usize;
    let end_x = (max_x.ceil() as isize + 1).max(0).min(doc_w as isize) as usize;
    let start_y = (min_y.floor() as isize).max(0).min(doc_h as isize) as usize;
    let end_y = (max_y.ceil() as isize + 1).max(0).min(doc_h as isize) as usize;

    for y in start_y..end_y {
        for x in start_x..end_x {
            let canvas_pt = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
            if let Some(src_pt) = quad.inverse(canvas_pt) {
                let local_x = src_pt.x - quad.src_corners[0].x;
                let local_y = src_pt.y - quad.src_corners[0].y;

                let (r, g, b, a) = sample_interpolated(src_patch, patch_w, patch_h, local_x, local_y, interp);
                if a > 0 {
                    let dst_idx = (y * doc_w as usize + x) * 4;
                    composite_pixel_in_place(dst_layer_pixels, dst_idx, r, g, b, a);
                }
            }
        }
    }
}

/// Renders a Mesh Grid warped patch onto target layer canvas
pub fn render_mesh_warped_patch(
    src_patch: &[u8],
    patch_w: u32,
    patch_h: u32,
    mesh: &MeshWarpGrid,
    interp: InterpolationMode,
    dst_layer_pixels: &mut [u8],
    doc_w: u32,
    doc_h: u32,
) {
    if patch_w == 0 || patch_h == 0 || src_patch.is_empty() {
        return;
    }

    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;
    let mut min_y = f32::MAX;
    let mut max_y = f32::MIN;

    for &v in &mesh.vertices {
        min_x = min_x.min(v.x);
        max_x = max_x.max(v.x);
        min_y = min_y.min(v.y);
        max_y = max_y.max(v.y);
    }

    let start_x = (min_x.floor() as isize).max(0).min(doc_w as isize) as usize;
    let end_x = (max_x.ceil() as isize + 1).max(0).min(doc_w as isize) as usize;
    let start_y = (min_y.floor() as isize).max(0).min(doc_h as isize) as usize;
    let end_y = (max_y.ceil() as isize + 1).max(0).min(doc_h as isize) as usize;

    for y in start_y..end_y {
        for x in start_x..end_x {
            let canvas_pt = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
            if let Some(src_pt) = mesh.inverse_sample(canvas_pt) {
                let (r, g, b, a) = sample_interpolated(src_patch, patch_w, patch_h, src_pt.x, src_pt.y, interp);
                if a > 0 {
                    let dst_idx = (y * doc_w as usize + x) * 4;
                    composite_pixel_in_place(dst_layer_pixels, dst_idx, r, g, b, a);
                }
            }
        }
    }
}

/// Renders a Thin Plate Spline (TPS) warped patch onto target layer canvas
pub fn render_tps_warped_patch(
    src_patch: &[u8],
    patch_w: u32,
    patch_h: u32,
    patch_origin: Vec2,
    tps: &ThinPlateSpline2D,
    interp: InterpolationMode,
    dst_layer_pixels: &mut [u8],
    doc_w: u32,
    doc_h: u32,
) {
    if patch_w == 0 || patch_h == 0 || src_patch.is_empty() || !tps.is_valid {
        return;
    }

    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;
    let mut min_y = f32::MAX;
    let mut max_y = f32::MIN;

    for &v in &tps.src_points {
        min_x = min_x.min(v.x);
        max_x = max_x.max(v.x);
        min_y = min_y.min(v.y);
        max_y = max_y.max(v.y);
    }

    // Add padding around pins for elastic deformation bounds
    let pad = 32.0_f32;
    let start_x = ((min_x - pad).floor() as isize).max(0).min(doc_w as isize) as usize;
    let end_x = ((max_x + pad).ceil() as isize + 1).max(0).min(doc_w as isize) as usize;
    let start_y = ((min_y - pad).floor() as isize).max(0).min(doc_h as isize) as usize;
    let end_y = ((max_y + pad).ceil() as isize + 1).max(0).min(doc_h as isize) as usize;

    for y in start_y..end_y {
        for x in start_x..end_x {
            let canvas_pt = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
            let src_canvas_pt = tps.evaluate(canvas_pt);
            let local_x = src_canvas_pt.x - patch_origin.x;
            let local_y = src_canvas_pt.y - patch_origin.y;

            if local_x >= 0.0 && local_x < patch_w as f32 && local_y >= 0.0 && local_y < patch_h as f32 {
                let (r, g, b, a) = sample_interpolated(src_patch, patch_w, patch_h, local_x, local_y, interp);
                if a > 0 {
                    let dst_idx = (y * doc_w as usize + x) * 4;
                    composite_pixel_in_place(dst_layer_pixels, dst_idx, r, g, b, a);
                }
            }
        }
    }
}

// =========================================================================
// Tests
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_affine_identity() {
        let t = AffineTransform2D::default();
        let pt = Vec2::new(50.0, 75.0);
        let forward = t.forward(pt);
        let inverse = t.inverse(forward);
        assert!((pt.x - inverse.x).abs() < 1e-4);
        assert!((pt.y - inverse.y).abs() < 1e-4);
    }

    #[test]
    fn test_affine_scale_rotate_flip() {
        let mut t = AffineTransform2D::new(Vec2::new(50.0, 50.0));
        t.scale = Vec2::new(2.0, 1.5);
        t.rotation_rad = std::f32::consts::FRAC_PI_4;
        t.flip_h = true;
        t.translation = Vec2::new(10.0, -20.0);

        let pt = Vec2::new(30.0, 40.0);
        let forward = t.forward(pt);
        let inverse = t.inverse(forward);
        assert!((pt.x - inverse.x).abs() < 1e-3);
        assert!((pt.y - inverse.y).abs() < 1e-3);
    }

    #[test]
    fn test_perspective_quad_homography() {
        let src = [
            Vec2::new(0.0, 0.0),
            Vec2::new(100.0, 0.0),
            Vec2::new(100.0, 100.0),
            Vec2::new(0.0, 100.0),
        ];
        let dst = [
            Vec2::new(10.0, 5.0),
            Vec2::new(90.0, 15.0),
            Vec2::new(110.0, 95.0),
            Vec2::new(5.0, 85.0),
        ];

        let quad = PerspectiveQuadTransform::new(src, dst);
        assert!(quad.is_valid);

        // Test corners backward map accurately
        for i in 0..4 {
            let src_mapped = quad.inverse(dst[i]).unwrap();
            assert!((src_mapped.x - src[i].x).abs() < 0.1);
            assert!((src_mapped.y - src[i].y).abs() < 0.1);
        }
    }

    #[test]
    fn test_mesh_grid_surface_and_inverse() {
        let mesh = MeshWarpGrid::new(3, 3, Vec2::ZERO, 100.0, 100.0);
        assert_eq!(mesh.rows, 3);
        assert_eq!(mesh.cols, 3);
        assert_eq!(mesh.vertices.len(), 9);

        // Center point
        let sample = mesh.inverse_sample(Vec2::new(50.0, 50.0)).unwrap();
        assert!((sample.x - 50.0).abs() < 0.5);
        assert!((sample.y - 50.0).abs() < 0.5);
    }

    #[test]
    fn test_thin_plate_spline() {
        let canvas_pins = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(100.0, 0.0),
            Vec2::new(100.0, 100.0),
            Vec2::new(0.0, 100.0),
            Vec2::new(50.0, 50.0),
        ];
        let patch_pins = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(100.0, 0.0),
            Vec2::new(100.0, 100.0),
            Vec2::new(0.0, 100.0),
            Vec2::new(55.0, 45.0), // Nudged center
        ];

        let tps = ThinPlateSpline2D::solve(&canvas_pins, &patch_pins);
        assert!(tps.is_valid);

        // Evaluate center pin
        let center_mapped = tps.evaluate(Vec2::new(50.0, 50.0));
        assert!((center_mapped.x - 55.0).abs() < 0.2);
        assert!((center_mapped.y - 45.0).abs() < 0.2);
    }
}

