use bevy::math::Affine3A;
use bevy::pbr::CubemapVisibleEntities;
use bevy::pbr::VisibleMeshEntities;
use bevy::prelude::*;
use bevy::render::primitives::CubemapFrusta;

use super::common::byte_deserialize;
use super::common::byte_serialize;
use super::deserialize;
use super::TDeserialize;
use super::TSerialize;

// =============================================================================

impl TSerialize for Transform {
    fn serialize(&self, w: &mut impl std::io::Write) {
        self.translation.serialize(w);
        self.rotation.serialize(w);
        self.scale.serialize(w);
    }
}
impl TDeserialize for Transform {
    fn deserialize(r: &mut impl std::io::Read) -> Self {
        Self {
            translation: deserialize(r),
            rotation: deserialize(r),
            scale: deserialize(r),
        }
    }
}

// =============================================================================

impl TSerialize for GlobalTransform {
    fn serialize(&self, w: &mut impl std::io::Write) {
        self.affine().serialize(w);
    }
}
impl TDeserialize for GlobalTransform {
    fn deserialize(r: &mut impl std::io::Read) -> Self {
        // we read BUT DO NOT USE. Bevy will overwrite
        let _: Affine3A = deserialize(r);
        GlobalTransform::IDENTITY
    }
}

// =============================================================================

impl TSerialize for Visibility {
    fn serialize(&self, w: &mut impl std::io::Write) {
        match self {
            Visibility::Inherited => 0i8.serialize(w),
            Visibility::Hidden => 1i8.serialize(w),
            Visibility::Visible => 2i8.serialize(w),
        }
    }
}
impl TDeserialize for Visibility {
    fn deserialize(r: &mut impl std::io::Read) -> Self {
        let index = i8::deserialize(r);
        match index {
            0 => Visibility::Inherited,
            1 => Visibility::Hidden,
            2 => Visibility::Visible,
            _ => unreachable!(),
        }
    }
}

// =============================================================================

impl TSerialize for InheritedVisibility {
    fn serialize(&self, w: &mut impl std::io::Write) {
        self.get().serialize(w);
    }
}
impl TDeserialize for InheritedVisibility {
    fn deserialize(r: &mut impl std::io::Read) -> Self {
        let _ = bool::deserialize(r);
        // discard! Bevy will overwrite
        InheritedVisibility::VISIBLE
    }
}

// =============================================================================

impl TSerialize for ViewVisibility {
    fn serialize(&self, w: &mut impl std::io::Write) {
        self.get().serialize(w);
    }
}
impl TDeserialize for ViewVisibility {
    fn deserialize(r: &mut impl std::io::Read) -> Self {
        let _ = bool::deserialize(r);
        // discard! Bevy will overwrite
        ViewVisibility::default()
    }
}

// =============================================================================

impl TSerialize for PointLight {
    fn serialize(&self, w: &mut impl std::io::Write) {
        unsafe { byte_serialize(self, w) };
    }
}
impl TDeserialize for PointLight {
    fn deserialize(r: &mut impl std::io::Read) -> Self {
        unsafe { byte_deserialize(r) }
    }
}

impl TSerialize for SpotLight {
    fn serialize(&self, w: &mut impl std::io::Write) {
        unsafe { byte_serialize(self, w) };
    }
}
impl TDeserialize for SpotLight {
    fn deserialize(r: &mut impl std::io::Read) -> Self {
        unsafe { byte_deserialize(r) }
    }
}

impl TSerialize for DirectionalLight {
    fn serialize(&self, w: &mut impl std::io::Write) {
        unsafe { byte_serialize(self, w) };
    }
}
impl TDeserialize for DirectionalLight {
    fn deserialize(r: &mut impl std::io::Read) -> Self {
        unsafe { byte_deserialize(r) }
    }
}

// =============================================================================

impl TSerialize for CubemapFrusta {
    fn serialize(&self, w: &mut impl std::io::Write) {
        // appears to be just a wad of bytes
        unsafe { byte_serialize(self, w) };
    }
}
impl TDeserialize for CubemapFrusta {
    fn deserialize(r: &mut impl std::io::Read) -> Self {
        unsafe { byte_deserialize(r) }
    }
}

// =============================================================================

impl TSerialize for CubemapVisibleEntities {
    fn serialize(&self, w: &mut impl std::io::Write) {
        for i in 0..6 {
            self.get(i).serialize(w);
        }
    }
}
impl TDeserialize for CubemapVisibleEntities {
    fn deserialize(r: &mut impl std::io::Read) -> Self {
        let mut ret = Self::default();
        for i in 0..6 {
            *(ret.get_mut(i)) = deserialize(r);
        }
        ret
    }
}

// =============================================================================

// automatically updated. can probably remove
impl TSerialize for VisibleMeshEntities {
    fn serialize(&self, _: &mut impl std::io::Write) {
        //self.entities.serialize(w);
    }
}

impl TDeserialize for VisibleMeshEntities {
    fn deserialize(_: &mut impl std::io::Read) -> Self {
        // Self {
        //     entities: deserialize(r),
        // }
        Self::default()
    }
}
