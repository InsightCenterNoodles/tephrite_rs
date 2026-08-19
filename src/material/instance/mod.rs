use bevy::{
    camera::visibility::{NoFrustumCulling, ViewVisibility},
    core_pipeline::core_3d::{Transparent3d, TransparentSortingInfo3d},
    ecs::{
        query::QueryItem,
        system::{
            SystemParamItem,
            lifetimeless::{Read, SRes},
        },
    },
    mesh::{MeshVertexBufferLayoutRef, VertexBufferLayout},
    pbr::{
        MATERIAL_BIND_GROUP_INDEX, MaterialExtractionSystems, MeshInputUniform, MeshPipeline,
        MeshPipelineKey, MeshPipelineSystems, MeshUniform, PreparedMaterial,
        RenderMaterialInstance, RenderMaterialInstances, RenderMeshInstances, SetMaterialBindGroup,
        SetMeshBindGroup, SetMeshViewBindGroup, SetMeshViewBindingArrayBindGroup, StandardMaterial,
        ViewKeyCache, material_uses_bindless_resources,
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
        renderer::RenderDevice,
        sync_component::SyncComponent,
        sync_world::*,
        view::ExtractedView,
        *,
    },
    shader::ShaderDefVal,
};
use bytemuck::{Pod, Zeroable};

pub const INSTANCING_SHADER_ASSET_PATH: &str =
    "embedded://tephrite_rs/material/instance/instancing.wgsl";

/// Convention:
/// - xyz: world-space position, w: packed rgba8 color
/// - xyz w: rotation quat, scalar last
/// - xyz: scale, w: unused
/// - xy: texture translation, zw: texture scale
#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct Instance {
    pub pos: Vec4,
    pub rot: Vec4,
    pub sca: Vec4,
    pub tex: Vec4,
}

impl Instance {
    pub fn new(position: Vec3, rotation: Quat, scale: Vec3, color: LinearRgba) -> Self {
        Self {
            pos: position.extend(pack_rgba8(color)),
            rot: Vec4::from(rotation),
            sca: scale.extend(0.0),
            tex: Vec4::new(0.0, 0.0, 1.0, 1.0),
        }
    }

    pub fn with_texture_transform(mut self, translation: Vec2, scale: Vec2) -> Self {
        self.tex = Vec4::new(translation.x, translation.y, scale.x, scale.y);
        self
    }

    pub fn set_position(&mut self, v: Vec3) {
        self.pos = v.extend(self.pos.w);
    }
}

fn pack_rgba8(color: LinearRgba) -> f32 {
    let [r, g, b, a] = color.to_u8_array().map(|x| x as u32);
    //dbg!(r, g, b, a);
    f32::from_bits(r | (g << 8) | (b << 16) | (a << 24))
}

/// This component stores instance data used for splatting.
///
/// It is required to disable frustum culling, as we cannot compute an efficient bounding box for instances.
#[derive(Component, Clone)]
#[require(NoFrustumCulling)]
pub struct Instances(Vec<Instance>);

impl Instances {
    pub fn new(instances: impl Into<Vec<Instance>>) -> Self {
        Self(instances.into())
    }

    pub fn instances(&self) -> &[Instance] {
        &self.0
    }

    pub fn instances_mut(&mut self) -> &mut Vec<Instance> {
        &mut self.0
    }
}

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

    #[inline(always)]
    unsafe fn read_fast<'a, S: crate::serialize::ByteSource<'a>>(r: &mut S) -> Self::Ret {
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

pub struct InstancedMaterialPlugin;

impl Plugin for InstancedMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ExtractComponentPlugin::<Instances>::default());

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .add_systems(
                ExtractSchedule,
                (
                    extract_instance_mesh_materials.in_set(MaterialExtractionSystems),
                    early_sweep_instance_mesh_materials.in_set(MaterialExtractionSystems),
                ),
            )
            .add_render_command::<Transparent3d, DrawCustom>()
            .init_resource::<SpecializedMeshPipelines<CustomPipeline>>()
            .add_systems(
                RenderStartup,
                init_custom_pipeline.after(MeshPipelineSystems),
            )
            .add_systems(
                Render,
                (
                    queue_custom.in_set(RenderSystems::QueueMeshes),
                    prepare_instance_buffers.in_set(RenderSystems::PrepareResources),
                ),
            );
    }
}

