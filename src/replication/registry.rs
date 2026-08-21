//! Runtime tables for mirrored ECS data.
//!
//! The replication protocol sends compact table IDs, not Rust type names or
//! enum tags. These tables provide the mapping from a `u16` table ID to the
//! concrete functions that know how to write, skip, apply, and remove a
//! particular component, asset, or resource type.
//!
//! The tables are intentionally built outside the transcript. `tephrite_rs::run`
//! applies the same [`crate::TephriteAppConfig`] to the logic and render apps,
//! so both processes register the same types in the same order. That deterministic
//! insertion order is the protocol contract.

use std::{any::TypeId, fmt::Debug};

use bevy::ecs::{entity::EntityHashSet, system::SystemState};
use bevy::prelude::*;

use crate::replication::components::IsReplicated;
use crate::serialize::transcript_writer::TranscriptWriteStateResource;
use crate::serialize::{ByteReader, ContextFromWorld, FastRead, FastWrite, RemappableAsset};

/// Compact identifier used on the wire for component, asset, and resource kinds.
///
/// IDs are assigned by registration order within each table. They are local to
/// their table: component `1`, asset `1`, and resource `1` are unrelated.
pub(crate) type TableId = u16;

pub(crate) type CollectComponentEntitiesFn = fn(&mut World, &mut Vec<Entity>);
pub(crate) type WriteComponentBaselineFn =
    fn(&World, &mut TranscriptWriteStateResource, Entity, TableId);
pub(crate) type WriteComponentChangesFn =
    fn(&mut World, &mut TranscriptWriteStateResource, &EntityHashSet, TableId);
pub(crate) type WriteComponentRemovalsFn =
    fn(&mut World, &mut TranscriptWriteStateResource, &EntityHashSet, TableId);
pub(crate) type ApplyComponentFn = for<'a> fn(Entity, &mut World, &mut ByteReader<'a>);
pub(crate) type SkipComponentFn = for<'a> fn(&mut World, &mut ByteReader<'a>);
pub(crate) type RemoveComponentFn = fn(Entity, &mut World);

pub(crate) type WriteAssetChangesFn = fn(&mut World, &mut TranscriptWriteStateResource, TableId);
pub(crate) type ApplyAssetFn = for<'a> fn(&mut World, &mut ByteReader<'a>);
pub(crate) type DropAssetFn = for<'a> fn(&mut World, &mut ByteReader<'a>);

pub(crate) type WriteResourceChangeFn = fn(&mut World, &mut TranscriptWriteStateResource, TableId);
pub(crate) type ApplyResourceFn = for<'a> fn(&mut World, &mut ByteReader<'a>);
pub(crate) type DropResourceFn = fn(&mut World);

#[derive(Clone)]
pub(crate) struct ComponentTableEntry {
    pub(crate) id: TableId,
    pub(crate) name: &'static str,
    pub(crate) collect_entities: CollectComponentEntitiesFn,
    pub(crate) write_baseline: WriteComponentBaselineFn,
    pub(crate) write_changes: WriteComponentChangesFn,
    pub(crate) write_removals: WriteComponentRemovalsFn,
    pub(crate) apply: ApplyComponentFn,
    pub(crate) skip: SkipComponentFn,
    pub(crate) remove: RemoveComponentFn,
}

#[derive(Clone)]
pub(crate) struct AssetTableEntry {
    pub(crate) id: TableId,
    pub(crate) name: &'static str,
    pub(crate) write_changes: WriteAssetChangesFn,
    pub(crate) apply: ApplyAssetFn,
    pub(crate) reserve: ApplyAssetFn,
    pub(crate) drop: DropAssetFn,
}

#[derive(Clone)]
pub(crate) struct ResourceTableEntry {
    pub(crate) id: TableId,
    pub(crate) name: &'static str,
    pub(crate) write_change: WriteResourceChangeFn,
    pub(crate) apply: ApplyResourceFn,
    pub(crate) drop: DropResourceFn,
}

