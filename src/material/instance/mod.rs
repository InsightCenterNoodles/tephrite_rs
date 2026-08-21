//! Explicit GPU instancing for meshes that use Bevy [`StandardMaterial`]s.
//!
//! This module is for cases where a single entity should draw many copies of
//! one mesh with per-instance transforms, colors, and UV transforms. It is
//! intentionally different from spawning many `Mesh3d` entities: the logic app
//! owns one [`Instances`] component, that component is replicated to the render
//! app, and the render app uploads the instance slice to one GPU vertex buffer.
//!
//! # Authoring API
//!
//! Add these components to the same entity:
//!
//! - [`Mesh3d`]: the mesh shared by all instances.
//! - [`InstanceMeshMaterial3d`]: the [`StandardMaterial`] used for PBR
//!   properties, textures, shadows, and alpha mode.
//! - [`Instances`]: the per-instance data.
//! - [`Transform`] and [`Visibility`] as usual.
//!
//! Each [`Instance`] supplies an entity-local transform, a color multiplier, and
//! a UV0 transform. The containing entity's [`GlobalTransform`] is also applied,
//! so moving the entity moves the entire instance set.
//!
//! # Render Path
//!
//! The visible pass reuses Bevy's mesh, view, material, light, and PBR bind
//! groups, then adds one custom bind group for entity-wide instancing data and
//! one instance-rate vertex buffer. The shadow pass uses a matching custom
//! prepass pipeline so instances cast shadows. Shadow receiving works through
//! Bevy's normal PBR lighting path by forwarding the `SHADOW_RECEIVER` mesh flag
//! into the shader, respecting [`bevy::light::NotShadowReceiver`].
//!
//! The instance buffer is cached on the render entity and only reallocated when
//! the instance count grows past the current capacity. Updating positions,
//! colors, or UV transforms in-place is therefore the fast path.

use bevy::{
    camera::visibility::{NoFrustumCulling, RenderLayers, ViewVisibility},
    core_pipeline::core_3d::{
        CORE_3D_DEPTH_FORMAT, Opaque3d, Opaque3dBatchSetKey, Opaque3dBinKey, Transparent3d,
        TransparentSortingInfo3d,
    },
    ecs::{
        query::QueryItem,
        system::{
            SystemParam, SystemParamItem,
            lifetimeless::{Read, SRes},
        },
    },
    material::{OpaqueRendererMethod, RenderPhaseType},
    mesh::{MeshVertexBufferLayoutRef, VertexBufferLayout},
    pbr::{
        LightEntity, LightKeyCache, MATERIAL_BIND_GROUP_INDEX, MaterialBindGroupAllocators,
        MaterialExtractionSystems, MeshFlags, MeshInputUniform, MeshPipeline, MeshPipelineKey,
        MeshPipelineSystems, MeshUniform, PreparedMaterial, PrepassPipeline,
        RenderMaterialInstance, RenderMeshInstanceFlags, RenderMeshInstances, SetMeshBindGroup,
        SetMeshViewBindGroup, SetMeshViewBindingArrayBindGroup, SetPrepassEmptyMaterialBindGroup,
        SetPrepassViewBindGroup, SetPrepassViewEmptyBindGroup, Shadow, ShadowBatchSetKey,
        ShadowBinKey, StandardMaterial, ViewKeyCache, alpha_mode_pipeline_key,
        material_uses_bindless_resources, setup_morph_and_skinning_defs,
    },
    prelude::*,
    render::{
        Extract, ExtractSchedule,
        batching::gpu_preprocessing::BatchedInstanceBuffers,
        erased_render_asset::ErasedRenderAssets,
        extract_component::*,
        mesh::{RenderMesh, RenderMeshBufferInfo, allocator::MeshAllocator},
        render_asset::RenderAssets,
        render_phase::*,
        render_resource::*,
        renderer::{RenderDevice, RenderQueue},
        sync_component::SyncComponent,
        sync_world::*,
        view::{
            ExtractedView, RenderShadowMapVisibleEntities, RenderVisibleEntities,
            RetainedViewEntity,
        },
        *,
    },
    shader::ShaderDefVal,
};
use bytemuck::{Pod, Zeroable};

pub const INSTANCING_SHADER_ASSET_PATH: &str =
    "embedded://tephrite_rs/material/instance/instancing.wgsl";
const INSTANCE_ENTITY_BIND_GROUP: usize = 4;

/// A single mesh instance drawn by [`Instances`].
///
/// This is plain GPU upload data, so it is compact and intentionally stores
/// several values in packed columns:
///
/// - `pos.xyz`: entity-local position.
/// - `pos.w`: packed `rgba8` color multiplier.
/// - `rot`: quaternion rotation, scalar last.
/// - `sca.xyz`: entity-local scale.
/// - `sca.w`: unused.
/// - `tex.xy`: UV0 translation.
/// - `tex.zw`: UV0 scale.
///
/// The instance transform is applied before the containing entity's transform.
/// The color is multiplied with the material base color and, if present, its
/// base color texture.
#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct Instance {
    pub pos: Vec4,
    pub rot: Vec4,
    pub sca: Vec4,
    pub tex: Vec4,
}

impl Instance {
    /// Create a new instance with an identity UV transform.
    pub fn new(position: Vec3, rotation: Quat, scale: Vec3, color: LinearRgba) -> Self {
        Self {
            pos: position.extend(pack_rgba8(color)),
            rot: Vec4::from(rotation),
            sca: scale.extend(0.0),
            tex: Vec4::new(0.0, 0.0, 1.0, 1.0),
        }
    }

    /// Set the instance UV0 transform by value.
    ///
    /// The shader computes `uv0 = translation + original_uv0 * scale`.
    pub fn with_texture_transform(mut self, translation: Vec2, scale: Vec2) -> Self {
        self.tex = Vec4::new(translation.x, translation.y, scale.x, scale.y);
        self
    }

    /// Update this instance's entity-local position without changing its color.
    pub fn set_position(&mut self, v: Vec3) {
        self.pos = v.extend(self.pos.w);
    }

    /// Update this instance's color multiplier without changing its position.
    pub fn set_color(&mut self, color: LinearRgba) {
        self.pos.w = pack_rgba8(color)
    }
}

/// Pack a color into one `f32` lane for the instance vertex buffer.
///
/// The shader reads this vertex attribute as `u32`; storing the bits in a
/// `f32` keeps the Rust-side instance layout as four `Vec4` columns.
fn pack_rgba8(color: LinearRgba) -> f32 {
    let [r, g, b, a] = color.to_u8_array().map(|x| x as u32);
    f32::from_bits(r | (g << 8) | (b << 16) | (a << 24))
}

