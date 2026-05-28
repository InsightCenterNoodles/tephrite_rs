use crate::{
    common::{
        DeferredRendering, EnvironmentLighting, OrderIndependantTransparency,
        ScreenSpaceAmbientOcclusionSettings, ScreenSpaceReflectionsSettings,
    },
    serialize::*,
};

use bevy::pbr::ScreenSpaceAmbientOcclusionQualityLevel;

impl_fast_serialize!(
    EnvironmentLighting,
    keep: {
        intensity, diffuse, specular, skybox_color
    },
    skip: {

    }
);

impl_fast_raw_item!(OrderIndependantTransparency);
impl_fast_raw_item!(DeferredRendering);

impl_fast_raw_item!(ScreenSpaceAmbientOcclusionQualityLevel);
impl_fast_raw_item!(ScreenSpaceAmbientOcclusionSettings);
impl_fast_raw_item!(ScreenSpaceReflectionsSettings);
