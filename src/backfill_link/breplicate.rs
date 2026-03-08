use bevy::app::{HierarchyPropagatePlugin, Inherited, Propagate, PropagateSet};
use bevy::prelude::*;

use super::components::*;
use crate::backfill;

pub struct ReplicationPlugin;

impl Plugin for ReplicationPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(HierarchyPropagatePlugin::<BReplicate>::new(PostUpdate));

        app.configure_sets(
            PostUpdate,
            super::sets::ReplicateSet::Entity.before(PropagateSet::<BReplicate>::default()),
        );
        app.configure_sets(
            PostUpdate,
            super::sets::ReplicateSet::Propagate.after(PropagateSet::<BReplicate>::default()),
        );

        app.add_systems(
            PostUpdate,
            mark_explicit_breplicate_sources.in_set(super::sets::ReplicateSet::Entity),
        );

        app.add_systems(
            PostUpdate,
            on_breplicate_added_ensure_bentity.in_set(super::sets::ReplicateSet::Propagate),
        );

        app.add_observer(on_remove);
    }
}

fn on_remove(
    trigger: On<Remove, BEntity>,
    query: Query<&BEntity>,
    session: NonSend<super::resources::Session>,
) {
    // this hook runs before removal. so this access is documented to be ok.
    let q = query
        .get(trigger.event().entity)
        .expect("Missing a component?");

    debug!("Destroying entity: {:?}", q.0);

    backfill::destroy_entity(&session.0, q.0);
}

// Keep existing API ergonomics: adding BReplicate directly marks this entity
// as a propagation source for descendants.
fn mark_explicit_breplicate_sources(
    mut commands: Commands,
    newly_replicated: Query<
        Entity,
        (
            Added<BReplicate>,
            Without<Propagate<BReplicate>>,
            Without<Inherited<BReplicate>>,
        ),
    >,
) {
    for entity in newly_replicated.iter() {
        commands.entity(entity).insert(Propagate(BReplicate));
    }
}

fn on_breplicate_added_ensure_bentity(
    mut commands: Commands,
    newly_replicated: Query<Entity, Added<BReplicate>>,
    has_bentity: Query<(), With<BEntity>>,
    session: NonSend<super::resources::Session>,
) {
    let mut next_id = || backfill::new_entity(&session.0);
    on_breplicate_added_ensure_bentity_with_allocator(
        &mut commands,
        &newly_replicated,
        &has_bentity,
        &mut next_id,
    );
}

fn on_breplicate_added_ensure_bentity_with_allocator(
    commands: &mut Commands,
    newly_replicated: &Query<Entity, Added<BReplicate>>,
    has_bentity: &Query<(), With<BEntity>>,
    next_id: &mut impl FnMut() -> backfill::EntityId,
) {
    for entity in newly_replicated.iter() {
        ensure_bentity(commands, entity, has_bentity, next_id);
    }
}

fn ensure_bentity(
    commands: &mut Commands,
    entity: Entity,
    has_bentity: &Query<(), With<BEntity>>,
    next_id: &mut impl FnMut() -> backfill::EntityId,
) {
    if !has_bentity.contains(entity) {
        let id = next_id();
        debug!("Added new entity {id:?} to bevy entity {entity}");
        commands.entity(entity).insert(BEntity(id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Resource, Default)]
    struct TestIdSource(i32);

    fn next_id(ids: &mut ResMut<TestIdSource>) -> backfill::EntityId {
        let out = backfill::EntityId(ids.0);
        ids.0 += 1;
        out
    }

    fn test_mark_explicit_breplicate_sources(
        mut commands: Commands,
        newly_replicated: Query<
            Entity,
            (
                Added<BReplicate>,
                Without<Propagate<BReplicate>>,
                Without<Inherited<BReplicate>>,
            ),
        >,
    ) {
        for entity in newly_replicated.iter() {
            commands.entity(entity).insert(Propagate(BReplicate));
        }
    }

    fn test_on_breplicate_added_ensure_bentity(
        mut commands: Commands,
        newly_replicated: Query<Entity, Added<BReplicate>>,
        has_bentity: Query<(), With<BEntity>>,
        mut ids: ResMut<TestIdSource>,
    ) {
        on_breplicate_added_ensure_bentity_with_allocator(
            &mut commands,
            &newly_replicated,
            &has_bentity,
            &mut || next_id(&mut ids),
        );
    }

    fn app_with_test_systems() -> App {
        let mut app = App::new();
        app.insert_resource(TestIdSource::default());
        app.add_plugins(HierarchyPropagatePlugin::<BReplicate>::new(PostUpdate));
        app.add_systems(
            PostUpdate,
            test_mark_explicit_breplicate_sources.before(PropagateSet::<BReplicate>::default()),
        );
        app.add_systems(
            PostUpdate,
            test_on_breplicate_added_ensure_bentity.after(PropagateSet::<BReplicate>::default()),
        );
        app
    }

    #[test]
    fn replicates_existing_subtree_when_breplicate_is_added() {
        let mut app = app_with_test_systems();

        let root = app.world_mut().spawn_empty().id();
        let child = app.world_mut().spawn(ChildOf(root)).id();
        let grandchild = app.world_mut().spawn(ChildOf(child)).id();

        app.world_mut().entity_mut(root).insert(BReplicate);
        app.update();

        let world = app.world();
        assert!(world.entity(root).contains::<BReplicate>());
        assert!(world.entity(child).contains::<BReplicate>());
        assert!(world.entity(grandchild).contains::<BReplicate>());
        assert!(world.entity(root).contains::<BEntity>());
        assert!(world.entity(child).contains::<BEntity>());
        assert!(world.entity(grandchild).contains::<BEntity>());
    }

    #[test]
    fn late_child_of_replicated_parent_inherits_replication() {
        let mut app = app_with_test_systems();

        let root = app.world_mut().spawn(BReplicate).id();
        app.update();

        let child = app.world_mut().spawn(ChildOf(root)).id();
        let grandchild = app.world_mut().spawn(ChildOf(child)).id();
        app.update();

        let world = app.world();
        assert!(world.entity(child).contains::<BReplicate>());
        assert!(world.entity(grandchild).contains::<BReplicate>());
        assert!(world.entity(child).contains::<BEntity>());
        assert!(world.entity(grandchild).contains::<BEntity>());
    }

    #[test]
    fn child_of_non_replicated_parent_does_not_inherit_replication() {
        let mut app = app_with_test_systems();

        let root = app.world_mut().spawn_empty().id();
        let child = app.world_mut().spawn(ChildOf(root)).id();
        app.update();

        let world = app.world();
        assert!(!world.entity(child).contains::<BReplicate>());
        assert!(!world.entity(child).contains::<BEntity>());
    }

    #[test]
    fn reparented_entity_out_of_tree_loses_breplicate() {
        let mut app = app_with_test_systems();

        let replicated_root = app.world_mut().spawn(BReplicate).id();
        let other_root = app.world_mut().spawn_empty().id();
        let child = app.world_mut().spawn(ChildOf(replicated_root)).id();

        app.update();
        assert!(app.world().entity(child).contains::<BReplicate>());

        app.world_mut()
            .entity_mut(child)
            .insert(ChildOf(other_root));
        app.update();

        assert!(!app.world().entity(child).contains::<BReplicate>());
    }
}
