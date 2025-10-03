use crate::replication::components::Replicated;
use crate::serialize::transcript_writer::*;
use crate::serialize::*;
use bevy::prelude::*;

use super::instruction::*;

// ============================================================================

fn implicit_replication_check(
    query: Query<
        Entity,
        (
            Added<Mesh3d>,
            Added<MeshMaterial3d<StandardMaterial>>,
            Without<Replicated>,
        ),
    >,
    mut commands: Commands,
) {
    for e in query.iter() {
        println!("Adding replication to meshmat {e:?}");
        commands.entity(e).insert(Replicated);
    }
}

/// Check for any added replicated entities. We use a marker to see who we should replicate
fn added_rep_check(
    query: Query<Entity, Added<Replicated>>,
    mut writer: NonSendMut<TranscriptWriteStateResource>,
) {
    for e in query.iter() {
        //println!("EVENT NEW ENTITY {e:?}");
        let dest: &mut TranscriptWriteStateResource = &mut writer;
        unsafe { ServerInstruction::EAdd(e).write_fast(dest) };
    }
}

/// Check for any removed replicated entities.
fn removed_rep_check(
    mut removal: RemovedComponents<Replicated>,
    mut writer: NonSendMut<TranscriptWriteStateResource>,
) {
    for e in removal.read() {
        //println!("EVENT DEL ENTITY {e:?}");
        let dest: &mut TranscriptWriteStateResource = &mut writer;
        unsafe { ServerInstruction::ERemove(e).write_fast(dest) };
    }
}

/// Plugin to replicate components
pub struct ReplicationWriterPlugin {
    children_count: u32,
}

impl ReplicationWriterPlugin {
    pub fn new(children_count: u32) -> Self {
        Self { children_count }
    }

    // pub fn check(app: &mut App) {
    //     app.world()
    //         .get_non_send_resource::<TranscriptWriteStateResource>()
    //         .unwrap();
    // }
}

impl Plugin for ReplicationWriterPlugin {
    fn build(&self, app: &mut App) {
        use super::replicated_components::*;
        use super::sets::*;

        let transcript = TranscriptWriterResource::new(self.children_count);

        app.insert_non_send_resource(transcript);

        // we want
        // - all asset deltas
        // - all resource deltas
        // - all adds
        // - all updates
        // - all removes
        // - do sync

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

        crate::replication::replicated_assets::setup_replicated_asset_systems(app);

        setup_replicated_systems(app);

        app.add_systems(Startup, setup_shmem);

        app.add_systems(
            Last,
            (implicit_replication_check, added_rep_check)
                .chain()
                .in_set(EntityStartDeltaPhase),
        );
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

        //println!("Setup writer {}", std::process::id());
    }
}

// =============================================================================

/// Watch for changes to parent-child relationships and write them to the
/// transcript
fn hierarchy_change_listener(
    h_event: Query<(Entity, &ChildOf), Changed<ChildOf>>,
    mut transcript: NonSendMut<TranscriptWriteStateResource>,
) {
    for (child, parent) in h_event.iter() {
        let dest: &mut TranscriptWriteStateResource = &mut transcript;
        unsafe {
            ServerInstruction::HChange(HierarchyChange {
                new_parent: Some(parent.0),
                child,
            })
            .write_fast(dest)
        };
    }
}

fn hierarchy_remove_listener(
    mut h_event: RemovedComponents<ChildOf>,
    mut transcript: NonSendMut<TranscriptWriteStateResource>,
) {
    for child in h_event.read() {
        let dest: &mut TranscriptWriteStateResource = &mut transcript;
        unsafe {
            ServerInstruction::HChange(HierarchyChange {
                new_parent: None,
                child,
            })
            .write_fast(dest)
        };
    }
}

// =============================================================================

/// The system set that all component replication efforts belong to
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
struct FinalSyncPhase;

fn setup_shmem(world: &mut World) {
    let mut transcript = world.non_send_resource_mut::<TranscriptWriterResource>();

    let session = transcript.prepare();

    world.insert_non_send_resource(session);
}

/// Core replication system. Handles obtaining a fresh transcript
fn root_system(world: &mut World) {
    //println!("PRODUCER END FRAME");
    let state = {
        let mut dest = world
            .remove_non_send_resource::<TranscriptWriteStateResource>()
            .unwrap();

        // finish transcript
        unsafe { ServerInstruction::EFrame(EndFrame).write_fast(&mut dest) };

        dest
    };

    // Commit all changes

    if let Some(n) = world
        .get_non_send_resource_mut::<TranscriptWriterResource>()
        .map(|mut core| {
            core.commit(state);

            // now get the next state
            core.prepare()
        })
    {
        world.insert_non_send_resource(n);
    }

    //println!("PRODUCER END FRAME COMPLETE");
}