#[derive(Resource, Default)]
/// Registry of mirrored component, asset, and resource types.
///
/// Users normally populate this through [`crate::TephriteAppConfig`] methods
/// such as `mirror_component::<T>()`. The lower-level
/// [`ReplicationRegistryAppExt`] trait is available when constructing Bevy apps
/// manually in tests or custom harnesses.
pub struct ReplicationRegistry {
    components: Vec<ComponentTableEntry>,
    component_types: std::collections::HashMap<TypeId, TableId>,
    assets: Vec<AssetTableEntry>,
    asset_types: std::collections::HashMap<TypeId, TableId>,
    resources: Vec<ResourceTableEntry>,
    resource_types: std::collections::HashMap<TypeId, TableId>,
}

impl ReplicationRegistry {
    /// Register a component for mirrored entity replication.
    ///
    /// Registration is idempotent for the same concrete type. The first
    /// successful registration assigns the next component table ID and stores
    /// the type-specific writer/reader callbacks used by the hot path.
    pub fn register_component<C>(&mut self) -> &mut Self
    where
        C: Component + FastWrite + FastRead<Ret = C> + 'static,
        C::Context: ContextFromWorld,
    {
        let type_id = TypeId::of::<C>();
        if self.component_types.contains_key(&type_id) {
            return self;
        }

        let id = self.components.len() as TableId;
        self.component_types.insert(type_id, id);
        self.components.push(ComponentTableEntry {
            id,
            name: std::any::type_name::<C>(),
            collect_entities: collect_entities_with_component::<C>,
            write_baseline: write_component_baseline::<C>,
            write_changes: write_component_changes::<C>,
            write_removals: write_component_removals::<C>,
            apply: apply_component::<C>,
            skip: skip_component::<C>,
            remove: remove_component::<C>,
        });
        self
    }

    /// Register an asset type for mirroring.
    ///
    /// The asset callback watches Bevy `AssetEvent<A>` messages on the logic
    /// side. The render callback updates the local `Assets<A>` collection and
    /// maintains the remote-to-local handle mapping through [`RemappableAsset`].
    pub fn register_asset<A>(&mut self) -> &mut Self
    where
        A: Asset + FastWrite + FastRead<Ret = A> + RemappableAsset + Debug + 'static,
        A::Context: ContextFromWorld,
    {
        let type_id = TypeId::of::<A>();
        if self.asset_types.contains_key(&type_id) {
            return self;
        }

        let id = self.assets.len() as TableId;
        self.asset_types.insert(type_id, id);
        self.assets.push(AssetTableEntry {
            id,
            name: std::any::type_name::<A>(),
            write_changes: write_asset_changes::<A>,
            apply: apply_asset::<A>,
            reserve: reserve_asset::<A>,
            drop: drop_asset::<A>,
        });
        self
    }

    /// Register a resource type for mirroring.
    ///
    /// Resources are sent as complete values when changed. If a previously
    /// existing resource disappears on the logic side, the render side removes
    /// its local copy.
    pub fn register_resource<R>(&mut self) -> &mut Self
    where
        R: Resource + FastWrite + FastRead<Ret = R> + 'static,
        R::Context: ContextFromWorld,
    {
        let type_id = TypeId::of::<R>();
        if self.resource_types.contains_key(&type_id) {
            return self;
        }

        let id = self.resources.len() as TableId;
        self.resource_types.insert(type_id, id);
        self.resources.push(ResourceTableEntry {
            id,
            name: std::any::type_name::<R>(),
            write_change: write_resource_change::<R>,
            apply: apply_resource::<R>,
            drop: drop_resource::<R>,
        });
        self
    }

    /// Names of registered component types in table order.
    ///
    /// Intended for diagnostics and tests. The transcript does not depend on
    /// these names in normal operation.
    pub fn component_names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.components.iter().map(|entry| entry.name)
    }

    /// Names of registered asset types in table order.
    pub fn asset_names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.assets.iter().map(|entry| entry.name)
    }

    /// Names of registered resource types in table order.
    pub fn resource_names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.resources.iter().map(|entry| entry.name)
    }

    pub(crate) fn components(&self) -> &[ComponentTableEntry] {
        &self.components
    }

    pub(crate) fn assets(&self) -> &[AssetTableEntry] {
        &self.assets
    }

    pub(crate) fn resources(&self) -> &[ResourceTableEntry] {
        &self.resources
    }

    pub(crate) fn component(&self, id: TableId) -> Option<&ComponentTableEntry> {
        self.components
            .get(id as usize)
            .filter(|entry| entry.id == id)
    }

    pub(crate) fn asset(&self, id: TableId) -> Option<&AssetTableEntry> {
        self.assets.get(id as usize).filter(|entry| entry.id == id)
    }

    pub(crate) fn resource(&self, id: TableId) -> Option<&ResourceTableEntry> {
        self.resources
            .get(id as usize)
            .filter(|entry| entry.id == id)
    }
}

