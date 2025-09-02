use bevy::prelude::*;

use super::{
    common::{byte_deserialize, byte_serialize},
    deserialize, TDeserialize, TSerialize,
};

impl<A: Asset> TSerialize for AssetId<A> {
    fn serialize(&self, w: &mut impl std::io::Write) {
        // this is, as they say, beyond unsafe. but it looks to be non-alloc all around
        unsafe { byte_serialize(self, w) };
    }
}
impl<A: Asset> TDeserialize for AssetId<A> {
    fn deserialize(r: &mut impl std::io::Read) -> Self {
        unsafe { byte_deserialize(r) }
    }
}

// =============================================================================

impl<A: Asset> TSerialize for Handle<A> {
    fn serialize(&self, w: &mut impl std::io::Write) {
        self.id().serialize(w);
    }
}
impl<A: Asset> TDeserialize for Handle<A> {
    fn deserialize(r: &mut impl std::io::Read) -> Self {
        Self::Weak(deserialize(r))
    }
}

// =============================================================================

impl TSerialize for Mesh3d {
    fn serialize(&self, w: &mut impl std::io::Write) {
        self.0.serialize(w);
    }
}

impl TDeserialize for Mesh3d {
    fn deserialize(r: &mut impl std::io::Read) -> Self {
        Self(deserialize(r))
    }
}

// =============================================================================

impl TSerialize for MeshMaterial3d<StandardMaterial> {
    fn serialize(&self, w: &mut impl std::io::Write) {
        self.0.serialize(w);
    }
}

impl TDeserialize for MeshMaterial3d<StandardMaterial> {
    fn deserialize(r: &mut impl std::io::Read) -> Self {
        Self(deserialize(r))
    }
}

// =============================================================================

#[cfg(test)]
mod tests {
    use bevy::asset::AssetIndex;

    use crate::transcript::test_serialization;

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
