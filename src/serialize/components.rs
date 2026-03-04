//! Fast serialization adapters for common Bevy components.
//!
//! Focuses on components frequently replicated between processes, keeping only
//! fields that affect rendering or spatial state and skipping fields that are
//! computed or overwritten by Bevy at runtime.
use bevy::prelude::*;

use crate::{
    common::{Head, SimulatorCamera3d},
    serialize::*,
};

// =============================================================================

// Serialize only the `Transform` fields that matter for rendering and logic.
impl_fast_serialize!(
    Transform,
    keep: {
        translation, rotation, scale
    },
    skip: {

    }
);

// =============================================================================

impl FastWrite for Head {
    unsafe fn write_fast(&self, _w: &mut impl ByteSink) {
        // nothing to do.
    }
}

impl FastRead for Head {
    type Ret = Self;

    unsafe fn read_fast<'a, S: ByteSource<'a>>(_r: &mut S) -> Self::Ret {
        Self
    }
}

// =============================================================================

impl_fast_raw_item!(SimulatorCamera3d);

// =============================================================================

// `Visibility` is treated as a raw POD value.
impl_fast_raw_item!(Visibility);

// =============================================================================

impl_fast_raw_item!(InheritedVisibility);

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

// Light components are treated as raw POD values for speed.
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

// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serialize::fast_io::{ByteReader, ByteWriter};
    use crate::serialize::fast_ser::{FastRead, FastWrite};

    fn roundtrip<T: FastWrite + FastRead<Ret = T>>(x: &T) -> T {
        let mut buf = [0u8; 256];
        let mut w = ByteWriter::new(&mut buf);
        unsafe { x.write_fast(&mut w) };
        let mut r = ByteReader::new(&buf);
        unsafe { T::read_fast(&mut r) }
    }

    #[test]
    fn transform_roundtrip_kept_fields() {
        let t = Transform {
            translation: Vec3::new(1.0, -2.0, 3.5),
            rotation: Quat::from_xyzw(0.1, 0.2, 0.3, 0.9),
            scale: Vec3::new(2.0, 0.5, -1.5),
        };

        let out = roundtrip(&t);
        assert_eq!(out.translation, t.translation);
        assert_eq!(out.rotation, t.rotation);
        assert_eq!(out.scale, t.scale);
    }
}
