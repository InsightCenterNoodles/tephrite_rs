use bevy::light::DirectionalLightShadowMap;

use crate::common::{
    DeferredRendering, EnvironmentLighting, OffAxisProjectionSettings,
    OrderIndependantTransparency, ScreenSpaceAmbientOcclusionSettings,
    ScreenSpaceReflectionsSettings,
};
use crate::replication::registry::ReplicationRegistry;

pub(crate) fn register_builtin_resources(registry: &mut ReplicationRegistry) {
    registry
        .register_resource::<EnvironmentLighting>()
        .register_resource::<OrderIndependantTransparency>()
        .register_resource::<ScreenSpaceAmbientOcclusionSettings>()
        .register_resource::<ScreenSpaceReflectionsSettings>()
        .register_resource::<DeferredRendering>()
        .register_resource::<DirectionalLightShadowMap>()
        .register_resource::<OffAxisProjectionSettings>();
}
