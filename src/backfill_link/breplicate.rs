use bevy::prelude::*;

use super::components::*;
use crate::backfill;

// --- plugin to register systems ---
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
    for root in newly_replicated.iter() {
        // ensure the root also has a BEntity
        ensure_bentity(&mut commands, root, &has_bentity, &session);

        // recursively push BReplicate (and BEntity) to all descendants
        propagate_down(
            root,
            &mut commands,
            &children_q,
            &has_breplicate,
            &has_bentity,
            &session,
        );
    }
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
    for (child, rel) in &added_edges {
        let parent = rel.parent(); // the parent Entity

        // if parent is replicated, ensure child (and its subtree) inherits
        if has_breplicate.contains(parent) {
            // add to this child
            ensure_breplicate(&mut commands, child, &has_breplicate);
            ensure_bentity(&mut commands, child, &has_bentity, &session);

            // then propagate further down from that child
            propagate_down(
                child,
                &mut commands,
                &children_q,
                &has_breplicate,
                &has_bentity,
                &session,
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
    session: &NonSend<super::resources::Session>,
) {
    if !has_bentity.contains(entity) {
        let id = backfill::new_entity(&session.0);
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
    session: &NonSend<super::resources::Session>,
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
        ensure_bentity(commands, e, has_bentity, session);

        // push this entity's children
        if let Ok(children) = children_q.get(e) {
            stack.extend(children.iter());
        }
    }
}
