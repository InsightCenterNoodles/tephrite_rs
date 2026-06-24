//! Fast serialization adapters for common Bevy components.
//!
//! Focuses on components frequently replicated between processes, keeping only
//! fields that affect rendering or spatial state and skipping fields that are
//! computed or overwritten by Bevy at runtime.
use bevy::{
    camera::visibility::RenderLayers,
    light::{NotShadowCaster, NotShadowReceiver, cascade::CascadeShadowConfig},
    prelude::*,
};

use crate::{common::Head, serialize::*};

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

// `Visibility` is treated as a raw POD value.
impl_fast_raw_item!(Visibility);

// =============================================================================

impl_fast_raw_item!(InheritedVisibility);

// =============================================================================

// Light components are treated as raw POD values for speed.
impl_fast_raw_item!(PointLight);
impl_fast_raw_item!(SpotLight);
impl_fast_raw_item!(DirectionalLight);

impl_fast_raw_item!(NotShadowCaster);
impl_fast_raw_item!(NotShadowReceiver);

impl_fast_serialize!(
    CascadeShadowConfig,
    keep: {
        bounds,
        overlap_proportion,
        minimum_distance
    },
    skip: {

    }
);

//

impl FastWrite for RenderLayers {
    #[inline(always)]
    unsafe fn write_fast(&self, w: &mut impl crate::serialize::fast_io::ByteSink) {
        // only support 1 layer for now
        let x = self.iter().next();
        unsafe { x.write_fast(w) }
    }
}
impl FastRead for RenderLayers {
    type Ret = RenderLayers;
    #[inline(always)]
    unsafe fn read_fast<'b, S: crate::serialize::fast_io::ByteSource<'b>>(r: &mut S) -> Self {
        let x = unsafe { Option::<usize>::read_fast(r) };
        Self::from_layers(x.as_slice())
    }
}

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
