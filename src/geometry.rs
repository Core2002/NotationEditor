//! Pure geometry for the hexagonal playfield.
//!
//! Six lanes point from the outer ring towards the centre.  Lane 0 starts at
//! 210° (lower left) and the lanes advance counter-clockwise in 60° steps, so
//! the physical key order `S D F J K L` also walks around the hexagon.

use bevy::math::Vec2;
use bevy::prelude::Resource;

use crate::chart::LANE_COUNT;

/// Degrees for each lane, starting lower-left and moving counter-clockwise.
pub const LANE_ANGLES_DEG: [f32; LANE_COUNT] = [210.0, 270.0, 330.0, 30.0, 90.0, 150.0];

/// Key legends, in lane order.
pub const LANE_KEYS: [&str; LANE_COUNT] = ["S", "D", "F", "J", "K", "L"];

/// Short colour-friendly lane names (used by the editor help).
pub const LANE_NAMES: [&str; LANE_COUNT] = ["S", "D", "F", "J", "K", "L"];

#[derive(Debug, Clone, Copy, PartialEq, Resource)]
pub struct Layout {
    pub center: Vec2,
    /// Radius of the judgement ring (the hexagon's *inradius*).
    pub judge_radius: f32,
    /// Radius at which notes spawn (the outer hexagon's *inradius*).
    pub spawn_radius: f32,
    /// Nominal note diameter in world units.
    pub note_size: f32,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            center: Vec2::new(0.0, -14.0),
            judge_radius: 92.0,
            spawn_radius: 302.0,
            note_size: 54.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoteMotion {
    pub position: Vec2,
    pub scale: f32,
    pub alpha: f32,
    pub radius: f32,
}

impl Layout {
    pub fn lane_angle_rad(lane: u8) -> f32 {
        LANE_ANGLES_DEG[(lane as usize) % LANE_COUNT].to_radians()
    }

    pub fn lane_dir(&self, lane: u8) -> Vec2 {
        let angle = Self::lane_angle_rad(lane);
        Vec2::new(angle.cos(), angle.sin())
    }

    /// World position on a lane at a given radius from the playfield centre.
    pub fn lane_point(&self, lane: u8, radius: f32) -> Vec2 {
        self.center + self.lane_dir(lane) * radius
    }

    pub fn judge_point(&self, lane: u8) -> Vec2 {
        self.lane_point(lane, self.judge_radius)
    }

    pub fn spawn_point(&self, lane: u8) -> Vec2 {
        self.lane_point(lane, self.spawn_radius)
    }

    /// Vertices of the outer hexagon.
    pub fn outer_hex(&self) -> [Vec2; LANE_COUNT] {
        hex_vertices(self.center, self.spawn_radius / 30.0_f32.to_radians().cos())
    }

    /// Vertices of the judgement hexagon.
    pub fn judge_hex(&self) -> [Vec2; LANE_COUNT] {
        hex_vertices(self.center, self.judge_radius / 30.0_f32.to_radians().cos())
    }

    /// Compute where a note should be drawn at `now` seconds.
    ///
    /// Returns `None` when the note is outside its approach window.
    pub fn note_motion(
        &self,
        lane: u8,
        hit_time: f64,
        now: f64,
        approach_secs: f64,
    ) -> Option<NoteMotion> {
        let approach = approach_secs.max(0.05);
        let remaining = hit_time - now;
        if remaining > approach + 0.10 || remaining < -0.10 {
            return None;
        }
        let progress = (remaining / approach).clamp(-0.12, 1.12) as f32;
        let radius = self.judge_radius + (self.spawn_radius - self.judge_radius) * progress;
        let fade_in = if remaining >= approach {
            0.0
        } else {
            ((approach - remaining) / 0.14).clamp(0.0, 1.0) as f32
        };
        let fade_out = ((remaining + 0.10) / 0.10).clamp(0.0, 1.0) as f32;
        let alpha = fade_in * fade_out;
        let scale = 0.72 + 0.28 * (1.0 - progress.clamp(0.0, 1.0));
        Some(NoteMotion {
            position: self.lane_point(lane, radius),
            scale,
            alpha,
            radius,
        })
    }
}

/// Six vertices of a regular hexagon whose vertices start at angle 0°.
pub fn hex_vertices(center: Vec2, vertex_radius: f32) -> [Vec2; 6] {
    let mut out = [Vec2::ZERO; 6];
    for (index, point) in out.iter_mut().enumerate() {
        let angle = std::f32::consts::TAU * index as f32 / 6.0;
        *point = center + Vec2::new(angle.cos(), angle.sin()) * vertex_radius;
    }
    out
}

/// Rotation-independent point-in-hexagon test, useful for background effects.
pub fn point_in_hex(point: Vec2, center: Vec2, inradius: f32) -> bool {
    let local = point - center;
    let c = 30.0_f32.to_radians().cos();
    let s = 30.0_f32.to_radians().sin();
    // Edge normals at 30°, 90° and 150°. The other three are redundant.
    let projections = [
        local.dot(Vec2::new(c, s)).abs(),
        local.dot(Vec2::Y).abs(),
        local.dot(Vec2::new(-c, s)).abs(),
    ];
    projections.into_iter().fold(0.0_f32, f32::max) <= inradius
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: Vec2, b: Vec2) {
        assert!((a - b).length() < 1e-4, "{a:?} != {b:?}");
    }

