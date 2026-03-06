use bevy::prelude::*;

use crate::common::Head;

/// Adds billboarding behavior for entities that carry [`Billboard`].
pub struct BillboardPlugin;

impl Plugin for BillboardPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, update_billboards);
    }
}

/// Rotate this entity so its local +Z axis points at a tracked target.
///
/// Target selection is:
/// 1. First entity with [`Head`]
/// 2. First entity with [`Camera3d`]
///
/// Be sure to add [`BillboardPlugin`] to your app to use this component.
#[derive(Debug, Default, Component, Clone, Copy, PartialEq, Eq)]
pub enum Billboard {
    /// Full pitch/yaw rotation to face the target.
    #[default]
    FullGimbal,
    /// Yaw-only: keep local Y upright, rotate only in the XZ plane.
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

        if matches!(*billboard, Billboard::XzAxisRestricted) {
            direction.y = 0.0;
        }

        let len_sq = direction.length_squared();
        if len_sq <= f32::EPSILON {
            continue;
        }

        direction /= len_sq.sqrt();
        current_local.rotation = rotation_looking_z(direction, Vec3::Y);
    }
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
