use bevy::ecs::entity::EntityHashMap;
use bevy::prelude::*;

use crate::replication::registry::{ReplicationRegistry, TableId};
use crate::serialize::transcript_reader::TranscriptReaderResource;
use crate::serialize::{ByteReader, FastRead};

use super::instruction::*;

// =============================================================================

/// Plugin for child processes. Reads a transcript and replicates entities, components, and assets.
pub struct ReplicationReaderPlugin;

impl Plugin for ReplicationReaderPlugin {
    fn build(&self, app: &mut App) {
        let transcript = TranscriptReaderResource::new();

        app.init_resource::<ReplicationRegistry>();
        register_builtin_replication_types(app.world_mut());
        app.insert_non_send(transcript);
        app.init_resource::<EntityMap>();

        app.add_systems(PreUpdate, child_system);
    }
}

fn register_builtin_replication_types(world: &mut World) {
    let mut registry = world.resource_mut::<ReplicationRegistry>();
    crate::replication::replicated_components::register_builtin_components(&mut registry);
    crate::replication::replicated_assets::register_builtin_assets(&mut registry);
    crate::replication::replicated_resources::register_builtin_resources(&mut registry);
}

// =============================================================================

/// Remap foreign entities to local
#[derive(Resource, Default)]
struct EntityMap(EntityHashMap<Entity>);

