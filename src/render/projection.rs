use bevy::camera::{CameraProjection, SubCameraView};
use bevy::prelude::*;
use bevy::reflect::Reflect;

use super::off_axis_projection::*;

/// Off-axis projection for a physical display surface.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component, Debug)]
pub struct OffAxisProjection {
    /// Physical screen quad corners in WORLD space.
    pub lower_left: Vec3,
    pub lower_right: Vec3,
    pub upper_right: Vec3,

    /// Near/far plane distances (positive).
    pub near: f32,
    pub far: f32,

    /// Cached projection matrix.
    pub proj: Mat4,

    /// Last render target dimensions passed in by Bevy.
    /// These are not required for the core math, but are useful for debugging.
    pub full_width: f32,
    pub full_height: f32,
}

impl OffAxisProjection {
    pub fn new(
        lower_left: Vec3,
        lower_right: Vec3,
        upper_right: Vec3,
        near: f32,
        far: f32,
    ) -> Self {
        assert!(near > 0.0, "near must be > 0");
        assert!(far > near, "far must be > near");

        Self {
            lower_left,
            lower_right,
            upper_right,
            near,
            far,
            proj: Mat4::IDENTITY,
            full_width: 1.0,
            full_height: 1.0,
        }
    }

    /// Compute frustum corners in VIEW SPACE for arbitrary near/far distances.
    ///
    /// Ordering matches Bevy's convention used by built-in projections:
    /// near: bottom-right, top-right, top-left, bottom-left
    /// far:  bottom-right, top-right, top-left, bottom-left
    fn frustum_corners_for_depths(&self, z_near: f32, z_far: f32) -> [Vec3A; 8] {
        assert!(z_near > 0.0);
        assert!(z_far > z_near);

        let inv_p = self.proj.inverse();

        let (zn, zf) = (1.0, 0.0);
        // NDC corners:
        // near: 0..3 = LL, LR, UR, UL
        // far:  4..7 = LL, LR, UR, UL
        let ndc: [Vec3; 8] = [
            Vec3::new(-1.0, -1.0, zn),
            Vec3::new(1.0, -1.0, zn),
            Vec3::new(1.0, 1.0, zn),
            Vec3::new(-1.0, 1.0, zn),
            Vec3::new(-1.0, -1.0, zf),
            Vec3::new(1.0, -1.0, zf),
            Vec3::new(1.0, 1.0, zf),
            Vec3::new(-1.0, 1.0, zf),
        ];

        let mut world: [Vec3A; 8] = [Vec3A::ZERO; 8];

        for (i, p) in ndc.iter().enumerate() {
            // clip-space homogeneous point
            let c = p.extend(1.0);

            // clip -> view
            let mut v = inv_p * c;
            if v.w == 0.0 {
                warn!("Invalid projection: w == 0 after inverse(P) * clip");
            }
            v /= v.w;

            // view -> world
            //let mut w = world_from_eye * v;
            // if w.w != 0.0 {
            //     w /= w.w;
            // }

            world[i] = v.truncate().into();
        }

        world
    }

    pub fn update_proj(&mut self, head: Vec3, head_rot: Quat) -> Transform {
        let desc = ScreenDesc {
            lower_left: self.lower_left.into(),
            lower_right: self.lower_right.into(),
            upper_right: self.upper_right.into(),
        };

        let mut new_tf = Mat4::IDENTITY;

        compute_off_axis_projection(
            &desc,
            head.as_dvec3(),
            head_rot.as_dquat(),
            true,
            self.near as f64,
            self.far as f64,
            &mut new_tf,
            &mut self.proj,
        );

        Transform::from_matrix(new_tf)
    }
}

impl CameraProjection for OffAxisProjection {
    fn get_clip_from_view(&self) -> Mat4 {
        self.proj
    }

    fn get_clip_from_view_for_sub(&self, _sub_view: &SubCameraView) -> Mat4 {
        self.proj
    }

    fn update(&mut self, width: f32, height: f32) {
        self.full_width = width;
        self.full_height = height;
    }

    fn far(&self) -> f32 {
        self.far
    }

    fn get_frustum_corners(&self, z_near: f32, z_far: f32) -> [Vec3A; 8] {
        self.frustum_corners_for_depths(z_near, z_far)
    }
}
