use bevy::prelude::*;

use crate::serialize::*;

impl<A: Asset> FastWrite for AssetId<A> {
    unsafe fn write_fast(&self, w: &mut impl ByteSink) {
        // this is, as they say, beyond unsafe. but it looks to be non-alloc all around
        unsafe { byte_serialize(self, w) };
    }
}
impl<A: Asset> FastRead for AssetId<A> {
    type Ret = Self;
    unsafe fn read_fast<'a, S: ByteSource<'a>>(r: &mut S)  -> Self::Ret {
        unsafe { byte_deserialize(r) }
    }
}

// =============================================================================

impl<A: Asset> FastWrite for Handle<A> {
    unsafe fn write_fast(&self, w: &mut impl ByteSink) {
        unsafe { self.id().write_fast(w) };
    }
}
impl<A: Asset> FastRead for Handle<A> {
    type Ret = Self;
    unsafe fn read_fast<'a, S: ByteSource<'a>>(r: &mut S)  -> Self::Ret {
        Self::Weak(unsafe { AssetId::<A>::read_fast(r) })
    }
}

// =============================================================================

impl_fast_newtype!(Mesh3d);

// =============================================================================

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
