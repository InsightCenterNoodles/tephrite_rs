//! Interactor state tracking and activation routing.
//!
//! Device backends send raw [`ButtonMessage`] and [`AxisMessage`] values. This
//! module stores the current button/axis state on interactor entities and routes
//! button presses to activation targets.

use bevy::prelude::*;

use super::{
    Activate, AxisMessage, ButtonEventKind, ButtonMessage, CanActivate, Controller, DTrackFlystick,
    GlobalActivate, GlobalInteractorAction, InteractionBounds, InteractorAction,
    InteractorActionEvent, InteractorActionEventKind, InteractorTrait, map_point,
};

/// Physical button identifier after device-specific input has been normalized.
///
/// Tephrite keeps this deliberately small and generic. Device-specific types
/// such as [`ControllerButton`](super::ControllerButton) and
/// [`FlystickButton`](super::FlystickButton) map into this enum before
/// higher-level semantic actions are derived.
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

/// Marker for an entity that represents a user's input device.
#[derive(Component, Default, Debug, Clone, Copy)]
pub enum Interactor {
    #[default]
    Controller,
    Flystick,
}

/// Current normalized button and analog state for an [`Interactor`].
///
/// Most application code should query this for continuous state, or observe
/// [`InteractorActionEvent`] for targeted press/release events.
#[derive(Debug, Component)]
#[require(Interactor)]
pub struct InteractorState {
    pub(crate) buttons: ButtonInput<InputButton>,
    pub(crate) analogs: Vec<Option<f32>>,

    pub(crate) translate: fn(action: InteractorAction) -> Option<InputButton>,
}

impl InteractorState {
    /// Create state storage configured for a specific interactor device kind.
    pub fn new(ty: Interactor) -> Self {
        let f = match ty {
            Interactor::Controller => Controller::button_for_action,
            Interactor::Flystick => DTrackFlystick::button_for_action,
        };
        Self {
            buttons: Default::default(),
            analogs: Default::default(),
            translate: f,
        }
    }

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

    /// Returns true if any semantic action was pressed this frame.
    pub fn any_just_pressed(&self, inputs: impl IntoIterator<Item = InteractorAction>) -> bool {
        self.buttons
            .any_just_pressed(inputs.into_iter().filter_map(self.translate))
    }

    /// Returns true if any semantic action was released this frame.
    pub fn any_just_released(&self, inputs: impl IntoIterator<Item = InteractorAction>) -> bool {
        self.buttons
            .any_just_released(inputs.into_iter().filter_map(self.translate))
    }

    /// Returns true if any semantic action is currently pressed.
    pub fn any_pressed(&self, inputs: impl IntoIterator<Item = InteractorAction>) -> bool {
        self.buttons
            .any_pressed(inputs.into_iter().filter_map(self.translate))
    }

    /// Returns true if this semantic action was pressed this frame.
    pub fn just_pressed(&self, input: InteractorAction) -> bool {
        let Some(input_button) = (self.translate)(input) else {
            return false;
        };
        self.buttons.just_pressed(input_button)
    }

    /// Returns true if this semantic action was released this frame.
    pub fn just_released(&self, input: InteractorAction) -> bool {
        let Some(input_button) = (self.translate)(input) else {
            return false;
        };
        self.buttons.just_released(input_button)
    }

    /// Returns true if this semantic action is currently pressed.
    pub fn pressed(&self, input: InteractorAction) -> bool {
        let Some(input_button) = (self.translate)(input) else {
            return false;
        };
        self.buttons.pressed(input_button)
    }
}

