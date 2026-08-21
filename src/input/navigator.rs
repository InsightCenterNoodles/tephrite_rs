//! Scene navigation helpers driven by an [`Interactor`](crate::input::Interactor).
//!
//! Add [`NavigatorMarker`] to the entity that should move in response to the
//! active interactor. [`NavigationPlugin`] supports a conventional controller
//! mapping and a DTrack flystick mapping.

use std::f32::consts::{PI, TAU};

use bevy::{math::DAffine3, prelude::*};

use crate::input::{
    Interactor, InteractorState,
    interactor_types::{
        Controller, ControllerButton, ControllerStick, DTrackFlystick, FlystickButton,
        FlystickStick, InteractorTrait,
    },
};

/// Navigation behavior for rotations around the interactor.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum NavigatorMode {
    /// Rotate the navigated object around its own origin.
    #[default]
    ObjectCentric,
    /// Rotate the navigated object around the interactor position.
    JoyCentric,
}

/// Marks an entity as the navigation target.
#[derive(Debug, Default, Clone, Component)]
pub struct NavigatorMarker;

/// Initial transform applied to navigation targets at startup and reset.
#[derive(Debug, Clone, Copy, Resource)]
pub struct InitialNavigatorTransform(pub Transform);

impl Default for InitialNavigatorTransform {
    fn default() -> Self {
        Self(Transform::IDENTITY)
    }
}

#[derive(Debug, Clone, Resource, Default)]
struct NavigatorSettings {
    mode: NavigatorMode,
    allow_x_rotation: bool,
}

/// Per-interactor transient state used by flystick navigation gestures.
#[derive(Debug, Default, Component)]
pub struct InteractorNavigatorState {
    last_yaw: Option<f32>,
    last_height: Option<f32>,
}

#[derive(Debug, Clone, Copy)]
enum FlystickNavigationOperation {
    None,
    Reset,
    RotateY(f32),
    VerticalDisplace(Vec3),
    Scale(f32),
    Pan(Vec3),
}

/// Plugin that updates [`NavigatorMarker`] transforms from interactor input.
#[derive(Debug)]
pub struct NavigationPlugin {
    settings: NavigatorSettings,
}

impl NavigationPlugin {
    /// Create a navigation plugin with the requested navigation mode.
    pub fn new(mode: NavigatorMode) -> Self {
        Self {
            settings: NavigatorSettings {
                mode,
                allow_x_rotation: true,
            },
        }
    }

    /// Enable or disable controller-driven X-axis rotation.
    pub fn with_x_rotation(mut self, allow: bool) -> Self {
        self.settings.allow_x_rotation = allow;
        self
    }
}

impl Plugin for NavigationPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.settings.clone());
        app.init_resource::<InitialNavigatorTransform>();
        app.add_systems(PostStartup, apply_initial_navigator_transform);
        app.add_systems(PreUpdate, initialize_interactor_navigator_state);
        app.add_systems(Update, initialize_added_navigators);
        app.add_systems(Update, on_tick);
    }
}

fn apply_initial_navigator_transform(
    initial: Res<InitialNavigatorTransform>,
    mut targets: Query<&mut Transform, With<NavigatorMarker>>,
) {
    for mut target_tf in &mut targets {
        *target_tf = initial.0;
    }
}

fn initialize_added_navigators(
    initial: Res<InitialNavigatorTransform>,
    mut targets: Query<&mut Transform, Added<NavigatorMarker>>,
) {
    for mut target_tf in &mut targets {
        *target_tf = initial.0;
    }
}

fn initialize_interactor_navigator_state(
    mut commands: Commands,
    interactors: Query<Entity, (With<Interactor>, Without<InteractorNavigatorState>)>,
) {
    for entity in &interactors {
        commands
            .entity(entity)
            .insert(InteractorNavigatorState::default());
    }
}