impl EntityMap {
    fn add(&mut self, foreign: Entity, world: &mut World) -> Entity {
        *(self.0.entry(foreign).or_insert_with(|| {
            world
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
fn child_system(world: &mut World) {
    // wait for transcript to be finished
    let Some(mut transcript) = world.remove_non_send::<TranscriptReaderResource>() else {
        return;
    };

    let result = transcript.consume_next(|_, _, slice| {
        consume_buffer(slice, world);
    });

    world.insert_non_send(transcript);

    if result.is_err() {
        debug!("Logic is requesting terminate...");
        if let Some(mut exit_events) = world.get_resource_mut::<Messages<AppExit>>() {
            exit_events.write(AppExit::Success);
        }
    }
}

#[inline(always)]
fn consume_buffer(bytes: &[u8], world: &mut World) {
    let mut bytes = ByteReader::new(bytes);

    loop {
        let instruction = unsafe { u8::read_fast(&mut bytes) };

        //println!("CHILD: {instruction:?}");

        match instruction {
            INSTRUCTION_COMPONENT_ADD => {
                let entity = unsafe { Entity::read_fast(&mut bytes) };
                let component_type = unsafe { TableId::read_fast(&mut bytes) };
                let Some(entry) = world
                    .resource::<ReplicationRegistry>()
                    .component(component_type)
                    .cloned()
                else {
                    panic!(
                        "unknown component table id {component_type}; cannot skip unsized payload"
                    );
                };

                let Some(local) = world.resource::<EntityMap>().map_opt(entity) else {
                    warn!("Skipping component update for unmapped entity {:?}", entity);
                    (entry.skip)(&mut bytes);
                    continue;
                };

                (entry.apply)(local, world, &mut bytes);
            }
            INSTRUCTION_COMPONENT_REMOVE => {
                let entity = unsafe { Entity::read_fast(&mut bytes) };
                let component_type = unsafe { TableId::read_fast(&mut bytes) };
                if let Some(local) = world.resource::<EntityMap>().map_opt(entity) {
                    if let Some(entry) = world
                        .resource::<ReplicationRegistry>()
                        .component(component_type)
                        .cloned()
                    {
                        (entry.remove)(local, world);
                    }
                }
            }
            INSTRUCTION_RESOURCE_UPDATE => {
                let resource_type = unsafe { TableId::read_fast(&mut bytes) };
                let Some(entry) = world
                    .resource::<ReplicationRegistry>()
                    .resource(resource_type)
                    .cloned()
                else {
                    warn!("Skipping unknown resource table id {resource_type}");
                    continue;
                };
                (entry.apply)(world, &mut bytes);
            }
            INSTRUCTION_RESOURCE_DROP => {
                let resource_type = unsafe { TableId::read_fast(&mut bytes) };
                if let Some(entry) = world
                    .resource::<ReplicationRegistry>()
                    .resource(resource_type)
                    .cloned()
                {
                    (entry.drop)(world);
                }
            }
            INSTRUCTION_ASSET_UPDATE => {
                let asset_type = unsafe { TableId::read_fast(&mut bytes) };
                let Some(entry) = world
                    .resource::<ReplicationRegistry>()
                    .asset(asset_type)
                    .cloned()
                else {
                    warn!("Skipping unknown asset table id {asset_type}");
                    continue;
                };
                (entry.apply)(world, &mut bytes);
            }
            INSTRUCTION_ASSET_DROP => {
                let asset_type = unsafe { TableId::read_fast(&mut bytes) };
                if let Some(entry) = world
                    .resource::<ReplicationRegistry>()
                    .asset(asset_type)
                    .cloned()
                {
                    (entry.drop)(world, &mut bytes);
                }
            }
            INSTRUCTION_ENTITY_ADD => {
                let entity = unsafe { Entity::read_fast(&mut bytes) };
                world.resource_scope(|world, mut map: Mut<EntityMap>| {
                    map.add(entity, world);
                });
            }
            INSTRUCTION_HIERARCHY_CHANGE => {
                let item = unsafe { HierarchyChange::read_fast(&mut bytes) };
                // Remap foreign IDs to local before applying hierarchy changes
                let Some(child_local) = world.resource::<EntityMap>().map_opt(item.child) else {
                    continue;
                };

                match item.new_parent {
                    Some(parent) => {
                        if let Some(parent_local) = world.resource::<EntityMap>().map_opt(parent) {
                            world.entity_mut(parent_local).add_child(child_local);
                        } else {
                            warn!(
                                "Skipping hierarchy parent {:?} for child {:?}: parent not mapped",
                                parent, item.child
                            );
                        }
                    }
                    None => {
                        world.entity_mut(child_local).remove::<ChildOf>();
                    }
                }
            }
            INSTRUCTION_END_FRAME => {
                return;
            }
            INSTRUCTION_ENTITY_REMOVE => {
                let entity = unsafe { Entity::read_fast(&mut bytes) };
                let local = world.resource_mut::<EntityMap>().map_remove(entity);
                if let Some(e) = local {
                    world.entity_mut(e).despawn();
                }
            }
            INSTRUCTION_COMPONENT_TABLE => {
                let id = unsafe { TableId::read_fast(&mut bytes) };
                let name = unsafe { String::read_fast(&mut bytes) };
                validate_table_name(world, "component", id, &name);
            }
            INSTRUCTION_ASSET_TABLE => {
                let id = unsafe { TableId::read_fast(&mut bytes) };
                let name = unsafe { String::read_fast(&mut bytes) };
                validate_table_name(world, "asset", id, &name);
            }
            INSTRUCTION_RESOURCE_TABLE => {
                let id = unsafe { TableId::read_fast(&mut bytes) };
                let name = unsafe { String::read_fast(&mut bytes) };
                validate_table_name(world, "resource", id, &name);
            }
            INSTRUCTION_RENDERER_PLUGIN => {
                let plugin = unsafe { String::read_fast(&mut bytes) };
                debug!("Transcript requires renderer plugin {plugin}");
            }
            _ => panic!("unknown transcript instruction {instruction}"),
        }
    }
}

fn validate_table_name(world: &World, kind: &str, id: TableId, remote_name: &str) {
    let registry = world.resource::<ReplicationRegistry>();
    let local_name = match kind {
        "component" => registry.component(id).map(|entry| entry.name),
        "asset" => registry.asset(id).map(|entry| entry.name),
        "resource" => registry.resource(id).map(|entry| entry.name),
        _ => None,
    };

    match local_name {
        Some(local_name) if local_name == remote_name => {}
        Some(local_name) => warn!(
            "Transcript {kind} table id {id} mismatch: logic has {remote_name}, renderer has {local_name}"
        ),
        None => {
            warn!("Transcript {kind} table id {id} is not registered on renderer: {remote_name}")
        }
    }
}

#[cfg(test)]
mod tests {
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

    fn consume_test_frame(bytes: &[u8], world: &mut World) {
        if !world.contains_resource::<ReplicationRegistry>() {
            world.init_resource::<ReplicationRegistry>();
            register_builtin_replication_types(world);
        }
        if !world.contains_resource::<EntityMap>() {
            world.init_resource::<EntityMap>();
        }
        consume_buffer(bytes, world);
    }

    #[test]
    fn component_update_without_entity_add_does_not_create_mapping() {
        let remote = Entity::from_bits(100);
        let transform = Transform::from_xyz(1.0, 2.0, 3.0);
        let bytes = encode_frame(|writer| unsafe {
            write_component_add(writer, remote, 1, &transform);
            ServerInstruction::EFrame(EndFrame).write_fast(writer);
        });

        let mut world = World::new();
        consume_test_frame(&bytes, &mut world);

        assert!(world.resource::<EntityMap>().map_opt(remote).is_none());
        let mut query = world.query::<&Transform>();
        assert_eq!(query.iter(&world).count(), 0);
    }

    #[test]
    fn entity_add_then_component_update_creates_mapped_entity() {
        let remote = Entity::from_bits(101);
        let transform = Transform::from_xyz(1.0, 2.0, 3.0);
        let bytes = encode_frame(|writer| unsafe {
            ServerInstruction::EAdd(remote).write_fast(writer);
            write_component_add(writer, remote, 1, &transform);
            ServerInstruction::EFrame(EndFrame).write_fast(writer);
        });

        let mut world = World::new();
        consume_test_frame(&bytes, &mut world);

        let local = world
            .resource::<EntityMap>()
            .map_opt(remote)
            .expect("entity should be mapped");
        assert_eq!(world.entity(local).get::<Transform>(), Some(&transform));
    }
}
