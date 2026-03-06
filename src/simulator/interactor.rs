use bevy::prelude::*;

use crate::{
    input::Interactor,
    prelude::{PropagateReplication, Replicated},
    remote_control::prelude::*,
};

pub(super) struct InteractorSimulatorPlugin;

impl Plugin for InteractorSimulatorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, setup_joystick);
        app.add_systems(Update, move_joystick);
    }
}

#[derive(Debug, Component)]
struct JoyDestination {
    world_pos: Vec3,
}

#[derive(Debug, Component)]
struct JoyManaged;

/// Logic to set up the joystick input system for the simulator.
fn setup_joystick(
    query: Query<(Entity, &Name), (With<Interactor>, Without<JoyManaged>)>,
    mut params: ResMut<RemoteControlDefinitions>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (entity, name) in query {
        let mut ec = commands.entity(entity);
        ec.observe(joystick_observer);
        ec.insert(JoyManaged);
        ec.insert((Replicated, PropagateReplication::default()));

        info!("Setting up joystick for {entity:?} {name:?}");

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

        params.push(PropertyDefinition {
            id: entity,
            label: format!("Interactor: {} Position", name.as_str()),
            control: PropertyControl::Vector3 {
                initial: Vec3::ZERO,
                step: 0.001,
            },
        });
    }
}

fn move_joystick(mut query: Query<(&mut Transform, &JoyDestination), With<Interactor>>) {
    for (mut tf, dest) in &mut query {
        tf.translation = tf.translation.lerp(dest.world_pos, 0.5);
    }
}

fn joystick_observer(trigger: On<RemoteControlEvent>, mut commands: Commands) {
    let Ok(pos) = trigger.value.clone().try_into() else {
        return;
    };

    info!("Updating joystick {} to position {}", trigger.entity, pos);

    commands
        .entity(trigger.entity)
        .insert(JoyDestination { world_pos: pos });
}
