use bevy::ecs::entity::EntityHashMap;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;

use crate::serialize::transcript_reader::TranscriptReaderResource;

use super::instruction::*;

// =============================================================================

/// Plugin for child processes. Reads a transcript and replicates entities, components, and assets.
pub struct ReplicationReaderPlugin;

impl Plugin for ReplicationReaderPlugin {
    fn build(&self, app: &mut App) {
        //println!("Building reader...");
        let transcript = TranscriptReaderResource::new();

        app.insert_non_send_resource(transcript);
        app.init_resource::<EntityMap>();
        app.init_resource::<RemoteAssetMap<Mesh>>();
        app.init_resource::<RemoteAssetMap<StandardMaterial>>();

        app.add_systems(Update, child_system);
    }
}

// =============================================================================

/// Remap foreign entities to local
#[derive(Resource, Default)]
struct EntityMap(EntityHashMap<Entity>);

impl EntityMap {
    fn map(&self, foreign: Entity) -> Entity {
        *self.0.get(&foreign).unwrap()
    }

    fn map_remove(&mut self, foreign: Entity) -> Entity {
        self.0.remove(&foreign).unwrap()
    }
}

// =============================================================================

/// A map of remote assetids to local asset handles
#[derive(Resource)]
struct RemoteAssetMap<T: Asset>(HashMap<AssetId<T>, Handle<T>>);

impl<T: Asset> Default for RemoteAssetMap<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<T: Asset> RemoteAssetMap<T> {
    /// Store a new remote asset
    fn insert(&mut self, asset_store: &mut Assets<T>, remote_id: AssetId<T>, asset: T) {
        let new = asset_store.add(asset);
        //println!("Remapping remote id {remote_id} to {}", new.id());

        self.0.insert(remote_id, new);
    }

    /// Drop an asset from the store
    fn remove(&mut self, asset_store: &mut Assets<T>, remote_id: AssetId<T>) {
        //println!("Unmapping remote {remote_id}");
        if let Some(local) = self.0.remove(&remote_id) {
            asset_store.remove(local.id());
        }
    }

    /// Remap a remote handle to a local asset
    fn fixup(&self, remote_id: AssetId<T>) -> Handle<T> {
        //println!("Fixing up handle {remote_id}");
        self.0.get(&remote_id).expect("missing asset!").clone()
    }
}

// =============================================================================

/// Primary child system to be run every update to obtain new records from the
/// transcript
///
fn child_system(
    mut transcript: NonSendMut<TranscriptReaderResource>,
    mut map: ResMut<EntityMap>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut mesh_map: ResMut<RemoteAssetMap<Mesh>>,
    mut material_map: ResMut<RemoteAssetMap<StandardMaterial>>,
) {
    // wait for transcript to be finished
    transcript.consume_next(|_, _, slice| {
        consume_buffer(
            slice,
            &mut map,
            &mut commands,
            &mut meshes,
            &mut materials,
            &mut mesh_map,
            &mut material_map,
        );
    });
}

#[inline(always)]
fn consume_buffer(
    bytes: &[u8],
    map: &mut EntityMap,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    mesh_map: &mut RemoteAssetMap<Mesh>,
    material_map: &mut RemoteAssetMap<StandardMaterial>,
) {
    use crate::serialize::*;
    let mut bytes = ByteReader::new(bytes);

    loop {
        let instruction = unsafe { ClientInstruction::read_fast(&mut bytes) };

        //println!("CHILD: {instruction:?}");

        match instruction {
            ClientInstruction::EAdd(entity) => {
                let local = commands.spawn_empty();

                map.0.insert(entity, local.id());
                //println!("Mapping entity {:?} {:?}", entity, local.id());
            }
            ClientInstruction::ERemove(entity) => {
                let local = map.map_remove(entity);
                commands.entity(local).despawn();
            }
            ClientInstruction::CAdd(item) => {
                let local = map.map(item.entity);

                use crate::replication::replicated_components::ReplicatedComponent;

                let component = match item.component {
                    ReplicatedComponent::Mesh3d(mesh3d) => ReplicatedComponent::Mesh3d(
                        bevy::prelude::Mesh3d(mesh_map.fixup(mesh3d.id())),
                    ),
                    ReplicatedComponent::StandardMatComponent(mesh_material3d) => {
                        ReplicatedComponent::StandardMatComponent(
                            MeshMaterial3d::<StandardMaterial>(
                                material_map.fixup(mesh_material3d.id()),
                            ),
                        )
                    }
                    x => x,
                };

                component.add_component(local, commands);
            }
            ClientInstruction::CRemove(item) => {
                let local = map.map(item.entity);

                item.component.remove_component(local, commands);
            }
            ClientInstruction::CAsset(item) => {
                use crate::replication::replicated_assets::AssetEnum;

                match *item.asset {
                    AssetEnum::Mesh(mesh) => mesh_map.insert(meshes, mesh.id, mesh.data),
                    AssetEnum::StandardMaterial(standard_material) => {
                        material_map.insert(materials, standard_material.id, standard_material.data)
                    }
                }
            }
            ClientInstruction::CDropAsset(drop_asset) => {
                use crate::replication::replicated_assets::ReplicatedAssetID;

                match drop_asset.id {
                    ReplicatedAssetID::Mesh(asset_id) => mesh_map.remove(meshes, asset_id),
                    ReplicatedAssetID::StandardMaterial(asset_id) => {
                        material_map.remove(materials, asset_id)
                    }
                };
            }
            ClientInstruction::HChange(item) => match item.new_parent {
                Some(parent) => {
                    commands.entity(parent).add_child(item.child);
                }
                None => {
                    commands.entity(item.child).remove::<ChildOf>();
                }
            },
            ClientInstruction::EFrame(_) => {
                return;
            }
        }
    }
}
