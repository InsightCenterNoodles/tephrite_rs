use bevy::prelude::*;

use crate::remote_control::common::PropertyValue;

/// Event stream emitted by the remote control server thread.
#[derive(Debug, Clone, EntityEvent)]
pub struct RemoteControlEvent {
    /// The property handle defined in [`crate::remote_control::property::PropertyDefinition`].
    ///
    /// Attach an observer to this entity to handle updates for that specific property.
    pub entity: Entity,
    /// The latest value submitted by the webpage.
    pub value: PropertyValue,
}

#[derive(Debug, Clone, Event)]
pub(crate) enum RemoteControlEventInternal {
    /// A property control changed on the webpage.
    PropertyChanged {
        /// The property handle defined in [`crate::remote_control::property::PropertyDefinition`].
        property: Entity,
        /// The latest value submitted by the webpage.
        value: PropertyValue,
    },
    /// The user clicked the auto-injected Quit button (or server is shutting down).
    QuitRequested,
}