    #[test]
    fn lanes_are_sixty_degrees_apart() {
        for lane in 0..LANE_COUNT {
            let a = Layout::lane_angle_rad(lane as u8);
            let b = Layout::lane_angle_rad(((lane + 1) % LANE_COUNT) as u8);
            let mut delta = (b - a).to_degrees();
            while delta < 0.0 {
                delta += 360.0;
            }
            assert!((delta - 60.0).abs() < 0.001);
        }
    }

    #[test]
    fn lane_directions_are_unit_vectors() {
        let layout = Layout::default();
        for lane in 0..LANE_COUNT as u8 {
            assert!((layout.lane_dir(lane).length() - 1.0).abs() < 1e-5);
        }
        // Lane 0 is lower-left, lane 3 is upper-right (directly opposite).
        approx(layout.lane_dir(0), Vec2::new(-0.866_025, -0.5));
        approx(layout.lane_dir(3), -layout.lane_dir(0));
    }

    #[test]
    fn hex_vertices_are_on_the_expected_circle() {
        let center = Vec2::new(10.0, -20.0);
        let points = hex_vertices(center, 100.0);
        for point in points {
            assert!(((point - center).length() - 100.0).abs() < 1e-4);
        }
        assert_eq!(points[0], center + Vec2::new(100.0, 0.0));
    }

    #[test]
    fn note_reaches_judge_ring_at_hit_time() {
        let layout = Layout::default();
        let motion = layout
            .note_motion(0, 5.0, 5.0, 1.8)
            .expect("visible at hit time");
        assert!((motion.radius - layout.judge_radius).abs() < 1e-3);
        assert!((motion.alpha - 1.0).abs() < 1e-3);
        approx(motion.position, layout.judge_point(0));
    }

    #[test]
    fn note_starts_at_outer_ring() {
        let layout = Layout::default();
        let motion = layout
            .note_motion(2, 5.0, 5.0 - 1.8, 1.8)
            .expect("visible at spawn time");
        assert!((motion.radius - layout.spawn_radius).abs() < 1e-3);
        assert!(motion.alpha < 0.01);
        approx(motion.position, layout.spawn_point(2));
    }

    #[test]
    fn notes_outside_window_are_hidden() {
        let layout = Layout::default();
        assert!(layout.note_motion(0, 5.0, 2.0, 1.8).is_none());
        assert!(layout.note_motion(0, 5.0, 5.5, 1.8).is_none());
        assert!(layout.note_motion(0, 5.0, 4.0, 1.8).is_some());
    }

    #[test]
    fn point_in_hex_matches_inradius() {
        let center = Vec2::ZERO;
        assert!(point_in_hex(Vec2::ZERO, center, 100.0));
        assert!(point_in_hex(Vec2::new(99.0, 0.0), center, 100.0));
        assert!(point_in_hex(Vec2::new(0.0, 100.0), center, 100.0));
        assert!(!point_in_hex(Vec2::new(0.0, 101.0), center, 100.0));
        // Along a vertex direction the hexagon extends to inradius/cos(30°).
        assert!(point_in_hex(Vec2::new(114.0, 0.0), center, 100.0));
        assert!(!point_in_hex(Vec2::new(116.0, 0.0), center, 100.0));
    }
}
