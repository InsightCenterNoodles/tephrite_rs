use bevy::prelude::*;

use crate::input::{AllInteractorState, Interactor, common::map_point};

use super::Joystick;

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

//TODO: can we merge the interactor marker, or get some extra info in there so we dont have to pull AllInteractorState

fn on_tick(
    mut target: Query<
        (&mut Transform, &GlobalTransform),
        (With<NavigatorMarker>, Without<Interactor>),
    >,
    mut joystick: Query<(Entity, &mut Transform, &GlobalTransform), With<Interactor>>,
    mut states: Res<AllInteractorState>,
    settings: Res<NavigatorSettings>,
    time: Res<Time>,
) {
    let Ok((mut target_tf, target_global_tf)) = target.single_mut() else {
        return;
    };

    // will fail if we add more interactors
    let Ok((joy_e, joystick_tf, joystick_global_tf)) = joystick.single_mut() else {
        return;
    };

    let Some(state) = states.state_for(joy_e) else {
        return;
    };

    let speed_meters_per_second = 2.0;
    let rotation_degrees_per_second = 40.0;

    let joy_world_position = joystick_global_tf.transform_point(Vec3::ZERO);

    if let Some(right_stick) = state.stick_state(Joystick::Right) {
        // direction vector

        let dir = vec3(right_stick.x, 0.0, right_stick.y);
        let mut global_dir = joystick_global_tf.affine().transform_vector3(dir);

        // flatten that y!
        global_dir.y = 0.0;

        let global_displace = global_dir * speed_meters_per_second * time.delta_secs();

        // map to the local of the entity

        let local_displace = target_global_tf
            .affine()
            .inverse()
            .transform_vector3(global_displace);

        target_tf.translation += local_displace;
    }

    if let Some(left_stick) = state.stick_state(Joystick::Left) {
        target_tf.translation += vec3(
            0.0,
            time.delta_secs() * speed_meters_per_second * left_stick.y,
            0.0,
        )
    }

    if let Some(dpad) = state.stick_state(Joystick::DPad) {
        let degrees = dpad.x;

        // left is 270 ish and right is 90 ish

        let dir = match degrees {
            255.0..285.0 => -1.0,
            75.0..105.0 => 1.0,
            _ => 0.0,
        };

        //rotation_degrees_per_second *
    }
}
