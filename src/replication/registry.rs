use std::{any::TypeId, fmt::Debug};

use bevy::ecs::{entity::EntityHashSet, system::SystemState};
use bevy::prelude::*;

use crate::replication::components::IsReplicated;
use crate::serialize::transcript_writer::TranscriptWriteStateResource;
use crate::serialize::{ByteReader, FastRead, FastWrite, RemappableAsset};

pub(crate) type TableId = u16;

pub(crate) type CollectComponentEntitiesFn = fn(&mut World, &mut Vec<Entity>);
pub(crate) type WriteComponentBaselineFn =
    fn(&World, &mut TranscriptWriteStateResource, Entity, TableId);
pub(crate) type WriteComponentChangesFn =
    fn(&mut World, &mut TranscriptWriteStateResource, &EntityHashSet, TableId);
pub(crate) type WriteComponentRemovalsFn =
    fn(&mut World, &mut TranscriptWriteStateResource, &EntityHashSet, TableId);
pub(crate) type ApplyComponentFn = for<'a> fn(Entity, &mut World, &mut ByteReader<'a>);
pub(crate) type SkipComponentFn = for<'a> fn(&mut ByteReader<'a>);
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
pub struct ReplicationRegistry {
    components: Vec<ComponentTableEntry>,
    component_types: std::collections::HashMap<TypeId, TableId>,
    assets: Vec<AssetTableEntry>,
    asset_types: std::collections::HashMap<TypeId, TableId>,
    resources: Vec<ResourceTableEntry>,
    resource_types: std::collections::HashMap<TypeId, TableId>,
    renderer_plugins: Vec<&'static str>,
}

impl ReplicationRegistry {
    pub fn register_renderer_plugin(&mut self, plugin: &'static str) -> &mut Self {
        if !self.renderer_plugins.contains(&plugin) {
            self.renderer_plugins.push(plugin);
        }
        self
    }

    pub fn register_component<C>(&mut self) -> &mut Self
    where
        C: Component + FastWrite + FastRead<Ret = C> + 'static,
    {
        self.register_component_with_plugins::<C>(&[])
    }

