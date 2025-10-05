//! Serialization for Bevy Asset identifiers and handles.
//!
//! `AssetId<A>` is serialized by raw bytes as POD. `Handle<A>` is serialized as
//! its `AssetId`, and is reconstructed as a `Weak` handle on read to avoid
//! implicit asset loads in the receiving process.
use bevy::prelude::*;

use crate::serialize::*;

impl<A: Asset> FastWrite for AssetId<A> {
    /// Serialize an `AssetId` by its raw bytes.
    unsafe fn write_fast(&self, w: &mut impl ByteSink) {
        // this is, as they say, beyond unsafe. but it looks to be non-alloc all around
        unsafe { byte_serialize(self, w) };
    }
}
impl<A: Asset> FastRead for AssetId<A> {
    type Ret = Self;
    /// Deserialize an `AssetId` from raw bytes.
    unsafe fn read_fast<'a, S: ByteSource<'a>>(r: &mut S) -> Self::Ret {
        unsafe { byte_deserialize(r) }
    }
}

// =============================================================================

impl<A: Asset> FastWrite for Handle<A> {
    /// Encode a `Handle` by its `AssetId`. Recreated as `Weak`.
    unsafe fn write_fast(&self, w: &mut impl ByteSink) {
        unsafe { self.id().write_fast(w) };
    }
}
impl<A: Asset> FastRead for Handle<A> {
    type Ret = Self;
    /// Decode a `Handle` from an `AssetId`, returning a `Weak` handle.
    unsafe fn read_fast<'a, S: ByteSource<'a>>(r: &mut S) -> Self::Ret {
        Self::Weak(unsafe { AssetId::<A>::read_fast(r) })
    }
}

// =============================================================================

// Newtype passthrough for `Mesh3d` (wraps `Handle<Mesh>`).
impl_fast_newtype!(Mesh3d);

// =============================================================================

// Newtype passthrough for `MeshMaterial3d<StandardMaterial>` (wraps `Handle<StandardMaterial>`).
impl_fast_newtype!(MeshMaterial3d<StandardMaterial>);

// =============================================================================

#[cfg(test)]
mod tests {
    use bevy::asset::AssetIndex;

    use super::*;

    #[test]
    fn asset_serialization() {
        let a: AssetId<Mesh> = AssetId::Index {
            index: AssetIndex::from_bits({
                let generation = 34;
                let index = 1124;
                ((generation as u64) << 32) | index as u64
            }),
            marker: Default::default(),
        };

        test_serialization(a, |x, y| x == y);

        let h = Handle::<Mesh>::Weak(a);

        test_serialization(h, |x, y| x == y);
    }
}
