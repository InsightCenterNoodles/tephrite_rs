use bevy::{ecs::system::entity_command::observe, platform::collections::HashSet, prelude::*};

/// Marker for the entity that represents a user's controller
#[derive(Component, Debug)]
pub struct Interactor;

/// A raw event for joystick button messages
#[derive(Message, Debug)]
pub struct ButtonEvent {
    /// The joystick this event came from
    pub from: Entity,
    pub kind: ButtonEventKind,
}

#[derive(Debug)]
pub enum ButtonEventKind {
    ButtonPressed(u8),
    ButtonReleased(u8),
    AnalogActive(u8, f32),
}

/// Can be Activated (clicked)
#[derive(Debug, Clone, PartialEq, Component, Default)]
pub struct CanActivate {
    button_down_map: HashSet<(Entity, u8)>,
}

/// Notification sent by input system that an Entity has been Activated (clicked)
#[derive(Debug, Clone, Copy, PartialEq, EntityEvent)]
pub struct Activate {
    pub entity: Entity, // kind for right/left?
}

pub(crate) struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        todo!()
    }
}

fn read_events(mut reader: MessageReader<ButtonEvent>) {
    for event in reader.read() {
        //
    }
}