fn extract_instance_mesh_materials(
    mut material_instances: ResMut<RenderMaterialInstances>,
    changed_materials_query: Extract<
        Query<
            (Entity, &ViewVisibility, &InstanceMeshMaterial3d),
            Or<(
                Changed<ViewVisibility>,
                Changed<InstanceMeshMaterial3d>,
                Changed<Instances>,
            )>,
        >,
    >,
) {
    let last_change_tick = material_instances.current_change_tick;

    for (entity, view_visibility, material) in &changed_materials_query {
        let entity = MainEntity::from(entity);
        if view_visibility.get() {
            material_instances.instances.insert(
                entity,
                RenderMaterialInstance {
                    asset_id: material.id().untyped(),
                    last_change_tick,
                },
            );
        } else {
            material_instances.instances.remove(&entity);
        }
    }
}

fn early_sweep_instance_mesh_materials(
    mut material_instances: ResMut<RenderMaterialInstances>,
    mut removed_materials_query: Extract<RemovedComponents<InstanceMeshMaterial3d>>,
) {
    let last_change_tick = material_instances.current_change_tick;

    for entity in removed_materials_query.read() {
        let entity = MainEntity::from(entity);
        let should_remove = material_instances
            .instances
            .get(&entity)
            .is_some_and(|instance| instance.last_change_tick != last_change_tick);
        if should_remove {
            material_instances.instances.remove(&entity);
        }
    }
}

fn queue_custom(
    transparent_3d_draw_functions: Res<DrawFunctions<Transparent3d>>,
    custom_pipeline: Res<CustomPipeline>,
    mut pipelines: ResMut<SpecializedMeshPipelines<CustomPipeline>>,
    pipeline_cache: Res<PipelineCache>,
    meshes: Res<RenderAssets<RenderMesh>>,
    render_mesh_instances: Res<RenderMeshInstances>,
    maybe_batched_instance_buffers: Option<
        Res<BatchedInstanceBuffers<MeshUniform, MeshInputUniform>>,
    >,
    material_meshes: Query<(Entity, &MainEntity), With<Instances>>,
    mut transparent_render_phases: ResMut<ViewSortedRenderPhases<Transparent3d>>,
    views: Query<&ExtractedView>,
    view_key_cache: Res<ViewKeyCache>,
) {
    let draw_custom = transparent_3d_draw_functions.read().id::<DrawCustom>();

    for view in &views {
        let Some(transparent_phase) = transparent_render_phases.get_mut(&view.retained_view_entity)
        else {
            continue;
        };

        let Some(&view_key) = view_key_cache.get(&view.retained_view_entity) else {
            continue;
        };

        for (entity, main_entity) in &material_meshes {
            let Some(mesh_instance) = render_mesh_instances.render_mesh_queue_data(*main_entity)
            else {
                continue;
            };
            let Some(mesh) = meshes.get(mesh_instance.mesh_asset_id()) else {
                continue;
            };
            let key = view_key
                | MeshPipelineKey::from_primitive_topology_and_strip_index(
                    mesh.primitive_topology(),
                    mesh.index_format(),
                );
            let pipeline = pipelines
                .specialize(&pipeline_cache, &custom_pipeline, key, &mesh.layout)
                .unwrap();
            transparent_phase.add_retained(Transparent3d {
                sorting_info: TransparentSortingInfo3d::Sorted {
                    mesh_center: bevy::pbr::get_mesh_instance_world_from_local(
                        *main_entity,
                        mesh_instance.current_uniform_index,
                        &render_mesh_instances,
                        maybe_batched_instance_buffers.as_deref(),
                    )
                    .transform_point3(
                        meshes
                            .get(mesh_instance.mesh_asset_id())
                            .unwrap()
                            .aabb_center,
                    ),
                    depth_bias: 0.0,
                },
                entity: (entity, *main_entity),
                pipeline,
                draw_function: draw_custom,
                distance: 0.0,
                batch_range: 0..1,
                extra_index: PhaseItemExtraIndex::None,
                indexed: true,
            });
        }
    }
}

