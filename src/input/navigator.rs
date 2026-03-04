use bevy::prelude::*;

use crate::input::{Interactor, InteractorState, common::map_point};

use super::JoystickType;

#[derive(Debug, Clone, Copy)]
pub enum NavigatorMode {
    ObjectCentric,
    JoyCentric,
}

#[derive(Debug, Component)]
pub struct NavigatorMarker;

#[derive(Debug, Clone, Resource)]
struct NavigatorSettings {
    mode: NavigatorMode,
}

#[derive(Debug)]
pub struct NavigationPlugin {
    settings: NavigatorSettings,
}

impl NavigationPlugin {
    pub fn new(mode: NavigatorMode) -> Self {
        Self {
            settings: NavigatorSettings { mode },
        }
    }
}

impl Plugin for NavigationPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.settings.clone());
        app.add_systems(Update, on_tick);
    }
}

fn on_tick(
    target: Query<(&mut Transform, Option<&ChildOf>), (With<NavigatorMarker>, Without<Interactor>)>,
    mut joystick: Query<(&GlobalTransform, &InteractorState), With<Interactor>>,
    parents: Query<&GlobalTransform>,
    settings: Res<NavigatorSettings>,
    time: Res<Time>,
) {
    for (mut target_tf, target_parent) in target {
        // TODO: will fail if we add more interactors
        let Ok((joystick_global_tf, state)) = joystick.single_mut() else {
            return;
        };

        let speed_meters_per_second = 2.0;
        let rotation_degrees_per_second = 40.0;

        let joy_world_position = joystick_global_tf.transform_point(Vec3::ZERO);

        if let Some(right_stick) = state.stick_state(JoystickType::Right) {
            // direction vector

            let dir = vec3(right_stick.x, 0.0, right_stick.y);
            let mut global_dir = joystick_global_tf.affine().transform_vector3(dir);

            // flatten that y!
            global_dir.y = 0.0;

            let global_displace = global_dir * speed_meters_per_second * time.delta_secs();

            // map to the local of the entity

            let parent_global_affine = target_parent
                .and_then(|parent| parents.get(parent.0).ok())
                .map(|parent_tf| parent_tf.affine())
                .unwrap_or_else(|| GlobalTransform::IDENTITY.affine());

            let local_displace = parent_global_affine
                .inverse()
                .transform_vector3(global_displace);

            target_tf.translation += local_displace;
        }

        if let Some(left_stick) = state.stick_state(JoystickType::Left) {
            target_tf.translation += vec3(
                0.0,
                time.delta_secs() * speed_meters_per_second * left_stick.y,
                0.0,
            )
        }

        if let Some(dpad) = state.stick_state(JoystickType::DPad) {
            let degrees = dpad.x;

            // left is 270 ish and right is 90 ish

            let dir = match degrees {
                255.0..285.0 => -1.0,
                75.0..105.0 => 1.0,
                _ => 0.0,
            };

            target_tf.rotate_axis(
                Dir3::Y,
                (rotation_degrees_per_second * time.delta_secs() * dir).to_radians(),
            );
        }

        const SCALE_FACTOR: f32 = 1.01;

        if state.buttons.pressed(super::JoystickButton::BL) {
            target_tf.scale =
                (target_tf.scale / SCALE_FACTOR).clamp(Vec3::splat(0.001), Vec3::splat(1000.0));
        }

        if state.buttons.pressed(super::JoystickButton::BR) {
            target_tf.scale =
                (target_tf.scale * SCALE_FACTOR).clamp(Vec3::splat(0.001), Vec3::splat(1000.0));
        }

        if state
            .buttons
            .just_pressed(crate::input::JoystickButton::Start)
        {
            *target_tf = Transform::IDENTITY;
        }
    }
}
