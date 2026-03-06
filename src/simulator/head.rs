use bevy::prelude::*;

use crate::{common::Head, remote_control::prelude::*};

pub(super) struct HeadSimulatorPlugin;

impl Plugin for HeadSimulatorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, setup_head_controls);
        app.add_systems(Update, move_head);
    }
}

#[derive(Debug, Component)]
struct HeadDestination {
    world_pos: Vec3,
}

#[derive(Debug, Component)]
struct HeadOrientationDestination {
    world_rot: Quat,
}

#[derive(Debug, Component)]
struct HeadManaged;

#[derive(Debug, Component)]
struct HeadOrientationControlTarget {
    head: Entity,
}

fn setup_head_controls(
    query: Query<(Entity, Option<&Name>, Option<&Transform>), (With<Head>, Without<HeadManaged>)>,
    mut params: ResMut<RemoteControlDefinitions>,
    mut commands: Commands,
) {
    for (entity, name, tf) in query {
        let mut ec = commands.entity(entity);
        ec.observe(head_position_observer);
        ec.insert(HeadManaged);

        let label = match name {
            Some(name) => format!("Head: {} Position", name.as_str()),
            None => format!("Head: {entity} Position"),
        };
        let orientation_label = match name {
            Some(name) => format!("Head: {} Look At Position", name.as_str()),
            None => format!("Head: {entity} Look At Position"),
        };

        info!("Setting up head property for {entity} with label '{label}'");

        let initial_pos = tf.map_or(Vec3::ZERO, |tf| tf.translation);
        let initial_look_at = tf
            .map(|tf| {
                let forward = tf.rotation * Vec3::NEG_Z;
                tf.translation + forward
            })
            .unwrap_or(Vec3::NEG_Z);
        let orientation_control = commands
            .spawn(HeadOrientationControlTarget { head: entity })
            .observe(head_orientation_observer)
            .id();

        params.push(PropertyDefinition {
            id: entity,
            label,
            control: PropertyControl::Vector3 {
                initial: initial_pos,
                step: 0.001,
            },
        });
        params.push(PropertyDefinition {
            id: orientation_control,
            label: orientation_label,
            control: PropertyControl::Vector3 {
                initial: initial_look_at,
                step: 0.001,
            },
        });
    }
}

fn move_head(
    mut query: Query<
        (
            &mut Transform,
            Option<&HeadDestination>,
            Option<&HeadOrientationDestination>,
        ),
        With<Head>,
    >,
) {
    for (mut tf, destination, orientation) in &mut query {
        if let Some(dest) = destination {
            tf.translation = tf.translation.lerp(dest.world_pos, 0.5);
        }
        if let Some(dest) = orientation {
            tf.rotation = tf.rotation.slerp(dest.world_rot, 0.5);
        }
    }
}

fn head_position_observer(trigger: On<RemoteControlEvent>, mut commands: Commands) {
    let Ok(pos) = trigger.value.clone().try_into() else {
        return;
    };

    info!("Updating head {} to position {}", trigger.entity, pos);

    commands
        .entity(trigger.entity)
        .insert(HeadDestination { world_pos: pos });
}

fn head_orientation_observer(
    trigger: On<RemoteControlEvent>,
    mut commands: Commands,
    controls: Query<&HeadOrientationControlTarget>,
    heads: Query<&Transform, With<Head>>,
) {
    let Ok(look_at_position): Result<Vec3, _> = trigger.value.clone().try_into() else {
        return;
    };
    let Ok(target) = controls.get(trigger.entity) else {
        return;
    };
    let Ok(head_tf) = heads.get(target.head) else {
        return;
    };

    let direction = look_at_position - head_tf.translation;
    if direction.length_squared() <= f32::EPSILON {
        return;
    }
    let world_rot = Quat::from_rotation_arc(Vec3::NEG_Z, direction.normalize());

    info!(
        "Updating head {} look-at target to {}",
        target.head, look_at_position
    );

    commands
        .entity(target.head)
        .insert(HeadOrientationDestination { world_rot });
}
