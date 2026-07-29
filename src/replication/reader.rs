use bevy::ecs::entity::EntityHashMap;
use bevy::prelude::*;

use crate::prelude::PointsMaterial;
use crate::serialize::transcript_reader::TranscriptReaderResource;

use super::instruction::*;

// =============================================================================

/// Plugin for child processes. Reads a transcript and replicates entities, components, and assets.
pub struct ReplicationReaderPlugin;

impl Plugin for ReplicationReaderPlugin {
    fn build(&self, app: &mut App) {
        let transcript = TranscriptReaderResource::new();

        app.insert_non_send(transcript);
        app.init_resource::<EntityMap>();

        app.add_systems(PreUpdate, child_system);
    }
}

// =============================================================================

/// Remap foreign entities to local
#[derive(Resource, Default)]
struct EntityMap(EntityHashMap<Entity>);

impl EntityMap {
    fn ensure(&mut self, foreign: Entity, commands: &mut Commands) -> Entity {
        *(self
            .0
            .entry(foreign)
            .or_insert_with(|| commands.spawn_empty().id()))
    }

    fn map_opt(&self, foreign: Entity) -> Option<Entity> {
        self.0.get(&foreign).copied()
    }

    fn map_remove(&mut self, foreign: Entity) -> Option<Entity> {
        self.0.remove(&foreign)
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
    mut point_materials: ResMut<Assets<PointsMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut fonts: ResMut<Assets<Font>>,
    mut exit_event: MessageWriter<AppExit>,
) {
    // wait for transcript to be finished
    let result = transcript.consume_next(|_, _, slice| {
        consume_buffer(
            slice,
            &mut map,
            &mut commands,
            &mut meshes,
            &mut materials,
            &mut point_materials,
            &mut images,
            &mut fonts,
        );
    });

    if result.is_err() {
        debug!("Logic is requesting terminate...");
        exit_event.write(AppExit::Success);
    }
}

#[inline(always)]
fn consume_buffer(
    bytes: &[u8],
    map: &mut EntityMap,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    point_materials: &mut Assets<PointsMaterial>,
    images: &mut Assets<Image>,
    fonts: &mut Assets<Font>,
) {
    use crate::serialize::*;
    let mut bytes = ByteReader::new(bytes);

    loop {
        let instruction = unsafe { ClientInstruction::read_fast(&mut bytes) };

        //println!("CHILD: {instruction:?}");

        match instruction {
            ClientInstruction::CAdd(item) => {
                let local = map.ensure(item.entity, commands);

                item.component.add_component(local, commands);
            }
            ClientInstruction::CRemove(item) => {
                let local = map.ensure(item.entity, commands);

                item.component.remove_component(local, commands);
            }
            ClientInstruction::ResourceUpdate(item) => {
                item.resource.add_resource(commands);
            }
            ClientInstruction::ResourceDrop(item) => {
                item.resource.remove_resource(commands);
            }
            ClientInstruction::CAsset(item) => {
                use crate::replication::replicated_assets::AssetEnum;

                match *item.asset {
                    AssetEnum::Mesh(x) => Mesh::set_mapping(x.id, x.data, meshes),
                    AssetEnum::StandardMaterial(x) => {
                        StandardMaterial::set_mapping(x.id, x.data, materials);
                    }
                    AssetEnum::PointsMaterial(x) => {
                        PointsMaterial::set_mapping(x.id, x.data, point_materials);
                    }
                    AssetEnum::Image(x) => Image::set_mapping(x.id, x.data, images),
                    AssetEnum::Font(x) => Font::set_mapping(x.id, x.data, fonts),
                }
            }
            ClientInstruction::CDropAsset(drop_asset) => {
                use crate::replication::replicated_assets::ReplicatedAssetID;

                match drop_asset.id {
                    ReplicatedAssetID::Mesh(id) => Mesh::clear_mapping(id, meshes),
                    ReplicatedAssetID::StandardMaterial(id) => {
                        StandardMaterial::clear_mapping(id, materials);
                    }
                    ReplicatedAssetID::PointsMaterial(id) => {
                        PointsMaterial::clear_mapping(id, point_materials);
                    }
                    ReplicatedAssetID::Image(id) => Image::clear_mapping(id, images),
                    ReplicatedAssetID::Font(id) => Font::clear_mapping(id, fonts),
                };
            }
            ClientInstruction::HChange(item) => {
                // Remap foreign IDs to local before applying hierarchy changes
                let child_local = map.ensure(item.child, commands);
                match item.new_parent {
                    Some(parent) => {
                        if let Some(parent_local) = map.map_opt(parent) {
                            commands.entity(parent_local).add_child(child_local);
                        } else {
                            warn!(
                                "Skipping hierarchy parent {:?} for child {:?}: parent not mapped",
                                parent, item.child
                            );
                            commands.entity(child_local).remove::<ChildOf>();
                        }
                    }
                    None => {
                        commands.entity(child_local).remove::<ChildOf>();
                    }
                }
            }
            ClientInstruction::EFrame(_) => {
                return;
            }
            ClientInstruction::ERemove(entity) => {
                if let Some(e) = map.map_remove(entity) {
                    commands.entity(e).despawn();
                }
            }
        }
    }
}
