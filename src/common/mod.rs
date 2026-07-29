use std::ops::Range;

use bevy::pbr::ScreenSpaceAmbientOcclusionQualityLevel;
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
    pub sorted_fragment_max_count: u32,
    pub fragments_per_pixel_average: f32,
    pub alpha_threshold: f32,
}

impl Default for OrderIndependantTransparency {
    fn default() -> Self {
        Self {
            sorted_fragment_max_count: 8,
            fragments_per_pixel_average: 4.0,
            alpha_threshold: 0.0,
        }
    }
}

/// Add this resource to make opaque materials use Bevy's deferred renderer by default.
///
/// This is required by some render effects, such as screen space reflections, but it has a
/// meaningful performance and compatibility cost, so Tephrite keeps it opt-in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource)]
pub struct DeferredRendering;

/// Add this resource to adjust the near/far clipping distances used by Tephrite's off-axis
/// projection cameras.
#[derive(Debug, Clone, Copy, PartialEq, Resource)]
pub struct OffAxisProjectionSettings {
    pub near: f32,
    pub far: f32,
}

impl Default for OffAxisProjectionSettings {
    fn default() -> Self {
        Self {
            near: 0.01,
            far: 100.0,
        }
    }
}

/// Add this resource to enable screen space ambient occlusion on the render camera.
/// This is a camera-attached Bevy component represented as a resource because Tephrite owns
/// the replicated render camera.
#[derive(Debug, Clone, Copy, PartialEq, Resource)]
pub struct ScreenSpaceAmbientOcclusionSettings {
    pub quality_level: ScreenSpaceAmbientOcclusionQualityLevel,
    pub constant_object_thickness: f32,
}

impl Default for ScreenSpaceAmbientOcclusionSettings {
    fn default() -> Self {
        Self {
            quality_level: ScreenSpaceAmbientOcclusionQualityLevel::Medium,
            constant_object_thickness: 0.25,
        }
    }
}

/// Add this resource to enable screen space reflections on the render camera.
/// This is a camera-attached Bevy component represented as a resource because Tephrite owns
/// the replicated render camera.
#[derive(Debug, Clone, PartialEq, Resource)]
pub struct ScreenSpaceReflectionsSettings {
    pub min_perceptual_roughness: Range<f32>,
    pub max_perceptual_roughness: Range<f32>,
    pub thickness: f32,
    pub linear_steps: u32,
    pub linear_march_exponent: f32,
    pub edge_fadeout: Range<f32>,
    pub bisection_steps: u32,
    pub use_secant: bool,
}

impl Default for ScreenSpaceReflectionsSettings {
    fn default() -> Self {
        Self {
            min_perceptual_roughness: 0.08..0.12,
            max_perceptual_roughness: 0.55..0.6,
            linear_steps: 10,
            bisection_steps: 5,
            use_secant: true,
            thickness: 0.25,
            linear_march_exponent: 1.0,
            edge_fadeout: 0.0..0.0,
        }
    }
}

#[derive(Debug, Event)]
pub(crate) struct TephExit;
