use bevy::prelude::*;

use crate::prelude::Replicated;

/// Marker for the entity that represents the user's head
/// We only support a single head at the moment.
#[derive(Component, Debug)]
pub struct Head;

/// Describes the entity that is the viewpoint for the simulator view.
#[derive(Component, Debug)]
#[require(Replicated)]
pub struct SimulatorCamera3d;

/// Add this resource to your scene to enable image based lighting
#[derive(Debug, Resource)]
pub struct EnvironmentLighting {
    pub intensity: f32,
    pub equirect: Handle<Image>,
}