/// Per-instance data for one instanced mesh entity.
///
/// `Instances` is replicated and extracted into the render world. Mutating the
/// existing `Vec<Instance>` is the intended way to animate large instance sets.
///
/// Frustum culling is automatically disabled for the host entity because the
/// instance cloud can be much larger than the source mesh AABB. Tephrite caches
/// the render-side GPU buffer capacity, so keeping the instance count stable
/// avoids allocation churn.
#[derive(Component, Clone)]
#[require(NoFrustumCulling)]
pub struct Instances(Vec<Instance>);

impl Instances {
    /// Create an instance component from any value that can become a `Vec<Instance>`.
    pub fn new(instances: impl Into<Vec<Instance>>) -> Self {
        Self(instances.into())
    }

    /// Borrow the current instance data.
    pub fn instances(&self) -> &[Instance] {
        &self.0
    }

    /// Mutably borrow the instance data for in-place animation.
    pub fn instances_mut(&mut self) -> &mut Vec<Instance> {
        &mut self.0
    }
}

/// The [`StandardMaterial`] used by an instanced mesh entity.
///
/// Do not add a regular `MeshMaterial3d<StandardMaterial>` to the same entity
/// unless you intentionally want Bevy to also draw one uninstanced copy of the
/// mesh.
#[derive(Component, Clone, Debug, Default, Deref, DerefMut, PartialEq, Eq)]
pub struct InstanceMeshMaterial3d(pub Handle<StandardMaterial>);

impl From<Handle<StandardMaterial>> for InstanceMeshMaterial3d {
    fn from(handle: Handle<StandardMaterial>) -> Self {
        Self(handle)
    }
}

impl crate::serialize::FastWrite for Instances {
    #[inline(always)]
    unsafe fn write_fast(&self, w: &mut impl crate::serialize::ByteSink) {
        w.put_pod_slice(&self.0);
    }
}

impl crate::serialize::FastRead for Instances {
    type Ret = Self;
    type Context = ();

    #[inline(always)]
    unsafe fn read_fast<'a, S: crate::serialize::ByteSource<'a>>(
        _: &mut Self::Context,
        r: &mut S,
    ) -> Self::Ret {
        Self(r.get_pod_vec::<Instance>())
    }
}

impl SyncComponent for Instances {
    type Target = Self;
}

impl ExtractComponent for Instances {
    type QueryData = &'static Instances;
    type QueryFilter = ();
    type Out = Self;

    fn extract_component(item: QueryItem<'_, '_, Self::QueryData>) -> Option<Self> {
        Some(item.clone())
    }
}

/// Render plugin for [`Instances`] and [`InstanceMeshMaterial3d`].
///
/// Tephrite adds this internally. Users typically interact with the public
/// components rather than adding this plugin directly.
pub struct InstancedMaterialPlugin;

impl Plugin for InstancedMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ExtractComponentPlugin::<Instances>::default());

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .init_resource::<InstanceRenderMaterialInstances>()
            .add_systems(
                ExtractSchedule,
                (
                    extract_instance_mesh_materials.in_set(MaterialExtractionSystems),
                    extract_instance_entity_transforms,
                ),
            )
            .add_render_command::<Opaque3d, DrawCustom>()
            .add_render_command::<Transparent3d, DrawCustom>()
            .add_render_command::<Shadow, DrawCustomShadow>()
            .init_resource::<SpecializedMeshPipelines<CustomPipeline>>()
            .init_resource::<SpecializedMeshPipelines<CustomShadowPipeline>>()
            .add_systems(
                RenderStartup,
                init_custom_pipelines.after(MeshPipelineSystems),
            )
            .add_systems(
                Render,
                (
                    ensure_custom_shadow_pipeline.in_set(RenderSystems::PrepareResources),
                    queue_custom.in_set(RenderSystems::QueueMeshes),
                    queue_custom_shadows
                        .after(bevy::pbr::queue_shadows)
                        .in_set(RenderSystems::QueueMeshes),
                    prepare_instance_buffers.in_set(RenderSystems::PrepareResources),
                ),
            );
    }
}

fn extract_instance_mesh_materials(
    mut material_instances: ResMut<InstanceRenderMaterialInstances>,
    materials_query: Extract<Query<(Entity, &ViewVisibility, &InstanceMeshMaterial3d)>>,
) {
    // Bevy's material queueing code normally discovers material handles from
    // `MeshMaterial3d<M>`. Instanced meshes use a separate component so they do
    // not get drawn once by Bevy's normal mesh path. This resource is the small
    // replacement lookup table our render commands use later.
    material_instances.instances.clear();

    for (entity, view_visibility, material) in &materials_query {
        let entity = MainEntity::from(entity);
        if view_visibility.get() {
            material_instances.instances.insert(
                entity,
                RenderMaterialInstance {
                    asset_id: material.id().untyped(),
                    last_change_tick: default(),
                },
            );
        }
    }
}

#[derive(Component, Clone, Copy)]
struct InstanceEntityTransform {
    // Entity-wide transform applied after each per-instance transform.
    translation: Vec3,
    rotation: Quat,
    scale: Vec3,
    // Bevy PBR mesh flags needed by fragment lighting. At the moment we only
    // forward shadow receiving, because that is the flag `apply_pbr_lighting`
    // checks before sampling shadow maps.
    flags: u32,
}

#[derive(Clone, Copy, Pod, ShaderType, Zeroable)]
#[repr(C)]
struct InstanceEntityUniform {
    /// `xyz`: entity translation, `w`: bindless material slot.
    pos: Vec4,
    /// Entity rotation quaternion, scalar last.
    rot: Vec4,
    /// `xyz`: entity scale, `w`: unused.
    sca: Vec4,
    /// `x`: Bevy PBR mesh flags for fragment lighting, remaining lanes unused.
    flags: UVec4,
}

impl InstanceEntityUniform {
    fn new(transform: InstanceEntityTransform, material_slot: u32) -> Self {
        Self {
            pos: transform.translation.extend(material_slot as f32),
            rot: Vec4::from(transform.rotation),
            sca: transform.scale.extend(0.0),
            flags: UVec4::new(transform.flags, 0, 0, 0),
        }
    }
}

