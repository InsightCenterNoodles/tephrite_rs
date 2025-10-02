use bevy::prelude::*;

/// The system set that all component replication efforts belong to
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ComponentDeltaPhase;

//#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
//struct ResourceSyncSet;

/// System set to manage asset changes
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct AssetDeltaPhase;

/// The system set that all component replication efforts belong to
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct EntityStartDeltaPhase;

/// The system set that all component replication efforts belong to
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct EntityEndDeltaPhase;
