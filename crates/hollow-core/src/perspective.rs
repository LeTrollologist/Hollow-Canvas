use glam::Vec2;
use serde::{Deserialize, Serialize};

/// Type of perspective grid projection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PerspectiveType {
    #[default]
    None,
    OnePoint,
    TwoPoint,
    ThreePoint,
}

impl PerspectiveType {
    pub fn name(&self) -> &'static str {
        match self {
            Self::None => "None (Off)",
            Self::OnePoint => "1-Point Perspective",
            Self::TwoPoint => "2-Point Perspective",
            Self::ThreePoint => "3-Point Perspective",
        }
    }
}

/// Perspective configuration and vanishing point state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerspectiveConfig {
    pub p_type: PerspectiveType,
    pub show_guides: bool,
    pub snap_enabled: bool,
    pub snap_strength: f32, // 0.0 to 1.0 (1.0 = strict projection)
    pub guide_density: u32, // Rays per vanishing point (e.g. 8 to 48)
    pub guide_opacity: f32, // 0.05 to 1.0
    pub horizon_y: f32,     // Canvas Y coordinate of the horizon line
    pub horizon_angle: f32, // Tilt angle in degrees (-45.0 to +45.0)
    pub vp1: Vec2,          // Primary/Left Vanishing Point (Canvas coords)
    pub vp2: Vec2,          // Right Vanishing Point (Canvas coords)
    pub vp3: Vec2,          // Vertical / Nadir / Zenith Vanishing Point (Canvas coords)
    pub horizon_color: [u8; 4],
    pub guide_color: [u8; 4],
}

impl Default for PerspectiveConfig {
    fn default() -> Self {
        Self {
            p_type: PerspectiveType::None,
            show_guides: true,
            snap_enabled: false,
            snap_strength: 1.0,
            guide_density: 16,
            guide_opacity: 0.35,
            horizon_y: 540.0,
            horizon_angle: 0.0,
            vp1: Vec2::new(960.0, 540.0),
            vp2: Vec2::new(2880.0, 540.0),
            vp3: Vec2::new(960.0, 2500.0),
            horizon_color: [60, 220, 240, 200], // Cyan horizon line
            guide_color: [120, 160, 255, 120],  // Soft blue guide rays
        }
    }
}

impl PerspectiveConfig {
    /// Initialize balanced default vanishing point coordinates for given canvas dimensions
    pub fn init_for_canvas(&mut self, width: u32, height: u32) {
        let w = width as f32;
        let h = height as f32;
        let cy = h * 0.5;

        self.horizon_y = cy;
        self.horizon_angle = 0.0;

        match self.p_type {
            PerspectiveType::None | PerspectiveType::OnePoint => {
                // 1-Point: Vanishing Point at canvas center
                self.vp1 = Vec2::new(w * 0.5, cy);
                self.vp2 = Vec2::new(w * 1.5, cy);
                self.vp3 = Vec2::new(w * 0.5, h * 2.0);
            }
            PerspectiveType::TwoPoint => {
                // 2-Point: Left VP placed outside left canvas, Right VP outside right canvas
                self.vp1 = Vec2::new(-w * 0.4, cy);
                self.vp2 = Vec2::new(w * 1.4, cy);
                self.vp3 = Vec2::new(w * 0.5, h * 2.0);
            }
            PerspectiveType::ThreePoint => {
                // 3-Point: 2 VPs along high horizon, 1 Vertical VP below (ground level / bird's eye)
                self.vp1 = Vec2::new(-w * 0.35, h * 0.35);
                self.vp2 = Vec2::new(w * 1.35, h * 0.35);
                self.vp3 = Vec2::new(w * 0.5, h * 2.2);
                self.horizon_y = h * 0.35;
            }
        }
    }

    /// Reset vanishing points to standard presets for the current canvas size
    pub fn reset_preset(&mut self, p_type: PerspectiveType, width: u32, height: u32) {
        self.p_type = p_type;
        self.init_for_canvas(width, height);
    }

    /// Returns list of active vanishing points for the current perspective type
    pub fn get_active_vps(&self) -> Vec<Vec2> {
        match self.p_type {
            PerspectiveType::None => Vec::new(),
            PerspectiveType::OnePoint => vec![self.vp1],
            PerspectiveType::TwoPoint => vec![self.vp1, self.vp2],
            PerspectiveType::ThreePoint => vec![self.vp1, self.vp2, self.vp3],
        }
    }