fn extract_instance_entity_transforms(
    mut commands: Commands,
    query: Extract<
        Query<
            (
                RenderEntity,
                &ViewVisibility,
                &GlobalTransform,
                Has<bevy::light::NotShadowReceiver>,
            ),
            With<Instances>,
        >,
    >,
) {
    for (render_entity, view_visibility, transform, not_shadow_receiver) in &query {
        if !view_visibility.get() {
            continue;
        }

        let (scale, rotation, translation) = transform.to_scale_rotation_translation();
        // StandardMaterial's fragment path reads this bit from Bevy's mesh
        // uniform array. Our shader cannot use that array index because the
        // hardware instance index is the per-particle index, so we mirror the
        // relevant entity-wide flag into our own uniform instead.
        let flags = if not_shadow_receiver {
            MeshFlags::empty()
        } else {
            MeshFlags::SHADOW_RECEIVER
        }
        .bits();
        commands
            .entity(render_entity)
            .insert(InstanceEntityTransform {
                translation,
                rotation,
                scale,
                flags,
            });
    }
}

#[derive(Resource, Default)]
struct InstanceRenderMaterialInstances {
    // Main entity -> prepared StandardMaterial instance.
    instances: bevy::platform::collections::HashMap<MainEntity, RenderMaterialInstance>,
}

