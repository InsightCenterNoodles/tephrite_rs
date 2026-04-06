use bevy::prelude::*;

use crate::input::common::map_point;

use super::*;

// TODO: make mappable
// Lets split this up. the raw ints are from VRPN. we will take those events, and translate to user facing API

#[derive(Debug, Clone, Copy)]
pub enum JoystickType {
    Left,
    Right,
    DPad,
}

#[derive(Debug, Default, Clone, Copy)]
pub enum JoystickAxis {
    LeftX,
    LeftY,
    RightX,
    RightY,
    DPad,
    #[default]
    Unknown,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
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
    #[default]
    Unknown,
}

/// Marker for the entity that represents a user's controller
#[derive(Component, Default, Debug)]
pub struct Interactor;

#[derive(Debug, Default, Component)]
#[require(Interactor)]
pub struct InteractorState {
    pub buttons: ButtonInput<JoystickButton>,
    analogs: Vec<Option<f32>>,
}

impl InteractorState {
    fn get_axis_value(&self, axis: JoystickAxis) -> Option<f32> {
        self.analogs.get(axis as usize).cloned().flatten()
    }

    fn set_axis_value(&mut self, axis: JoystickAxis, value: Option<f32>) {
        if let Some(v) = self.analogs.get_mut(axis as usize) {
            *v = value;
        }
    }

    pub fn stick_state(&self, stick: JoystickType) -> Option<Vec2> {
        let (a, b) = match stick {
            JoystickType::Left => (JoystickAxis::LeftX, JoystickAxis::LeftY),
            JoystickType::Right => (JoystickAxis::RightX, JoystickAxis::RightY),
            JoystickType::DPad => (JoystickAxis::DPad, JoystickAxis::DPad),
        };

        // a stick is valid if either one of its axis is not null

        match (self.get_axis_value(a), self.get_axis_value(b)) {
            (None, None) => None,
            (None, Some(y)) => Some(vec2(0.0, y)),
            (Some(x), None) => Some(vec2(x, 0.0)),
            (Some(x), Some(y)) => Some(vec2(x, y)),
        }
    }
}

// The states of all interactors
//#[derive(Debug, Resource, Default)]
//pub struct AllInteractorState(EntityHashMap<InteractorState>);

//impl AllInteractorState {
//    pub fn state_for(&self, entity: Entity) -> Option<&InteractorState> {
//        self.0.get(&entity)
//    }
//}

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

fn on_joy_removal(trigger: On<Remove, Interactor>, all_activated: Query<&mut CanActivate>) {
    let e = trigger.entity;

    for mut joy in all_activated {
        joy.button_down_map.remove(&e);
    }
}

fn reset_current_states(all_buttons: Query<&mut InteractorState>) {
    for mut x in all_buttons {
        x.buttons.clear();
    }
}

fn update_current_states(
    mut button_reader: MessageReader<ButtonMessage>,
    mut axis_reader: MessageReader<AxisMessage>,
    mut states: Query<&mut InteractorState>,
) {
    for event in button_reader.read() {
        //println!("BUTTON E {event:?}");
        let Ok(mut state) = states.get_mut(event.from) else {
            continue;
        };

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
    for mut state in &mut states {
        for axis in [
            JoystickAxis::LeftX,
            JoystickAxis::LeftY,
            JoystickAxis::RightX,
            JoystickAxis::RightY,
        ] {
            if let Some(axis_value) = state.get_axis_value(axis) {
                if axis_value.abs() < 0.01 {
                    state.set_axis_value(axis, None);
                }
            }
        }
    }

    for event in axis_reader.read() {
        let Ok(mut state) = states.get_mut(event.from) else {
            continue;
        };

        let l = state.analogs.len();

        if l <= (event.axis as usize) {
            state.analogs.resize((l * 2).max(32), None);
        }
        state.analogs[event.axis as usize] = Some(event.value);
    }
}

// TODO: use something better than the global transform? itll be a frame out of date.
// TODO: we are just picking our first intersection
fn read_events(
    mut reader: MessageReader<ButtonMessage>,
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
                        .insert(interactor_button);
                }
                ButtonEventKind::ButtonReleased(interactor_button) => {
                    let was_down = active
                        .button_down_map
                        .entry(event.from)
                        .or_default()
                        .remove(&interactor_button);

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
