use std::io::Write;

use bevy::prelude::Entity;
use teph_macro::{serde_enum_framework, TSerialize};

use super::ReplicatedComponentID;
use crate::replication::ReplicatedAssetID;
use crate::transcript::{
    common::{byte_deserialize, byte_serialize},
    deserialize,
    macros::raw_item_helper,
    TDeserialize, TSerialize,
};

/// An instruction where an entity has been added
#[derive(Debug, TSerialize)]
pub struct EntityAdded {
    pub entity: Entity,
}

/// An instruction where an entity has been removed
#[derive(Debug, TSerialize)]
pub struct EntityRemoved {
    pub entity: Entity,
}

/// An instruction where a component has been added to an entity.
///
/// NOTE: You must write/read the component bundle with the ReplicatedComponent
/// set of encoders and decoders RIGHT AFTER.
#[derive(Debug)]
pub struct ComponentAdded {
    pub entity: Entity,
    // then parse out the component!
}

impl TSerialize for ComponentAdded {
    fn serialize(&self, w: &mut impl Write) {
        self.entity.serialize(w);
    }
}

impl TDeserialize for ComponentAdded {
    fn deserialize(r: &mut impl std::io::Read) -> Self {
        Self {
            entity: deserialize(r),
        }
    }
}

/// An instruction that a component should be removed.
#[derive(Debug, TSerialize)]
pub struct ComponentRemoved {
    pub entity: Entity,
    pub component: ReplicatedComponentID,
}

/// An instruction to replicate an asset.
///
/// NOTE: You must read/write the AssetEnum bundle DIRECTLY AFTER
#[derive(Debug)]
pub struct ReplicateAsset; // then parse out the asset!

impl TSerialize for ReplicateAsset {
    fn serialize(&self, _: &mut impl Write) {}
}

impl TDeserialize for ReplicateAsset {
    fn deserialize(_: &mut impl std::io::Read) -> Self {
        Self
    }
}

/// An instruction to remove an asset
#[derive(Debug, TSerialize)]
pub struct DropAsset {
    pub id: ReplicatedAssetID,
}

/// An instruction to change hierarchy of an entity.
///
/// This is a special instruction as the components under the hood are protected
/// and we can support deltas better this way
#[derive(Debug)]
pub struct HierarchyChange{
    pub new_parent: Option<Entity>,
    pub child: Entity,
}
raw_item_helper!(HierarchyChange);

/// An instruction to stop parsing instructions.
///
/// THIS MUST BE THE FINAL INSTRUCTION. Failure to have this one means you
/// will be reading off into uninitialized memory in the transcript!
#[derive(Debug)]
pub struct EndFrame;

impl TSerialize for EndFrame {
    fn serialize(&self, _: &mut impl Write) {}
}

impl TDeserialize for EndFrame {
    fn deserialize(_: &mut impl std::io::Read) -> Self {
        Self
    }
}

// Build the read/write machinery
serde_enum_framework!(
    Instruction,
    EntityAdded,
    EntityRemoved,
    ComponentAdded,
    ComponentRemoved,
    ReplicateAsset,
    DropAsset,
    HierarchyChange,
    EndFrame
);

impl TSerialize for Entity {
    fn serialize(&self, w: &mut impl std::io::Write) {
        self.to_bits().serialize(w);
    }
}

impl TDeserialize for Entity {
    fn deserialize(r: &mut impl std::io::Read) -> Self {
        Entity::from_bits(deserialize(r))
    }
}
