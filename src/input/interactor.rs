use bevy::prelude::*;

use crate::input::{common::map_point, interactor_types::InteractorTrait};

use super::*;

// TODO: make mappable
// Lets split this up. the raw ints are from VRPN. we will take those events, and translate to user facing API

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputButton {
    Button0,
    Button1,
    Button2,
    Button3,
    Button4,
    Button5,
    Button6,
    Button7,
    Button8,
    Button9,
    #[default]
    Unknown,
}

/// Marker for the entity that represents a user's controller
#[derive(Component, Default, Debug)]
pub enum Interactor {
    #[default]
    Controller,
    Flystick,
}

#[derive(Debug, Default, Component)]
#[require(Interactor)]
pub struct InteractorState {
    pub(crate) buttons: ButtonInput<InputButton>,
    pub(crate) analogs: Vec<Option<f32>>,
}

impl InteractorState {
    pub(crate) fn get_axis_value(&self, axis: usize) -> Option<f32> {
        self.analogs.get(axis as usize).cloned().flatten()
    }

    fn decay_channels(&mut self, axii: &[usize]) {
        for axis in axii {
            if let Some(v) = self.analogs.get_mut(*axis) {
                if let Some(content) = v {
                    if content.abs() < 0.01 {
                        *v = None;
                    }
                }
            }
        }
    }
}

pub struct InteractorPlugin;

impl Plugin for InteractorPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(on_joy_removal);

        app.add_systems(
            PreUpdate,
            (
                reset_current_states,
                update_current_states,
                translate_action_events,
                read_events,
            )
                .chain(),
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
    mut states: Query<(&Interactor, &mut InteractorState)>,
) {
    for event in button_reader.read() {
        //println!("BUTTON E {event:?}");
        let Ok((_, mut state)) = states.get_mut(event.from) else {
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
    for (ty, mut state) in &mut states {
        let to_decay = match ty {
            Interactor::Controller => super::interactor_types::Controller::decay(),
            Interactor::Flystick => super::interactor_types::DTrackFlystick::decay(),
        };

        state.decay_channels(to_decay);
    }

    for event in axis_reader.read() {
        let Ok((_, mut state)) = states.get_mut(event.from) else {
            continue;
        };

        let l = state.analogs.len();

        if l <= (event.axis as usize) {
            state.analogs.resize((l * 2).max(32), None);
        }
        state.analogs[event.axis as usize] = Some(event.value);
    }
}

fn translate_action_events(
    mut reader: MessageReader<ButtonMessage>,
    interactors: Query<&Interactor>,
    mut commands: Commands,
) {
    for event in reader.read() {
        let Ok(interactor) = interactors.get(event.from) else {
            continue;
        };

        let (button, pressed) = match event.kind {
            ButtonEventKind::ButtonPressed(button) => (button, true),
            ButtonEventKind::ButtonReleased(button) => (button, false),
        };

        let action = match interactor {
            Interactor::Controller => {
                super::interactor_types::Controller::action_for_button(button)
            }
            Interactor::Flystick => {
                super::interactor_types::DTrackFlystick::action_for_button(button)
            }
        };

        let Some(action) = action else {
            continue;
        };

        let kind = if pressed {
            InteractorActionEventKind::Pressed(action)
        } else {
            InteractorActionEventKind::Released(action)
        };

        commands.trigger(InteractorActionEvent {
            interactor: event.from,
            kind,
        });
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
