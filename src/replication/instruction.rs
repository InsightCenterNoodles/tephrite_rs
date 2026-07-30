use bevy::prelude::{Asset, AssetId, Entity};

use crate::replication::registry::TableId;
use crate::serialize::*;

pub(crate) const INSTRUCTION_COMPONENT_ADD: u8 = 0;
pub(crate) const INSTRUCTION_COMPONENT_REMOVE: u8 = 1;
pub(crate) const INSTRUCTION_ASSET_UPDATE: u8 = 2;
pub(crate) const INSTRUCTION_ASSET_DROP: u8 = 3;
pub(crate) const INSTRUCTION_RESOURCE_UPDATE: u8 = 4;
pub(crate) const INSTRUCTION_RESOURCE_DROP: u8 = 5;
pub(crate) const INSTRUCTION_HIERARCHY_CHANGE: u8 = 6;
pub(crate) const INSTRUCTION_ENTITY_REMOVE: u8 = 7;
pub(crate) const INSTRUCTION_END_FRAME: u8 = 8;
pub(crate) const INSTRUCTION_ENTITY_ADD: u8 = 9;

#[inline(always)]
pub(crate) unsafe fn write_component_add<T: FastWrite>(
    w: &mut impl ByteSink,
    entity: Entity,
    component_type: TableId,
    component: &T,
) {
    unsafe {
        INSTRUCTION_COMPONENT_ADD.write_fast(w);
        entity.write_fast(w);
        component_type.write_fast(w);
        component.write_fast(w);
    }
}

#[inline(always)]
pub(crate) unsafe fn write_component_remove(
    w: &mut impl ByteSink,
    entity: Entity,
    component_type: TableId,
) {
    unsafe {
        INSTRUCTION_COMPONENT_REMOVE.write_fast(w);
        entity.write_fast(w);
        component_type.write_fast(w);
    }
}

#[inline(always)]
pub(crate) unsafe fn write_asset_update<A: Asset + FastWrite>(
    w: &mut impl ByteSink,
    asset_type: TableId,
    id: AssetId<A>,
    asset: &A,
) {
    unsafe {
        INSTRUCTION_ASSET_UPDATE.write_fast(w);
        asset_type.write_fast(w);
        id.write_fast(w);
        asset.write_fast(w);
    }
}

#[inline(always)]
pub(crate) unsafe fn write_asset_drop<A: Asset>(
    w: &mut impl ByteSink,
    asset_type: TableId,
    id: AssetId<A>,
) {
    unsafe {
        INSTRUCTION_ASSET_DROP.write_fast(w);
        asset_type.write_fast(w);
        id.write_fast(w);
    }
}

#[inline(always)]
pub(crate) unsafe fn write_resource_update<R: FastWrite>(
    w: &mut impl ByteSink,
    resource_type: TableId,
    resource: &R,
) {
    unsafe {
        INSTRUCTION_RESOURCE_UPDATE.write_fast(w);
        resource_type.write_fast(w);
        resource.write_fast(w);
    }
}

#[inline(always)]
pub(crate) unsafe fn write_resource_drop(w: &mut impl ByteSink, resource_type: TableId) {
    unsafe {
        INSTRUCTION_RESOURCE_DROP.write_fast(w);
        resource_type.write_fast(w);
    }
}

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

#[derive(Debug)]
pub(crate) enum ServerInstruction {
    HChange(HierarchyChange),
    ERemove(Entity),
    EFrame(EndFrame),
    EAdd(Entity),
}

impl FastWrite for ServerInstruction {
    #[inline(always)]
    unsafe fn write_fast(&self, w: &mut impl ByteSink) {
        unsafe {
            match self {
                Self::HChange(x) => {
                    INSTRUCTION_HIERARCHY_CHANGE.write_fast(w);
                    x.write_fast(w);
                }
                Self::ERemove(x) => {
                    INSTRUCTION_ENTITY_REMOVE.write_fast(w);
                    x.write_fast(w);
                }
                Self::EFrame(x) => {
                    INSTRUCTION_END_FRAME.write_fast(w);
                    x.write_fast(w);
                }
                Self::EAdd(x) => {
                    INSTRUCTION_ENTITY_ADD.write_fast(w);
                    x.write_fast(w);
                }
            }
        }
    }
}

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
