use bevy::prelude::*;

use crate::input::InputButton;

/// A raw event for joystick button messages
#[derive(Message, Debug)]
pub struct ButtonMessage {
    /// The joystick this event came from
    pub from: Entity,
    pub kind: ButtonEventKind,
}

/// A raw event for joystick axis messages
#[derive(Message, Debug)]
pub struct AxisMessage {
    /// The joystick this event came from
    pub from: Entity,
    pub axis: u8,
    pub value: f32,
}

/// A button event from an Interactor
#[derive(Debug)]
pub enum ButtonEventKind {
    ButtonPressed(InputButton),
    ButtonReleased(InputButton),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InteractorAction {
    Primary,
    Secondary,
    Menu,
    ResetView,
    Previous,
    Next,
}

/// A semantic action event from an [`Interactor`](crate::input::Interactor).
#[derive(Debug, Clone, Copy, PartialEq, Event)]
pub struct InteractorActionEvent {
    pub interactor: Entity,
    pub kind: InteractorActionEventKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InteractorActionEventKind {
    Pressed(InteractorAction),
    Released(InteractorAction),
}

/// Notification sent by input system that an Entity has been Activated (clicked)
#[derive(Debug, Clone, Copy, PartialEq, EntityEvent)]
pub struct Activate {
    pub entity: Entity, // kind for right/left?
    pub button: InputButton,
}

/// Notification sent by input system that an undirected activation event has occured
#[derive(Debug, Clone, Copy, PartialEq, Event)]
pub struct GlobalActivate {
    pub button: InputButton,
}
