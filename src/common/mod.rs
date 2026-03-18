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

/// Add this resource to enable the use of order independant transparency. This is useful for rendering
/// transparent objects and can eliminate flickering. Comes at a high memory cost.
#[derive(Debug, Resource)]
pub struct OrderIndependantTransparency {
    pub layer_count: i32,
    pub alpha_threshold: f32,
}

impl Default for OrderIndependantTransparency {
    fn default() -> Self {
        Self {
            layer_count: 8,
            alpha_threshold: 0.0,
        }
    }
}

#[derive(Debug, Event)]
pub(crate) struct TephExit;
