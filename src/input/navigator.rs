use bevy::prelude::*;

use crate::input::{Interactor, InteractorState, common::map_point};

use super::JoystickType;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum NavigatorMode {
    #[default]
    ObjectCentric,
    JoyCentric,
}

#[derive(Debug, Component)]
pub struct NavigatorMarker;

#[derive(Debug, Clone, Resource, Default)]
struct NavigatorSettings {
    mode: NavigatorMode,
    allow_x_rotation: bool,
}

#[derive(Debug)]
pub struct NavigationPlugin {
    settings: NavigatorSettings,
}

impl NavigationPlugin {
    pub fn new(mode: NavigatorMode) -> Self {
        Self {
            settings: NavigatorSettings { mode, allow_x_rotation: true },
        }
    }
    
    pub fn with_x_rotation(mut self, allow: bool) -> Self {
        self.settings.allow_x_rotation = allow;
        self
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

            // 15 degree bounds

            let dir_x = match degrees {
                0.0..15.0 => -1.0,
                345.0..370.0 => -1.0,
                165.0..195.0 => 1.0,
                _ => 0.0,
            };

            let dir_y = match degrees {
                255.0..285.0 => -1.0,
                75.0..105.0 => 1.0,
                _ => 0.0,
            };

            let mut rotation = Quat::from_rotation_y((rotation_degrees_per_second * time.delta_secs() * dir_y).to_radians());
            
            if (settings.allow_x_rotation) {
                rotation = rotation * Quat::from_rotation_x((rotation_degrees_per_second * time.delta_secs() * dir_x).to_radians());
            } 
            dbg!(settings.allow_x_rotation);
            if (settings.mode == NavigatorMode::JoyCentric) {
                let parent_global_affine = target_parent
                    .and_then(|parent| parents.get(parent.0).ok())
                    .map(|parent_tf| parent_tf.affine())
                    .unwrap_or_else(|| GlobalTransform::IDENTITY.affine());

                let joystick_pivot = parent_global_affine.inverse().transform_point3(joy_world_position);
                target_tf.translation = joystick_pivot + rotation * (target_tf.translation - joystick_pivot);
            }
            
            target_tf.rotation = rotation * target_tf.rotation;
   
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
