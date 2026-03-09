//! Serialization for Bevy Asset identifiers and handles.
//!
//! `AssetId<A>` is serialized by raw bytes as POD. `Handle<A>` is serialized as
//! its `AssetId`, and is reconstructed as a `Weak` handle on read to avoid
//! implicit asset loads in the receiving process.
use std::fmt::Debug;

use bevy::{platform::collections::HashMap, prelude::*};

use crate::serialize::*;

impl<A: Asset> FastWrite for AssetId<A> {
    /// Serialize an `AssetId` by its raw bytes.
    unsafe fn write_fast(&self, w: &mut impl ByteSink) {
        //debug!("Write asset ID {}", self);
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
        // being explicit here to make sure we dont have unintended conversion
        let id: AssetId<A> = self.id();
        debug!("Write asset handle {id}");
        unsafe { id.write_fast(w) };
    }
}

pub trait RemappableAsset {
    fn with_remapper<F: FnOnce(&HashMap<AssetId<Self>, Handle<Self>>)>(func: F);
    fn with_remapper_mut<F: FnOnce(&mut HashMap<AssetId<Self>, Handle<Self>>)>(func: F);

    fn reserve_mapping(id: AssetId<Self>, assets: &mut Assets<Self>)
    where
        Self: bevy::prelude::Asset,
        Self: Sized + Debug,
    {
        debug!("Reserve asset {id}");
        let handle = assets.reserve_handle();

        Self::with_remapper_mut(|map| {
            map.insert(id, handle.clone());
        });

        // This never fires. we never call this function, so we can probably remove it.
        panic!("NOPE");
    }

    fn set_mapping(id: AssetId<Self>, asset: Self, assets: &mut Assets<Self>)
    where
        Self: bevy::prelude::Asset,
        Self: Sized + Debug,
    {
        //debug!("install asset: {} {:?}", id, asset);

        Self::with_remapper_mut(move |map| {
            if map.contains_key(&id) {
                // mapping exists, update asset
                debug!("Update asset {id}");
                assets.insert(id, asset).expect("insert should not fail");
            } else {
                // this is new
                let handle = assets.add(asset);
                debug!("New asset {id} MAPPING TO {}", handle.id());
                map.insert(id, handle.clone());
            }
        });
    }

    fn remap_to_local(id: AssetId<Self>) -> Option<Handle<Self>>
    where
        Self: bevy::prelude::Asset,
        Self: Sized,
    {
        let mut ret = None;

        Self::with_remapper(|map| {
            ret = map.get(&id).cloned();
        });

        debug!(
            "REMAPPING INCOMING {id} TO {:?}",
            ret.as_ref().map(|x| x.id())
        );

        ret
    }

    fn clear_mapping(id: AssetId<Self>, assets: &mut Assets<Self>)
    where
        Self: bevy::prelude::Asset,
        Self: Sized,
    {
        assets.remove(id);
        Self::with_remapper_mut(|map| {
            map.remove(&id);
        });
    }
}

impl<A: Asset + RemappableAsset> FastRead for Handle<A> {
    type Ret = Self;
    /// Decode a `Handle` from an `AssetId`
    unsafe fn read_fast<'a, S: ByteSource<'a>>(r: &mut S) -> Self::Ret {
        let id = unsafe { AssetId::<A>::read_fast(r) };
        debug!("Reading handle {}", id);
        match A::remap_to_local(id) {
            Some(x) => x,
            None => {
                panic!("Using made up {} asset {id}!", std::any::type_name::<A>());
            }
        }
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

        let h = a.clone();

        test_serialization(h, |x, y| x == y);
    }
}
