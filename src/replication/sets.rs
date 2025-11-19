use bevy::prelude::*;

/// The system set that all component replication efforts belong to
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ComponentDeltaPhase;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResourceSyncSet;

/// System set to manage asset changes
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum AssetDeltaPhase {
    Priority0,
    Priority1,
    Priority2,
}

/// The system set that all component replication efforts belong to
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct EntityStartDeltaPhase;

/// The system set that all component replication efforts belong to
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct EntityEndDeltaPhase;
