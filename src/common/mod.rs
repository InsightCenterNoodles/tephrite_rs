use bevy::prelude::*;

/// Marker for the entity that represents the user's head
/// We only support a single head at the moment.
#[derive(Component, Debug)]
pub struct Head;

/// Add this resource to your scene to enable image based lighting.
/// We don't support multiple environments at the moment. And we don't support the bevy environment system yet.
#[derive(Debug, Resource)]
pub struct EnvironmentLighting {
    pub intensity: f32,
    pub diffuse: Handle<Image>,
    pub specular: Handle<Image>,
    pub skybox_color: Option<Color>,
}