    pub fn register_component_with_plugins<C>(
        &mut self,
        renderer_plugins: &[&'static str],
    ) -> &mut Self
    where
        C: Component + FastWrite + FastRead<Ret = C> + 'static,
    {
        let type_id = TypeId::of::<C>();
        if self.component_types.contains_key(&type_id) {
            return self;
        }

        let id = self.components.len() as TableId;
        for plugin in renderer_plugins {
            self.register_renderer_plugin(plugin);
        }
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

    pub fn register_asset<A>(&mut self) -> &mut Self
    where
        A: Asset + FastWrite + FastRead<Ret = A> + RemappableAsset + Debug + 'static,
    {
        self.register_asset_with_plugins::<A>(&[])
    }

    pub fn register_asset_with_plugins<A>(&mut self, renderer_plugins: &[&'static str]) -> &mut Self
    where
        A: Asset + FastWrite + FastRead<Ret = A> + RemappableAsset + Debug + 'static,
    {
        let type_id = TypeId::of::<A>();
        if self.asset_types.contains_key(&type_id) {
            return self;
        }

        let id = self.assets.len() as TableId;
        for plugin in renderer_plugins {
            self.register_renderer_plugin(plugin);
        }
        self.asset_types.insert(type_id, id);
        self.assets.push(AssetTableEntry {
            id,
            name: std::any::type_name::<A>(),
            write_changes: write_asset_changes::<A>,
            apply: apply_asset::<A>,
            drop: drop_asset::<A>,
        });
        self
    }

    pub fn register_resource<R>(&mut self) -> &mut Self
    where
        R: Resource + FastWrite + FastRead<Ret = R> + 'static,
    {
        self.register_resource_with_plugins::<R>(&[])
    }

    pub fn register_resource_with_plugins<R>(
        &mut self,
        renderer_plugins: &[&'static str],
    ) -> &mut Self
    where
        R: Resource + FastWrite + FastRead<Ret = R> + 'static,
    {
        let type_id = TypeId::of::<R>();
        if self.resource_types.contains_key(&type_id) {
            return self;
        }

        let id = self.resources.len() as TableId;
        for plugin in renderer_plugins {
            self.register_renderer_plugin(plugin);
        }
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

    pub(crate) fn components(&self) -> &[ComponentTableEntry] {
        &self.components
    }

    pub(crate) fn assets(&self) -> &[AssetTableEntry] {
        &self.assets
    }

    pub(crate) fn resources(&self) -> &[ResourceTableEntry] {
        &self.resources
    }

    pub(crate) fn renderer_plugins(&self) -> &[&'static str] {
        &self.renderer_plugins
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
    fn replicate_component<C>(&mut self) -> &mut Self
    where
        C: Component + FastWrite + FastRead<Ret = C> + 'static;

    fn replicate_component_with_plugins<C>(
        &mut self,
        renderer_plugins: &[&'static str],
    ) -> &mut Self
    where
        C: Component + FastWrite + FastRead<Ret = C> + 'static;

    fn replicate_asset<A>(&mut self) -> &mut Self
    where
        A: Asset + FastWrite + FastRead<Ret = A> + RemappableAsset + Debug + 'static;

    fn replicate_asset_with_plugins<A>(&mut self, renderer_plugins: &[&'static str]) -> &mut Self
    where
        A: Asset + FastWrite + FastRead<Ret = A> + RemappableAsset + Debug + 'static;

    fn replicate_resource<R>(&mut self) -> &mut Self
    where
        R: Resource + FastWrite + FastRead<Ret = R> + 'static;

    fn replicate_resource_with_plugins<R>(
        &mut self,
        renderer_plugins: &[&'static str],
    ) -> &mut Self
    where
        R: Resource + FastWrite + FastRead<Ret = R> + 'static;

    fn require_renderer_plugin(&mut self, renderer_plugin: &'static str) -> &mut Self;
}

impl ReplicationRegistryAppExt for App {
    fn replicate_component<C>(&mut self) -> &mut Self
    where
        C: Component + FastWrite + FastRead<Ret = C> + 'static,
    {
        self.init_resource::<ReplicationRegistry>();
        self.world_mut()
            .resource_mut::<ReplicationRegistry>()
            .register_component::<C>();
        self
    }

    fn replicate_component_with_plugins<C>(
        &mut self,
        renderer_plugins: &[&'static str],
    ) -> &mut Self
    where
        C: Component + FastWrite + FastRead<Ret = C> + 'static,
    {
        self.init_resource::<ReplicationRegistry>();
        self.world_mut()
            .resource_mut::<ReplicationRegistry>()
            .register_component_with_plugins::<C>(renderer_plugins);
        self
    }

    fn replicate_asset<A>(&mut self) -> &mut Self
    where
        A: Asset + FastWrite + FastRead<Ret = A> + RemappableAsset + Debug + 'static,
    {
        self.init_resource::<ReplicationRegistry>();
        self.world_mut()
            .resource_mut::<ReplicationRegistry>()
            .register_asset::<A>();
        self
    }

    fn replicate_asset_with_plugins<A>(&mut self, renderer_plugins: &[&'static str]) -> &mut Self
    where
        A: Asset + FastWrite + FastRead<Ret = A> + RemappableAsset + Debug + 'static,
    {
        self.init_resource::<ReplicationRegistry>();
        self.world_mut()
            .resource_mut::<ReplicationRegistry>()
            .register_asset_with_plugins::<A>(renderer_plugins);
        self
    }

    fn replicate_resource<R>(&mut self) -> &mut Self
    where
        R: Resource + FastWrite + FastRead<Ret = R> + 'static,
    {
        self.init_resource::<ReplicationRegistry>();
        self.world_mut()
            .resource_mut::<ReplicationRegistry>()
            .register_resource::<R>();
        self
    }

    fn replicate_resource_with_plugins<R>(&mut self, renderer_plugins: &[&'static str]) -> &mut Self
    where
        R: Resource + FastWrite + FastRead<Ret = R> + 'static,
    {
        self.init_resource::<ReplicationRegistry>();
        self.world_mut()
            .resource_mut::<ReplicationRegistry>()
            .register_resource_with_plugins::<R>(renderer_plugins);
        self
    }

    fn require_renderer_plugin(&mut self, renderer_plugin: &'static str) -> &mut Self {
        self.init_resource::<ReplicationRegistry>();
        self.world_mut()
            .resource_mut::<ReplicationRegistry>()
            .register_renderer_plugin(renderer_plugin);
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
{
    let component = unsafe { C::read_fast(reader) };
    if let Ok(mut entity) = world.get_entity_mut(entity) {
        entity.insert(component);
    }
}

fn skip_component<C>(reader: &mut ByteReader<'_>)
where
    C: Component + FastRead<Ret = C>,
{
    let _ = unsafe { C::read_fast(reader) };
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
                AssetEvent::Added { id } | AssetEvent::Modified { id } => {
                    if let Some(asset) = assets.get(*id) {
                        unsafe {
                            crate::replication::instruction::write_asset_update(
                                dest, asset_type, *id, asset,
                            );
                        }
                    }
                }
                AssetEvent::Removed { id } => unsafe {
                    crate::replication::instruction::write_asset_drop(dest, asset_type, *id);
                },
                AssetEvent::Unused { id: _ } | AssetEvent::LoadedWithDependencies { id: _ } => {}
            }
        }

        cached.state.apply(world);
    });
}

fn apply_asset<A>(world: &mut World, reader: &mut ByteReader<'_>)
where
    A: Asset + FastRead<Ret = A> + RemappableAsset + Debug,
{
    let id = unsafe { AssetId::<A>::read_fast(reader) };
    let asset = unsafe { A::read_fast(reader) };
    let mut assets = world.resource_mut::<Assets<A>>();
    A::set_mapping(id, asset, &mut assets);
}

fn drop_asset<A>(world: &mut World, reader: &mut ByteReader<'_>)
where
    A: Asset + RemappableAsset,
{
    let id = unsafe { AssetId::<A>::read_fast(reader) };
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
{
    let resource = unsafe { R::read_fast(reader) };
    world.insert_resource(resource);
}

fn drop_resource<R: Resource>(world: &mut World) {
    world.remove_resource::<R>();
}
