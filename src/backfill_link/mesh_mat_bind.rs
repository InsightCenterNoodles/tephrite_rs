use bevy::prelude::*;

use super::components::*;
use super::convert::*;
use super::resources::Session;
use super::sets::*;

use crate::backfill;
use crate::backfill_link::assets::AssetCache;

fn watch_mesh_change(
    mut events: MessageReader<AssetEvent<Mesh>>,
    meshes: Res<Assets<Mesh>>,
    mut cache: NonSendMut<AssetCache>,
    session: NonSend<Session>,
) {
    for e in events.read() {
        match e {
            AssetEvent::Added { id } | AssetEvent::Modified { id } => {
                if let Some(asset) = meshes.get(*id) {
                    if let Some(converted) = convert_mesh(&session.0, asset) {
                        debug!("Converted new mesh {id} {:?}", converted.as_ptr());
                        cache.meshes.insert(*id, converted);
                    } else {
                        warn!("Mesh {id} unsupported for conversion; skipping");
                    }
                } else {
                    debug!("Mesh {id} is placeholder...")
                }
            }
            _ => {}
        }
    }
}

fn watch_image_change(
    mut events: MessageReader<AssetEvent<Image>>,
    assets: Res<Assets<Image>>,
    mut cache: NonSendMut<AssetCache>,
    session: NonSend<Session>,
) {
    for e in events.read() {
        match e {
            AssetEvent::Added { id } | AssetEvent::Modified { id } => {
                if let Some(asset) = assets.get(*id) {
                    // debug!(
                    //     "QUICK DUMP {id:?} bytes {:?}",
                    //     asset
                    //         .data
                    //         .as_ref()
                    //         .map(|x| x.iter().take(25).collect::<Vec<_>>())
                    // );
                    if let Some(converted) = convert_texture(&session.0, asset) {
                        debug!("Converted new image {id} {:?}", converted.as_ptr());
                        cache
                            .textures
                            .insert(*id, (converted, asset.sampler.clone()));
                    } else {
                        warn!("Image {id} unsupported for conversion; skipping");
                    }
                } else {
                    debug!("Image {id} is placeholder...")
                }
            }
            _ => {}
        }
    }
}

fn watch_material_change(
    mut events: MessageReader<AssetEvent<StandardMaterial>>,
    materials: Res<Assets<StandardMaterial>>,
    mut cache: NonSendMut<AssetCache>,
    session: NonSend<Session>,
) {
    for e in events.read() {
        match e {
            AssetEvent::Added { id } | AssetEvent::Modified { id } => {
                if let Some(asset) = materials.get(*id) {
                    if let Some(converted) = convert_material(&session.0, asset, &cache.textures) {
                        debug!("Converted new material {id} {:?}", converted.as_ptr());
                        cache.materials.insert(*id, converted);
                    } else {
                        warn!("Material {id} failed conversion; skipping");
                    }
                } else {
                    debug!("Material {id} is placeholder...")
                }
            }
            _ => {}
        }
    }
}

// build (or rebuild) binding from the current assets/handles
fn build_binding_for(
    bentity: &BEntity,
    mesh_handle: AssetId<Mesh>,
    mat_handle: AssetId<StandardMaterial>,
    cache: &mut NonSendMut<AssetCache>,
    session: &NonSend<Session>,
) -> BRenderBinding {
    // The cache _should_ have our content now

    let mesh = cache.meshes.get(&mesh_handle).unwrap();
    let material = cache.materials.get(&mat_handle).unwrap();

    let mesh_handle_ffi = mesh.clone();
    let material_handle_ffi = material.clone();

    backfill::add_renderable(
        &session.0,
        bentity.0,
        &mesh_handle_ffi,
        &material_handle_ffi,
    );

    debug!(
        "Installing new renderable to {:?}: mesh {} and mat {} ",
        bentity.0, mesh_handle, mat_handle,
    );

    BRenderBinding {
        mesh_handle,
        mat_handle,
    }
}

// A small helper: does entity have both parts required to be renderable?
fn is_renderable(
    mesh: Option<&Mesh3d>,
    mat: Option<&MeshMaterial3d<StandardMaterial>>,
) -> Option<(AssetId<Mesh>, AssetId<StandardMaterial>)> {
    let mesh_h = mesh?;
    let mat_h = mat?;
    Some((mesh_h.id(), mat_h.id()))
}

// --- Plugin to wire everything together ---
pub struct RenderableBindingPlugin;

impl Plugin for RenderableBindingPlugin {
    fn build(&self, app: &mut App) {
        app.insert_non_send_resource(AssetCache::default());
        app
            // scans for “renderable now” (handle added/changed) and collects asset events
            .add_systems(
                PostUpdate,
                (
                    watch_image_change,
                    watch_mesh_change,
                    watch_material_change,
                    sync_binding_on_renderability_and_asset_changes,
                )
                    .chain()
                    .in_set(RenderableSet::Detect),
            )
            // if BEntity appears later on an already-renderable entity
            .add_systems(
                PostUpdate,
                (
                    on_bentity_added_attach_binding_if_renderable,
                    update_visibility,
                )
                    .chain()
                    .in_set(RenderableSet::Refresh)
                    .after(RenderableSet::Detect),
            )
            // remove binding if prerequisites vanish
            .add_systems(
                PostUpdate,
                remove_binding_when_prereqs_missing
                    .in_set(RenderableSet::Cleanup)
                    .after(RenderableSet::Refresh),
            );
    }
}

