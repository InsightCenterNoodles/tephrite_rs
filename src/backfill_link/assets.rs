use bevy::image::ImageSampler;
use bevy::prelude::*;

use crate::backfill;

/// Cannot send
type AssetMap<A, H> = std::collections::HashMap<AssetId<A>, H>;

pub(crate) type MeshMap = AssetMap<Mesh, backfill::FMeshHandle>;
pub(crate) type MaterialMap = AssetMap<StandardMaterial, backfill::FMaterialHandle>;
pub(crate) type TextureMap = AssetMap<Image, (backfill::FTextureHandle, ImageSampler)>;

// Non send.
#[derive(Default)]
pub(crate) struct AssetCache {
    pub(crate) meshes: MeshMap,
    pub(crate) materials: MaterialMap,
    pub(crate) textures: TextureMap,
}

impl AssetCache {
    pub(crate) fn clear(&mut self) {
        self.materials.clear();
        self.meshes.clear();
        self.textures.clear();
    }
}
