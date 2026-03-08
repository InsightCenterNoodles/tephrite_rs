use bevy::{app::Propagate, camera::visibility::*, ecs::bundle::Bundle, prelude::Component};

/// A component indicating that the entity should be replicated.
///
/// When added to an entity, the entity and supported components will be
/// replicated to all children processes. At the moment, this must be manually
/// added to your entities:
///
/// ```ignore
/// # use bevy::prelude::*;
/// # use tephrite_rs::prelude::*;
/// # fn comp(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
/// commands.spawn((
///     Mesh3d(meshes.add(Circle::new(4.0))),
///     MeshMaterial3d(materials.add(Color::WHITE)),
///     Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
///     Replicated, // <-- Add this component!
///     PropagateReplication::default(), // <-- Add this if you want children to be replicated as well!
/// ));
/// # }
/// ```
/// TODO: move to HierarchyPropagatePlugin
#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
#[component(immutable)]
#[require(Visibility, InheritedVisibility)]
pub struct Replicated;

//pub type PropagateReplication = Propagate(Replicated);
#[derive(Bundle)]
pub struct PropagateReplication {
    replication: Propagate<Replicated>,
}

impl Default for PropagateReplication {
    fn default() -> Self {
        Self {
            replication: Propagate(Replicated),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::app::HierarchyPropagatePlugin;
    use bevy::prelude::*;

    fn app_with_replication_propagation() -> App {
        let mut app = App::new();
        app.add_plugins(HierarchyPropagatePlugin::<Replicated>::new(PostUpdate));
        app
    }

    #[test]
    fn propagate_replication_marks_descendants() {
        let mut app = app_with_replication_propagation();

        let root = app
            .world_mut()
            .spawn((Replicated, PropagateReplication::default()))
            .id();
        let child = app.world_mut().spawn(ChildOf(root)).id();
        let grandchild = app.world_mut().spawn(ChildOf(child)).id();

        app.update();

        let world = app.world();
        assert!(world.entity(root).contains::<Replicated>());
        assert!(world.entity(child).contains::<Replicated>());
        assert!(world.entity(grandchild).contains::<Replicated>());
    }

    #[test]
    fn late_child_inherits_replication_when_parent_propagates() {
        let mut app = app_with_replication_propagation();

        let root = app
            .world_mut()
            .spawn((Replicated, PropagateReplication::default()))
            .id();
        app.update();

        let late_child = app.world_mut().spawn(ChildOf(root)).id();
        app.update();

        assert!(app.world().entity(late_child).contains::<Replicated>());
    }

    #[test]
    fn replicated_without_propagate_replication_does_not_propagate() {
        let mut app = app_with_replication_propagation();

        let root = app.world_mut().spawn(Replicated).id();
        let child = app.world_mut().spawn(ChildOf(root)).id();

        app.update();

        assert!(!app.world().entity(child).contains::<Replicated>());
    }

    #[test]
    fn reparent_out_of_propagated_tree_removes_replication() {
        let mut app = app_with_replication_propagation();

        let replicated_root = app
            .world_mut()
            .spawn((Replicated, PropagateReplication::default()))
            .id();
        let other_root = app.world_mut().spawn_empty().id();
        let child = app.world_mut().spawn(ChildOf(replicated_root)).id();

        app.update();
        assert!(app.world().entity(child).contains::<Replicated>());

        app.world_mut()
            .entity_mut(child)
            .insert(ChildOf(other_root));
        app.update();

        assert!(!app.world().entity(child).contains::<Replicated>());
    }
}