#[derive(SystemParam)]
struct QueueCustomParams<'w, 's> {
    opaque_3d_draw_functions: Res<'w, DrawFunctions<Opaque3d>>,
    transparent_3d_draw_functions: Res<'w, DrawFunctions<Transparent3d>>,
    custom_pipeline: Res<'w, CustomPipeline>,
    pipelines: ResMut<'w, SpecializedMeshPipelines<CustomPipeline>>,
    pipeline_cache: Res<'w, PipelineCache>,
    meshes: Res<'w, RenderAssets<RenderMesh>>,
    render_mesh_instances: Res<'w, RenderMeshInstances>,
    render_material_instances: Res<'w, InstanceRenderMaterialInstances>,
    render_materials: Res<'w, ErasedRenderAssets<PreparedMaterial>>,
    mesh_allocator: Res<'w, MeshAllocator>,
    maybe_batched_instance_buffers:
        Option<Res<'w, BatchedInstanceBuffers<MeshUniform, MeshInputUniform>>>,
    material_meshes: Query<'w, 's, (Entity, &'static MainEntity), With<Instances>>,
    opaque_render_phases: ResMut<'w, ViewBinnedRenderPhases<Opaque3d>>,
    transparent_render_phases: ResMut<'w, ViewSortedRenderPhases<Transparent3d>>,
    views: Query<'w, 's, &'static ExtractedView>,
    view_key_cache: Res<'w, ViewKeyCache>,
}

fn queue_custom(mut params: QueueCustomParams) {
    // Queue the visible pass. This intentionally mirrors the parts of Bevy's
    // material queueing that choose a specialized mesh pipeline, phase, and
    // sort/bin key, while substituting our draw command and shader.
    let draw_opaque = params.opaque_3d_draw_functions.read().id::<DrawCustom>();
    let draw_transparent = params
        .transparent_3d_draw_functions
        .read()
        .id::<DrawCustom>();

    for view in &params.views {
        let Some(&view_key) = params.view_key_cache.get(&view.retained_view_entity) else {
            continue;
        };

        for (entity, main_entity) in &params.material_meshes {
            let Some(mesh_instance) = params
                .render_mesh_instances
                .render_mesh_queue_data(*main_entity)
            else {
                continue;
            };
            let Some(mesh) = params.meshes.get(mesh_instance.mesh_asset_id()) else {
                continue;
            };
            let Some(material_instance) =
                params.render_material_instances.instances.get(main_entity)
            else {
                continue;
            };
            let Some(material) = params.render_materials.get(material_instance.asset_id) else {
                // debug!(
                //     target: "tephrite_rs::material::instance",
                //     main_entity = ?main_entity,
                //     render_entity = ?entity,
                //     asset_id = ?material_instance.asset_id,
                //     "skipping instanced visible queue: missing prepared material"
                // );
                continue;
            };
            let Some(mesh_slabs) = params
                .mesh_allocator
                .mesh_slabs(&mesh_instance.mesh_asset_id())
            else {
                continue;
            };

            let mut material_key_bits: MeshPipelineKey =
                material.properties.mesh_pipeline_key_bits.downcast();
            // `PreparedMaterial` carries the StandardMaterial feature bits, but
            // alpha mode still depends on the current view MSAA settings.
            material_key_bits.insert(alpha_mode_pipeline_key(
                material.properties.alpha_mode,
                &Msaa::from_samples(view_key.msaa_samples()),
            ));

            let key = view_key
                | MeshPipelineKey::from_primitive_topology_and_strip_index(
                    mesh.primitive_topology(),
                    mesh.index_format(),
                )
                | material_key_bits;
            let pipeline = params
                .pipelines
                .specialize(
                    &params.pipeline_cache,
                    &params.custom_pipeline,
                    key,
                    &mesh.layout,
                )
                .unwrap();

            match material.properties.render_phase_type {
                RenderPhaseType::Opaque
                    if material.properties.render_method == OpaqueRendererMethod::Forward =>
                {
                    let Some(opaque_phase) = params
                        .opaque_render_phases
                        .get_mut(&view.retained_view_entity)
                    else {
                        continue;
                    };
                    opaque_phase.add(
                        Opaque3dBatchSetKey {
                            pipeline,
                            draw_function: draw_opaque,
                            material_bind_group_index: Some(material.binding.group.0),
                            slabs: mesh_slabs,
                            lightmap_slab: mesh_instance
                                .shared
                                .lightmap_slab_index()
                                .map(|index| *index),
                        },
                        Opaque3dBinKey {
                            asset_id: mesh_instance.mesh_asset_id().into(),
                        },
                        (entity, *main_entity),
                        mesh_instance.current_uniform_index,
                        BinnedRenderPhaseType::UnbatchableMesh,
                    );
                }
                RenderPhaseType::Transparent | RenderPhaseType::Transmissive => {
                    let Some(transparent_phase) = params
                        .transparent_render_phases
                        .get_mut(&view.retained_view_entity)
                    else {
                        continue;
                    };
                    transparent_phase.add_retained(Transparent3d {
                        sorting_info: TransparentSortingInfo3d::Sorted {
                            mesh_center: bevy::pbr::get_mesh_instance_world_from_local(
                                *main_entity,
                                mesh_instance.current_uniform_index,
                                &params.render_mesh_instances,
                                params.maybe_batched_instance_buffers.as_deref(),
                            )
                            .transform_point3(mesh.aabb_center),
                            depth_bias: material.properties.depth_bias,
                        },
                        entity: (entity, *main_entity),
                        pipeline,
                        draw_function: draw_transparent,
                        distance: 0.0,
                        batch_range: 0..1,
                        extra_index: PhaseItemExtraIndex::None,
                        indexed: mesh_slabs.index_slab_id.is_some(),
                    });
                }
                _ => {}
            }
        }
    }
}

fn queue_custom_shadows(
    shadow_draw_functions: Res<DrawFunctions<Shadow>>,
    custom_pipeline: Option<Res<CustomShadowPipeline>>,
    mut pipelines: ResMut<SpecializedMeshPipelines<CustomShadowPipeline>>,
    pipeline_cache: Res<PipelineCache>,
    meshes: Res<RenderAssets<RenderMesh>>,
    render_mesh_instances: Res<RenderMeshInstances>,
    render_material_instances: Res<InstanceRenderMaterialInstances>,
    render_materials: Res<ErasedRenderAssets<PreparedMaterial>>,
    mesh_allocator: Res<MeshAllocator>,
    light_key_cache: Option<Res<LightKeyCache>>,
    shadow_render_phases: Option<ResMut<ViewBinnedRenderPhases<Shadow>>>,
    view_light_entities: Query<(&LightEntity, &ExtractedView, &RenderLayers)>,
    shadow_map_visible_entities_query: Query<&RenderShadowMapVisibleEntities>,
    material_meshes: Query<(Entity, &MainEntity), With<Instances>>,
) {
    // Queue the shadow-map pass for each light subview. Bevy's built-in shadow
    // queue only knows about its own draw functions, so instanced meshes need
    // to add their own shadow phase items after the standard queue runs.
    let Some(custom_pipeline) = custom_pipeline else {
        return;
    };
    let Some(light_key_cache) = light_key_cache else {
        return;
    };
    let Some(mut shadow_render_phases) = shadow_render_phases else {
        return;
    };

    let draw_shadow = shadow_draw_functions.read().id::<DrawCustomShadow>();

    for (light_entity, extracted_view_light, view_light_render_layers) in &view_light_entities {
        let Some(shadow_phase) =
            shadow_render_phases.get_mut(&extracted_view_light.retained_view_entity)
        else {
            continue;
        };
        let Some(&light_key) = light_key_cache.get(&extracted_view_light.retained_view_entity)
        else {
            continue;
        };

        let visible_entities = get_shadow_map_visible_entities(
            &shadow_map_visible_entities_query,
            light_entity,
            extracted_view_light,
        );

        let Some(visible_entities) = visible_entities.get::<Mesh3d>() else {
            continue;
        };

        for (render_entity, main_entity) in &material_meshes {
            // Use Bevy's shadow visibility results. This keeps render layers,
            // light frusta, and shadow-caster filtering aligned with regular
            // meshes.
            if !shadow_visible_entities_contains(visible_entities, *main_entity) {
                continue;
            }
            let Some(mesh_instance) = render_mesh_instances.render_mesh_queue_data(*main_entity)
            else {
                continue;
            };
            if !mesh_instance
                .flags()
                .contains(RenderMeshInstanceFlags::SHADOW_CASTER)
            {
                continue;
            }

            let mesh_layers = mesh_instance.render_layers.as_ref().unwrap_or_default();
            if !view_light_render_layers.intersects(mesh_layers) {
                continue;
            }

            let Some(material_instance) = render_material_instances.instances.get(main_entity)
            else {
                continue;
            };
            let Some(material) = render_materials.get(material_instance.asset_id) else {
                // debug!(
                //     target: "tephrite_rs::material::instance",
                //     main_entity = ?main_entity,
                //     render_entity = ?render_entity,
                //     asset_id = ?material_instance.asset_id,
                //     "skipping instanced shadow queue: missing prepared material"
                // );
                continue;
            };
            if !material.properties.shadows_enabled {
                continue;
            }

            let Some(mesh) = meshes.get(mesh_instance.mesh_asset_id()) else {
                continue;
            };
            let Some(mesh_slabs) = mesh_allocator.mesh_slabs(&mesh_instance.mesh_asset_id()) else {
                continue;
            };

            let key = light_key
                | MeshPipelineKey::from_bits_retain(mesh.key_bits.bits())
                | MeshPipelineKey::from_primitive_topology_and_strip_index(
                    mesh.primitive_topology(),
                    mesh.index_format(),
                );
            let pipeline = pipelines
                .specialize(&pipeline_cache, &custom_pipeline, key, &mesh.layout)
                .unwrap();

            shadow_phase.add(
                ShadowBatchSetKey {
                    pipeline,
                    draw_function: draw_shadow,
                    material_bind_group_index: None,
                    slabs: mesh_slabs,
                },
                ShadowBinKey {
                    asset_id: mesh_instance.mesh_asset_id().into(),
                },
                (render_entity, *main_entity),
                mesh_instance.current_uniform_index,
                BinnedRenderPhaseType::NonMesh,
            );
        }
    }
}

fn shadow_visible_entities_contains(
    visible_entities: &bevy::render::view::RenderVisibleEntitiesClass,
    main_entity: MainEntity,
) -> bool {
    // Shadow visibility can be stored in either CPU-culled or GPU-culled lists.
    visible_entities
        .entities_cpu_culling
        .iter()
        .any(|(_, visible_entity)| *visible_entity == main_entity)
        || visible_entities
            .entities_gpu_culling
            .contains_key(&main_entity)
}

fn get_shadow_map_visible_entities<'w, 's: 'w>(
    shadow_map_visible_entities_query: &'w Query<'w, 's, &'_ RenderShadowMapVisibleEntities>,
    light_entity: &'_ LightEntity,
    extracted_view_light: &'_ ExtractedView,
) -> &'w RenderVisibleEntities {
    // Directional, point, and spot lights store their shadow-map visible
    // entities under slightly different retained view keys.
    match light_entity {
        LightEntity::Directional { light_entity, .. } => shadow_map_visible_entities_query
            .get(*light_entity)
            .expect("Failed to get directional light visible entities")
            .subviews
            .get(&extracted_view_light.retained_view_entity)
            .expect("Failed to get directional light visible entities for cascade"),
        LightEntity::Point {
            light_entity,
            face_index,
        } => {
            let retained_view_entity = RetainedViewEntity {
                main_entity: extracted_view_light.retained_view_entity.main_entity,
                auxiliary_entity: MainEntity::from(Entity::PLACEHOLDER),
                subview_index: *face_index as u32,
            };
            shadow_map_visible_entities_query
                .get(*light_entity)
                .expect("Failed to get point light visible entities")
                .subviews
                .get(&retained_view_entity)
                .expect("Failed to get point light visible entity for face")
        }
        LightEntity::Spot { light_entity } => {
            let retained_view_entity = RetainedViewEntity {
                main_entity: extracted_view_light.retained_view_entity.main_entity,
                auxiliary_entity: MainEntity::from(Entity::PLACEHOLDER),
                subview_index: 0,
            };
            shadow_map_visible_entities_query
                .get(*light_entity)
                .expect("Failed to get spot light visible entities")
                .subviews
                .get(&retained_view_entity)
                .expect("Failed to get spot light visible entity for view")
        }
    }
}

