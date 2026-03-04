use std::fmt::Debug;

use bevy::prelude::Entity;

use crate::replication::replicated_assets::{AssetEnum, AssetEnumRef};
use crate::replication::replicated_components::{ReplicatedComponent, ReplicatedComponentRef};
use crate::replication::replicated_resources::{
    ReplicatedResource, ReplicatedResourceID, ReplicatedResourceRef,
};
use crate::replication::{
    replicated_assets::ReplicatedAssetID, replicated_components::ReplicatedComponentID,
};
use crate::serialize::*;

// MARK: Server side

/// An instruction where a component has been added to an entity.
#[derive(Debug)]
pub(crate) struct ServerComponentAdded<'a> {
    pub entity: Entity,
    pub component: ReplicatedComponentRef<'a>,
}

impl<'a> FastWrite for ServerComponentAdded<'a> {
    unsafe fn write_fast(&self, w: &mut impl ByteSink) {
        unsafe {
            self.entity.write_fast(w);
            self.component.write_fast(w);
        }
    }
}

/// An instruction that a component should be removed.
#[derive(Debug)]
pub(crate) struct ServerComponentRemoved {
    pub entity: Entity,
    pub component: ReplicatedComponentID,
}

impl FastWrite for ServerComponentRemoved {
    unsafe fn write_fast(&self, w: &mut impl ByteSink) {
        unsafe {
            self.entity.write_fast(w);
            self.component.write_fast(w);
        }
    }
}

/// An instruction where a resource should be updated
#[derive(Debug)]
pub(crate) struct ServerResourceUpdate<'a> {
    pub resource: ReplicatedResourceRef<'a>,
}

impl<'a> FastWrite for ServerResourceUpdate<'a> {
    unsafe fn write_fast(&self, w: &mut impl ByteSink) {
        unsafe {
            self.resource.write_fast(w);
        }
    }
}

/// An instruction that a resource should be removed.
#[derive(Debug)]
pub(crate) struct ResourceDrop {
    pub resource: ReplicatedResourceID,
}

impl FastWrite for ResourceDrop {
    unsafe fn write_fast(&self, w: &mut impl ByteSink) {
        unsafe {
            self.resource.write_fast(w);
        }
    }
}

impl FastRead for ResourceDrop {
    type Ret = Self;

    unsafe fn read_fast<'a, S: ByteSource<'a>>(r: &mut S) -> Self::Ret {
        unsafe {
            Self {
                resource: ReplicatedResourceID::read_fast(r),
            }
        }
    }
}

/// An instruction to replicate an asset.
#[derive(Debug)]
pub(crate) struct ServerReplicateAsset<'a> {
    pub asset: AssetEnumRef<'a>,
}

impl_fast_serialize_write_only!(ServerReplicateAsset<'a>,
    lifetime: 'a,
    keep: {
        asset
    },
    skip: {
    }
);

/// An instruction to reserve an asset handle
#[derive(Debug)]
pub(crate) struct ReserveAsset {
    pub id: ReplicatedAssetID,
}

impl_fast_serialize!(ReserveAsset,
    keep: {
        id
    },
    skip: {
    }
);

/// An instruction to remove an asset
#[derive(Debug)]
pub(crate) struct DropAsset {
    pub id: ReplicatedAssetID,
}

impl_fast_serialize!(DropAsset,
    keep: {
        id
    },
    skip: {
    }
);

/// An instruction to change hierarchy of an entity.
///
/// This is a special instruction as the components under the hood are protected
/// and we can support deltas better this way
#[derive(Debug)]
pub(crate) struct HierarchyChange {
    pub new_parent: Option<Entity>,
    pub child: Entity,
}
impl_fast_raw_item!(HierarchyChange);

/// An instruction to stop parsing instructions for this frame.
///
/// THIS MUST BE THE FINAL INSTRUCTION FOR A FRAME. Failure to have this one means you
/// will be reading off into uninitialized memory in the transcript!
#[derive(Debug)]
pub struct EndFrame;

