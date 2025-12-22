use bevy::prelude::*;

use crate::input::{JoystickAxis, JoystickButton};

/// A raw event for joystick button messages
#[derive(Message, Debug)]
pub struct ButtonEvent {
    /// The joystick this event came from
    pub from: Entity,
    pub kind: ButtonEventKind,
}

/// A raw event for joystick axis messages
#[derive(Message, Debug)]
pub struct AxisEvent {
    /// The joystick this event came from
    pub from: Entity,
    pub axis: JoystickAxis,
    pub value: f32,
}

/// A button event from an Interactor
#[derive(Debug)]
pub enum ButtonEventKind {
    ButtonPressed(JoystickButton),
    ButtonReleased(JoystickButton),
}

/// Notification sent by input system that an Entity has been Activated (clicked)
#[derive(Debug, Clone, Copy, PartialEq, EntityEvent)]
pub struct Activate {
    pub entity: Entity, // kind for right/left?
    pub button: JoystickButton,
}

/// Notification sent by input system that an undirected activation event has occured
#[derive(Debug, Clone, Copy, PartialEq, Event)]
pub struct GlobalActivate {
    pub button: JoystickButton,
}
