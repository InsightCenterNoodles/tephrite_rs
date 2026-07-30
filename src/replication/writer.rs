use bevy::ecs::{entity::EntityHashSet, system::SystemState};
use bevy::prelude::*;

use crate::replication::components::IsReplicated;
use crate::replication::registry::{ComponentTableEntry, ReplicationRegistry};
use crate::serialize::transcript_writer::*;
use crate::serialize::*;

use super::instruction::*;

#[derive(Default, Resource)]
struct TrackedEntities {
    entities: EntityHashSet,
}

#[derive(Resource)]
struct CachedRemovedChildOf {
    state: SystemState<RemovedComponents<'static, 'static, ChildOf>>,
}

/// Plugin to replicate world state into the transcript.
///
/// The writer assumes [`ReplicationRegistry`] has already been populated by the
/// shared Tephrite app configuration before this plugin is added. That is what
/// gives the logic and render processes matching table IDs without sending table
/// definitions over shared memory.
pub struct ReplicationWriterPlugin {
    children_count: u32,
}

impl ReplicationWriterPlugin {
    pub fn new(children_count: u32) -> Self {
        Self { children_count }
    }
}

impl Plugin for ReplicationWriterPlugin {
    fn build(&self, app: &mut App) {
        let transcript = TranscriptWriterResource::new(self.children_count);

        app.init_resource::<ReplicationRegistry>();
        app.insert_non_send(transcript);
        app.init_resource::<TrackedEntities>();
        app.add_systems(Startup, setup_shmem);
        app.add_systems(Update, watch_for_exit);
        app.add_systems(Last, write_replication_frame);
    }
}

fn setup_shmem(world: &mut World) {
    debug!("Starting up shared memory");
    let mut transcript = world.non_send_mut::<TranscriptWriterResource>();
    let session = transcript.prepare().expect("should not fail at start");
    world.insert_non_send(session);
}

fn watch_for_exit(mut res: NonSendMut<TranscriptWriterResource>, reader: MessageReader<AppExit>) {
    if reader.len() > 0 {
        info!("Exit triggered");
        res.shutdown();
    }
}

fn write_replication_frame(world: &mut World) {
    let Some(mut dest) = world.remove_non_send::<TranscriptWriteStateResource>() else {
        return;
    };

    let mut tracked = world
        .remove_resource::<TrackedEntities>()
        .unwrap_or_default();
    // Clone the descriptor tables before mutating the world. Each descriptor is
    // just a small bundle of table ID and function pointers, so this keeps the
    // exclusive system simple without holding long-lived borrows on the
    // registry resource.
    let component_entries = world
        .resource::<ReplicationRegistry>()
        .components()
        .to_vec();
    let asset_entries = world.resource::<ReplicationRegistry>().assets().to_vec();
    let resource_entries = world.resource::<ReplicationRegistry>().resources().to_vec();

    // Entity tracking is implicit: any entity with a mirrored component is
    // tracked, and every ancestor up to the root is tracked too. That preserves
    // hierarchy and transform context in the renderer even when only a leaf has
    // a renderable component.
    let mut newly_tracked = EntityHashSet::default();
    discover_tracked_entities(
        world,
        &component_entries,
        &mut tracked.entities,
        &mut newly_tracked,
    );

    for entity in newly_tracked.iter().copied() {
        unsafe { ServerInstruction::EAdd(entity).write_fast(&mut dest) };
    }

    // Assets/resources are emitted before component baselines so handles in
    // newly mirrored components can be resolved by the render process in the
    // same frame.
    for entry in &asset_entries {
        (entry.write_changes)(world, &mut dest, entry.id);
    }

    for entry in &resource_entries {
        (entry.write_change)(world, &mut dest, entry.id);
    }

    for entity in newly_tracked.iter().copied() {
        for entry in &component_entries {
            (entry.write_baseline)(world, &mut dest, entity, entry.id);
        }
    }

    // Non-baseline component changes follow. New entities are excluded inside
    // each table entry's callback so a freshly tracked component is not sent
    // twice.
    for entry in &component_entries {
        (entry.write_changes)(world, &mut dest, &newly_tracked, entry.id);
    }

    write_hierarchy_changes(world, &mut dest, &tracked.entities, &newly_tracked);

    for entry in &component_entries {
        (entry.write_removals)(world, &mut dest, &tracked.entities, entry.id);
    }

    write_entity_removals(world, &mut dest, &mut tracked.entities);

    unsafe { ServerInstruction::EFrame(EndFrame).write_fast(&mut dest) };
    world.insert_resource(tracked);
    commit_frame(world, dest);
}

fn commit_frame(world: &mut World, state: TranscriptWriteStateResource) {
    let Some(mut writer) = world.get_non_send_mut::<TranscriptWriterResource>() else {
        return;
    };

    if writer.commit(state).is_err() {
        return;
    }

    let Ok(next) = writer.prepare() else {
        return;
    };

    world.insert_non_send(next);
}

fn discover_tracked_entities(
    world: &mut World,
    component_entries: &[ComponentTableEntry],
    tracked: &mut EntityHashSet,
    newly_tracked: &mut EntityHashSet,
) {
    let mut candidates = Vec::new();
    for entry in component_entries {
        (entry.collect_entities)(world, &mut candidates);
    }

    for entity in candidates {
        track_entity_and_ancestors(world, tracked, newly_tracked, entity);
    }
}

