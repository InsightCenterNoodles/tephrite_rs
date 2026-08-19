//! Serialization for Bevy Asset identifiers and handles.
//!
//! `AssetId<A>` is serialized by raw bytes as POD. `Handle<A>` is serialized as
//! its `AssetId`, and is reconstructed as a `Weak` handle on read to avoid
//! implicit asset loads in the receiving process.
use std::{fmt::Debug, marker::PhantomData};

use bevy::{platform::collections::HashMap, prelude::*};

use crate::material::InstanceMeshMaterial3d;
use crate::{prelude::PointsMaterial, serialize::*};

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

    fn set_mapping(id: AssetId<Self>, asset: Self, assets: &mut Assets<Self>)
    where
        Self: bevy::prelude::Asset,
        Self: Sized + Debug,
    {
        Self::with_remapper_mut(move |map| {
            if let Some(local) = map.get(&id).cloned() {
                // Mapping exists: this asset is represented by a client-local ID.
                // Always write to the local ID so deferred handles stay valid.
                debug!("Update asset {id} (local {})", local.id());
                assets
                    .insert(local.id(), asset)
                    .expect("insert should not fail");
            } else {
                // New mapping: create a fresh local asset and remember the remote->local mapping.
                let handle = assets.add(asset);
                debug!("New asset {id} mapping to local {}", handle.id());
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

    fn remap_to_local_or_reserve(id: AssetId<Self>) -> Handle<Self>
    where
        Self: bevy::prelude::Asset,
        Self: Sized,
    {
        if let Some(handle) = Self::remap_to_local(id) {
            return handle;
        }

        let local = Handle::Uuid(bevy::asset::uuid::Uuid::new_v4(), PhantomData);

        warn!(
            "Missing asset mapping for {} id {id}; reserving client-local placeholder {}",
            std::any::type_name::<Self>(),
            local.id()
        );

        Self::with_remapper_mut(|map| {
            map.insert(id, local.clone());
        });

        local
    }

    fn clear_mapping(id: AssetId<Self>, assets: &mut Assets<Self>)
    where
        Self: bevy::prelude::Asset,
        Self: Sized,
    {
        Self::with_remapper_mut(|map| {
            if let Some(local) = map.remove(&id) {
                assets.remove(local.id());
            }
        });
    }
}

impl<A: Asset + RemappableAsset> FastRead for Handle<A> {
    type Ret = Self;
    /// Decode a `Handle` from an `AssetId`
    unsafe fn read_fast<'a, S: ByteSource<'a>>(r: &mut S) -> Self::Ret {
        let id = unsafe { AssetId::<A>::read_fast(r) };
        debug!("Reading handle {}", id);
        A::remap_to_local_or_reserve(id)
    }
}

// =============================================================================

// Newtype passthrough for `Mesh3d` (wraps `Handle<Mesh>`).
impl_fast_newtype!(Mesh3d);

// =============================================================================

// Newtype passthrough for `MeshMaterial3d<StandardMaterial>` (wraps `Handle<StandardMaterial>`).
impl_fast_newtype!(MeshMaterial3d<StandardMaterial>);

impl_fast_newtype!(MeshMaterial3d<PointsMaterial>);

impl_fast_newtype!(InstanceMeshMaterial3d);

// =============================================================================

#[cfg(test)]
mod tests {
    use bevy::asset::AssetIndex;
    use bevy::asset::RenderAssetUsages;
    use bevy::mesh::PrimitiveTopology;

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

    #[test]
    fn unknown_remote_id_gets_stable_local_placeholder() {
        let remote_id: AssetId<Mesh> = AssetId::Index {
            index: AssetIndex::from_bits({
                let generation = 9;
                let index = 42;
                ((generation as u64) << 32) | index as u64
            }),
            marker: Default::default(),
        };

        let local1 = Mesh::remap_to_local_or_reserve(remote_id);
        let local2 = Mesh::remap_to_local_or_reserve(remote_id);

        assert_eq!(local1.id(), local2.id());
        assert_ne!(local1.id(), remote_id);
    }

    #[test]
    fn placeholder_mapping_is_fulfilled_when_real_asset_arrives() {
        let remote_id: AssetId<Mesh> = AssetId::Index {
            index: AssetIndex::from_bits({
                let generation = 11;
                let index = 77;
                ((generation as u64) << 32) | index as u64
            }),
            marker: Default::default(),
        };

        let local = Mesh::remap_to_local_or_reserve(remote_id);
        assert!(matches!(local.id(), AssetId::Uuid { .. }));

        let mut assets = Assets::<Mesh>::default();
        let mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );

        Mesh::set_mapping(remote_id, mesh, &mut assets);

        assert!(assets.get(local.id()).is_some());
        assert_eq!(
            Mesh::remap_to_local(remote_id).map(|h| h.id()),
            Some(local.id())
        );
    }
}
