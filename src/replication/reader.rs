use bevy::ecs::entity::EntityHashMap;
use bevy::prelude::*;

use crate::backfill_link::components::BReplicate;
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

        app.add_systems(Update, child_system);
    }
}

// =============================================================================

/// Remap foreign entities to local
#[derive(Resource, Default)]
struct EntityMap(EntityHashMap<Entity>);

impl EntityMap {
    fn map(&self, foreign: Entity) -> Entity {
        *self.0.get(&foreign).expect("unknown entity")
    }

    fn map_remove(&mut self, foreign: Entity) -> Entity {
        self.0.remove(&foreign).expect("unknown entity")
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
    mut images: ResMut<Assets<Image>>,
    mut exit_event: MessageWriter<AppExit>,
) {
    // wait for transcript to be finished
    let result = transcript.consume_next(|_, _, slice| {
        let result = consume_buffer(
            slice,
            &mut map,
            &mut commands,
            &mut meshes,
            &mut materials,
            &mut images,
        );

        if let ConsumeResult::Halt = result {
            exit_event.write(AppExit::Success);
        }
    });

    if result.is_err() {
        debug!("Logic is requesting terminate...");
        exit_event.write(AppExit::Success);
    }
}

enum ConsumeResult {
    Continue,
    Halt,
}

#[must_use]
#[inline(always)]
fn consume_buffer(
    bytes: &[u8],
    map: &mut EntityMap,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
) -> ConsumeResult {
    use crate::serialize::*;
    let mut bytes = ByteReader::new(bytes);

    loop {
        let instruction = unsafe { ClientInstruction::read_fast(&mut bytes) };

        //println!("CHILD: {instruction:?}");

        match instruction {
            ClientInstruction::EAdd(entity) => {
                let local = commands.spawn((BReplicate, Transform::default()));

                map.0.insert(entity, local.id());
                debug!("Mapping entity {:?} -> {:?}", entity, local.id());
            }
            ClientInstruction::ERemove(entity) => {
                let local = map.map_remove(entity);
                if let Ok(mut x) = commands.get_entity(local) {
                    x.despawn();
                }
                debug!("Removing entity {:?} -> {:?}", entity, local);
            }
            ClientInstruction::CAdd(item) => {
                let local = map.map(item.entity);

                item.component.add_component(local, commands);
            }
            ClientInstruction::CRemove(item) => {
                let local = map.map(item.entity);

                item.component.remove_component(local, commands);
            }
            ClientInstruction::CAsset(item) => {
                use crate::replication::replicated_assets::AssetEnum;

                match *item.asset {
                    AssetEnum::Mesh(x) => Mesh::install_mapping(x.id, x.data, meshes),
                    AssetEnum::StandardMaterial(x) => {
                        StandardMaterial::install_mapping(x.id, x.data, materials);
                    }
                    AssetEnum::Image(x) => Image::install_mapping(x.id, x.data, images),
                }
            }
            ClientInstruction::CDropAsset(drop_asset) => {
                use crate::replication::replicated_assets::ReplicatedAssetID;

                match drop_asset.id {
                    ReplicatedAssetID::Mesh(id) => Mesh::clear_mapping(id, meshes),
                    ReplicatedAssetID::StandardMaterial(id) => {
                        StandardMaterial::clear_mapping(id, materials);
                    }
                    ReplicatedAssetID::Image(id) => Image::clear_mapping(id, images),
                };
            }
            ClientInstruction::HChange(item) => {
                debug!("Heirarchy change {item:?}");
                // Remap foreign IDs to local before applying hierarchy changes
                let child_local = map.map(item.child);
                match item.new_parent {
                    Some(parent) => {
                        let parent_local = map.map(parent);
                        commands.entity(parent_local).add_child(child_local);
                    }
                    None => {
                        commands.entity(child_local).remove::<ChildOf>();
                    }
                }
            }
            ClientInstruction::EFrame(_) => {
                return ConsumeResult::Continue;
            }
            ClientInstruction::Halt(_) => {
                return ConsumeResult::Halt;
            }
        }
    }
}