#[derive(Component)]
struct InstanceBuffer {
    buffer: Buffer,
    length: usize,
}

fn prepare_instance_buffers(
    mut commands: Commands,
    query: Query<(Entity, &MainEntity, &Instances)>,
    render_device: Res<RenderDevice>,
    render_material_instances: Res<RenderMaterialInstances>,
    render_materials: Res<ErasedRenderAssets<PreparedMaterial>>,
) {
    for (entity, main_entity, instance_data) in &query {
        let mut instances = instance_data.instances().to_vec();
        if let Some(material_instance) = render_material_instances
            .instances
            .get(main_entity)
        {
            if let Some(material) = render_materials.get(material_instance.asset_id) {
                let slot = u32::from(material.binding.slot) as f32;
                for instance in &mut instances {
                    instance.sca.w = slot;
                }
            }
        }

        let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("instance data buffer"),
            contents: bytemuck::cast_slice(&instances),
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
        });
        commands.entity(entity).insert(InstanceBuffer {
            buffer,
            length: instances.len(),
        });
    }
}

#[derive(Resource)]
struct CustomPipeline {
    shader: Handle<Shader>,
    mesh_pipeline: MeshPipeline,
    material_layout: BindGroupLayoutDescriptor,
    bindless: bool,
}

fn init_custom_pipeline(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mesh_pipeline: Res<MeshPipeline>,
    render_device: Res<RenderDevice>,
) {
    commands.insert_resource(CustomPipeline {
        shader: asset_server.load(INSTANCING_SHADER_ASSET_PATH),
        mesh_pipeline: mesh_pipeline.clone(),
        material_layout: StandardMaterial::bind_group_layout_descriptor(&render_device),
        bindless: material_uses_bindless_resources::<StandardMaterial>(&render_device),
    });
}

impl SpecializedMeshPipeline for CustomPipeline {
    type Key = MeshPipelineKey;

    fn specialize(
        &self,
        key: Self::Key,
        layout: &MeshVertexBufferLayoutRef,
    ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
        let mut descriptor = self.mesh_pipeline.specialize(key, layout)?;

        descriptor.vertex.shader_defs.push(ShaderDefVal::UInt(
            "MATERIAL_BIND_GROUP".into(),
            MATERIAL_BIND_GROUP_INDEX as u32,
        ));
        if self.bindless {
            descriptor.vertex.shader_defs.push("BINDLESS".into());
        }
        descriptor.vertex.shader = self.shader.clone();
        descriptor.vertex.buffers.push(VertexBufferLayout {
            array_stride: size_of::<Instance>() as u64,
            step_mode: VertexStepMode::Instance,
            attributes: vec![
                VertexAttribute {
                    format: VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 3, // shader locations 0-2 are taken up by Position, Normal and UV attributes
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
        });
        descriptor
            .layout
            .insert(MATERIAL_BIND_GROUP_INDEX, self.material_layout.clone());
        let fragment = descriptor.fragment.as_mut().unwrap();
        fragment.shader = self.shader.clone();
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

type DrawCustom = (
    SetItemPipeline,
    SetMeshViewBindGroup<0>,
    SetMeshViewBindingArrayBindGroup<1>,
    SetMeshBindGroup<2>,
    SetMaterialBindGroup<MATERIAL_BIND_GROUP_INDEX>,
    DrawMeshInstanced,
);

struct DrawMeshInstanced;

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
        // A borrow check workaround.
        let mesh_allocator = mesh_allocator.into_inner();

        let Some(mesh_asset_id) = render_mesh_instances.mesh_asset_id(item.main_entity()) else {
            return RenderCommandResult::Skip;
        };
        let Some(gpu_mesh) = meshes.into_inner().get(mesh_asset_id) else {
            return RenderCommandResult::Skip;
        };
        let Some(instance_buffer) = instance_buffer else {
            return RenderCommandResult::Skip;
        };
        let Some(vertex_buffer_slice) = mesh_allocator.mesh_vertex_slice(&mesh_asset_id) else {
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
}
