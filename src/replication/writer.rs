use crate::replication::components::Replicated;
use crate::serialize::transcript_writer::*;
use crate::serialize::*;
use bevy::prelude::*;

use super::instruction::*;

// ============================================================================

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
}

impl Plugin for ReplicationWriterPlugin {
    fn build(&self, app: &mut App) {
        use super::replicated_components::*;
        use super::sets::*;

        let mut transcript = TranscriptWriterResource::new(self.children_count);

        let session = transcript.prepare();

        app.insert_non_send_resource(transcript);
        app.insert_non_send_resource(session);

        // we want
        // - all asset deltas
        // - all resource deltas
        // - all adds
        // - all updates
        // - all removes
        // - do sync

        crate::replication::replicated_assets::setup_replicated_asset_systems(app);

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

/// Core replication system. Handles obtaining a fresh transcript
fn root_system(world: &mut World) {
    let state = {
        let mut dest = world
            .remove_non_send_resource::<TranscriptWriteStateResource>()
            .unwrap();

        let odest: &mut TranscriptWriteStateResource = &mut dest;

        // finish transcript
        unsafe { ServerInstruction::EFrame(EndFrame).write_fast(odest) };

        dest
    };

    // Commit all changes
    if let Some(mut core) = world.get_non_send_resource_mut::<TranscriptWriterResource>() {
        core.commit(state);

        // now get the next state
        core.prepare();
    }
}
