use crate::transcript::deserialize;
use crate::transcript::transcript_reader::TranscriptReader;
use bevy::ecs::entity::EntityHashMap;
use bevy::pbr::CubemapVisibleEntities;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::render::primitives::CubemapFrusta;

use super::ReplicatedAssetID;
use super::instruction::{
    ComponentAdded, ComponentRemoved, DecodeInstruction, DropAsset, EndFrame, EntityAdded,
    EntityRemoved, ReplicateAsset, decode_Instruction,
};
use super::writer::{
    ComponentUpdate, DecodeAssetEnum, DecodeReplicatedComponent, ReplicatedMesh,
    ReplicatedStandardMaterial, decode_AssetEnum, decode_ReplicatedComponent,
};

// =============================================================================

/// Plugin for child processes. Reads a transcript and replicates entities, components, and assets.
pub struct ReplicationReaderPlugin;

impl Plugin for ReplicationReaderPlugin {
    fn build(&self, app: &mut App) {
        println!("Building reader...");
        let transcript = TranscriptReader::new();

        app.insert_non_send_resource(transcript);
        app.init_resource::<EntityMap>();
        app.init_resource::<RemoteAssetMap<Mesh>>();
        app.init_resource::<RemoteAssetMap<StandardMaterial>>();

        app.add_systems(Update, child_system);
    }
}

// =============================================================================

/// Remap foreign entities to local
#[derive(Resource, Default)]
struct EntityMap(EntityHashMap<Entity>);

impl EntityMap {
    fn map(&self, foreign: Entity) -> Entity {
        *self.0.get(&foreign).unwrap()
    }

    fn map_remove(&mut self, foreign: Entity) -> Entity {
        self.0.remove(&foreign).unwrap()
    }
}

// =============================================================================

/// A map of remote assetids to local asset handles
#[derive(Resource)]
struct RemoteAssetMap<T: Asset>(HashMap<AssetId<T>, Handle<T>>);

impl<T: Asset> Default for RemoteAssetMap<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<T: Asset> RemoteAssetMap<T> {
    /// Store a new remote asset
    fn insert(&mut self, asset_store: &mut Assets<T>, remote_id: AssetId<T>, asset: T) {
        let new = asset_store.add(asset);
        //println!("Remapping remote id {remote_id} to {}", new.id());

        self.0.insert(remote_id, new);
    }

    /// Drop an asset from the store
    fn remove(&mut self, asset_store: &mut Assets<T>, remote_id: AssetId<T>) {
        //println!("Unmapping remote {remote_id}");
        if let Some(local) = self.0.remove(&remote_id) {
            asset_store.remove(local.id());
        }
    }

    /// Remap a remote handle to a local asset
    fn fixup(&self, remote_id: AssetId<T>) -> Handle<T> {
        //println!("Fixing up handle {remote_id}");
        self.0.get(&remote_id).expect("missing asset!").clone()
    }
}

// =============================================================================

/// Primary child system to be run every update to obtain new records from the
/// transcript
///
fn child_system(
    transcript: NonSendMut<TranscriptReader>,
    mut map: ResMut<EntityMap>,
    commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut mesh_map: ResMut<RemoteAssetMap<Mesh>>,
    mut material_map: ResMut<RemoteAssetMap<StandardMaterial>>,
) {
    // wait for transcript to be finished
    transcript.barrier();

    let mut decoder = Decoder {
        cursor: std::io::Cursor::new(transcript.get_slice()),
        at_end: false,
        last_entity: Entity::PLACEHOLDER,
        map: &mut map,
        commands,
        meshes: &mut meshes,
        materials: &mut materials,
        mesh_map: &mut mesh_map,
        material_map: &mut material_map,
    };

    // cheese the lifetime system
    let ptr: *mut std::io::Cursor<&[u8]> = &mut decoder.cursor;

    loop {
        decode_Instruction(unsafe { &mut *ptr }, &mut decoder);
        if decoder.at_end {
            break;
        }
    }

    transcript.barrier();
    // hand control back to root
}

// =============================================================================

/// Structure to support instruction read operations
struct Decoder<'a> {
    cursor: std::io::Cursor<&'a [u8]>,
    at_end: bool,
    last_entity: Entity,

    map: &'a mut EntityMap,
    commands: Commands<'a, 'a>,
    meshes: &'a mut Assets<Mesh>,
    materials: &'a mut Assets<StandardMaterial>,
    mesh_map: &'a mut RemoteAssetMap<Mesh>,
    material_map: &'a mut RemoteAssetMap<StandardMaterial>,
}

