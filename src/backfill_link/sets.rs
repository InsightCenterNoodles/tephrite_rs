use bevy::prelude::*;

/// Orderable sets that multiple plugins can target.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum ReplicateSet {
    Entity,
    /// React to Added<BReplicate>, propagate to descendants, and assign BEntity.
    Propagate,
    /// React to Added<ChildOf> edges so new children inherit BReplicate/BEntity.
    InheritOnChildLink,
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum RenderableSet {
    /// Detect entities that are (now) renderable (mesh+material) and/or whose asset *handles* changed.
    Detect,
    /// Refresh / (re)build the binding when either handles or asset *data* changed.
    Refresh,
    /// Remove binding when prerequisites disappear (lost mesh/material/BEntity).
    Cleanup,
}

pub struct PipelineOrderPlugin;

impl Plugin for PipelineOrderPlugin {
    fn build(&self, app: &mut App) {
        // All of this runs inside the standard `Update` schedule.
        // Chain guarantees strict A → B → C ordering within the same schedule tick.
        app.configure_sets(
            PostUpdate,
            (
                ReplicateSet::Entity,
                // Replication first: BEntity must exist before renderable logic tries to attach.
                ReplicateSet::Propagate,
                ReplicateSet::InheritOnChildLink,
                // Then renderability passes.
                RenderableSet::Detect,
                RenderableSet::Refresh,
                // And finally cleanup.
                RenderableSet::Cleanup,
            )
                .chain(),
        );
    }
}
