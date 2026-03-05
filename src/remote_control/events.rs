use bevy::prelude::*;

use crate::remote_control::common::PropertyValue;

/// Event stream emitted by the remote control server thread.
#[derive(Debug, Clone, Message)]
pub enum RemoteControlMessage {
    /// A property control changed on the webpage.
    PropertyChanged {
        /// The property handle defined in [`PropertyDefinition`].
        property: Entity,
        /// The latest value submitted by the webpage.
        value: PropertyValue,
    },
    /// The user clicked the auto-injected Quit button (or server is shutting down).
    QuitRequested,
}
