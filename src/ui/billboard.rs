use bevy::prelude::*;

use crate::common::Head;

/// Adds billboarding behavior for entities that carry [`Billboard`].
pub struct BillboardPlugin;

impl Plugin for BillboardPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, update_billboards);
    }
}

/// Rotate this entity so one of its local axes points at a tracked target.
///
/// Target selection is:
/// 1. First entity with [`Head`]
/// 2. First entity with [`Camera3d`]
///
/// Be sure to add [`BillboardPlugin`] to your app to use this component.
#[derive(Debug, Default, Component, Clone, Copy, PartialEq, Eq)]
pub enum Billboard {
    /// Full pitch/yaw rotation with local +Z facing the target.
    #[default]
    FullGimbal,
    /// Yaw-only with local +Z facing the target: keep local Y upright, rotate only in the XZ plane.
    XzAxisRestricted,
    /// Custom billboarding behavior.
    Custom {
        /// How much freedom the billboard rotation has.
        mode: BillboardMode,
        /// Local axis that should point toward the target.
        facing_axis: BillboardAxis,
    },
}

impl Billboard {
    /// Full pitch/yaw rotation with a selected local axis facing the target.
    pub const fn full_gimbal_with_axis(facing_axis: BillboardAxis) -> Self {
        Self::Custom {
            mode: BillboardMode::FullGimbal,
            facing_axis,
        }
    }

    /// Yaw-only rotation with a selected local axis facing the target.
    pub const fn xz_axis_restricted_with_axis(facing_axis: BillboardAxis) -> Self {
        Self::Custom {
            mode: BillboardMode::XzAxisRestricted,
            facing_axis,
        }
    }

    const fn mode(self) -> BillboardMode {
        match self {
            Self::FullGimbal => BillboardMode::FullGimbal,
            Self::XzAxisRestricted => BillboardMode::XzAxisRestricted,
            Self::Custom { mode, .. } => mode,
        }
    }

    const fn facing_axis(self) -> BillboardAxis {
        match self {
            Self::FullGimbal | Self::XzAxisRestricted => BillboardAxis::Z,
            Self::Custom { facing_axis, .. } => facing_axis,
        }
    }
}

/// Local axis that should face the billboard target.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum BillboardAxis {
    X,
    Y,
    #[default]
    Z,
    NegativeX,
    NegativeY,
    NegativeZ,
}

impl BillboardAxis {
    const fn vector(self) -> Vec3 {
        match self {
            Self::X => Vec3::X,
            Self::Y => Vec3::Y,
            Self::Z => Vec3::Z,
            Self::NegativeX => Vec3::NEG_X,
            Self::NegativeY => Vec3::NEG_Y,
            Self::NegativeZ => Vec3::NEG_Z,
        }
    }
}

/// Rotation freedom for a billboard.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum BillboardMode {
    /// Full pitch/yaw rotation to face the target.
    #[default]
    FullGimbal,
    /// Yaw-only: keep world Y upright, rotate only in the XZ plane.
    XzAxisRestricted,
}

fn update_billboards(
    target_heads: Query<&GlobalTransform, With<Head>>,
    target_cameras: Query<&GlobalTransform, With<Camera3d>>,
    mut billboards: Query<(&Billboard, &GlobalTransform, &mut Transform)>,
) {
    let target_translation = target_heads
        .iter()
        .next()
        .or_else(|| target_cameras.iter().next())
        .map(GlobalTransform::translation);

    let Some(target_translation) = target_translation else {
        return;
    };

    for (billboard, current_global, mut current_local) in &mut billboards {
        let mut direction = target_translation - current_global.translation();

        if matches!(billboard.mode(), BillboardMode::XzAxisRestricted) {
            direction.y = 0.0;
        }

        let len_sq = direction.length_squared();
        if len_sq <= f32::EPSILON {
            continue;
        }

        direction /= len_sq.sqrt();
        current_local.rotation = rotation_looking_axis(direction, billboard.facing_axis(), Vec3::Y);
    }
}

fn rotation_looking_axis(direction: Vec3, facing_axis: BillboardAxis, fallback_up: Vec3) -> Quat {
    let align_selected_axis_to_z = Quat::from_rotation_arc(facing_axis.vector(), Vec3::Z);
    rotation_looking_z(direction, fallback_up) * align_selected_axis_to_z
}

fn rotation_looking_z(direction: Vec3, fallback_up: Vec3) -> Quat {
    let up = if direction.cross(fallback_up).length_squared() <= f32::EPSILON {
        Vec3::X
    } else {
        fallback_up
    };

    let right = up.cross(direction).normalize();
    let corrected_up = direction.cross(right);
    let basis = Mat3::from_cols(right, corrected_up, direction);
    Quat::from_mat3(&basis)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_vec3_near(actual: Vec3, expected: Vec3) {
        assert!(
            actual.distance(expected) < 1e-5,
            "expected {expected:?}, got {actual:?}"
        );
    }

    #[test]
    fn billboard_axis_vectors_match_expected_local_axes() {
        assert_eq!(BillboardAxis::X.vector(), Vec3::X);
        assert_eq!(BillboardAxis::Y.vector(), Vec3::Y);
        assert_eq!(BillboardAxis::Z.vector(), Vec3::Z);
        assert_eq!(BillboardAxis::NegativeX.vector(), Vec3::NEG_X);
        assert_eq!(BillboardAxis::NegativeY.vector(), Vec3::NEG_Y);
        assert_eq!(BillboardAxis::NegativeZ.vector(), Vec3::NEG_Z);
    }

    #[test]
    fn rotation_looking_axis_points_selected_axis_at_direction() {
        let direction = Vec3::new(0.4, 0.5, -0.75).normalize();

        for axis in [
            BillboardAxis::X,
            BillboardAxis::Y,
            BillboardAxis::Z,
            BillboardAxis::NegativeX,
            BillboardAxis::NegativeY,
            BillboardAxis::NegativeZ,
        ] {
            let rotation = rotation_looking_axis(direction, axis, Vec3::Y);
            assert_vec3_near(rotation.mul_vec3(axis.vector()), direction);
        }
    }

    #[test]
    fn default_variants_use_positive_z_axis() {
        assert_eq!(Billboard::FullGimbal.facing_axis(), BillboardAxis::Z);
        assert_eq!(Billboard::XzAxisRestricted.facing_axis(), BillboardAxis::Z);
    }
}
