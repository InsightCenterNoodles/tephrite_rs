//! Simulator integration for [`crate::input::Interactor`] entities.
//!
//! Interactors are replicated and tagged with [`RemoteControlTransform`], then
//! decorated with axis meshes for quick visual identification.

use std::time::Duration;

use bevy::prelude::*;

use crate::{
    input::{ButtonMessage, Interactor, JoystickButton},
    prelude::{PropagateReplication, Replicated},
    remote_control::{
        RemoteControlDefinitions,
        events::RemoteControlEvent,
        prelude::{PropertyControl, PropertyDefinition},
        use_cases::RemoteControlTransform,
    },
};

pub(super) struct InteractorSimulatorPlugin;

impl Plugin for InteractorSimulatorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, setup_joystick);
        app.add_systems(Update, release_system);
    }
}

#[derive(Debug, Component)]
struct JoyManaged;

/// Set up simulator interactor entities for replication and remote control.
fn setup_joystick(
    query: Query<(Entity, Option<&Name>), (With<Interactor>, Without<JoyManaged>)>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut definitions: ResMut<RemoteControlDefinitions>,
) {
    for (entity, name) in query {
        let mut ec = commands.entity(entity);
        ec.insert(JoyManaged);
        ec.insert((Replicated, PropagateReplication::default()));
        ec.insert(RemoteControlTransform {
            position: true,
            rotation: true,
            ..Default::default()
        });
        ec.observe(button_control_observer);

        let axis_mesh = meshes.add(Cuboid::from_length(0.05));
        let arrow_mesh = meshes.add(Cuboid::from_length(0.035));

        // Add axis mesh and a forward arrow to the joystick for easy identification.
        ec.with_children(|parent| {
            let axis_thickness = 0.35;

            // X-axis (red).
            parent.spawn((
                Mesh3d(axis_mesh.clone()),
                MeshMaterial3d(materials.add(Color::linear_rgb(1.0, 0.0, 0.0))),
                Transform::from_scale(vec3(2.0, axis_thickness, axis_thickness)),
            ));
            // Y-axis (green).
            parent.spawn((
                Mesh3d(axis_mesh.clone()),
                MeshMaterial3d(materials.add(Color::linear_rgb(0.0, 1.0, 0.0))),
                Transform::from_scale(vec3(axis_thickness, 2.0, axis_thickness)),
            ));
            // Z-axis (blue).
            parent.spawn((
                Mesh3d(axis_mesh),
                MeshMaterial3d(materials.add(Color::linear_rgb(0.0, 0.0, 1.0))),
                Transform::from_scale(vec3(axis_thickness, axis_thickness, 2.0)),
            ));

            // Forward arrow (+Z): shaft and two angled head segments.
            let arrow_material = materials.add(Color::linear_rgb(1.0, 1.0, 1.0));
            parent.spawn((
                Mesh3d(arrow_mesh.clone()),
                MeshMaterial3d(arrow_material.clone()),
                Transform::from_xyz(0.0, 0.0, -0.065).with_scale(vec3(0.7, 0.7, 2.6)),
            ));
            parent.spawn((
                Mesh3d(arrow_mesh.clone()),
                MeshMaterial3d(arrow_material.clone()),
                Transform::from_xyz(0.015, 0.0, -0.11)
                    .with_rotation(Quat::from_rotation_y(0.65))
                    .with_scale(vec3(0.55, 0.55, 1.2)),
            ));
            parent.spawn((
                Mesh3d(arrow_mesh),
                MeshMaterial3d(arrow_material),
                Transform::from_xyz(-0.015, 0.0, -0.11)
                    .with_rotation(Quat::from_rotation_y(-0.65))
                    .with_scale(vec3(0.55, 0.55, 1.2)),
            ));
        });

        let label = match name {
            Some(name) => format!("{} Buttons", name.as_str()),
            None => format!("Entity {entity} Buttons"),
        };

        definitions.push(PropertyDefinition {
            id: entity,
            aspect_id: BUTTON_ASPECT,
            label: format!(
                "{label} (A, B, X, Y, BL, BR, TL, TR, Back, Start)[:ms duration, default 500 ms]"
            ),
            control: PropertyControl::String {
                initial: Default::default(),
            },
        });
    }
}

const BUTTON_ASPECT: u32 = 10;

#[derive(Debug, Component)]
struct PendingReleases(Vec<(JoystickButton, f32)>);

///
fn button_control_observer(
    trigger: On<RemoteControlEvent>,
    mut commands: Commands,
    mut items: Query<Option<&mut PendingReleases>, With<JoyManaged>>,
    mut writer: MessageWriter<ButtonMessage>,
) {
    if trigger.event().aspect_id != BUTTON_ASPECT {
        return;
    }

    let Ok(target): Result<String, _> = trigger.value.clone().try_into() else {
        return;
    };

    let joystick = trigger.entity;
    let target_button = target.to_lowercase();
    let mut target_button = target_button.as_str();
    let mut duration = 500u64;

    let parts = target_button.split_once(":");

    if let Some((new_target, new_duration)) = parts {
        target_button = new_target;
        duration = new_duration.parse().unwrap_or(500).clamp(1, 10000);
    }

    let button = match target_button {
        "a" => JoystickButton::A,
        "b" => JoystickButton::B,
        "x" => JoystickButton::X,
        "y" => JoystickButton::Y,
        "bl" => JoystickButton::BL,
        "br" => JoystickButton::BR,
        "tl" => JoystickButton::TL,
        "tr" => JoystickButton::TR,
        "back" => JoystickButton::Back,
        "start" => JoystickButton::Start,
        x => {
            info!("Unknown button: {x}");
            return;
        }
    };

    info!("Injecting button press {button:?}");
    writer.write(ButtonMessage {
        from: joystick,
        kind: crate::input::ButtonEventKind::ButtonPressed(button),
    });

    let release_when = Duration::from_millis(duration).as_secs_f32();

    match items.get_mut(trigger.entity).ok().and_then(|x| x) {
        Some(mut x) => {
            x.0.push((button, release_when));
        }
        None => {
            commands
                .entity(trigger.entity)
                .insert(PendingReleases(vec![(button, release_when)]));
        }
    }
}

fn release_system(
    items: Query<(Entity, &mut PendingReleases)>,
    mut writer: MessageWriter<ButtonMessage>,
    time: Res<Time>,
) {
    for (e, mut pending) in items {
        pending.0.retain_mut(|x| {
            x.1 -= time.delta_secs();

            if x.1 < 0.0 {
                info!("Injecting button release {:?}", x.0);
                writer.write(ButtonMessage {
                    from: e,
                    kind: crate::input::ButtonEventKind::ButtonReleased(x.0),
                });
                false
            } else {
                true
            }
        });
    }
}
