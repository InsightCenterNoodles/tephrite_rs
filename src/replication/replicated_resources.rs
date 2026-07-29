use bevy::ecs::system::SystemState;
use bevy::light::DirectionalLightShadowMap;
use bevy::prelude::*;

use super::instruction::*;
use crate::common::{
    DeferredRendering, EnvironmentLighting, OffAxisProjectionSettings,
    OrderIndependantTransparency, ScreenSpaceAmbientOcclusionSettings,
    ScreenSpaceReflectionsSettings,
};
use crate::serialize::transcript_writer::TranscriptWriteStateResource;
use crate::serialize::*;

macro_rules! resource_helpers {
    ($world:expr, $dest:expr) => {};
    ($world:expr, $dest:expr, $T:ty $(, $rest:ty)* $(,)?) => {
        write_resource_change::<$T>($world, $dest);
        resource_helpers!($world, $dest $(, $rest)*);
    };
}

macro_rules! make_actual_enum {
    ($( $tag:expr, $T:tt ),* $(,)?) => {
        create_serialize_enum_simple!(
            ReplicatedResourceID,
            u8,
            {
                $( ($tag, $T), )*
            }
        );

        create_serialize_enum!(
            ReplicatedResource,
            u8,
            {
                $( ($tag, $T, $T), )*
            }
        );

        create_serialize_enum_write_only!(
            ReplicatedResourceRef,
            u8,
            lifetime: 'b,
            {
                $( ($tag, $T, &'b $T), )*
            }
        );

        $(
            impl<'a> From<&'a $T> for ReplicatedResourceRef<'a> {
                fn from(v: &'a $T) -> Self {
                    Self::$T(v)
                }
            }
        )*

        trait IntoResourceID {
            const IDENTIFIER: ReplicatedResourceID;
        }

        impl ReplicatedResourceID {
            pub fn remove_resource(self, commands: &mut Commands) {
                match self {
                    $( ReplicatedResourceID::$T => commands.remove_resource::<$T>(), )*
                };
            }
        }

        impl ReplicatedResource {
            pub fn add_resource(self, commands: &mut Commands) {
                match self {
                    $( ReplicatedResource::$T(x) => commands.insert_resource(x), )*
                };
            }
        }

        $(
            impl IntoResourceID for $T {
                const IDENTIFIER: ReplicatedResourceID = ReplicatedResourceID::$T;
            }
        )*
    };
}

macro_rules! detect_resource_changes {
    ($( ($id:expr, $type:tt) ),* $(,)?) => {
        make_actual_enum!( $( $id, $type ),* );

        pub(crate) fn write_changed_resources(
            world: &mut World,
            dest: &mut TranscriptWriteStateResource,
        ) {
            resource_helpers!(world, dest, $( $type ),*);
        }
    }
}

#[derive(Resource)]
struct CachedResourceState<R: Resource> {
    state: SystemState<Option<Res<'static, R>>>,
    existed: bool,
}

fn write_resource_change<R>(world: &mut World, dest: &mut TranscriptWriteStateResource)
where
    R: Resource + FastWrite + IntoResourceID,
    for<'a> &'a R: Into<ReplicatedResourceRef<'a>>,
{
    if !world.contains_resource::<CachedResourceState<R>>() {
        let state = SystemState::new(world);
        world.insert_resource(CachedResourceState::<R> {
            state,
            existed: false,
        });
    }

    world.resource_scope(|world, mut cached: Mut<CachedResourceState<R>>| {
        let resource = cached.state.get_mut(world);
        match resource {
            Ok(Some(resource)) => {
                if resource.is_changed() {
                    unsafe {
                        ServerInstruction::ResourceUpdate(ServerResourceUpdate {
                            resource: (&*resource).into(),
                        })
                        .write_fast(dest);
                    }
                }
                cached.existed = true;
            }
            Ok(None) if cached.existed => {
                cached.existed = false;
                unsafe {
                    ServerInstruction::ResourceDrop(ResourceDrop {
                        resource: R::IDENTIFIER,
                    })
                    .write_fast(dest);
                }
            }
            Ok(None) => {}
            Err(_) => {}
        }
        cached.state.apply(world);
    });
}

detect_resource_changes!(
    (0, EnvironmentLighting),
    (1, OrderIndependantTransparency),
    (2, ScreenSpaceAmbientOcclusionSettings),
    (3, ScreenSpaceReflectionsSettings),
    (4, DeferredRendering),
    (5, DirectionalLightShadowMap),
    (6, OffAxisProjectionSettings),
);