#[derive(Component)]
struct InstanceBuffer {
    // GPU vertex buffer containing tightly-packed `Instance` values.
    buffer: Buffer,
    // Number of live instances in `buffer`.
    length: usize,
    // Allocated instance capacity. We grow geometrically and reuse the buffer
    // while `length <= capacity`.
    capacity: usize,
}

#[derive(Component)]
struct InstanceEntityBindGroup {
    // Kept alive so the bind group can reference it.
    _buffer: Buffer,
    bind_group: BindGroup,
}

fn prepare_instance_buffers(
    mut commands: Commands,
    mut query: Query<(
        Entity,
        &MainEntity,
        &Instances,
        &InstanceEntityTransform,
        Option<&mut InstanceBuffer>,
        Option<&InstanceEntityBindGroup>,
    )>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    custom_pipeline: Res<CustomPipeline>,
    render_material_instances: Res<InstanceRenderMaterialInstances>,
    render_materials: Res<ErasedRenderAssets<PreparedMaterial>>,
) {
    for (
        entity,
        main_entity,
        instance_data,
        entity_transform,
        instance_buffer,
        entity_bind_group,
    ) in &mut query
    {
        let instances = instance_data.instances();
        // The material slot is only meaningful for bindless materials. For the
        // non-bindless path it is harmless and ignored by the shader.
        let material_slot = render_material_instances
            .instances
            .get(main_entity)
            .and_then(|material_instance| render_materials.get(material_instance.asset_id))
            .map_or(0, |material| u32::from(material.binding.slot));

        let instance_bytes = bytemuck::cast_slice(instances);
        match instance_buffer {
            Some(mut instance_buffer) if instance_buffer.capacity >= instances.len() => {
                // Fast path: rewrite the existing GPU buffer in-place.
                if !instance_bytes.is_empty() {
                    render_queue.write_buffer(&instance_buffer.buffer, 0, instance_bytes);
                }
                instance_buffer.length = instances.len();
            }
            _ => {
                // Grow capacity to the next power of two so minor count changes
                // do not allocate every frame.
                let capacity = instances.len().max(1).next_power_of_two();
                let buffer = render_device.create_buffer(&BufferDescriptor {
                    label: Some("instance data buffer"),
                    size: (capacity * size_of::<Instance>()) as u64,
                    usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                if !instance_bytes.is_empty() {
                    render_queue.write_buffer(&buffer, 0, instance_bytes);
                }
                commands.entity(entity).insert(InstanceBuffer {
                    buffer,
                    length: instances.len(),
                    capacity,
                });
            }
        }

        let uniform = InstanceEntityUniform::new(*entity_transform, material_slot);
        let uniform_bytes = bytemuck::bytes_of(&uniform);
        if let Some(entity_bind_group) = entity_bind_group {
            // The bind group can be reused; only the uniform content changes.
            render_queue.write_buffer(&entity_bind_group._buffer, 0, uniform_bytes);
        } else {
            let uniform_buffer = render_device.create_buffer(&BufferDescriptor {
                label: Some("instance entity uniform buffer"),
                size: size_of::<InstanceEntityUniform>() as u64,
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            render_queue.write_buffer(&uniform_buffer, 0, uniform_bytes);
            let bind_group = render_device.create_bind_group(
                "instance entity bind group",
                &custom_pipeline.instance_entity_layout,
                &BindGroupEntries::single(uniform_buffer.as_entire_binding()),
            );
            commands.entity(entity).insert(InstanceEntityBindGroup {
                _buffer: uniform_buffer,
                bind_group,
            });
        }
    }
}

#[derive(Resource)]
struct CustomPipeline {
    // Shared WGSL used for both the visible and shadow pipelines.
    shader: Handle<Shader>,
    // Bevy's base mesh pipeline provides view, mesh, lighting, depth, MSAA,
    // topology, and most shader-def specialization logic.
    mesh_pipeline: MeshPipeline,
    // StandardMaterial's bind group layout is still used unchanged.
    material_layout: BindGroupLayoutDescriptor,
    // Extra per-entity instancing uniform layout inserted at group 4.
    instance_entity_layout_descriptor: BindGroupLayoutDescriptor,
    instance_entity_layout: BindGroupLayout,
    // Whether the current renderer prepares StandardMaterial bindlessly.
    bindless: bool,
}

#[derive(Resource)]
struct CustomShadowPipeline {
    shader: Handle<Shader>,
    prepass_pipeline: PrepassPipeline,
    instance_entity_layout_descriptor: BindGroupLayoutDescriptor,
}

fn init_custom_pipelines(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mesh_pipeline: Res<MeshPipeline>,
    render_device: Res<RenderDevice>,
) {
    let shader = asset_server.load(INSTANCING_SHADER_ASSET_PATH);
    // This uniform is read in vertex shaders for transforms and in fragment
    // shaders for PBR mesh flags, so both stages need visibility.
    let instance_entity_layout_descriptor = BindGroupLayoutDescriptor::new(
        "instance_entity_bind_group_layout",
        &BindGroupLayoutEntries::single(
            ShaderStages::VERTEX_FRAGMENT,
            binding_types::uniform_buffer::<InstanceEntityUniform>(false),
        ),
    );
    let instance_entity_layout = render_device.create_bind_group_layout(
        instance_entity_layout_descriptor.label.as_ref(),
        &instance_entity_layout_descriptor.entries,
    );

    commands.insert_resource(CustomPipeline {
        shader: shader.clone(),
        mesh_pipeline: mesh_pipeline.clone(),
        material_layout: StandardMaterial::bind_group_layout_descriptor(&render_device),
        instance_entity_layout_descriptor,
        instance_entity_layout,
        bindless: material_uses_bindless_resources::<StandardMaterial>(&render_device),
    });
}

fn ensure_custom_shadow_pipeline(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    prepass_pipeline: Option<Res<PrepassPipeline>>,
    custom_pipeline: Option<Res<CustomPipeline>>,
    custom_shadow_pipeline: Option<Res<CustomShadowPipeline>>,
) {
    // `PrepassPipeline` may not exist in every render configuration. Build the
    // shadow pipeline lazily once Bevy has inserted it, and skip shadow queuing
    // until then.
    if custom_shadow_pipeline.is_some() {
        return;
    }

    if let (Some(prepass_pipeline), Some(custom_pipeline)) = (prepass_pipeline, custom_pipeline) {
        commands.insert_resource(CustomShadowPipeline {
            shader: asset_server.load(INSTANCING_SHADER_ASSET_PATH),
            prepass_pipeline: prepass_pipeline.clone(),
            instance_entity_layout_descriptor: custom_pipeline
                .instance_entity_layout_descriptor
                .clone(),
        });
    }
}

impl SpecializedMeshPipeline for CustomPipeline {
    type Key = MeshPipelineKey;

    fn specialize(
        &self,
        key: Self::Key,
        layout: &MeshVertexBufferLayoutRef,
    ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
        // Start from Bevy's mesh pipeline so all view/light/depth layouts and
        // mesh shader defs remain compatible with StandardMaterial.
        let mut descriptor = self.mesh_pipeline.specialize(key, layout)?;

        // Replace only the shader entry points and add our extra instance-rate
        // vertex buffer and entity uniform bind group.
        descriptor.vertex.shader_defs.push(ShaderDefVal::UInt(
            "MATERIAL_BIND_GROUP".into(),
            MATERIAL_BIND_GROUP_INDEX as u32,
        ));
        if self.bindless {
            descriptor.vertex.shader_defs.push("BINDLESS".into());
        }
        descriptor.vertex.shader = self.shader.clone();
        descriptor.vertex.entry_point = Some("vertex".into());
        descriptor
            .vertex
            .buffers
            .push(instance_vertex_buffer_layout());
        descriptor
            .layout
            .insert(MATERIAL_BIND_GROUP_INDEX, self.material_layout.clone());
        descriptor.layout.insert(
            INSTANCE_ENTITY_BIND_GROUP,
            self.instance_entity_layout_descriptor.clone(),
        );
        let fragment = descriptor.fragment.as_mut().unwrap();
        fragment.shader = self.shader.clone();
        fragment.entry_point = Some("fragment".into());
        fragment.shader_defs.push(ShaderDefVal::UInt(
            "MATERIAL_BIND_GROUP".into(),
            MATERIAL_BIND_GROUP_INDEX as u32,
        ));
        if self.bindless {
            fragment.shader_defs.push("BINDLESS".into());
        }
        Ok(descriptor)
    }
}

impl SpecializedMeshPipeline for CustomShadowPipeline {
    type Key = MeshPipelineKey;

    fn specialize(
        &self,
        key: Self::Key,
        layout: &MeshVertexBufferLayoutRef,
    ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
        // The shadow path is a compact prepass-style pipeline. It only needs
        // mesh positions plus our instance transform data, but must preserve
        // Bevy's shadow-view shader defs such as unclipped orthographic depth.
        let mut shader_defs = Vec::new();
        shader_defs.push("PREPASS_PIPELINE".into());
        shader_defs.push("VERTEX_OUTPUT_INSTANCE_INDEX".into());

        let view_projection = key.intersection(MeshPipelineKey::VIEW_PROJECTION_RESERVED_BITS);
        if view_projection == MeshPipelineKey::VIEW_PROJECTION_NONSTANDARD {
            shader_defs.push("VIEW_PROJECTION_NONSTANDARD".into());
        } else if view_projection == MeshPipelineKey::VIEW_PROJECTION_PERSPECTIVE {
            shader_defs.push("VIEW_PROJECTION_PERSPECTIVE".into());
        } else if view_projection == MeshPipelineKey::VIEW_PROJECTION_ORTHOGRAPHIC {
            shader_defs.push("VIEW_PROJECTION_ORTHOGRAPHIC".into());
        }
        if key.contains(MeshPipelineKey::DEPTH_PREPASS) {
            shader_defs.push("DEPTH_PREPASS".into());
        }

        let mut vertex_attributes = vec![Mesh::ATTRIBUTE_POSITION.at_shader_location(0)];
        // Keep morphing/skinning related defs in sync with Bevy's prepass
        // pipeline, even though common Tephrite instance meshes are static.
        let mesh_layout = setup_morph_and_skinning_defs(
            &self.prepass_pipeline.mesh_layouts,
            layout,
            8,
            &key,
            &mut shader_defs,
            &mut vertex_attributes,
            self.prepass_pipeline.skins_use_uniform_buffers,
        );
        let vertex_buffer_layout = layout.0.get_layout(&vertex_attributes)?;

        let needs_unclipped_depth = key.contains(MeshPipelineKey::UNCLIPPED_DEPTH_ORTHO);
        let unclipped_depth =
            needs_unclipped_depth && self.prepass_pipeline.depth_clip_control_supported;
        let emulate_unclipped_depth =
            needs_unclipped_depth && !self.prepass_pipeline.depth_clip_control_supported;
        if emulate_unclipped_depth {
            shader_defs.push("UNCLIPPED_DEPTH_ORTHO_EMULATION".into());
        }

        Ok(RenderPipelineDescriptor {
            vertex: VertexState {
                shader: self.shader.clone(),
                entry_point: Some("shadow_vertex".into()),
                shader_defs: shader_defs.clone(),
                buffers: vec![vertex_buffer_layout, instance_vertex_buffer_layout()],
                ..default()
            },
            fragment: emulate_unclipped_depth.then(|| FragmentState {
                shader: self.shader.clone(),
                entry_point: Some("shadow_fragment".into()),
                shader_defs,
                targets: Vec::new(),
                ..default()
            }),
            layout: vec![
                self.prepass_pipeline.view_layout_no_motion_vectors.clone(),
                self.prepass_pipeline.empty_layout.clone(),
                mesh_layout,
                self.prepass_pipeline.empty_layout.clone(),
                self.instance_entity_layout_descriptor.clone(),
            ],
            primitive: PrimitiveState {
                topology: key.primitive_topology(),
                strip_index_format: key.strip_index_format(),
                unclipped_depth,
                ..default()
            },
            depth_stencil: Some(DepthStencilState {
                format: CORE_3D_DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(CompareFunction::GreaterEqual),
                stencil: StencilState {
                    front: StencilFaceState::IGNORE,
                    back: StencilFaceState::IGNORE,
                    read_mask: 0,
                    write_mask: 0,
                },
                bias: DepthBiasState {
                    constant: 0,
                    slope_scale: 0.0,
                    clamp: 0.0,
                },
            }),
            multisample: MultisampleState {
                count: key.msaa_samples(),
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            label: Some("instanced_shadow_pipeline".into()),
            ..default()
        })
    }
}

fn instance_vertex_buffer_layout() -> VertexBufferLayout {
    // Locations 0..2 are the source mesh attributes. Locations 3..7 are
    // instance-rate attributes consumed by `instancing.wgsl`.
    VertexBufferLayout {
        array_stride: size_of::<Instance>() as u64,
        step_mode: VertexStepMode::Instance,
        attributes: vec![
            VertexAttribute {
                format: VertexFormat::Float32x3,
                offset: 0,
                shader_location: 3,
            },
            VertexAttribute {
                format: VertexFormat::Uint32,
                offset: VertexFormat::Float32x3.size(),
                shader_location: 4,
            },
            VertexAttribute {
                format: VertexFormat::Float32x4,
                offset: VertexFormat::Float32x4.size(),
                shader_location: 5,
            },
            VertexAttribute {
                format: VertexFormat::Float32x4,
                offset: VertexFormat::Float32x4.size() * 2,
                shader_location: 6,
            },
            VertexAttribute {
                format: VertexFormat::Float32x4,
                offset: VertexFormat::Float32x4.size() * 3,
                shader_location: 7,
            },
        ],
    }
}

type DrawCustom = (
    // Bind groups:
    // 0: view, 1: view binding arrays, 2: mesh uniforms,
    // 3: StandardMaterial, 4: instanced entity uniform.
    SetItemPipeline,
    SetMeshViewBindGroup<0>,
    SetMeshViewBindingArrayBindGroup<1>,
    SetMeshBindGroup<2>,
    SetInstanceMaterialBindGroup<MATERIAL_BIND_GROUP_INDEX>,
    SetInstanceEntityBindGroup<INSTANCE_ENTITY_BIND_GROUP>,
    DrawMeshInstanced,
);

type DrawCustomShadow = (
    // Shadow/prepass bind groups:
    // 0: shadow view, 1: empty view arrays, 2: mesh uniforms,
    // 3: empty material, 4: instanced entity uniform.
    SetItemPipeline,
    SetPrepassViewBindGroup<0>,
    SetPrepassViewEmptyBindGroup<1>,
    SetMeshBindGroup<2>,
    SetPrepassEmptyMaterialBindGroup<3>,
    SetInstanceEntityBindGroup<INSTANCE_ENTITY_BIND_GROUP>,
    DrawMeshInstanced,
);

struct DrawMeshInstanced;

struct SetInstanceMaterialBindGroup<const I: usize>;

struct SetInstanceEntityBindGroup<const I: usize>;

impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetInstanceMaterialBindGroup<I> {
    type Param = (
        SRes<ErasedRenderAssets<PreparedMaterial>>,
        SRes<InstanceRenderMaterialInstances>,
        SRes<MaterialBindGroupAllocators>,
    );
    type ViewQuery = ();
    type ItemQuery = ();

    #[inline]
    fn render<'w>(
        item: &P,
        _view: (),
        _item_query: Option<()>,
        (materials, material_instances, material_bind_group_allocator): SystemParamItem<
            'w,
            '_,
            Self::Param,
        >,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        // Prepared materials are stored by untyped asset id. The material's
        // allocator owns the actual bind groups; this command resolves and
        // binds the one selected when the material was prepared.
        let materials = materials.into_inner();
        let material_instances = material_instances.into_inner();
        let material_bind_group_allocators = material_bind_group_allocator.into_inner();

        let Some(material_instance) = material_instances.instances.get(&item.main_entity()) else {
            // debug!(
            //     target: "tephrite_rs::material::instance",
            //     main_entity = ?item.main_entity(),
            //     "skipping instanced material bind: missing material instance"
            // );
            return RenderCommandResult::Skip;
        };
        let Some(material_bind_group_allocator) =
            material_bind_group_allocators.get(&material_instance.asset_id.type_id())
        else {
            // debug!(
            //     target: "tephrite_rs::material::instance",
            //     main_entity = ?item.main_entity(),
            //     asset_id = ?material_instance.asset_id,
            //     "skipping instanced material bind: missing material bind group allocator"
            // );
            return RenderCommandResult::Skip;
        };
        let Some(material) = materials.get(material_instance.asset_id) else {
            // debug!(
            //     target: "tephrite_rs::material::instance",
            //     main_entity = ?item.main_entity(),
            //     asset_id = ?material_instance.asset_id,
            //     "skipping instanced material bind: missing prepared material"
            // );
            return RenderCommandResult::Skip;
        };
        let Some(material_bind_group) = material_bind_group_allocator.get(material.binding.group)
        else {
            // debug!(
            //     target: "tephrite_rs::material::instance",
            //     main_entity = ?item.main_entity(),
            //     group = ?material.binding.group,
            //     "skipping instanced material bind: missing material bind group"
            // );
            return RenderCommandResult::Skip;
        };
        let Some(bind_group) = material_bind_group.bind_group() else {
            // debug!(
            //     target: "tephrite_rs::material::instance",
            //     main_entity = ?item.main_entity(),
            //     group = ?material.binding.group,
            //     "skipping instanced material bind: bind group not ready"
            // );
            return RenderCommandResult::Skip;
        };

        pass.set_bind_group(I, bind_group, &[]);
        RenderCommandResult::Success
    }
}

impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetInstanceEntityBindGroup<I> {
    type Param = ();
    type ViewQuery = ();
    type ItemQuery = Read<InstanceEntityBindGroup>;

    #[inline]
    fn render<'w>(
        _item: &P,
        _view: (),
        bind_group: Option<&'w InstanceEntityBindGroup>,
        _param: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        // Bind the per-entity uniform that carries the host transform, bindless
        // material slot, and PBR mesh flags.
        let Some(bind_group) = bind_group else {
            // debug!(
            //     target: "tephrite_rs::material::instance",
            //     main_entity = ?item.main_entity(),
            //     "skipping instanced entity bind: missing bind group"
            // );
            return RenderCommandResult::Skip;
        };

        pass.set_bind_group(I, &bind_group.bind_group, &[]);
        RenderCommandResult::Success
    }
}

impl<P: PhaseItem> RenderCommand<P> for DrawMeshInstanced {
    type Param = (
        SRes<RenderAssets<RenderMesh>>,
        SRes<RenderMeshInstances>,
        SRes<MeshAllocator>,
    );
    type ViewQuery = ();
    type ItemQuery = Read<InstanceBuffer>;

    #[inline]
    fn render<'w>(
        item: &P,
        _view: (),
        instance_buffer: Option<&'w InstanceBuffer>,
        (meshes, render_mesh_instances, mesh_allocator): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        // This is the only draw command that differs materially from Bevy's
        // standard mesh draw: vertex buffer 0 is the mesh, vertex buffer 1 is
        // our instance data, and the draw range uses the live instance count.
        // A borrow check workaround.
        let mesh_allocator = mesh_allocator.into_inner();

        let Some(mesh_asset_id) = render_mesh_instances.mesh_asset_id(item.main_entity()) else {
            // debug!(
            //     target: "tephrite_rs::material::instance",
            //     main_entity = ?item.main_entity(),
            //     "skipping instanced draw: missing mesh asset id"
            // );
            return RenderCommandResult::Skip;
        };
        let Some(gpu_mesh) = meshes.into_inner().get(mesh_asset_id) else {
            // debug!(
            //     target: "tephrite_rs::material::instance",
            //     main_entity = ?item.main_entity(),
            //     mesh_asset_id = ?mesh_asset_id,
            //     "skipping instanced draw: missing gpu mesh"
            // );
            return RenderCommandResult::Skip;
        };
        let Some(instance_buffer) = instance_buffer else {
            // debug!(
            //     target: "tephrite_rs::material::instance",
            //     main_entity = ?item.main_entity(),
            //     "skipping instanced draw: missing instance buffer"
            // );
            return RenderCommandResult::Skip;
        };
        let Some(vertex_buffer_slice) = mesh_allocator.mesh_vertex_slice(&mesh_asset_id) else {
            // debug!(
            //     target: "tephrite_rs::material::instance",
            //     main_entity = ?item.main_entity(),
            //     mesh_asset_id = ?mesh_asset_id,
            //     "skipping instanced draw: missing mesh vertex slice"
            // );
            return RenderCommandResult::Skip;
        };

        pass.set_vertex_buffer(0, vertex_buffer_slice.buffer.slice(..));
        pass.set_vertex_buffer(1, instance_buffer.buffer.slice(..));

        match &gpu_mesh.buffer_info {
            RenderMeshBufferInfo::Indexed {
                index_format,
                count,
            } => {
                let Some(index_buffer_slice) = mesh_allocator.mesh_index_slice(&mesh_asset_id)
                else {
                    // debug!(
                    //     target: "tephrite_rs::material::instance",
                    //     main_entity = ?item.main_entity(),
                    //     mesh_asset_id = ?mesh_asset_id,
                    //     "skipping instanced draw: missing mesh index slice"
                    // );
                    return RenderCommandResult::Skip;
                };

                pass.set_index_buffer(index_buffer_slice.buffer.slice(..), *index_format);
                pass.draw_indexed(
                    index_buffer_slice.range.start..(index_buffer_slice.range.start + count),
                    vertex_buffer_slice.range.start as i32,
                    0..instance_buffer.length as u32,
                );
            }
            RenderMeshBufferInfo::NonIndexed => {
                pass.draw(vertex_buffer_slice.range, 0..instance_buffer.length as u32);
            }
        }
        RenderCommandResult::Success
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::offset_of;

    #[test]
    fn instance_layout_matches_shader_columns() {
        assert_eq!(size_of::<Instance>(), size_of::<Vec4>() * 4);
        assert_eq!(offset_of!(Instance, pos), 0);
        assert_eq!(offset_of!(Instance, rot), size_of::<Vec4>());
        assert_eq!(offset_of!(Instance, sca), size_of::<Vec4>() * 2);
        assert_eq!(offset_of!(Instance, tex), size_of::<Vec4>() * 3);
    }

    #[test]
    fn instance_entity_uniform_layout_matches_shader() {
        assert_eq!(size_of::<InstanceEntityUniform>(), size_of::<Vec4>() * 4);
        assert_eq!(offset_of!(InstanceEntityUniform, pos), 0);
        assert_eq!(offset_of!(InstanceEntityUniform, rot), size_of::<Vec4>());
        assert_eq!(
            offset_of!(InstanceEntityUniform, sca),
            size_of::<Vec4>() * 2
        );
        assert_eq!(
            offset_of!(InstanceEntityUniform, flags),
            size_of::<Vec4>() * 3
        );
    }

    #[test]
    fn instance_packs_color_into_position_w() {
        let instance = Instance::new(
            Vec3::ZERO,
            Quat::IDENTITY,
            Vec3::ONE,
            LinearRgba::new(1.0, 0.5, 0.0, 1.0),
        );

        assert_eq!(instance.pos.w.to_bits(), 0xff0080ff);
    }

    #[test]
    fn instance_packs_white_as_rgba8() {
        let instance = Instance::new(Vec3::ZERO, Quat::IDENTITY, Vec3::ONE, LinearRgba::WHITE);

        assert_eq!(instance.pos.w.to_bits(), 0xffffffff);
    }

    #[test]
    fn instance_entity_uniform_packs_transform_and_material_slot() {
        let rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let uniform = InstanceEntityUniform::new(
            InstanceEntityTransform {
                translation: Vec3::new(10.0, 20.0, 30.0),
                rotation,
                scale: Vec3::new(5.0, 6.0, 7.0),
                flags: MeshFlags::SHADOW_RECEIVER.bits(),
            },
            42,
        );

        assert!(
            uniform
                .pos
                .xyz()
                .abs_diff_eq(Vec3::new(10.0, 20.0, 30.0), 0.0001)
        );
        assert_eq!(uniform.pos.w, 42.0);
        assert!(Quat::from_vec4(uniform.rot).abs_diff_eq(rotation, 0.0001));
        assert!(
            uniform
                .sca
                .xyz()
                .abs_diff_eq(Vec3::new(5.0, 6.0, 7.0), 0.0001)
        );
        assert_eq!(uniform.sca.w, 0.0);
        assert_eq!(uniform.flags.x, MeshFlags::SHADOW_RECEIVER.bits());
        assert_eq!(uniform.flags.yzw(), UVec3::ZERO);
    }
}
