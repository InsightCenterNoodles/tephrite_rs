use bevy::{
    camera::primitives::Aabb,
    ecs::{entity::EntityHashMap, system::entity_command::observe},
    platform::collections::HashSet,
    prelude::*,
    render::render_resource::ShaderType,
};

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

/// The states of all interactors
#[derive(Debug, Resource, Default)]
pub struct AllInteractorState(EntityHashMap<InteractorState>);

/// A raw event for joystick button messages
#[derive(Message, Debug)]
pub struct ButtonEvent {
    /// The joystick this event came from
    pub from: Entity,
    pub kind: ButtonEventKind,
}

/// A button event from an Interactor
#[derive(Debug)]
pub enum ButtonEventKind {
    ButtonPressed(InteractorButton),
    ButtonReleased(InteractorButton),
    AnalogActive(u8, f32),
}

/// Can be Activated (clicked)
#[derive(Debug, Clone, PartialEq, Component, Default)]
pub struct CanActivate {
    button_down_map: HashSet<(Entity, u8)>,
}

/// The bounding box of an interactor, events inside this box will be channeled to the host entity
#[derive(Debug, Component)]
pub struct InteractionBounds {
    aabb: Aabb,
}

/// Notification sent by input system that an Entity has been Activated (clicked)
#[derive(Debug, Clone, Copy, PartialEq, EntityEvent)]
pub struct Activate {
    pub entity: Entity, // kind for right/left?
}

pub(crate) struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PreUpdate,
            (reset_current_states, update_current_states).chain(),
        );
    }
}

fn reset_current_states(mut state: ResMut<AllInteractorState>) {
    for x in state.bypass_change_detection().0.values_mut() {
        x.buttons.clear();
    }
}

fn update_current_states(
    mut reader: MessageReader<ButtonEvent>,
    mut state: ResMut<AllInteractorState>,
) {
    for event in reader.read() {
        let state = state.0.entry(event.from).or_default();

        match event.kind {
            ButtonEventKind::ButtonPressed(interactor_button) => {
                state.buttons.press(interactor_button)
            }
            ButtonEventKind::ButtonReleased(interactor_button) => {
                state.buttons.release(interactor_button)
            }
            ButtonEventKind::AnalogActive(id, val) => {
                if state.analogs.len() <= id.into() {
                    state.analogs.resize(state.analogs.len() * 2, 0.0);
                }
                state.analogs[id as usize] = val;
            }
        }
    }
}

fn read_events(mut reader: MessageReader<ButtonEvent>) {
    if reader.is_empty() {
        return;
    }

    for event in reader.read() {
        //
    }
}

fn recursive_input() {}
