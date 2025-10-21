use bevy::prelude::*;

use super::{components::BEntity, resources::Session, sets::ReplicateSet};

pub struct TransformPlugin;

impl Plugin for TransformPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            (update_tfs, update_parent)
                .chain()
                .in_set(ReplicateSet::InheritOnChildLink),
        );
    }
}

fn update_tfs(
    query: Query<(&BEntity, &Transform), Or<(Changed<Transform>, Added<BEntity>)>>,
    session: NonSend<Session>,
) {
    for (be, tf) in &query {
        crate::backfill::set_transform(&session.0, be.0, &tf.compute_matrix());
    }
}

fn update_parent(
    query: Query<(&BEntity, &ChildOf, Option<&Transform>), Changed<ChildOf>>,
    b_ent_check: Query<&BEntity>,
    session: NonSend<Session>,
) {
    for (be, parent, tf) in &query {
        if let Ok(parent_e) = b_ent_check.get(parent.0) {
            crate::backfill::set_parent(&session.0, be.0, parent_e.0);
            if let Some(tf) = tf {
                crate::backfill::set_transform(&session.0, be.0, &tf.compute_matrix());
            }
        }
    }
}
