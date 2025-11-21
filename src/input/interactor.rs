use bevy::ecs::entity::EntityHashMap;
use bevy::prelude::*;

use crate::input::common::map_point;

use super::*;

// TODO: make mappable
const LEFT_STICK_X: u8 = 0;
const LEFT_STICK_Y: u8 = 1;
const RIGHT_STICK_X: u8 = 2;
const RIGHT_STICK_Y: u8 = 5;
const D_PAD: u8 = 8;

const X: u8 = 0;
const A: u8 = 1;
const B: u8 = 2;
const Y: u8 = 3;
const BL: u8 = 4;
const BR: u8 = 5;
const TL: u8 = 6;
const TR: u8 = 7;
const BACK: u8 = 8;
const START: u8 = 9;

pub enum Joystick {
    Left,
    Right,
    DPad,
}

pub enum JoystickButton {
    X,
    Y,
    A,
    B,
    BL,
    BR,
    TL,
    TR,
    Back,
    Start,
}

/// Marker for the entity that represents a user's controller
#[derive(Component, Debug)]
pub struct Interactor;

/// A button on an interactor
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InteractorButton(pub(crate) u8);

#[derive(Debug, Default)]
pub struct InteractorState {
    buttons: ButtonInput<InteractorButton>,
    analogs: Vec<f32>,
}

impl InteractorState {
    fn get_axis_value(&self, axis: u8) -> f32 {
        self.analogs.get(axis as usize).cloned().unwrap_or_default()
    }

    pub fn stick_state(&self, stick: Joystick) -> Option<Vec2> {
        let (a, b) = match stick {
            Joystick::Left => (LEFT_STICK_X, LEFT_STICK_Y),
            Joystick::Right => (RIGHT_STICK_X, RIGHT_STICK_Y),
            Joystick::DPad => (D_PAD, D_PAD),
        };

        let ret = vec2(self.get_axis_value(a), self.get_axis_value(b));

        if ret.length() > 0.075 {
            Some(ret)
        } else {
            None
        }
    }

    // TODO BAD API
    pub fn button(&self, button: JoystickButton) -> bool {
        let index = match button {
            JoystickButton::X => X,
            JoystickButton::Y => Y,
            JoystickButton::A => A,
            JoystickButton::B => B,
            JoystickButton::BL => BL,
            JoystickButton::BR => BR,
            JoystickButton::TL => TL,
            JoystickButton::TR => TR,
            JoystickButton::Back => BACK,
            JoystickButton::Start => START,
        };

        self.buttons.pressed(InteractorButton(index))
    }
}

/// The states of all interactors
#[derive(Debug, Resource, Default)]
pub struct AllInteractorState(EntityHashMap<InteractorState>);

impl AllInteractorState {
    pub fn state_for(&self, entity: Entity) -> Option<&InteractorState> {
        self.0.get(&entity)
    }
}

pub struct InteractorPlugin;

impl Plugin for InteractorPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(on_joy_removal);

        app.add_systems(
            PreUpdate,
            (reset_current_states, update_current_states, read_events).chain(),
        );
    }
}

fn on_joy_removal(
    trigger: On<Remove, Interactor>,
    mut state: ResMut<AllInteractorState>,
    all_interactables: Query<&mut CanActivate>,
) {
    let e = trigger.entity;

    state.0.remove(&e);

    for mut joy in all_interactables {
        joy.button_down_map.remove(&e);
    }
}

fn reset_current_states(mut state: ResMut<AllInteractorState>) {
    for x in state.bypass_change_detection().0.values_mut() {
        x.buttons.clear();
    }
}

fn update_current_states(
    mut button_reader: MessageReader<ButtonEvent>,
    mut axis_reader: MessageReader<AxisEvent>,
    mut state: ResMut<AllInteractorState>,
) {
    for event in button_reader.read() {
        println!("BUTTON E {event:?}");
        let state = state.0.entry(event.from).or_default();

        match event.kind {
            ButtonEventKind::ButtonPressed(interactor_button) => {
                state.buttons.press(interactor_button)
            }
            ButtonEventKind::ButtonReleased(interactor_button) => {
                state.buttons.release(interactor_button)
            }
        }
    }

    // add decay factor
    for state in state.0.values_mut() {
        for a in &mut state.analogs {
            *a = *a * 0.9;

            if *a < 0.01 {
                *a = 0.0;
            }
        }
    }

    for event in axis_reader.read() {
        let state = state.0.entry(event.from).or_default();
        //debug!("Read event {} {}", state.analogs.len(), event.axis);

        if state.analogs.len() <= event.axis.into() {
            debug!("Axis revise");
            state.analogs.resize((state.analogs.len() * 2).max(32), 0.0);
        }
        state.analogs[event.axis as usize] = event.value;
    }
}

// TODO: use something better than the global transform? itll be a frame out of date.
// TODO: we are just picking our first intersection
fn read_events(
    mut reader: MessageReader<ButtonEvent>,
    mut root_query: Query<(
        Entity,
        &InteractionBounds,
        &GlobalTransform,
        &mut CanActivate,
    )>,
    joy_query: Query<&GlobalTransform, With<Interactor>>,
    mut commands: Commands,
) {
    //let mut handled = false;

    for event in reader.read() {
        let Ok(joy_tf) = joy_query.get(event.from) else {
            continue;
        };

        // for now, the origin of the interactor is our activation point

        let activation_point = Vec3::ZERO;

        for (this_entity, bounds, tf, mut active) in root_query.iter_mut() {
            // map to our local

            let local = map_point(activation_point, joy_tf, tf);

            let local = Aabb3d::from_point_cloud(Isometry3d::default(), std::iter::once(local));

            // local in bounds?

            if !bounds.aabb.contains(&local) {
                continue;
            }

            // in bounds

            match event.kind {
                ButtonEventKind::ButtonPressed(interactor_button) => {
                    active
                        .button_down_map
                        .entry(event.from)
                        .or_default()
                        .insert(interactor_button.0);
                }
                ButtonEventKind::ButtonReleased(interactor_button) => {
                    let was_down = active
                        .button_down_map
                        .entry(event.from)
                        .or_default()
                        .remove(&interactor_button.0);

                    if was_down {
                        // emit action
                        commands.trigger(Activate {
                            entity: this_entity,
                            button: interactor_button,
                        });

                        return;
                    }
                }
            }
        }

        // No target. Send unbounded event

        match event.kind {
            ButtonEventKind::ButtonPressed(interactor_button) => {
                commands.trigger(GlobalActivate {
                    button: interactor_button,
                });
            }
            ButtonEventKind::ButtonReleased(_interactor_button) => {}
        }
    }
}