impl FastWrite for EndFrame {
    #[inline(always)]
    unsafe fn write_fast(&self, _w: &mut impl ByteSink) {}
}
impl FastRead for EndFrame {
    type Ret = Self;
    #[inline(always)]
    unsafe fn read_fast<'a, S: ByteSource<'a>>(_r: &mut S) -> Self {
        Self
    }
}

/// An instruction to halt and shutdown.
#[derive(Debug)]
pub struct Halt;

impl FastWrite for Halt {
    #[inline(always)]
    unsafe fn write_fast(&self, _w: &mut impl ByteSink) {}
}
impl FastRead for Halt {
    type Ret = Self;
    #[inline(always)]
    unsafe fn read_fast<'a, S: ByteSource<'a>>(_r: &mut S) -> Self {
        Self
    }
}

// Build the read/write machinery
create_serialize_enum_write_only!(
    ServerInstruction,
    u8,
    lifetime: 'a,
    {
        (0, EAdd, Entity),
        (1, ERemove, Entity),
        (2, CAdd, ServerComponentAdded<'a>),
        (3, CRemove, ServerComponentRemoved),
        (4, CAsset, ServerReplicateAsset<'a>),
        (5, CDropAsset, DropAsset),
        (6, CPrepAsset, ReserveAsset),
        (7, ResourceUpdate, ServerResourceUpdate<'a>),
        (8, ResourceDrop, ResourceDrop),
        (9, HChange, HierarchyChange),
        (10, EFrame, EndFrame),
    }
);

impl FastWrite for Entity {
    #[inline(always)]
    unsafe fn write_fast(&self, w: &mut impl ByteSink) {
        unsafe { self.to_bits().write_fast(w) };
    }
}
impl FastRead for Entity {
    type Ret = Self;
    #[inline(always)]
    unsafe fn read_fast<'a, S: ByteSource<'a>>(r: &mut S) -> Self {
        Entity::from_bits(read_fast(r))
    }
}

// ==================================================================
// MARK: Client side

/// An instruction where a component has been added to an entity.
#[derive(Debug)]
pub(crate) struct ClientComponentAdded {
    pub entity: Entity,
    pub component: ReplicatedComponent,
}

impl_fast_serialize!(ClientComponentAdded,
    keep: {
        entity,
        component
    },
    skip: {
    }
);

/// An instruction that a component should be removed.
#[derive(Debug)]
pub(crate) struct ClientComponentRemoved {
    pub entity: Entity,
    pub component: ReplicatedComponentID,
}

impl_fast_serialize!(ClientComponentRemoved,
    keep: {
        entity,
        component
    },
    skip: {
    }
);

/// An instruction where a resource has been added.
#[derive(Debug)]
pub(crate) struct ClientResourceUpdate {
    pub resouce: ReplicatedResource,
}

impl_fast_serialize!(ClientResourceUpdate,
    keep: {
        resouce
    },
    skip: {
    }
);

/// An instruction to replicate an asset.
#[derive(Debug)]
pub(crate) struct ClientReplicateAsset {
    pub asset: Box<AssetEnum>,
}

impl_fast_serialize!(ClientReplicateAsset,
    keep: {
        asset
    },
    skip: {
    }
);

// Build the read/write machinery
create_serialize_enum!(
    ClientInstruction,
    u8,
    {
        (0, EAdd, Entity),
        (1, ERemove, Entity),
        (2, CAdd, ClientComponentAdded),
        (3, CRemove, ClientComponentRemoved),
        (4, CAsset, ClientReplicateAsset),
        (5, CDropAsset, DropAsset),
        (6, CPrepAsset, ReserveAsset),
        (7, ResourceUpdate, ClientResourceUpdate),
        (8, ResourceDrop, ResourceDrop),
        (9, HChange, HierarchyChange),
        (10, EFrame, EndFrame),
    }
);
