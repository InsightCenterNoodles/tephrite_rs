use bevy::prelude::*;

use crate::{
    input::Interactor,
    prelude::{PropagateReplication, Replicated},
    remote_control::use_cases::RemoteControlTransform,
};

pub(super) struct InteractorSimulatorPlugin;

impl Plugin for InteractorSimulatorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, setup_joystick);
    }
}

#[derive(Debug, Component)]
struct JoyManaged;

/// Logic to set up the joystick input system for the simulator.
fn setup_joystick(
    query: Query<Entity, (With<Interactor>, Without<JoyManaged>)>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for entity in query {
        let mut ec = commands.entity(entity);
        ec.insert(JoyManaged);
        ec.insert((Replicated, PropagateReplication::default()));
        ec.insert(RemoteControlTransform {
            position: true,
            rotation: true,
            ..Default::default()
        });

        let mesh = meshes.add(Cuboid::from_length(0.2));

        // Add axis mesh to the joystick for easy identification.
        ec.with_children(|parent| {
            // X-axis (red).
            parent.spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(materials.add(Color::linear_rgb(1.0, 0.0, 0.0))),
                Transform::from_scale(vec3(2.0, 1.0, 1.0)),
            ));
            // Y-axis (green).
            parent.spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(materials.add(Color::linear_rgb(0.0, 1.0, 0.0))),
                Transform::from_scale(vec3(1.0, 2.0, 1.0)),
            ));
            // Z-axis (blue).
            parent.spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(materials.add(Color::linear_rgb(0.0, 0.0, 1.0))),
                Transform::from_scale(vec3(1.0, 1.0, 2.0)),
            ));
        });
    }
}
