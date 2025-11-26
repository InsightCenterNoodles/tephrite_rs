use bevy::{app::Propagate, camera::visibility::*, ecs::bundle::Bundle, prelude::Component};

/// A component indicating that the entity should be replicated.
///
/// When added to an entity, the entity and supported components will be
/// replicated to all children processes. At the moment, this must be manually
/// added to your entities:
///
/// ```
/// # use bevy::prelude::*;
/// # use tephrite_rs::prelude::*;
/// # fn comp(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
/// commands.spawn((
///     Mesh3d(meshes.add(Circle::new(4.0))),
///     MeshMaterial3d(materials.add(Color::WHITE)),
///     Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
///     Replicated, // <-- Add this component!
/// ));
/// # }
/// ```
/// TODO: move to HierarchyPropagatePlugin
#[derive(Component, Debug, Clone, Copy, PartialEq)]
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