fn on_tick(
    mut target: Query<
        (&mut Transform, Option<&ChildOf>),
        (With<NavigatorMarker>, Without<Interactor>),
    >,
    mut joystick: Query<(
        &Interactor,
        &GlobalTransform,
        &InteractorState,
        &mut InteractorNavigatorState,
    )>,
    parents: Query<&GlobalTransform>,
    settings: Res<NavigatorSettings>,
    initial: Res<InitialNavigatorTransform>,
    time: Res<Time>,
) {
    // TODO: will fail if we add more interactors
    let Ok((interactor, joystick_global_tf, state, mut navigator_state)) = joystick.single_mut()
    else {
        warn_once!("Zero or multiple interactors detected, navigation system disabled.");
        return;
    };

    match interactor {
        Interactor::Controller => {
            for (mut target_tf, target_parent) in &mut target {
                on_tick_controller(
                    &mut target_tf,
                    joystick_global_tf,
                    state,
                    target_parent.and_then(|x| parents.get(x.0).ok()),
                    &settings,
                    &initial,
                    &time,
                );
            }
        }
        Interactor::Flystick => {
            let operation = flystick_navigation_operation(
                joystick_global_tf,
                state,
                &time,
                &mut navigator_state,
            );

            for (mut target_tf, target_parent) in &mut target {
                apply_flystick_navigation(
                    &mut target_tf,
                    target_parent.and_then(|x| parents.get(x.0).ok()),
                    &initial,
                    operation,
                );
            }
        }
    }
}

fn apply_flystick_navigation(
    target_tf: &mut Transform,
    parent_global_tf: Option<&GlobalTransform>,
    initial: &InitialNavigatorTransform,
    operation: FlystickNavigationOperation,
) {
    match operation {
        FlystickNavigationOperation::None => {}
        FlystickNavigationOperation::Reset => {
            *target_tf = initial.0;
        }
        FlystickNavigationOperation::RotateY(delta_yaw) => {
            target_tf.rotation = Quat::from_rotation_y(delta_yaw) * target_tf.rotation;
        }
        FlystickNavigationOperation::VerticalDisplace(global_displace)
        | FlystickNavigationOperation::Pan(global_displace) => {
            target_tf.translation += local_displace(global_displace, parent_global_tf);
        }
        FlystickNavigationOperation::Scale(scale_factor) => {
            target_tf.scale =
                (target_tf.scale * scale_factor).clamp(Vec3::splat(0.001), Vec3::splat(1000.0));
        }
    }
}

fn on_tick_controller(
    target_tf: &mut Transform,
    interactor_global_tf: &GlobalTransform,
    interactor_state: &InteractorState,
    parent_global_tf: Option<&GlobalTransform>,
    settings: &NavigatorSettings,
    initial: &InitialNavigatorTransform,
    time: &Time,
) {
    let speed_meters_per_second = 2.0;
    let rotation_degrees_per_second = 40.0;

    let joy_world_position = interactor_global_tf.transform_point(Vec3::ZERO);

    if let Some(right_stick) = Controller::stick_state(ControllerStick::Right, interactor_state) {
        // direction vector

        let dir = vec3(right_stick.x, 0.0, right_stick.y);
        let mut global_dir = interactor_global_tf.affine().transform_vector3(dir);

        // flatten that y!
        global_dir.y = 0.0;

        let global_displace = global_dir * speed_meters_per_second * time.delta_secs();

        // map to the local of the entity

        let parent_global_affine = parent_global_tf
            .map(|parent_tf| parent_tf.affine())
            .unwrap_or_else(|| GlobalTransform::IDENTITY.affine());

        let local_displace = parent_global_affine
            .inverse()
            .transform_vector3(global_displace);

        target_tf.translation += local_displace;
    }

    if let Some(left_stick) = Controller::stick_state(ControllerStick::Left, interactor_state) {
        target_tf.translation += vec3(
            0.0,
            time.delta_secs() * speed_meters_per_second * left_stick.y,
            0.0,
        )
    }

    if let Some(dpad) = Controller::stick_state(ControllerStick::DPad, interactor_state) {
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

        let mut rotation = Quat::from_rotation_y(
            (rotation_degrees_per_second * time.delta_secs() * dir_y).to_radians(),
        );

        if settings.allow_x_rotation {
            rotation = rotation
                * Quat::from_rotation_x(
                    (rotation_degrees_per_second * time.delta_secs() * dir_x).to_radians(),
                );
        }
        //dbg!(settings.allow_x_rotation);
        if settings.mode == NavigatorMode::JoyCentric {
            let parent_global_affine: DAffine3 = parent_global_tf
                .map(|parent_tf| parent_tf.affine())
                .unwrap_or_default()
                .as_daffine3();

            let joystick_pivot = parent_global_affine
                .inverse()
                .transform_point3(joy_world_position.into());

            let tf = joystick_pivot
                + rotation.as_dquat() * (target_tf.translation.as_dvec3() - joystick_pivot);

            target_tf.translation = tf.as_vec3();
        }

        target_tf.rotation = rotation * target_tf.rotation;
    }

    const SCALE_FACTOR: f32 = 1.01;

    if Controller::pressed(ControllerButton::BL, interactor_state) {
        target_tf.scale =
            (target_tf.scale / SCALE_FACTOR).clamp(Vec3::splat(0.001), Vec3::splat(1000.0));
    }

    if Controller::pressed(ControllerButton::BR, interactor_state) {
        target_tf.scale =
            (target_tf.scale * SCALE_FACTOR).clamp(Vec3::splat(0.001), Vec3::splat(1000.0));
    }

    if Controller::just_pressed(ControllerButton::Start, interactor_state) {
        *target_tf = initial.0;
    }
}