impl<'a> DecodeInstruction for Decoder<'a> {
    fn handle_entityadded(&mut self, item: EntityAdded) {
        let local = self.commands.spawn_empty();

        self.map.0.insert(item.entity, local.id());
        //println!("Mapping entity {:?} {:?}", item.entity, local.id());
    }

    fn handle_entityremoved(&mut self, item: EntityRemoved) {
        let local = self.map.map_remove(item.entity);
        self.commands.entity(local).despawn();
    }

    fn handle_componentadded(&mut self, item: ComponentAdded) {
        let local = self.map.map(item.entity);

        self.last_entity = local;

        //println!("Component add entity {:?}", local);

        // cheese the lifetime system
        let ptr: *mut std::io::Cursor<&[u8]> = &mut self.cursor;

        decode_ReplicatedComponent(unsafe { &mut *ptr }, self);
    }

    fn handle_componentremoved(&mut self, item: ComponentRemoved) {
        let local = self.map.map(item.entity);
        item.component.remove_component(local, &mut self.commands);
    }

    fn handle_replicateasset(&mut self, _: ReplicateAsset) {
        //println!("Rep asset");
        //nothing to parse but the asset

        // cheese the lifetime system
        let ptr: *mut std::io::Cursor<&[u8]> = &mut self.cursor;

        decode_AssetEnum(unsafe { &mut *ptr }, self);
    }

    fn handle_dropasset(&mut self, item: DropAsset) {
        match item.id {
            ReplicatedAssetID::Mesh(asset_id) => self.mesh_map.remove(self.meshes, asset_id),
            ReplicatedAssetID::StandardMaterial(asset_id) => {
                self.material_map.remove(self.materials, asset_id)
            }
        };
    }

    fn handle_hierarchychange(&mut self, item: super::instruction::HierarchyChange) {
        match item.new_parent {
            Some(parent) => {
                self.commands.entity(parent).add_child(item.child);
            }
            None => {
                self.commands.entity(item.child).remove::<ChildOf>();
            }
        }
    }

    fn handle_endframe(&mut self, _: EndFrame) {
        self.at_end = true;
    }
}

impl<'a> DecodeReplicatedComponent for Decoder<'a> {
    fn handle_head(&mut self, item: crate::common::Head) {
        item.update_component(self.last_entity, &mut self.commands);
    }
    fn handle_transform(&mut self, item: Transform) {
        item.update_component(self.last_entity, &mut self.commands);
    }
    fn handle_globaltransform(&mut self, item: GlobalTransform) {
        item.update_component(self.last_entity, &mut self.commands);
    }
    fn handle_visibility(&mut self, item: Visibility) {
        item.update_component(self.last_entity, &mut self.commands);
    }
    fn handle_inheritedvisibility(&mut self, item: InheritedVisibility) {
        item.update_component(self.last_entity, &mut self.commands);
    }
    fn handle_viewvisibility(&mut self, item: ViewVisibility) {
        item.update_component(self.last_entity, &mut self.commands);
    }
    fn handle_pointlight(&mut self, item: PointLight) {
        item.update_component(self.last_entity, &mut self.commands);
    }
    fn handle_directionallight(&mut self, item: DirectionalLight) {
        item.update_component(self.last_entity, &mut self.commands);
    }
    fn handle_spotlight(&mut self, item: SpotLight) {
        item.update_component(self.last_entity, &mut self.commands);
    }
    fn handle_mesh3d(&mut self, item: Mesh3d) {
        let item = Mesh3d(self.mesh_map.fixup(item.id()));
        item.update_component(self.last_entity, &mut self.commands);
    }
    fn handle_standardmatcomponent(&mut self, item: MeshMaterial3d<StandardMaterial>) {
        let item = MeshMaterial3d::<StandardMaterial>(self.material_map.fixup(item.id()));
        item.update_component(self.last_entity, &mut self.commands);
    }
    fn handle_cubemapfrusta(&mut self, item: CubemapFrusta) {
        item.update_component(self.last_entity, &mut self.commands);
    }
    fn handle_cubemapvisibleentities(&mut self, item: CubemapVisibleEntities) {
        item.update_component(self.last_entity, &mut self.commands);
    }
}

impl<'a> DecodeAssetEnum for Decoder<'a> {
    fn handle_replicatedmesh(&mut self, item: ReplicatedMesh) {
        //println!("Rep mesh {:?}", item.id);
        let mesh = deserialize(&mut self.cursor);

        self.mesh_map.insert(self.meshes, item.id, mesh);
    }
    fn handle_replicatedstandardmaterial(&mut self, item: ReplicatedStandardMaterial) {
        //println!("Rep standard mat {:?}", item.id);
        let mat = deserialize(&mut self.cursor);

        self.material_map.insert(self.materials, item.id, mat);
    }
}