pub trait ReplicationRegistryAppExt {
    /// Register a component type on this app's [`ReplicationRegistry`].
    fn replicate_component<C>(&mut self) -> &mut Self
    where
        C: Component + FastWrite + FastRead<Ret = C> + 'static,
        C::Context: ContextFromWorld;

    /// Register an asset type on this app's [`ReplicationRegistry`].
    fn replicate_asset<A>(&mut self) -> &mut Self
    where
        A: Asset + FastWrite + FastRead<Ret = A> + RemappableAsset + Debug + 'static,
        A::Context: ContextFromWorld;

    /// Register a resource type on this app's [`ReplicationRegistry`].
    fn replicate_resource<R>(&mut self) -> &mut Self
    where
        R: Resource + FastWrite + FastRead<Ret = R> + 'static,
        R::Context: ContextFromWorld;
}

impl ReplicationRegistryAppExt for App {
    fn replicate_component<C>(&mut self) -> &mut Self
    where
        C: Component + FastWrite + FastRead<Ret = C> + 'static,
        C::Context: ContextFromWorld,
    {
        self.init_resource::<ReplicationRegistry>();
        self.world_mut()
            .resource_mut::<ReplicationRegistry>()
            .register_component::<C>();
        self
    }

    fn replicate_asset<A>(&mut self) -> &mut Self
    where
        A: Asset + FastWrite + FastRead<Ret = A> + RemappableAsset + Debug + 'static,
        A::Context: ContextFromWorld,
    {
        self.init_resource::<ReplicationRegistry>();
        self.world_mut()
            .resource_mut::<ReplicationRegistry>()
            .register_asset::<A>();
        self
    }

    fn replicate_resource<R>(&mut self) -> &mut Self
    where
        R: Resource + FastWrite + FastRead<Ret = R> + 'static,
        R::Context: ContextFromWorld,
    {
        self.init_resource::<ReplicationRegistry>();
        self.world_mut()
            .resource_mut::<ReplicationRegistry>()
            .register_resource::<R>();
        self
    }
}

#[derive(Resource)]
struct CachedRemovedComponents<C: Component> {
    state: SystemState<RemovedComponents<'static, 'static, C>>,
}

#[derive(Resource)]
struct CachedAssetReader<A: Asset> {
    state: SystemState<(
        MessageReader<'static, 'static, AssetEvent<A>>,
        Res<'static, Assets<A>>,
    )>,
}

#[derive(Resource)]
struct CachedResourceState<R: Resource> {
    state: SystemState<Option<Res<'static, R>>>,
    existed: bool,
}

fn collect_entities_with_component<C: Component>(world: &mut World, out: &mut Vec<Entity>) {
    let mut query = world.query_filtered::<Entity, With<C>>();
    out.extend(query.iter(world));
}

fn write_component_baseline<C>(
    world: &World,
    dest: &mut TranscriptWriteStateResource,
    entity: Entity,
    component_type: TableId,
) where
    C: Component + FastWrite,
{
    let Some(component) = world.get::<C>(entity) else {
        return;
    };

    unsafe {
        crate::replication::instruction::write_component_add(
            dest,
            entity,
            component_type,
            component,
        );
    }
}

