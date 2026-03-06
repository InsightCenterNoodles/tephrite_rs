use bevy::prelude::*;

/// Marker for the entity that represents the user's head
/// We only support a single head at the moment.
#[derive(Component, Debug)]
pub struct Head;

/// Add this resource to your scene to enable image based lighting
#[derive(Debug, Resource)]
pub struct EnvironmentLighting {
    pub intensity: f32,
    pub equirect: Handle<Image>,
}
