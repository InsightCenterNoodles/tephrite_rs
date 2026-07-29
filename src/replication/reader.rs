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
    fn add(&mut self, foreign: Entity, commands: &mut Commands) -> Entity {
        *(self.0.entry(foreign).or_insert_with(|| {
            commands
                .spawn((Transform::default(), InheritedVisibility::default()))
                .id()
        }))
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
                let Some(local) = map.map_opt(item.entity) else {
                    warn!("Skipping component update for unmapped entity {:?}", item.entity);
                    continue;
                };

                item.component.add_component(local, commands);
            }
            ClientInstruction::CRemove(item) => {
                if let Some(local) = map.map_opt(item.entity) {
                    item.component.remove_component(local, commands);
                }
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
            ClientInstruction::EAdd(entity) => {
                map.add(entity, commands);
            }
            ClientInstruction::HChange(item) => {
                // Remap foreign IDs to local before applying hierarchy changes
                let Some(child_local) = map.map_opt(item.child) else {
                    continue;
                };

                match item.new_parent {
                    Some(parent) => {
                        if let Some(parent_local) = map.map_opt(parent) {
                            commands.entity(parent_local).add_child(child_local);
                        } else {
                            warn!(
                                "Skipping hierarchy parent {:?} for child {:?}: parent not mapped",
                                parent, item.child
                            );
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

#[cfg(test)]
mod tests {
    use bevy::ecs::world::CommandQueue;

    use super::*;
    use crate::serialize::*;

    fn encode_frame(write: impl FnOnce(&mut ByteWriter<'_>)) -> Vec<u8> {
        let mut bytes = vec![0; 4096];
        let mut writer = ByteWriter::new(&mut bytes);
        write(&mut writer);
        let len = writer.position();
        bytes.truncate(len);
        bytes
    }

    fn consume_test_frame(bytes: &[u8], world: &mut World, map: &mut EntityMap) {
        let mut queue = CommandQueue::default();
        let mut commands = Commands::new(&mut queue, world);
        let mut meshes = Assets::<Mesh>::default();
        let mut materials = Assets::<StandardMaterial>::default();
        let mut point_materials = Assets::<PointsMaterial>::default();
        let mut images = Assets::<Image>::default();
        let mut fonts = Assets::<Font>::default();

        consume_buffer(
            bytes,
            map,
            &mut commands,
            &mut meshes,
            &mut materials,
            &mut point_materials,
            &mut images,
            &mut fonts,
        );

        queue.apply(world);
    }

    #[test]
    fn component_update_without_entity_add_does_not_create_mapping() {
        let remote = Entity::from_bits(100);
        let transform = Transform::from_xyz(1.0, 2.0, 3.0);
        let bytes = encode_frame(|writer| unsafe {
            ServerInstruction::CAdd(ServerComponentAdded {
                entity: remote,
                component: (&transform).into(),
            })
            .write_fast(writer);
            ServerInstruction::EFrame(EndFrame).write_fast(writer);
        });

        let mut world = World::new();
        let mut map = EntityMap::default();
        consume_test_frame(&bytes, &mut world, &mut map);

        assert!(map.map_opt(remote).is_none());
        let mut query = world.query::<&Transform>();
        assert_eq!(query.iter(&world).count(), 0);
    }

    #[test]
    fn entity_add_then_component_update_creates_mapped_entity() {
        let remote = Entity::from_bits(101);
        let transform = Transform::from_xyz(1.0, 2.0, 3.0);
        let bytes = encode_frame(|writer| unsafe {
            ServerInstruction::EAdd(remote).write_fast(writer);
            ServerInstruction::CAdd(ServerComponentAdded {
                entity: remote,
                component: (&transform).into(),
            })
            .write_fast(writer);
            ServerInstruction::EFrame(EndFrame).write_fast(writer);
        });

        let mut world = World::new();
        let mut map = EntityMap::default();
        consume_test_frame(&bytes, &mut world, &mut map);

        let local = map.map_opt(remote).expect("entity should be mapped");
        assert_eq!(world.entity(local).get::<Transform>(), Some(&transform));
    }
}
