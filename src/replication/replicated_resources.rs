use super::instruction::*;
use super::sets::*;
use bevy::prelude::*;

use crate::common::{
    DeferredRendering, EnvironmentLighting, OrderIndependantTransparency,
    ScreenSpaceAmbientOcclusionSettings, ScreenSpaceReflectionsSettings,
};

use crate::serialize::transcript_writer::*;
use crate::serialize::*;

/// Builds systems to detect resource changes
macro_rules! detect_resource_changes_impl {
    ($app:expr) => {};

    ($app:expr, $T:ty, $( $rest:ty ),+ ) => {
        detect_resource_changes_impl!($app, $T);
        detect_resource_changes_impl!($app, $($rest),*);
    };

    ($app:expr, $T:tt) => {
        $app.add_systems(Last,
            (
                |
                    query: Option<Res<$T>>,
                    mut writer: NonSendMut<TranscriptWriteStateResource>
                |
                {
                    let dest: &mut TranscriptWriteStateResource = &mut writer;
                    let Some(resource) = &query else {
                        return;
                    };

                    if !resource.is_changed() {
                        return;
                    }

                    let resource : & $T = resource;

                    unsafe {
                        ServerInstruction::ResourceUpdate(
                            ServerResourceUpdate {
                                resource: resource.into()
                            }
                        ).write_fast(dest);
                    }
                }
            ).in_set(ResourceSyncSet)
        );

        $app.add_systems(Last,
            (
                |
                    query: Option<Res<$T>>,
                    mut writer: NonSendMut<TranscriptWriteStateResource>,
                    mut res_existed: Local<bool>,
                |
                {

                    if query.is_some() {
                        // the resource exists!
                        *res_existed = true;

                    } else if *res_existed {
                        // the resource does not exist, but we remember it existed!
                        // (it was removed)

                        // forget about it!
                        *res_existed = false;

                        let dest: &mut TranscriptWriteStateResource = &mut writer;

                        unsafe {
                            ServerInstruction::ResourceDrop(
                                ResourceDrop{ resource: <$T>::IDENTIFIER }
                            ).write_fast(dest);
                        }
                    }


                }
            ).in_set(ResourceSyncSet)

        );
    };
}

/// Builds a lot of replecation machinery for components
macro_rules! make_actual_enum {
    // Match on a repeating pattern of (ident: type)
    ($( $tag:expr, $T:tt ),* $(,)? ) => {

        create_serialize_enum_simple!(
            ReplicatedResourceID,
            u8,
            {
                $(
                    ($tag, $T),
                )*
            }
        );

        create_serialize_enum!(
            ReplicatedResource,
            u8,
            {
                $(
                    ($tag, $T, $T),
                )*
            }
        );

        create_serialize_enum_write_only!(
            ReplicatedResourceRef,
            u8,
            lifetime: 'b,
            {
                $(
                    ($tag, $T, &'b $T),
                )*
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
            const IDENTIFIER : ReplicatedResourceID;
        }

        impl ReplicatedResourceID {
            pub fn remove_resource(self, commands: &mut Commands) {
                match self {
                    $(
                        ReplicatedResourceID::$T => commands.remove_resource::<$T>(),
                    )*
                };
            }
        }

        impl ReplicatedResource {
            pub fn add_resource(self, commands: &mut Commands) {
                match self {
                    $(
                        ReplicatedResource::$T(x) => commands.insert_resource(x),
                    )*
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

/// Macro to build machinery to detect changes to a list of components
macro_rules! detect_resource_changes {
    ($( ($id:expr, $type:tt) ),* ) => {
        make_actual_enum!(

            $(
                $id, $type
            ),*

        );
        pub(crate) fn setup_replicated_resource_systems(app: &mut App) {
            detect_resource_changes_impl!(app, $($type),*);
        }
    }
}

detect_resource_changes!(
    (0, EnvironmentLighting),
    (1, OrderIndependantTransparency),
    (2, ScreenSpaceAmbientOcclusionSettings),
    (3, ScreenSpaceReflectionsSettings),
    (4, DeferredRendering)
);
