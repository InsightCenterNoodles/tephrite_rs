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
#[derive(Debug, Component, Clone, Copy, PartialEq, Eq)]
pub enum Billboard {
    /// Full pitch/yaw rotation with the selected local axis facing the target.
    FullGimbal(BillboardAxis),
    /// Y-axis-only rotation with the selected local axis facing the target.
    YRotation(BillboardAxis),
}

impl Default for Billboard {
    fn default() -> Self {
        Self::FullGimbal(BillboardAxis::Z)
    }
}

impl Billboard {
    const fn facing_axis(self) -> BillboardAxis {
        match self {
            Self::FullGimbal(axis) | Self::YRotation(axis) => axis,
        }
    }

    const fn only_rotate_y(self) -> bool {
        matches!(self, Self::YRotation(_))
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

fn update_billboards(
    target_heads: Query<&GlobalTransform, With<Head>>,
    target_cameras: Query<&GlobalTransform, With<Camera3d>>,
    parent_transforms: Query<&GlobalTransform>,
    mut billboards: Query<(
        &Billboard,
        &GlobalTransform,
        &mut Transform,
        Option<&ChildOf>,
    )>,
) {
    let target_translation = target_heads
        .iter()
        .next()
        .or_else(|| target_cameras.iter().next())
        .map(GlobalTransform::translation);

    let Some(target_translation) = target_translation else {
        return;
    };

    for (billboard, current_global, mut current_local, child_of) in &mut billboards {
        let parent_global = child_of.and_then(|child_of| parent_transforms.get(child_of.0).ok());
        let mut direction = direction_to_target(
            target_translation,
            current_global,
            current_local.translation,
            parent_global,
        );

        if billboard.only_rotate_y() {
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

fn direction_to_target(
    target_translation: Vec3,
    current_global: &GlobalTransform,
    current_local_translation: Vec3,
    parent_global: Option<&GlobalTransform>,
) -> Vec3 {
    parent_global
        .map(|parent_global| {
            parent_global
                .affine()
                .inverse()
                .transform_point3(target_translation)
                - current_local_translation
        })
        .unwrap_or_else(|| target_translation - current_global.translation())
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
        assert_eq!(Billboard::default().facing_axis(), BillboardAxis::Z);
        assert_eq!(
            Billboard::FullGimbal(BillboardAxis::Z).facing_axis(),
            BillboardAxis::Z
        );
        assert_eq!(
            Billboard::YRotation(BillboardAxis::Z).facing_axis(),
            BillboardAxis::Z
        );
    }

    #[test]
    fn billboard_under_rotated_parent_faces_world_space_target() {
        let parent_translation = Vec3::new(2.0, 0.5, -1.0);
        let parent_rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let parent_global = GlobalTransform::from(
            Transform::from_translation(parent_translation).with_rotation(parent_rotation),
        );
        let billboard_local_translation = Vec3::new(1.0, 0.25, -0.5);
        let billboard_world_translation =
            parent_translation + parent_rotation * billboard_local_translation;
        let target_translation = Vec3::new(-3.0, 2.0, 4.0);

        let direction = direction_to_target(
            target_translation,
            &GlobalTransform::from_translation(billboard_world_translation),
            billboard_local_translation,
            Some(&parent_global),
        )
        .normalize();
        let local_rotation = rotation_looking_axis(direction, BillboardAxis::Z, Vec3::Y);
        let world_facing = parent_rotation * (local_rotation * Vec3::Z);
        let expected_world_facing = (target_translation - billboard_world_translation).normalize();

        assert_vec3_near(world_facing, expected_world_facing);
    }
}
