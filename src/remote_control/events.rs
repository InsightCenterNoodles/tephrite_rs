use bevy::prelude::*;

use crate::remote_control::common::PropertyValue;

/// Event stream emitted by the remote control server thread.
#[derive(Debug, Clone, EntityEvent)]
pub struct RemoteControlEvent {
    /// The property handle defined in [`crate::remote_control::property::PropertyDefinition`].
    ///
    /// Attach an observer to this entity to handle updates for that specific property.
    pub entity: Entity,
    /// Secondary per-entity discriminator from the matching property definition.
    pub aspect_id: u32,
    /// The latest value submitted by the webpage.
    pub value: PropertyValue,
}

#[derive(Debug, Clone, Event)]
pub(crate) enum RemoteControlEventInternal {
    /// A property control changed on the webpage.
    PropertyChanged {
        /// The property handle defined in [`crate::remote_control::property::PropertyDefinition`].
        property: Entity,
        /// Secondary per-entity discriminator from the matching property definition.
        aspect_id: u32,
        /// The latest value submitted by the webpage.
        value: PropertyValue,
    },
    /// The user clicked the auto-injected Quit button (or server is shutting down).
    QuitRequested,
}
