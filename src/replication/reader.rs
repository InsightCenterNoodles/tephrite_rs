use bevy::ecs::entity::EntityHashMap;
use bevy::prelude::*;

use crate::replication::registry::{ReplicationRegistry, TableId};
use crate::serialize::transcript_reader::TranscriptReaderResource;
use crate::serialize::*;

use super::instruction::*;

// =============================================================================

/// Plugin for child processes. Reads a transcript and mirrors logic-world ECS
/// state into the render app.
///
/// Like the writer, this plugin expects [`ReplicationRegistry`] to have already
/// been populated by the shared Tephrite app configuration. The transcript only
/// carries compact table IDs, so table construction must be deterministic across
/// processes.
pub struct ReplicationReaderPlugin;

impl Plugin for ReplicationReaderPlugin {
    fn build(&self, app: &mut App) {
        let transcript = TranscriptReaderResource::new();

        app.init_resource::<ReplicationRegistry>();
        app.insert_non_send(transcript);
        app.init_resource::<EntityMap>();

        app.add_systems(PreUpdate, child_system);
    }
}

// =============================================================================

/// Remap foreign entities to local
///
/// Entity IDs are process-local in Bevy. The transcript uses logic-process
/// entity IDs as stable foreign keys; this map stores the render-process entity
/// that represents each foreign entity.
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

