use bevy::prelude::*;

/// Map a point in the space of one object to the space of another
#[inline]
pub fn map_point(p_in_local_a: Vec3, a: &GlobalTransform, b: &GlobalTransform) -> Vec3 {
    let global = a.transform_point(p_in_local_a);

    b.affine().inverse().transform_point3(global)
}
