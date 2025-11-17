pub(crate) use super::components::Replicated;
use super::instruction::*;
use super::sets::*;
use bevy::prelude::*;

use crate::common::Head;

use crate::serialize::transcript_writer::*;
use crate::serialize::*;

/// Builds systems to detect component changes
macro_rules! detect_component_changes_impl {
    ($app:expr) => {};

    ($app:expr, $T:ty, $( $rest:ty ),+ ) => {
        detect_component_changes_impl!($app, $T);
        detect_component_changes_impl!($app, $($rest),*);
    };

    ($app:expr, $T:tt) => {
        $app.add_systems(Last,
            (| query: Query<(Entity, &Replicated, & $T), Changed<$T>>, mut writer: NonSendMut<TranscriptWriteStateResource>| {
                for (e, _, component) in query.iter() {
                    // println!("CHANGED {e:?} {component:?}");
                    let dest: &mut TranscriptWriteStateResource = &mut writer;
                    let component: & $T = component;

                    unsafe {
                            ServerInstruction::CAdd(
                            ServerComponentAdded {
                                entity: e,
                                component: component.into()
                            }
                        ).write_fast(dest);
                    }
                }
        }).in_set(ComponentDeltaPhase));

        $app.add_systems(Last,
            (|
                mut removal: RemovedComponents<$T>,
                mut writer: NonSendMut<TranscriptWriteStateResource>,
                query: Query<(Entity, &Replicated)>,
            | {
                for e in removal.read() {
                    let Ok(repli_ent) = query.get(e) else {
                        continue;
                    };

                    {
                        //println!("REMOVED {repli_ent:?} {}", stringify!($T));
                        let dest: &mut TranscriptWriteStateResource = &mut writer;

                        unsafe {
                            ServerInstruction::CRemove(
                                ServerComponentRemoved{ entity: repli_ent.0, component: <$T>::IDENTIFIER }
                            ).write_fast(dest);
                        }
                    }

                    //writer.add(Instruction::Removed(repli_ent.0, <$T>::IDENTIFIER))
                }
            }).in_set(ComponentDeltaPhase)
        );
    };
}

/// Builds a lot of replecation machinery for components
macro_rules! make_actual_enum {
    // Match on a repeating pattern of (ident: type)
    ($( $tag:expr, $T:tt ),* $(,)? ) => {

        create_serialize_enum_simple!(
            ReplicatedComponentID,
            u8,
            {
                $(
                    ($tag, $T),
                )*
            }
        );

        create_serialize_enum!(
            ReplicatedComponent,
            u8,
            {
                $(
                    ($tag, $T, $T),
                )*
            }
        );

        create_serialize_enum_write_only!(
            ReplicatedComponentRef,
            u8,
            lifetime: 'b,
            {
                $(
                    ($tag, $T, &'b $T),
                )*
            }
        );

        $(
            impl<'a> From<&'a $T> for ReplicatedComponentRef<'a> {
                fn from(v: &'a $T) -> Self {
                    Self::$T(v)
                }
            }
        )*

        trait IntoComponentID {
            const IDENTIFIER : ReplicatedComponentID;
        }

        impl ReplicatedComponentID {
            pub fn remove_component(self, e: Entity, commands: &mut Commands) {
                match self {
                    $(
                        ReplicatedComponentID::$T => commands.entity(e).remove::<$T>(),
                    )*
                };
            }
        }

        impl ReplicatedComponent {
            pub fn add_component(self, e: Entity, commands: &mut Commands) {
                match self {
                    $(
                        ReplicatedComponent::$T(x) => commands.entity(e).insert(x),
                    )*
                };
            }
        }

        $(
            impl IntoComponentID for $T {
                const IDENTIFIER: ReplicatedComponentID = ReplicatedComponentID::$T;
            }
        )*
    };
}

/// Macro to build machinery to detect changes to a list of components
macro_rules! detect_component_changes {
    ($( ($id:expr, $type:tt) ),* ) => {
        make_actual_enum!(

            $(
                $id, $type
            ),*

        );
        pub(crate) fn setup_replicated_systems(app: &mut App) {
            detect_component_changes_impl!(app, $($type),*);
        }
    }
}

type StandardMatComponent = MeshMaterial3d<StandardMaterial>;

detect_component_changes!(
    (0, Head),
    (1, Transform),
    (2, Visibility),
    (3, PointLight),
    (4, DirectionalLight),
    (5, SpotLight),
    (6, Mesh3d),
    (7, StandardMatComponent)
);
