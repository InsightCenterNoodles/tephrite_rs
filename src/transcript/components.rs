use bevy::prelude::*;

use crate::serialize::*;

// =============================================================================

impl_fast_serialize!(
    Transform,
    keep: {
        translation, rotation, scale
    },
    skip: {

    }
);

// =============================================================================

// UNNEEDED??

// impl FastWrite for GlobalTransform {
//     unsafe fn write_fast(&self, w: &mut crate::serialize::fast_io::ByteWriter) {
//         self.affine().serialize(w);
//     }
// }
// impl FastRead for GlobalTransform {
//     type Ret = Self;
//     unsafe fn read_fast(r: &mut crate::serialize::fast_io::ByteReader) -> Self {
//         // we read BUT DO NOT USE. Bevy will overwrite
//         let _: Affine3A = deserialize(r);
//         GlobalTransform::IDENTITY
//     }
// }

// =============================================================================

impl_fast_raw_item!(Visibility);

// =============================================================================

// impl FastWrite for InheritedVisibility {
//     unsafe fn write_fast(&self, w: &mut crate::serialize::fast_io::ByteWriter) {
//         self.get().serialize(w);
//     }
// }
// impl FastRead for InheritedVisibility {
//     unsafe fn read_fast(r: &mut crate::serialize::fast_io::ByteReader) -> Self {
//         let _ = bool::deserialize(r);
//         // discard! Bevy will overwrite
//         InheritedVisibility::VISIBLE
//     }
// }

// =============================================================================

// impl FastWrite for ViewVisibility {
//     unsafe fn write_fast(&self, w: &mut crate::serialize::fast_io::ByteWriter) {
//         self.get().serialize(w);
//     }
// }
// impl FastRead for ViewVisibility {
//     unsafe fn read_fast(r: &mut crate::serialize::fast_io::ByteReader) -> Self {
//         let _ = bool::deserialize(r);
//         // discard! Bevy will overwrite
//         ViewVisibility::default()
//     }
// }

// =============================================================================

impl_fast_raw_item!(PointLight);
impl_fast_raw_item!(SpotLight);
impl_fast_raw_item!(DirectionalLight);

// impl FastWrite for PointLight {
//     unsafe fn write_fast(&self, w: &mut crate::serialize::fast_io::ByteWriter) {
//         unsafe { byte_serialize(self, w) };
//     }
// }
// impl FastRead for PointLight {
//     unsafe fn read_fast(r: &mut crate::serialize::fast_io::ByteReader) -> Self {
//         unsafe { byte_deserialize(r) }
//     }
// }

// impl FastWrite for SpotLight {
//     unsafe fn write_fast(&self, w: &mut crate::serialize::fast_io::ByteWriter) {
//         unsafe { byte_serialize(self, w) };
//     }
// }
// impl FastRead for SpotLight {
//     unsafe fn read_fast(r: &mut crate::serialize::fast_io::ByteReader) -> Self {
//         unsafe { byte_deserialize(r) }
//     }
// }

// impl FastWrite for DirectionalLight {
//     unsafe fn write_fast(&self, w: &mut crate::serialize::fast_io::ByteWriter) {
//         unsafe { byte_serialize(self, w) };
//     }
// }
// impl FastRead for DirectionalLight {
//     unsafe fn read_fast(r: &mut crate::serialize::fast_io::ByteReader) -> Self {
//         unsafe { byte_deserialize(r) }
//     }
// }

// =============================================================================

//impl_fast_raw_item!(CubemapFrusta);

// impl FastWrite for CubemapFrusta {
//     unsafe fn write_fast(&self, w: &mut crate::serialize::fast_io::ByteWriter) {
//         // appears to be just a wad of bytes
//         unsafe { byte_serialize(self, w) };
//     }
// }
// impl FastRead for CubemapFrusta {
//     unsafe fn read_fast(r: &mut crate::serialize::fast_io::ByteReader) -> Self {
//         unsafe { byte_deserialize(r) }
//     }
// }

// =============================================================================

// impl FastWrite for CubemapVisibleEntities {
//     unsafe fn write_fast(&self, w: &mut crate::serialize::fast_io::ByteWriter) {
//         for i in 0..6 {
//             unsafe { self.get(i).write_fast(w) };
//         }
//     }
// }
// impl FastRead for CubemapVisibleEntities {
//     type Ret = Self;
//     unsafe fn read_fast(r: &mut crate::serialize::fast_io::ByteReader) -> Self {
//         let mut ret = Self::default();
//         for i in 0..6 {
//             *(ret.get_mut(i)) = unsafe { FastRead::read_fast(r) }
//         }
//         ret
//     }
// }

// =============================================================================

// // automatically updated. can probably remove
// impl FastWrite for VisibleMeshEntities {
//     fn serialize(&self, _: &mut impl std::io::Write) {
//         //self.entities.serialize(w);
//     }
// }

// impl FastRead for VisibleMeshEntities {
//     fn deserialize(_: &mut impl std::io::Read) -> Self {
//         // Self {
//         //     entities: deserialize(r),
//         // }
//         Self::default()
//     }
// }