fn write_component_changes<C>(
    world: &mut World,
    dest: &mut TranscriptWriteStateResource,
    newly_tracked: &EntityHashSet,
    component_type: TableId,
) where
    C: Component + FastWrite,
{
    // Changed components on newly tracked entities are handled by the baseline
    // pass, which sends every registered component currently present on that
    // entity. Skipping them here prevents duplicate component payloads in the
    // first tracked frame.
    let mut query = world.query_filtered::<(Entity, &C), (Changed<C>, With<IsReplicated>)>();
    let mut items = Vec::new();

    for (entity, component) in query.iter(world) {
        if !newly_tracked.contains(&entity) {
            items.push((entity, component));
        }
    }

    for (entity, component) in items {
        unsafe {
            crate::replication::instruction::write_component_add(
                dest,
                entity,
                component_type,
                component,
            );
        }
    }
}

fn write_component_removals<C>(
    world: &mut World,
    dest: &mut TranscriptWriteStateResource,
    tracked: &EntityHashSet,
    component_type: TableId,
) where
    C: Component,
{
    // RemovedComponents readers are system state, so cache one per mirrored
    // component type. This lets the exclusive writer system keep precise
    // ordering without allocating a Bevy system per component type.
    if !world.contains_resource::<CachedRemovedComponents<C>>() {
        let state = SystemState::new(world);
        world.insert_resource(CachedRemovedComponents::<C> { state });
    }

    world.resource_scope(|world, mut cached: Mut<CachedRemovedComponents<C>>| {
        let Ok(mut removals) = cached.state.get_mut(world) else {
            return;
        };
        for entity in removals.read() {
            if tracked.contains(&entity) {
                unsafe {
                    crate::replication::instruction::write_component_remove(
                        dest,
                        entity,
                        component_type,
                    );
                }
            }
        }
        cached.state.apply(world);
    });
}