/// create/refresh when renderable or asset data changes
fn sync_binding_on_renderability_and_asset_changes(
    mut commands: Commands,

    // Detect asset *data* changes via events
    mut mesh_events: MessageReader<AssetEvent<Mesh>>,
    mut material_events: MessageReader<AssetEvent<StandardMaterial>>,

    // Detect when the *handles* change or are added
    q_renderables: Query<(
        Entity,
        &BEntity,
        Option<&BRenderBinding>,
        &Mesh3d,
        &MeshMaterial3d<StandardMaterial>,
    )>,

    // Also capture handle changes explicitly
    q_handle_changed_with_bentity: Query<
        Entity,
        (
            With<BEntity>,
            Or<(Changed<Mesh3d>, Changed<MeshMaterial3d<StandardMaterial>>)>,
        ),
    >,

    mut cache: NonSendMut<AssetCache>,
    session: NonSend<Session>,
) {
    use std::collections::HashSet;

    // Collect sets of changed asset IDs this frame
    let mut changed_meshes: HashSet<AssetId<Mesh>> = HashSet::new();
    for ev in mesh_events.read() {
        match ev {
            AssetEvent::Added { id } | AssetEvent::Modified { id } => {
                changed_meshes.insert(*id);
            }
            AssetEvent::Removed { id } => {
                // Underlying asset vanished. Treat as change: we’ll rebuild (or later remove).
                changed_meshes.insert(*id);
            }
            _ => {}
        }
    }

    let mut changed_materials: HashSet<AssetId<StandardMaterial>> = HashSet::new();
    for ev in material_events.read() {
        match ev {
            AssetEvent::Added { id } | AssetEvent::Modified { id } => {
                changed_materials.insert(*id);
            }
            AssetEvent::Removed { id } => {
                changed_materials.insert(*id);
            }
            _ => {}
        }
    }

    // if !changed_materials.is_empty() || !changed_meshes.is_empty() {
    //     dbg!(&changed_materials);
    //     dbg!(&changed_meshes);
    // }

    // Fast path: if no handle changes AND no asset changes, we still need to handle
    // the case where an entity *just* became renderable this frame (Added<Mesh3d>/Material).
    // The query above includes all BEntity + renderable; we’ll check whether a refresh is needed.

    for (entity, bentity, existing_binding, mesh3d, mat3d) in &q_renderables {
        let mesh_h = mesh3d.id();
        let mat_h = mat3d.id();

        let handle_changed_for_this_entity = q_handle_changed_with_bentity.contains(entity);

        let asset_changed_for_this_entity =
            changed_meshes.contains(&mesh_h) || changed_materials.contains(&mat_h);

        let is_renderable_but_no_binding = existing_binding
            .map(|b| b.mesh_handle != mesh_h || b.mat_handle != mat_h)
            .unwrap_or(true);

        let needs_refresh = handle_changed_for_this_entity
            || asset_changed_for_this_entity
            || is_renderable_but_no_binding;

        if needs_refresh {
            debug!(
                "Needs refresh, bindings changed, {} {} {}",
                handle_changed_for_this_entity,
                asset_changed_for_this_entity,
                is_renderable_but_no_binding
            );
            let new_binding = build_binding_for(bentity, mesh_h, mat_h, &mut cache, &session);
            commands.entity(entity).insert(new_binding);
        }
    }
}

/// If BEntity arrives *after* renderable, attach (or refresh) binding
fn on_bentity_added_attach_binding_if_renderable(
    mut commands: Commands,
    q: Query<
        (
            Entity,
            &BEntity,
            Option<&BRenderBinding>,
            Option<&Mesh3d>,
            Option<&MeshMaterial3d<StandardMaterial>>,
        ),
        Added<BEntity>,
    >,
    mut cache: NonSendMut<AssetCache>,
    session: NonSend<Session>,
) {
    for (entity, bentity, existing_binding, mesh_opt, mat_opt) in &q {
        if let Some((mesh_h, mat_h)) = is_renderable(mesh_opt, mat_opt)
            && existing_binding.is_none()
        {
            debug!("Add new bindings");
            let new_binding = build_binding_for(bentity, mesh_h, mat_h, &mut cache, &session);
            commands.entity(entity).insert(new_binding);
        }
    }
}

/// remove binding if prerequisites go away (lost mesh/material/BEntity)
fn remove_binding_when_prereqs_missing(
    mut commands: Commands,
    q_to_remove: Query<
        (
            Entity,
            Option<&Mesh3d>,
            Option<&MeshMaterial3d<StandardMaterial>>,
            Option<&BEntity>,
        ),
        With<BRenderBinding>,
    >,
) {
    for (entity, mesh_opt, mat_opt, bentity_opt) in &q_to_remove {
        let still_has_all = mesh_opt.is_some() && mat_opt.is_some() && bentity_opt.is_some();
        if !still_has_all {
            commands.entity(entity).remove::<BRenderBinding>();
        }
    }
}

fn update_visibility(
    query: Query<(&BEntity, &InheritedVisibility), Changed<InheritedVisibility>>,
    session: NonSend<Session>,
) {
    for (bent, vis) in query {
        //debug!("Update vis {vis:?}");
        unsafe {
            backfill::ffi::fs_set_visible(session.0.as_ptr(), bent.0.into(), vis.get() as u8)
        };
    }
}
