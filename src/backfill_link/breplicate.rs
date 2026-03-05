use bevy::prelude::*;

use super::components::*;
use crate::backfill;

// --- plugin to register systems ---
// TODO: Move to HierarchyPropagatePlugin
pub struct ReplicationPlugin;

impl Plugin for ReplicationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            (
                on_breplicate_added_propagate_to_children,
                on_childof_added_inherit_breplicate,
            )
                .chain()
                .in_set(super::sets::ReplicateSet::Entity),
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

// ---- system #1: react to Added<BReplicate> on any entity ----

fn on_breplicate_added_propagate_to_children(
    mut commands: Commands,
    // entities that *just* got BReplicate
    newly_replicated: Query<Entity, Added<BReplicate>>,
    // read-only access to hierarchy
    children_q: Query<&Children>,
    // quick presence checks
    has_breplicate: Query<(), With<BReplicate>>,
    has_bentity: Query<(), With<BEntity>>,
    session: NonSend<super::resources::Session>,
) {
    let mut next_id = || backfill::new_entity(&session.0);
    on_breplicate_added_propagate_to_children_with_allocator(
        &mut commands,
        &newly_replicated,
        &children_q,
        &has_breplicate,
        &has_bentity,
        &mut next_id,
    );
}

// ---- system #2: react to Added<ChildOf> so late-added children inherit ----

fn on_childof_added_inherit_breplicate(
    mut commands: Commands,
    // (child, parent) edges that *just* appeared
    added_edges: Query<(Entity, &ChildOf), Added<ChildOf>>,
    // for looking up siblings/descendants once child is known
    children_q: Query<&Children>,
    // presence checks
    has_breplicate: Query<(), With<BReplicate>>,
    has_bentity: Query<(), With<BEntity>>,
    session: NonSend<super::resources::Session>,
) {
    let mut next_id = || backfill::new_entity(&session.0);
    on_childof_added_inherit_breplicate_with_allocator(
        &mut commands,
        &added_edges,
        &children_q,
        &has_breplicate,
        &has_bentity,
        &mut next_id,
    );
}

fn on_breplicate_added_propagate_to_children_with_allocator(
    commands: &mut Commands,
    newly_replicated: &Query<Entity, Added<BReplicate>>,
    children_q: &Query<&Children>,
    has_breplicate: &Query<(), With<BReplicate>>,
    has_bentity: &Query<(), With<BEntity>>,
    next_id: &mut impl FnMut() -> backfill::EntityId,
) {
    for root in newly_replicated.iter() {
        // ensure the root also has a BEntity
        ensure_bentity(commands, root, has_bentity, next_id);

        // recursively push BReplicate (and BEntity) to all descendants
        propagate_down(
            root,
            commands,
            children_q,
            has_breplicate,
            has_bentity,
            next_id,
        );
    }
}

fn on_childof_added_inherit_breplicate_with_allocator(
    commands: &mut Commands,
    added_edges: &Query<(Entity, &ChildOf), Added<ChildOf>>,
    children_q: &Query<&Children>,
    has_breplicate: &Query<(), With<BReplicate>>,
    has_bentity: &Query<(), With<BEntity>>,
    next_id: &mut impl FnMut() -> backfill::EntityId,
) {
    for (child, rel) in added_edges.iter() {
        let parent = rel.parent(); // the parent Entity

        // if parent is replicated, ensure child (and its subtree) inherits
        if has_breplicate.contains(parent) {
            // add to this child
            ensure_breplicate(commands, child, has_breplicate);
            ensure_bentity(commands, child, has_bentity, next_id);

            // then propagate further down from that child
            propagate_down(
                child,
                commands,
                children_q,
                has_breplicate,
                has_bentity,
                next_id,
            );
        }
    }
}

// ---- helpers ----

fn ensure_breplicate(
    commands: &mut Commands,
    entity: Entity,
    has_breplicate: &Query<(), With<BReplicate>>,
) {
    if !has_breplicate.contains(entity) {
        commands.entity(entity).insert(BReplicate);
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

/// Depth-first walk from `start`, adding `BReplicate` and `BEntity` to all descendants.
/// Uses `Children` to find descendants; safe if there are none.
fn propagate_down(
    start: Entity,
    commands: &mut Commands,
    children_q: &Query<&Children>,
    has_breplicate: &Query<(), With<BReplicate>>,
    has_bentity: &Query<(), With<BEntity>>,
    next_id: &mut impl FnMut() -> backfill::EntityId,
) {
    // iterative DFS to avoid deep recursion on big trees
    let mut stack = Vec::new();

    // seed stack with direct children of `start`
    if let Ok(children) = children_q.get(start) {
        stack.extend(children.iter());
    }

    while let Some(e) = stack.pop() {
        // ensure both components
        ensure_breplicate(commands, e, has_breplicate);
        ensure_bentity(commands, e, has_bentity, next_id);

        // push this entity's children
        if let Ok(children) = children_q.get(e) {
            stack.extend(children.iter());
        }
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

    fn test_on_breplicate_added_propagate_to_children(
        mut commands: Commands,
        newly_replicated: Query<Entity, Added<BReplicate>>,
        children_q: Query<&Children>,
        has_breplicate: Query<(), With<BReplicate>>,
        has_bentity: Query<(), With<BEntity>>,
        mut ids: ResMut<TestIdSource>,
    ) {
        on_breplicate_added_propagate_to_children_with_allocator(
            &mut commands,
            &newly_replicated,
            &children_q,
            &has_breplicate,
            &has_bentity,
            &mut || next_id(&mut ids),
        );
    }

    fn test_on_childof_added_inherit_breplicate(
        mut commands: Commands,
        added_edges: Query<(Entity, &ChildOf), Added<ChildOf>>,
        children_q: Query<&Children>,
        has_breplicate: Query<(), With<BReplicate>>,
        has_bentity: Query<(), With<BEntity>>,
        mut ids: ResMut<TestIdSource>,
    ) {
        on_childof_added_inherit_breplicate_with_allocator(
            &mut commands,
            &added_edges,
            &children_q,
            &has_breplicate,
            &has_bentity,
            &mut || next_id(&mut ids),
        );
    }

    fn app_with_test_systems() -> App {
        let mut app = App::new();
        app.insert_resource(TestIdSource::default());
        app.add_systems(
            PostUpdate,
            (
                test_on_breplicate_added_propagate_to_children,
                test_on_childof_added_inherit_breplicate,
            )
                .chain(),
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
}