    /// Get allowable perspective guide directions at a specific canvas coordinate
    pub fn get_directions_at(&self, pos: Vec2) -> Vec<Vec2> {
        let mut dirs = Vec::with_capacity(4);

        match self.p_type {
            PerspectiveType::None => {}
            PerspectiveType::OnePoint => {
                // Radial ray towards VP1
                let d1 = (self.vp1 - pos).normalize_or_zero();
                if d1.length_squared() > 1e-4 {
                    dirs.push(d1);
                }
                // True Horizontal axis
                let rad = self.horizon_angle.to_radians();
                let horiz = Vec2::new(rad.cos(), rad.sin());
                dirs.push(horiz);
                // True Vertical axis (perpendicular to horizon)
                let vert = Vec2::new(-rad.sin(), rad.cos());
                dirs.push(vert);
            }
            PerspectiveType::TwoPoint => {
                // Radial ray towards VP1 (Left)
                let d1 = (self.vp1 - pos).normalize_or_zero();
                if d1.length_squared() > 1e-4 {
                    dirs.push(d1);
                }
                // Radial ray towards VP2 (Right)
                let d2 = (self.vp2 - pos).normalize_or_zero();
                if d2.length_squared() > 1e-4 {
                    dirs.push(d2);
                }
                // Vertical axis (perpendicular to horizon line)
                let rad = self.horizon_angle.to_radians();
                let vert = Vec2::new(-rad.sin(), rad.cos());
                dirs.push(vert);
            }
            PerspectiveType::ThreePoint => {
                // Radial ray towards VP1 (Left)
                let d1 = (self.vp1 - pos).normalize_or_zero();
                if d1.length_squared() > 1e-4 {
                    dirs.push(d1);
                }
                // Radial ray towards VP2 (Right)
                let d2 = (self.vp2 - pos).normalize_or_zero();
                if d2.length_squared() > 1e-4 {
                    dirs.push(d2);
                }
                // Radial ray towards VP3 (Nadir / Zenith)
                let d3 = (self.vp3 - pos).normalize_or_zero();
                if d3.length_squared() > 1e-4 {
                    dirs.push(d3);
                }
            }
        }

        dirs
    }

    /// Constrain a moving stroke point onto the best-matching perspective axis.
    /// Returns `(constrained_point, active_axis_unit_vector)`.
    pub fn constrain_stroke_point(
        &self,
        stroke_start: Vec2,
        current_pos: Vec2,
        locked_axis: Option<Vec2>,
    ) -> (Vec2, Vec2) {
        if self.p_type == PerspectiveType::None || !self.snap_enabled {
            return (current_pos, Vec2::ZERO);
        }

        let delta = current_pos - stroke_start;
        let dist = delta.length();

        // If very close to start point, don't snap yet to avoid jitter
        if dist < 2.5 {
            return (current_pos, locked_axis.unwrap_or(Vec2::X));
        }

        let move_dir = delta / dist;

        // If we already have a locked axis for this continuous stroke, project onto it
        let best_axis = if let Some(axis) = locked_axis {
            axis
        } else {
            // Find the allowable direction that has the highest absolute dot product alignment
            let candidates = self.get_directions_at(stroke_start);
            if candidates.is_empty() {
                return (current_pos, Vec2::ZERO);
            }

            let mut max_dot = -1.0_f32;
            let mut chosen = candidates[0];

            for &cand in &candidates {
                let dot = (move_dir.dot(cand)).abs();
                if dot > max_dot {
                    max_dot = dot;
                    chosen = cand;
                }
            }
            chosen
        };

        // Project delta onto the chosen axis
        let proj_dist = delta.dot(best_axis);
        let snapped_delta = best_axis * proj_dist;

        // Blend between raw and snapped based on snap_strength
        let strength = self.snap_strength.clamp(0.0, 1.0);
        let final_delta = delta * (1.0 - strength) + snapped_delta * strength;
        let constrained_pt = stroke_start + final_delta;

        (constrained_pt, best_axis)
    }

    /// Compute a set of radial rays emanating from a vanishing point across canvas bounds
    pub fn generate_rays_for_vp(
        &self,
        vp: Vec2,
        canvas_w: f32,
        canvas_h: f32,
        num_rays: u32,
    ) -> Vec<(Vec2, Vec2)> {
        let num_rays = num_rays.clamp(4, 72);
        let mut rays = Vec::with_capacity(num_rays as usize);

        // Define corners and perimeter sample points of the canvas
        let corners = [
            Vec2::new(0.0, 0.0),
            Vec2::new(canvas_w, 0.0),
            Vec2::new(canvas_w, canvas_h),
            Vec2::new(0.0, canvas_h),
        ];

        // Find min and max angles from VP to canvas corners
        let mut angles: Vec<f32> = corners
            .iter()
            .map(|c| {
                let diff = *c - vp;
                diff.y.atan2(diff.x)
            })
            .collect();

        angles.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let min_angle = angles.first().copied().unwrap_or(0.0);
        let max_angle = angles.last().copied().unwrap_or(std::f32::consts::PI * 2.0);
        let angle_span = (max_angle - min_angle).abs();

        // Calculate a ray length large enough to cross the canvas completely
        let ray_len = (canvas_w * canvas_w + canvas_h * canvas_h).sqrt() * 2.5 + vp.length();

        for i in 0..num_rays {
            let t = (i as f32) / ((num_rays - 1).max(1) as f32);
            let angle = min_angle + (t - 0.25) * (angle_span * 1.5);
            let dir = Vec2::new(angle.cos(), angle.sin());
            let end_pt = vp + dir * ray_len;
            rays.push((vp, end_pt));
        }

        rays
    }
}
