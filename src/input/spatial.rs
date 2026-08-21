//! Coordinate-space helpers used by interaction and hover tests.

use bevy::prelude::*;

/// Map a point from one entity's local space into another entity's local space.
#[inline]
pub fn map_point(p_in_local_a: Vec3, a: &GlobalTransform, b: &GlobalTransform) -> Vec3 {
    let global = a.transform_point(p_in_local_a);

    b.affine().inverse().transform_point3(global)
}
