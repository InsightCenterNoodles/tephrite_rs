use bevy::prelude::*;

/// Marker for the entity that represents the user's head
/// We only support a single head at the moment.
#[derive(Component, Debug)]
pub struct Head;
