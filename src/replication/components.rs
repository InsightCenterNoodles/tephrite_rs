use bevy::prelude::Component;

/// Marker for entities that are tracked for replication.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub(crate) struct IsReplicated;
