//! Simulator integration for [`crate::common::Head`] entities.
//!
//! Heads are tagged with [`RemoteControlTransform`] so they get transform
//! controls from the shared remote-control use-case pipeline.

use bevy::prelude::*;

use crate::{common::Head, remote_control::use_cases::RemoteControlTransform};

pub(super) struct HeadSimulatorPlugin;

impl Plugin for HeadSimulatorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, setup_head_controls);
    }
}

#[derive(Debug, Component)]
struct HeadManaged;

/// Attach remote-control transform behavior to newly discovered head entities.
fn setup_head_controls(
    query: Query<Entity, (With<Head>, Without<HeadManaged>)>,
    mut commands: Commands,
) {
    for entity in query {
        let mut ec = commands.entity(entity);
        ec.insert(HeadManaged);

        ec.insert(RemoteControlTransform {
            position: true,
            rotation: true,
            ..Default::default()
        });
    }
}