fn flystick_navigation_operation(
    interactor_global_tf: &GlobalTransform,
    interactor_state: &InteractorState,
    time: &Time,
    navigator_state: &mut InteractorNavigatorState,
) -> FlystickNavigationOperation {
    let speed_meters_per_second = 2.0;

    if DTrackFlystick::just_pressed(FlystickButton::JoystickButton, interactor_state) {
        navigator_state.last_yaw = None;
        navigator_state.last_height = None;
        return FlystickNavigationOperation::Reset;
    }

    if DTrackFlystick::pressed(FlystickButton::RightWhiteButton, interactor_state) {
        navigator_state.last_height = None;

        let yaw = yaw_from_global_transform(interactor_global_tf);
        let operation = navigator_state
            .last_yaw
            .map(|last_yaw| FlystickNavigationOperation::RotateY(wrap_angle(yaw - last_yaw)))
            .unwrap_or(FlystickNavigationOperation::None);

        navigator_state.last_yaw = Some(yaw);
        return operation;
    }

    navigator_state.last_yaw = None;

    if DTrackFlystick::pressed(FlystickButton::LeftWhiteButton, interactor_state) {
        let height = interactor_global_tf.translation().y;
        let operation = if let Some(last_height) = navigator_state.last_height {
            let delta_height = height - last_height;

            if DTrackFlystick::pressed(FlystickButton::Trigger, interactor_state) {
                FlystickNavigationOperation::Scale(2.0_f32.powf(delta_height))
            } else {
                FlystickNavigationOperation::VerticalDisplace(Vec3::Y * delta_height)
            }
        } else {
            FlystickNavigationOperation::None
        };

        navigator_state.last_height = Some(height);
        return operation;
    }

    navigator_state.last_height = None;

    let Some(stick) = DTrackFlystick::stick_state(FlystickStick::Stick, interactor_state) else {
        return FlystickNavigationOperation::None;
    };

    let dir = vec3(stick.x, 0.0, -stick.y);
    let mut global_dir = interactor_global_tf.affine().transform_vector3(dir);
    global_dir.y = 0.0;

    FlystickNavigationOperation::Pan(global_dir * speed_meters_per_second * time.delta_secs())
}

fn local_displace(global_displace: Vec3, parent_global_tf: Option<&GlobalTransform>) -> Vec3 {
    let parent_global_affine = parent_global_tf
        .map(|parent_tf| parent_tf.affine())
        .unwrap_or_else(|| GlobalTransform::IDENTITY.affine());

    parent_global_affine
        .inverse()
        .transform_vector3(global_displace)
}

fn yaw_from_global_transform(transform: &GlobalTransform) -> f32 {
    let mut forward = transform.affine().transform_vector3(Vec3::Z);
    forward.y = 0.0;

    if forward.length_squared() == 0.0 {
        return 0.0;
    }

    forward.x.atan2(forward.z)
}

fn wrap_angle(angle: f32) -> f32 {
    (angle + PI).rem_euclid(TAU) - PI
}
