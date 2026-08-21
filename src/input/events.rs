//! Raw input messages and public semantic input events.
//!
//! Backends write raw messages into Bevy's message buffers. Tephrite converts
//! those into entity-targeted events when an interactor overlaps a
//! [`CanActivate`](crate::input::CanActivate) target, or global events when no
//! target handled the input.

use bevy::prelude::*;

use crate::input::InputButton;

/// Raw button message emitted by an input backend.
#[derive(Message, Debug)]
pub struct ButtonMessage {
    /// Interactor entity that produced the event.
    pub from: Entity,
    pub kind: ButtonEventKind,
}

/// Raw analog-axis message emitted by an input backend.
#[derive(Message, Debug)]
pub struct AxisMessage {
    /// Interactor entity that produced the event.
    pub from: Entity,
    /// Normalized device axis index.
    pub axis: u8,
    /// Axis value in backend-defined units.
    pub value: f32,
}

/// Raw button transition from an interactor.
#[derive(Debug)]
pub enum ButtonEventKind {
    ButtonPressed(InputButton),
    ButtonReleased(InputButton),
}

/// Semantic action understood across supported interactor types.
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
#[derive(Debug, Clone, Copy, PartialEq, EntityEvent)]
pub struct InteractorActionEvent {
    /// Target entity that received the action.
    pub entity: Entity,
    /// Interactor entity that produced the action.
    pub interactor: Entity,
    /// Press/release transition and semantic action.
    pub kind: InteractorActionEventKind,
}

/// Press/release transition for a semantic interactor action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InteractorActionEventKind {
    Pressed(InteractorAction),
    Released(InteractorAction),
}

/// Notification sent when an entity has been activated.
#[derive(Debug, Clone, Copy, PartialEq, EntityEvent)]
pub struct Activate {
    /// Target entity that was activated.
    pub entity: Entity,
    /// Raw button responsible for the activation.
    pub button: InputButton,
}

/// Notification sent when a button press was not handled by any target.
#[derive(Debug, Clone, Copy, PartialEq, Event)]
pub struct GlobalActivate {
    /// Interactor entity that produced the activation.
    pub interactor: Entity,
    /// Raw button responsible for the activation.
    pub button: InputButton,
}

/// Notification sent when a semantic action was not handled by any target.
#[derive(Debug, Clone, Copy, PartialEq, Event)]
pub struct GlobalInteractorAction {
    /// Interactor entity that produced the action.
    pub interactor: Entity,
    /// Press/release transition and semantic action.
    pub action: InteractorActionEventKind,
}
