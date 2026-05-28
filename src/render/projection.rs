use bevy::camera::CameraProjection;
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

    // Is the eye left or right, for stereo applications.
    pub is_left: bool,

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
        is_left: bool,
    ) -> Self {
        assert!(near > 0.0, "near must be > 0");
        assert!(far > near, "far must be > near");

        Self {
            lower_left,
            lower_right,
            upper_right,
            is_left,
            near,
            far,
            proj: Mat4::IDENTITY,
            full_width: 1.0,
            full_height: 1.0,
        }
    }

    pub fn set_clip_distances(&mut self, near: f32, far: f32) {
        if near <= 0.0 {
            warn!("Ignoring off-axis projection near distance {near}; near must be > 0");
            return;
        }

        if far <= near {
            warn!("Ignoring off-axis projection far distance {far}; far must be > near");
            return;
        }

        self.near = near;
        self.far = far;
    }

    /// Compute frustum corners
    fn frustum_corners_for_depths(&self, z_near: f32, z_far: f32) -> [Vec3A; 8] {
        let inv = self.get_clip_from_view().inverse();

        let ndc_corners = [
            Vec3::new(1.0, -1.0, 1.0),  // bottom right
            Vec3::new(1.0, 1.0, 1.0),   // top right
            Vec3::new(-1.0, 1.0, 1.0),  // top left
            Vec3::new(-1.0, -1.0, 1.0), // bottom left
        ];

        let mut out = [Vec3A::ZERO; 8];

        for (i, ndc) in ndc_corners.into_iter().enumerate() {
            let p = inv.project_point3(ndc); // view-space point on that frustum edge ray

            let near_p = p * (z_near / p.z);
            let far_p = p * (z_far / p.z);

            out[i] = near_p.into();
            out[i + 4] = far_p.into();
        }

        out
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
            self.is_left,
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

    fn get_clip_from_view_for_sub(&self, _sub_view: &bevy::camera::SubCameraView) -> Mat4 {
        panic!(
            "OffAxisProjection does not support Bevy SubCameraView. Configure each physical sub-view as a separate render window instead."
        );
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