/// Primary reader system.
///
/// This is an exclusive world system because applying a transcript can touch
/// arbitrary registered component, asset, and resource types. Keeping this as a
/// single system also preserves exact instruction ordering within each frame.
fn child_system(world: &mut World) {
    // Temporarily remove the non-send transcript reader so the consume callback
    // can borrow `world` exclusively while parsing the frame.
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
        // Every instruction begins with a compact opcode. Dynamic payloads then
        // carry a per-table `TableId` that dispatches into `ReplicationRegistry`.
        let instruction = unsafe { u8::easy_read_fast(&mut bytes) };

        //println!("CHILD: {instruction:?}");

        match instruction {
            INSTRUCTION_COMPONENT_ADD => {
                let entity = unsafe { Entity::easy_read_fast(&mut bytes) };
                let component_type = unsafe { TableId::easy_read_fast(&mut bytes) };
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
                    // Component payloads are typed but not length-prefixed, so
                    // use the table entry's typed skip function to keep parsing
                    // aligned after this update.
                    (entry.skip)(world, &mut bytes);
                    continue;
                };

                (entry.apply)(local, world, &mut bytes);
            }
            INSTRUCTION_COMPONENT_REMOVE => {
                let entity = unsafe { Entity::easy_read_fast(&mut bytes) };
                let component_type = unsafe { TableId::easy_read_fast(&mut bytes) };
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
                let resource_type = unsafe { TableId::easy_read_fast(&mut bytes) };
                let Some(entry) = world
                    .resource::<ReplicationRegistry>()
                    .resource(resource_type)
                    .cloned()
                else {
                    panic!("Failing on unknown resource table id {resource_type}");
                };
                (entry.apply)(world, &mut bytes);
            }
            INSTRUCTION_RESOURCE_DROP => {
                let resource_type = unsafe { TableId::easy_read_fast(&mut bytes) };
                if let Some(entry) = world
                    .resource::<ReplicationRegistry>()
                    .resource(resource_type)
                    .cloned()
                {
                    (entry.drop)(world);
                }
            }
            INSTRUCTION_ASSET_UPDATE => {
                let asset_type = unsafe { TableId::easy_read_fast(&mut bytes) };
                let Some(entry) = world
                    .resource::<ReplicationRegistry>()
                    .asset(asset_type)
                    .cloned()
                else {
                    panic!("Failing on unknown asset table id {asset_type}");
                };
                (entry.apply)(world, &mut bytes);
            }
            INSTRUCTION_ASSET_RESERVE => {
                let asset_type = unsafe { TableId::easy_read_fast(&mut bytes) };
                let Some(entry) = world
                    .resource::<ReplicationRegistry>()
                    .asset(asset_type)
                    .cloned()
                else {
                    panic!("Failing on unknown asset table id {asset_type}");
                };
                (entry.reserve)(world, &mut bytes);
            }
            INSTRUCTION_ASSET_DROP => {
                let asset_type = unsafe { TableId::easy_read_fast(&mut bytes) };
                if let Some(entry) = world
                    .resource::<ReplicationRegistry>()
                    .asset(asset_type)
                    .cloned()
                {
                    (entry.drop)(world, &mut bytes);
                }
            }
            INSTRUCTION_ENTITY_ADD => {
                let entity = unsafe { Entity::easy_read_fast(&mut bytes) };
                world.resource_scope(|world, mut map: Mut<EntityMap>| {
                    // EAdd is the only instruction allowed to create a foreign
                    // entity mapping. Component updates for unknown entities are
                    // ignored instead of implicitly spawning.
                    map.add(entity, world);
                });
            }
            INSTRUCTION_HIERARCHY_CHANGE => {
                let item = unsafe { HierarchyChange::easy_read_fast(&mut bytes) };
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
                let entity = unsafe { Entity::easy_read_fast(&mut bytes) };
                let local = world.resource_mut::<EntityMap>().map_remove(entity);
                if let Some(e) = local {
                    world.entity_mut(e).despawn();
                }
            }
            _ => panic!("unknown transcript instruction {instruction}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serialize::ByteWriter;

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
            crate::replication::register_builtin_replication_types(world);
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

    #[test]
    fn component_remove_removes_mapped_component() {
        let remote = Entity::from_bits(102);
        let transform = Transform::from_xyz(1.0, 2.0, 3.0);
        let bytes = encode_frame(|writer| unsafe {
            ServerInstruction::EAdd(remote).write_fast(writer);
            write_component_add(writer, remote, 1, &transform);
            write_component_remove(writer, remote, 1);
            ServerInstruction::EFrame(EndFrame).write_fast(writer);
        });

        let mut world = World::new();
        consume_test_frame(&bytes, &mut world);

        let local = world
            .resource::<EntityMap>()
            .map_opt(remote)
            .expect("entity should stay mapped");
        assert!(world.entity(local).get::<Transform>().is_none());
    }

    #[test]
    fn entity_remove_despawns_mapped_entity_and_clears_mapping() {
        let remote = Entity::from_bits(103);
        let bytes = encode_frame(|writer| unsafe {
            ServerInstruction::EAdd(remote).write_fast(writer);
            ServerInstruction::ERemove(remote).write_fast(writer);
            ServerInstruction::EFrame(EndFrame).write_fast(writer);
        });

        let mut world = World::new();
        consume_test_frame(&bytes, &mut world);

        assert!(world.resource::<EntityMap>().map_opt(remote).is_none());
        let mut query = world.query::<&Transform>();
        assert_eq!(query.iter(&world).count(), 0);
    }

    #[test]
    fn hierarchy_change_reparents_mapped_entities() {
        let parent = Entity::from_bits(104);
        let child = Entity::from_bits(105);
        let bytes = encode_frame(|writer| unsafe {
            ServerInstruction::EAdd(parent).write_fast(writer);
            ServerInstruction::EAdd(child).write_fast(writer);
            ServerInstruction::HChange(HierarchyChange {
                new_parent: Some(parent),
                child,
            })
            .write_fast(writer);
            ServerInstruction::EFrame(EndFrame).write_fast(writer);
        });

        let mut world = World::new();
        consume_test_frame(&bytes, &mut world);

        let parent_local = world.resource::<EntityMap>().map_opt(parent).unwrap();
        let child_local = world.resource::<EntityMap>().map_opt(child).unwrap();

        assert_eq!(
            world.entity(child_local).get::<ChildOf>().map(|p| p.0),
            Some(parent_local)
        );
    }

    #[test]
    fn hierarchy_change_to_none_unparents_mapped_entity() {
        let parent = Entity::from_bits(106);
        let child = Entity::from_bits(107);
        let bytes = encode_frame(|writer| unsafe {
            ServerInstruction::EAdd(parent).write_fast(writer);
            ServerInstruction::EAdd(child).write_fast(writer);
            ServerInstruction::HChange(HierarchyChange {
                new_parent: Some(parent),
                child,
            })
            .write_fast(writer);
            ServerInstruction::HChange(HierarchyChange {
                new_parent: None,
                child,
            })
            .write_fast(writer);
            ServerInstruction::EFrame(EndFrame).write_fast(writer);
        });

        let mut world = World::new();
        consume_test_frame(&bytes, &mut world);

        let child_local = world.resource::<EntityMap>().map_opt(child).unwrap();
        assert!(world.entity(child_local).get::<ChildOf>().is_none());
    }

    #[test]
    #[should_panic(expected = "Failing on unknown asset table id 999")]
    fn unknown_asset_update_table_panics_before_stream_misalignment() {
        let bytes = encode_frame(|writer| unsafe {
            INSTRUCTION_ASSET_UPDATE.write_fast(writer);
            999u16.write_fast(writer);
            ServerInstruction::EFrame(EndFrame).write_fast(writer);
        });

        let mut world = World::new();
        consume_test_frame(&bytes, &mut world);
    }

    #[test]
    #[should_panic(expected = "Failing on unknown resource table id 999")]
    fn unknown_resource_update_table_panics_before_stream_misalignment() {
        let bytes = encode_frame(|writer| unsafe {
            INSTRUCTION_RESOURCE_UPDATE.write_fast(writer);
            999u16.write_fast(writer);
            ServerInstruction::EFrame(EndFrame).write_fast(writer);
        });

        let mut world = World::new();
        consume_test_frame(&bytes, &mut world);
    }
}