fn apply_component<C>(entity: Entity, world: &mut World, reader: &mut ByteReader<'_>)
where
    C: Component + FastRead<Ret = C>,
    C::Context: ContextFromWorld,
{
    let component = <C::Context as ContextFromWorld>::with_world(world, |context| unsafe {
        C::read_fast(context, reader)
    });

    if let Ok(mut entity) = world.get_entity_mut(entity) {
        entity.insert(component);
    }
}

fn skip_component<C>(world: &mut World, reader: &mut ByteReader<'_>)
where
    C: Component + FastRead<Ret = C>,
    C::Context: ContextFromWorld,
{
    // Component payloads are not length-prefixed. If a component update targets
    // an unmapped entity, we still need to deserialize and discard the payload
    // so the instruction stream remains aligned.
    let _ = <C::Context as ContextFromWorld>::with_world(world, |context| unsafe {
        C::read_fast(context, reader)
    });
}

fn remove_component<C: Component>(entity: Entity, world: &mut World) {
    if let Ok(mut entity) = world.get_entity_mut(entity) {
        entity.remove::<C>();
    }
}

fn init_asset_reader<A: Asset>(world: &mut World) {
    if !world.contains_resource::<CachedAssetReader<A>>() {
        let state = SystemState::new(world);
        world.insert_resource(CachedAssetReader::<A> { state });
    }
}

fn write_asset_changes<A>(
    world: &mut World,
    dest: &mut TranscriptWriteStateResource,
    asset_type: TableId,
) where
    A: Asset + FastWrite,
{
    init_asset_reader::<A>(world);

    world.resource_scope(|world, mut cached: Mut<CachedAssetReader<A>>| {
        let Ok((mut events, assets)) = cached.state.get_mut(world) else {
            return;
        };

        for event in events.read() {
            match event {
                AssetEvent::Added { id }
                | AssetEvent::Modified { id }
                | AssetEvent::LoadedWithDependencies { id } => {
                    if let Some(asset) = assets.get(*id) {
                        // debug!(
                        //     target: "tephrite_rs::replication::assets",
                        //     asset_type = std::any::type_name::<A>(),
                        //     asset_id = ?id,
                        //     "writing asset update"
                        // );
                        unsafe {
                            crate::replication::instruction::write_asset_update(
                                dest, asset_type, *id, asset,
                            );
                        }
                    } else {
                        // debug!(
                        //     target: "tephrite_rs::replication::assets",
                        //     asset_type = std::any::type_name::<A>(),
                        //     asset_id = ?id,
                        //     "missing asset for update"
                        // );
                        unsafe {
                            crate::replication::instruction::write_asset_reserve(
                                dest, asset_type, *id,
                            );
                        }
                    }
                }
                AssetEvent::Removed { id } => unsafe {
                    // debug!(
                    //     target: "tephrite_rs::replication::assets",
                    //     asset_type = std::any::type_name::<A>(),
                    //     asset_id = ?id,
                    //     "writing asset drop"
                    // );
                    crate::replication::instruction::write_asset_drop(dest, asset_type, *id);
                },
                AssetEvent::Unused { id: _ } => {}
            }
        }

        cached.state.apply(world);
    });
}

fn apply_asset<A>(world: &mut World, reader: &mut ByteReader<'_>)
where
    A: Asset + FastRead<Ret = A> + RemappableAsset + Debug,
    A::Context: ContextFromWorld,
{
    let id = unsafe { AssetId::<A>::read_fast(&mut (), reader) };

    let asset = <A::Context as ContextFromWorld>::with_world(world, |context| unsafe {
        A::read_fast(context, reader)
    });

    // debug!(
    //     target: "tephrite_rs::replication::assets",
    //     asset_type = std::any::type_name::<A>(),
    //     asset_id = ?id,
    //     "applying asset update"
    // );
    let mut assets = world.resource_mut::<Assets<A>>();
    A::set_mapping(id, asset, &mut assets);
}

fn reserve_asset<A>(world: &mut World, reader: &mut ByteReader<'_>)
where
    A: Asset + FastRead<Ret = A> + RemappableAsset + Debug,
{
    let id = unsafe { AssetId::<A>::read_fast(&mut (), reader) };
    // debug!(
    //     target: "tephrite_rs::replication::assets",
    //     asset_type = std::any::type_name::<A>(),
    //     asset_id = ?id,
    //     "reserving asset"
    // );
    let mut assets = world.resource_mut::<Assets<A>>();
    A::remap_to_local_or_reserve(id, &mut assets);
}

fn drop_asset<A>(world: &mut World, reader: &mut ByteReader<'_>)
where
    A: Asset + RemappableAsset,
{
    let id = unsafe { AssetId::<A>::read_fast(&mut (), reader) };
    // debug!(
    //     target: "tephrite_rs::replication::assets",
    //     asset_type = std::any::type_name::<A>(),
    //     asset_id = ?id,
    //     "applying asset drop"
    // );
    let mut assets = world.resource_mut::<Assets<A>>();
    A::clear_mapping(id, &mut assets);
}

fn write_resource_change<R>(
    world: &mut World,
    dest: &mut TranscriptWriteStateResource,
    resource_type: TableId,
) where
    R: Resource + FastWrite,
{
    // The resource tracker remembers whether the resource existed last time so
    // removal can be mirrored. Bevy's `Res<R>::is_changed` covers initial
    // insertion and later mutations.
    if !world.contains_resource::<CachedResourceState<R>>() {
        let state = SystemState::new(world);
        world.insert_resource(CachedResourceState::<R> {
            state,
            existed: false,
        });
    }

    world.resource_scope(|world, mut cached: Mut<CachedResourceState<R>>| {
        let resource = cached.state.get_mut(world);
        match resource {
            Ok(Some(resource)) => {
                if resource.is_changed() {
                    unsafe {
                        crate::replication::instruction::write_resource_update(
                            dest,
                            resource_type,
                            &*resource,
                        );
                    }
                }
                cached.existed = true;
            }
            Ok(None) if cached.existed => {
                cached.existed = false;
                unsafe {
                    crate::replication::instruction::write_resource_drop(dest, resource_type);
                }
            }
            Ok(None) => {}
            Err(_) => {}
        }
        cached.state.apply(world);
    });
}

fn apply_resource<R>(world: &mut World, reader: &mut ByteReader<'_>)
where
    R: Resource + FastRead<Ret = R>,
    R::Context: ContextFromWorld,
{
    let resource = <R::Context as ContextFromWorld>::with_world(world, |ctx| unsafe {
        R::read_fast(ctx, reader)
    });

    world.insert_resource(resource);
}

fn drop_resource<R: Resource>(world: &mut World) {
    world.remove_resource::<R>();
}