impl Default for InteractorState {
    fn default() -> Self {
        Self::new(Interactor::default())
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
    joy_query: Query<(&Interactor, &GlobalTransform)>,
    mut commands: Commands,
) {
    //let mut handled = false;

    'events: for event in reader.read() {
        let Ok((interactor, joy_tf)) = joy_query.get(event.from) else {
            continue;
        };

        // for now, the origin of the interactor is our activation point

        let activation_point = Vec3::ZERO;

        for (this_entity, bounds, tf, mut active) in root_query.iter_mut() {
            if !active.enable {
                continue;
            }

            // map to our local

            let local = map_point(activation_point, joy_tf, tf);

            if !bounds.contains_point(local) {
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

                    if let Some(action) = action_for_button(interactor, interactor_button) {
                        commands.trigger(InteractorActionEvent {
                            entity: this_entity,
                            interactor: event.from,
                            kind: InteractorActionEventKind::Pressed(action),
                        });
                    }
                    continue 'events;
                }
                ButtonEventKind::ButtonReleased(interactor_button) => {
                    let was_down = active
                        .button_down_map
                        .entry(event.from)
                        .or_default()
                        .remove(&interactor_button);

                    if was_down {
                        if let Some(action) = action_for_button(interactor, interactor_button) {
                            commands.trigger(InteractorActionEvent {
                                entity: this_entity,
                                interactor: event.from,
                                kind: InteractorActionEventKind::Released(action),
                            });
                        }

                        commands.trigger(Activate {
                            entity: this_entity,
                            button: interactor_button,
                        });

                        continue 'events;
                    }

                    continue 'events;
                }
            }
        }

        // No target. Send unbounded event

        match event.kind {
            ButtonEventKind::ButtonPressed(interactor_button) => {
                commands.trigger(GlobalActivate {
                    interactor: event.from,
                    button: interactor_button,
                });

                if let Some(action) = action_for_button(interactor, interactor_button) {
                    commands.trigger(GlobalInteractorAction {
                        interactor: event.from,
                        action: InteractorActionEventKind::Pressed(action),
                    });
                }
            }
            ButtonEventKind::ButtonReleased(interactor_button) => {
                if let Some(action) = action_for_button(interactor, interactor_button) {
                    commands.trigger(GlobalInteractorAction {
                        interactor: event.from,
                        action: InteractorActionEventKind::Released(action),
                    });
                }
            }
        }
    }
}

fn action_for_button(interactor: &Interactor, button: InputButton) -> Option<InteractorAction> {
    match interactor {
        Interactor::Controller => super::interactor_types::Controller::action_for_button(button),
        Interactor::Flystick => super::interactor_types::DTrackFlystick::action_for_button(button),
    }
}

#[cfg(test)]
mod tests {
    use bevy::math::bounding::Aabb3d;

    use super::*;

    #[derive(Debug, Default, Resource)]
    struct ActionLog(Vec<(Entity, Entity, InteractorActionEventKind)>);

    #[derive(Debug, Default, Resource)]
    struct GlobalActionLog(Vec<GlobalInteractorAction>);

    #[test]
    fn targeted_entities_receive_interactor_action_events() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(crate::input::InputPlugin);
        app.init_resource::<ActionLog>();
        app.init_resource::<GlobalActionLog>();
        app.add_observer(
            |trigger: On<GlobalInteractorAction>, mut log: ResMut<GlobalActionLog>| {
                log.0.push(*trigger.event());
            },
        );

        let interactor = app
            .world_mut()
            .spawn((
                Interactor::Controller,
                InteractorState::new(Interactor::Controller),
                GlobalTransform::IDENTITY,
            ))
            .id();

        let bounds = Aabb3d::from_point_cloud(
            Isometry3d::default(),
            [Vec3::splat(-1.0), Vec3::splat(1.0)].into_iter(),
        );

        let target = app
            .world_mut()
            .spawn((
                InteractionBounds::aabb(bounds),
                CanActivate::default(),
                GlobalTransform::IDENTITY,
            ))
            .observe(
                |trigger: On<InteractorActionEvent>, mut log: ResMut<ActionLog>| {
                    let event = trigger.event();
                    log.0.push((event.entity, event.interactor, event.kind));
                },
            )
            .id();

        app.world_mut()
            .resource_mut::<Messages<ButtonMessage>>()
            .write(ButtonMessage {
                from: interactor,
                kind: ButtonEventKind::ButtonPressed(InputButton::Button1),
            });
        app.update();

        app.world_mut()
            .resource_mut::<Messages<ButtonMessage>>()
            .write(ButtonMessage {
                from: interactor,
                kind: ButtonEventKind::ButtonReleased(InputButton::Button1),
            });
        app.update();

        assert_eq!(
            app.world().resource::<ActionLog>().0,
            vec![
                (
                    target,
                    interactor,
                    InteractorActionEventKind::Pressed(InteractorAction::Primary),
                ),
                (
                    target,
                    interactor,
                    InteractorActionEventKind::Released(InteractorAction::Primary),
                ),
            ]
        );
        assert!(app.world().resource::<GlobalActionLog>().0.is_empty());
    }
}
