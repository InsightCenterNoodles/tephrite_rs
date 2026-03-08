//! Reusable remote-control patterns built on top of raw property definitions.
//!
//! The primary use-case in this module is [`RemoteControlTransform`], which
//! exposes position/look-at/scale controls for any tagged entity.

use bevy::prelude::*;

use crate::remote_control::{
    RemoteControlDefinitions,
    events::RemoteControlEvent,
    prelude::{PropertyControl, PropertyDefinition},
};

/// Expose this entity's transform to remote control. Use fields to select which aspect of the transform to expose.
/// Use a Name component to provide a human-friendly identity for the controls.
#[derive(Debug, Component)]
#[component(immutable)]
pub struct RemoteControlTransform {
    /// Expose translation controls.
    pub position: bool,
    /// Expose look-at controls (vector interpreted as world-space target).
    pub rotation: bool,
    /// Expose scale controls.
    pub scale: bool,
}

impl Default for RemoteControlTransform {
    fn default() -> Self {
        Self {
            position: true,
            rotation: true,
            scale: false,
        }
    }
}

#[derive(Debug, Component)]
struct RemoteControlTransformManaged;

/// Target position received from remote control.
#[derive(Debug, Component)]
struct RemoteControlPositionDestination {
    world_pos: Vec3,
}

/// Target orientation received from remote control.
#[derive(Debug, Component)]
struct RemoteControlOrientationDestination {
    world_rot: Quat,
}

/// Target scale received from remote control.
#[derive(Debug, Component)]
struct RemoteControlScaleDestination {
    world_scale: Vec3,
}

/// Aspect ID for translation controls.
const POSITION_ASPECT: u32 = 0;
/// Aspect ID for look-at controls.
const LOOK_AT_ASPECT: u32 = 1;
/// Aspect ID for scale controls.
const SCALE_ASPECT: u32 = 2;

/// Plugin registering built-in remote-control use cases.
pub(super) struct UseCasesPlugin;

impl Plugin for UseCasesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, setup);
        app.add_systems(Update, move_items);
    }
}

fn setup(
    query: Query<
        (
            Entity,
            Option<&Name>,
            Option<&Transform>,
            &RemoteControlTransform,
        ),
        (Without<RemoteControlTransformManaged>,),
    >,
    mut params: ResMut<RemoteControlDefinitions>,
    mut commands: Commands,
) {
    for (entity, name, tf, remote_ops) in query {
        let mut ec = commands.entity(entity);
        ec.observe(control_observer);
        ec.insert(RemoteControlTransformManaged);

        let label = match name {
            Some(name) => format!("{} Position", name.as_str()),
            None => format!("Entity {entity} Position"),
        };
        let orientation_label = match name {
            Some(name) => format!("{} Look At Position", name.as_str()),
            None => format!("Entity {entity} Look At Position"),
        };
        let scale_label = match name {
            Some(name) => format!("{} Scale", name.as_str()),
            None => format!("Entity {entity} Scale"),
        };

        info!("Setting up properties for {entity} with label '{label}'");

        let initial_pos = tf.map_or(Vec3::ZERO, |tf| tf.translation);
        let initial_look_at = tf
            .map(|tf| {
                let forward = tf.rotation * Vec3::NEG_Z;
                tf.translation + forward
            })
            .unwrap_or(Vec3::NEG_Z);

        let initial_scale = tf.map_or(Vec3::ONE, |tf| tf.scale);

        if remote_ops.position {
            params.push(PropertyDefinition {
                id: entity,
                aspect_id: POSITION_ASPECT,
                label,
                control: PropertyControl::Vector3 {
                    initial: initial_pos,
                    step: 0.001,
                },
            });
        }
        if remote_ops.rotation {
            params.push(PropertyDefinition {
                id: entity,
                aspect_id: LOOK_AT_ASPECT,
                label: orientation_label,
                control: PropertyControl::Vector3 {
                    initial: initial_look_at,
                    step: 0.001,
                },
            });
        }
        if remote_ops.scale {
            params.push(PropertyDefinition {
                id: entity,
                aspect_id: SCALE_ASPECT,
                label: scale_label,
                control: PropertyControl::Vector3 {
                    initial: initial_scale,
                    step: 0.001,
                },
            });
        }
    }
}

/// Smoothly move entities toward remote-controlled transform destinations.
fn move_items(
    mut query: Query<
        (
            Entity,
            &mut Transform,
            Option<&RemoteControlPositionDestination>,
            Option<&RemoteControlOrientationDestination>,
            Option<&RemoteControlScaleDestination>,
        ),
        With<RemoteControlTransformManaged>,
    >,
    mut commands: Commands,
) {
    for (entity, mut tf, destination, orientation, scale) in &mut query {
        if let Some(dest) = destination {
            if dest.world_pos.distance_squared(tf.translation) < 0.00001 {
                commands
                    .entity(entity)
                    .remove::<RemoteControlPositionDestination>();
                tf.translation = dest.world_pos;
            } else {
                tf.translation = tf.translation.lerp(dest.world_pos, 0.5);
            }
        }
        if let Some(dest) = orientation {
            if dest.world_rot.angle_between(tf.rotation) < 0.00001 {
                commands
                    .entity(entity)
                    .remove::<RemoteControlOrientationDestination>();
                tf.rotation = dest.world_rot;
            } else {
                tf.rotation = tf.rotation.slerp(dest.world_rot, 0.5);
            }
        }
        if let Some(dest) = scale {
            if dest.world_scale.distance_squared(tf.scale) < 0.00001 {
                commands
                    .entity(entity)
                    .remove::<RemoteControlScaleDestination>();
                tf.scale = dest.world_scale;
            } else {
                tf.scale = tf.scale.lerp(dest.world_scale, 0.5);
            }
        }
    }
}

/// Route incoming transform property events by `aspect_id`.
fn control_observer(
    trigger: On<RemoteControlEvent>,
    mut commands: Commands,
    items: Query<&Transform, With<RemoteControlTransformManaged>>,
) {
    let Ok(target): Result<Vec3, _> = trigger.value.clone().try_into() else {
        return;
    };

    match trigger.event().aspect_id {
        POSITION_ASPECT => {
            //info!("Updating head {} to position {}", trigger.entity, target);
            commands
                .entity(trigger.entity)
                .insert(RemoteControlPositionDestination { world_pos: target });
        }
        LOOK_AT_ASPECT => {
            let Ok(item_tf) = items.get(trigger.entity) else {
                return;
            };

            let direction = target - item_tf.translation;
            if direction.length_squared() <= f32::EPSILON {
                return;
            }
            let world_rot = Quat::from_rotation_arc(Vec3::NEG_Z, direction.normalize());

            info!(
                "Updating head {} look-at target to {}",
                trigger.entity, target
            );

            commands
                .entity(trigger.entity)
                .insert(RemoteControlOrientationDestination { world_rot });
        }
        SCALE_ASPECT => {
            commands
                .entity(trigger.entity)
                .insert(RemoteControlScaleDestination {
                    world_scale: target,
                });
        }
        _ => {}
    }
}
