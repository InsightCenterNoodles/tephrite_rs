use bevy::light::cascade::CascadeShadowConfig;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;

use crate::common::Head;
use crate::prelude::{InstanceMeshMaterial3d, Instances, PointsMaterial};
use crate::replication::registry::ReplicationRegistry;

type StandardMatComponent = MeshMaterial3d<StandardMaterial>;
type PointsMatComponent = MeshMaterial3d<PointsMaterial>;
type RLayers = bevy::camera::visibility::RenderLayers;

pub(crate) fn register_builtin_components(registry: &mut ReplicationRegistry) {
    registry
        .register_component::<Head>()
        .register_component::<Transform>()
        .register_component::<Visibility>()
        .register_component::<PointLight>()
        .register_component::<DirectionalLight>()
        .register_component::<SpotLight>()
        .register_component::<Mesh3d>()
        .register_component::<StandardMatComponent>()
        .register_component::<PointsMatComponent>()
        .register_component::<InstanceMeshMaterial3d>()
        .register_component::<Gizmo>()
        .register_component::<Instances>()
        .register_component::<InheritedVisibility>()
        .register_component::<NotShadowCaster>()
        .register_component::<NotShadowReceiver>()
        .register_component::<CascadeShadowConfig>()
        .register_component::<TextColor>()
        .register_component::<TextFont>()
        .register_component::<TextLayout>()
        .register_component::<TextSpan>()
        .register_component::<RLayers>();
}
