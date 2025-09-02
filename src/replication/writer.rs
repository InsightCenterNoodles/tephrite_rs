use crate::common::Head;
use crate::replication::components::Replicated;
use crate::transcript::transcript_writer::TranscriptWriter;
use crate::transcript::{TDeserialize, TSerialize, deserialize};
use bevy::pbr::CubemapVisibleEntities;
use bevy::prelude::*;
use bevy::render::primitives::CubemapFrusta;
use teph_macro::{TSerialize, serde_enum_framework};

use super::instruction::{
    ComponentAdded, ComponentRemoved, DropAsset, EncodeInstruction, EndFrame, EntityAdded,
    EntityRemoved, HierarchyChange, ReplicateAsset,
};

/// ============================================================================

/// The system set that all component replication efforts belong to
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
struct ComponentDeltaPhase;

//#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
//struct ResourceSyncSet;

/// System set to manage asset changes
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
struct AssetDeltaPhase;

/// Builds systems to detect component changes
macro_rules! detect_component_changes_impl {
    ($app:expr) => {};

    ($app:expr, $T:ty, $( $rest:ty ),+ ) => {
        detect_component_changes_impl!($app, $T);
        detect_component_changes_impl!($app, $($rest),*);
    };

    ($app:expr, $T:ty) => {
        $app.add_systems(Last,
            (| query: Query<(Entity, &Replicated, & $T), Changed<$T>>, mut writer: NonSendMut<TranscriptWriter>| {
                for (e, _, component) in query.iter() {
                    //println!("CHANGED {e:?} {component:?}");
                    let dest: &mut TranscriptWriter = &mut writer;
                    ComponentAdded{entity: e}.encode_to(dest);
                    component.encode_to(dest);
                }
        }).in_set(ComponentDeltaPhase));

        $app.add_systems(Last,
            (|
                mut removal: RemovedComponents<$T>,
                mut writer: NonSendMut<TranscriptWriter>,
                query: Query<(Entity, &Replicated)>,
            | {
                for e in removal.read() {
                    let Ok(repli_ent) = query.get(e) else {
                        continue;
                    };

                    {
                        //println!("REMOVED {repli_ent:?} {}", stringify!($T));
                        let dest: &mut TranscriptWriter = &mut writer;

                        ComponentRemoved{ entity: repli_ent.0, component: <$T>::IDENTIFIER }.encode_to(dest);
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
    ($( $tag:ident $T:ty ),* $(,)? ) => {
        serde_enum_framework!(
            ReplicatedComponent,
            $(
                $tag,
            )*
        );

        #[derive(Debug, TSerialize, Clone)]
        pub enum ReplicatedComponentID {
            $(
                $tag,
            )*
        }

        trait IntoComponentID {
            const IDENTIFIER : ReplicatedComponentID;
        }

        pub trait ComponentUpdate: Sized + Component {
            fn update_component(self, e: Entity, commands: &mut Commands) {
                commands.entity(e).insert(self);
            }
        }
        $(
            impl ComponentUpdate for $T {}
        )*

        impl ReplicatedComponentID {
            pub fn remove_component(self, e: Entity, commands: &mut Commands) {
                match self {
                    $(
                        ReplicatedComponentID::$tag => commands.entity(e).remove::<$T>(),
                    )*
                };
            }
        }

        $(
            impl IntoComponentID for $T {
                const IDENTIFIER: ReplicatedComponentID = ReplicatedComponentID::$tag;
            }
        )*
    };
}

/// Macro to build machinery to detect changes to a list of components
macro_rules! detect_component_changes {
    ($( $type:tt ),* ) => {
        make_actual_enum!(

            $(
                $type $type
            ),*

        );
        fn setup_replicated_systems(app: &mut App) {
            detect_component_changes_impl!(app, $($type),*);
        }
    }
}

// macro_rules! detect_resource_changes {
//     ($app:expr) => {};

//     ($app:expr, $T:ty, $( $rest:ty ),+ ) => {
//         detect_resource_changes!($app, $T);
//         detect_resource_changes!($app, $($rest),*);
//     };

//     ($app:expr, $T:ty) => {
//         $app.add_systems(Last,
//                 (|to_check: Res<$T>| {
//                     if to_check.is_changed() {
//                         // logic
//                     }
//                 }).in_set(ReplicationResourceWriteSystemSet))

//     };
// }

//detect_resource_changes!()

type StandardMatComponent = MeshMaterial3d<StandardMaterial>;

detect_component_changes!(
    Head,
    Transform,
    GlobalTransform,
    Visibility,
    InheritedVisibility,
    ViewVisibility,
    PointLight,
    DirectionalLight,
    SpotLight,
    Mesh3d,
    StandardMatComponent,
    CubemapFrusta,
    CubemapVisibleEntities
);

/// The system set that all component replication efforts belong to
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
struct EntityStartDeltaPhase;

/// The system set that all component replication efforts belong to
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
struct EntityEndDeltaPhase;

/// Check for any added replicated entities. We use a marker to see who we should replicate
fn added_rep_check(
    query: Query<Entity, Added<Replicated>>,
    mut writer: NonSendMut<TranscriptWriter>,
) {
    for e in query.iter() {
        //println!("EVENT NEW ENTITY {e:?}");
        let dest: &mut TranscriptWriter = &mut writer;
        EntityAdded { entity: e }.encode_to(dest);
    }
}

/// Check for any removed replicated entities.
fn removed_rep_check(
    mut removal: RemovedComponents<Replicated>,
    mut writer: NonSendMut<TranscriptWriter>,
) {
    for e in removal.read() {
        //println!("EVENT DEL ENTITY {e:?}");
        let dest: &mut TranscriptWriter = &mut writer;
        EntityRemoved { entity: e }.encode_to(dest);
    }
}

/// Replicated asset id
#[derive(Clone, TSerialize, Debug)]
pub enum ReplicatedAssetID {
    Mesh(AssetId<Mesh>),
    StandardMaterial(AssetId<StandardMaterial>),
}

// Replicated asset

pub struct ReplicatedAssetPack<T: Asset> {
    pub id: AssetId<T>,
}

pub type ReplicatedMesh = ReplicatedAssetPack<Mesh>;
pub type ReplicatedStandardMaterial = ReplicatedAssetPack<StandardMaterial>;

serde_enum_framework!(AssetEnum, ReplicatedMesh, ReplicatedStandardMaterial);

macro_rules! make_change_detection {
    ($name:ident, $A:tt) => {
        impl TSerialize for ReplicatedAssetPack<$A> {
            fn serialize(&self, w: &mut impl std::io::Write) {
                self.id.serialize(w);
            }
        }

        impl TDeserialize for ReplicatedAssetPack<$A> {
            fn deserialize(r: &mut impl std::io::Read) -> Self {
                Self { id: deserialize(r) }
            }
        }

        fn $name(
            mut ev_asset: EventReader<AssetEvent<$A>>,
            assets: Res<Assets<$A>>,
            mut writer: NonSendMut<TranscriptWriter>,
        ) {
            //println!("Checking for deltas to {}", stringify!($A));
            for e in ev_asset.read() {
                //println!("EVENT {e:?}");
                match e {
                    AssetEvent::Added { id } => {
                        let asset = assets.get(*id).expect("obtaining changed asset");

                        let dest: &mut TranscriptWriter = &mut writer;
                        ReplicateAsset.encode_to(dest);
                        ReplicatedAssetPack::<$A> { id: *id }.encode_to(dest);
                        asset.serialize(dest);
                    }
                    AssetEvent::Modified { id } => {
                        let asset = assets.get(*id).expect("obtaining changed asset");

                        let dest: &mut TranscriptWriter = &mut writer;
                        ReplicateAsset.encode_to(dest);
                        ReplicatedAssetPack::<$A> { id: *id }.encode_to(dest);
                        asset.serialize(dest);
                    }
                    AssetEvent::Removed { id } => {
                        let dest: &mut TranscriptWriter = &mut writer;
                        DropAsset {
                            id: ReplicatedAssetID::$A(*id),
                        }
                        .encode_to(dest);
                    }
                    _ => {} // do nothing for NOW
                            //AssetEvent::Unused { id } => todo!(),
                            //AssetEvent::LoadedWithDependencies { id } => todo!(),
                }
            }
        }
    };
}

make_change_detection!(mesh_change_detection, Mesh);
make_change_detection!(material_change_detection, StandardMaterial);

/// Plugin to replicate components
pub struct ReplicationWriterPlugin {
    children_count: u32,
}

impl ReplicationWriterPlugin {
    pub fn new(children_count: u32) -> Self {
        Self { children_count }
    }
}

impl Plugin for ReplicationWriterPlugin {
    fn build(&self, app: &mut App) {
        let transcript = TranscriptWriter::new(self.children_count + 1);

        app.insert_non_send_resource(transcript);

        // we want
        // - all asset deltas
        // - all resource deltas
        // - all adds
        // - all updates
        // - all removes
        // - do sync

        app.add_systems(Last, material_change_detection.in_set(AssetDeltaPhase));
        app.add_systems(Last, mesh_change_detection.in_set(AssetDeltaPhase));

        setup_replicated_systems(app);

        app.add_systems(Last, added_rep_check.in_set(EntityStartDeltaPhase));
        app.add_systems(
            Last,
            (hierarchy_change_listener, hierarchy_remove_listener)
                .chain()
                .after(added_rep_check)
                .in_set(EntityStartDeltaPhase),
        );
        app.add_systems(Last, removed_rep_check.in_set(EntityEndDeltaPhase));

        app.add_systems(Last, root_system.in_set(FinalSyncPhase));

        //app.configure_sets(Last, ResourceSyncSet.before(ComponentDeltaPhase));
        //app.configure_sets(Last, AssetDeltaPhase.before(ResourceSyncSet));

        app.configure_sets(
            Last,
            (
                EntityStartDeltaPhase, // slight changes here otherwise events get lost?
                AssetDeltaPhase,
                ComponentDeltaPhase,
                EntityEndDeltaPhase,
                FinalSyncPhase,
            )
                .chain(),
        );
    }
}

// =============================================================================

/// Watch for changes to parent-child relationships and write them to the
/// transcript
fn hierarchy_change_listener(
    h_event: Query<(Entity, &ChildOf), Changed<ChildOf>>,
    mut transcript: NonSendMut<TranscriptWriter>,
) {
    for (child, parent) in h_event.iter() {
        let dest: &mut TranscriptWriter = &mut transcript;
        HierarchyChange {
            new_parent: Some(parent.0),
            child,
        }
        .encode_to(dest);
    }
}

fn hierarchy_remove_listener(
    mut h_event: RemovedComponents<ChildOf>,
    mut transcript: NonSendMut<TranscriptWriter>,
) {
    for child in h_event.read() {
        let dest: &mut TranscriptWriter = &mut transcript;
        HierarchyChange {
            new_parent: None,
            child,
        }
        .encode_to(dest);
    }
}

// =============================================================================

/// The system set that all component replication efforts belong to
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
struct FinalSyncPhase;

/// Core replication system. Handles obtaining a fresh transcript
fn root_system(mut transcript: NonSendMut<TranscriptWriter>) {
    {
        let dest: &mut TranscriptWriter = &mut transcript;
        // finish transcript
        EndFrame.encode_to(dest);
    }

    // join barrier so everyone can proceed
    transcript.barrier();
    // children are processing...
    transcript.barrier();

    // processing complete, clear and go to the top
    transcript.reset();
}