fn track_entity_and_ancestors(
    world: &mut World,
    tracked: &mut EntityHashSet,
    newly_tracked: &mut EntityHashSet,
    entity: Entity,
) {
    let mut current = Some(entity);

    while let Some(entity) = current {
        let Ok(entity_ref) = world.get_entity(entity) else {
            break;
        };
        current = entity_ref.get::<ChildOf>().map(|parent| parent.0);

        if tracked.insert(entity) {
            newly_tracked.insert(entity);
            // `IsReplicated` is internal bookkeeping used by filtered change
            // queries. It is intentionally not part of the mirrored component
            // table.
            world.entity_mut(entity).insert(IsReplicated);
        }
    }
}

fn write_hierarchy_changes(
    world: &mut World,
    dest: &mut TranscriptWriteStateResource,
    tracked: &EntityHashSet,
    newly_tracked: &EntityHashSet,
) {
    // Newly tracked entities need an initial hierarchy snapshot. Later frames
    // only send ChildOf changes/removals for entities already in the tracked
    // set.
    for entity in newly_tracked.iter().copied() {
        write_hierarchy_for_entity(world, dest, tracked, entity);
    }

    let mut query =
        world.query_filtered::<(Entity, &ChildOf), (Changed<ChildOf>, With<IsReplicated>)>();
    let mut changed = Vec::new();
    for (entity, child_of) in query.iter(world) {
        if !newly_tracked.contains(&entity) {
            changed.push((entity, child_of.0));
        }
    }

    for (entity, parent) in changed {
        let new_parent = tracked.contains(&parent).then_some(parent);
        unsafe {
            ServerInstruction::HChange(HierarchyChange {
                new_parent,
                child: entity,
            })
            .write_fast(dest);
        }
    }

    if !world.contains_resource::<CachedRemovedChildOf>() {
        let state = SystemState::new(world);
        world.insert_resource(CachedRemovedChildOf { state });
    }

    world.resource_scope(|world, mut cached: Mut<CachedRemovedChildOf>| {
        let Ok(mut removals) = cached.state.get_mut(world) else {
            return;
        };
        for child in removals.read() {
            if tracked.contains(&child) {
                unsafe {
                    ServerInstruction::HChange(HierarchyChange {
                        new_parent: None,
                        child,
                    })
                    .write_fast(dest);
                }
            }
        }
        cached.state.apply(world);
    });
}

fn write_hierarchy_for_entity(
    world: &World,
    dest: &mut TranscriptWriteStateResource,
    tracked: &EntityHashSet,
    entity: Entity,
) {
    let Some(child_of) = world.get::<ChildOf>(entity) else {
        return;
    };

    let new_parent = tracked.contains(&child_of.0).then_some(child_of.0);
    unsafe {
        ServerInstruction::HChange(HierarchyChange {
            new_parent,
            child: entity,
        })
        .write_fast(dest);
    }
}

fn write_entity_removals(
    world: &World,
    dest: &mut TranscriptWriteStateResource,
    tracked: &mut EntityHashSet,
) {
    let removed: Vec<_> = tracked
        .iter()
        .copied()
        .filter(|entity| world.get_entity(*entity).is_err())
        .collect();

    for entity in removed {
        tracked.remove(&entity);
        unsafe { ServerInstruction::ERemove(entity).write_fast(dest) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_component_tracks_entity_and_ancestors_to_root() {
        let mut world = World::new();
        let root = world.spawn(Transform::from_xyz(1.0, 0.0, 0.0)).id();
        let mid = world.spawn((Transform::default(), ChildOf(root))).id();
        let leaf = world.spawn((Mesh3d::default(), ChildOf(mid))).id();

        let mut tracked = EntityHashSet::default();
        let mut newly_tracked = EntityHashSet::default();

        let mut registry = ReplicationRegistry::default();
        crate::replication::replicated_components::register_builtin_components(&mut registry);
        let component_entries = registry.components().to_vec();

        discover_tracked_entities(
            &mut world,
            &component_entries,
            &mut tracked,
            &mut newly_tracked,
        );

        assert!(tracked.contains(&root));
        assert!(tracked.contains(&mid));
        assert!(tracked.contains(&leaf));
        assert!(world.entity(root).contains::<IsReplicated>());
        assert!(world.entity(mid).contains::<IsReplicated>());
        assert!(world.entity(leaf).contains::<IsReplicated>());
    }

    #[test]
    fn tracked_entity_remains_tracked_after_losing_supported_components() {
        let mut world = World::new();
        let entity = world.spawn(Transform::default()).id();

        let mut tracked = EntityHashSet::default();
        let mut newly_tracked = EntityHashSet::default();
        let mut registry = ReplicationRegistry::default();
        crate::replication::replicated_components::register_builtin_components(&mut registry);
        let component_entries = registry.components().to_vec();
        discover_tracked_entities(
            &mut world,
            &component_entries,
            &mut tracked,
            &mut newly_tracked,
        );

        world.entity_mut(entity).remove::<Transform>();
        newly_tracked.clear();
        discover_tracked_entities(
            &mut world,
            &component_entries,
            &mut tracked,
            &mut newly_tracked,
        );

        assert!(tracked.contains(&entity));
        assert!(newly_tracked.is_empty());
    }
}
