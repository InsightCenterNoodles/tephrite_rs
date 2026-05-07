use bevy::{
    asset::Asset,
    color::LinearRgba,
    prelude::{Material, Mesh},
    reflect::TypePath,
    render::render_resource::{AsBindGroup, ShaderType},
    shader::{ShaderDefVal, ShaderRef},
};

#[derive(Debug, Clone, Copy, ShaderType)] // ShaderType
pub struct PointsShaderSettings {
    pub point_size: f32,
    pub color: LinearRgba,
}

impl Default for PointsShaderSettings {
    fn default() -> Self {
        Self {
            point_size: 1.,
            color: LinearRgba::WHITE,
        }
    }
}

#[derive(AsBindGroup, Debug, Clone, Copy, TypePath, Asset)]
#[bind_group_data(PointsMaterialKey)]
pub struct PointsMaterial {
    #[uniform(0)]
    pub settings: PointsShaderSettings,
    pub depth_bias: f32,
    pub use_vertex_color: bool,
}

impl Default for PointsMaterial {
    fn default() -> Self {
        Self {
            settings: PointsShaderSettings::default(),
            depth_bias: 0.,
            use_vertex_color: true,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PointsMaterialKey {
    use_vertex_color: bool,
}

impl From<&PointsMaterial> for PointsMaterialKey {
    fn from(material: &PointsMaterial) -> Self {
        PointsMaterialKey {
            use_vertex_color: material.use_vertex_color,
        }
    }
}

impl Material for PointsMaterial {
    fn vertex_shader() -> ShaderRef {
        "embedded://tephrite_rs/material/points/points.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "embedded://tephrite_rs/material/points/points.wgsl".into()
    }

    fn depth_bias(&self) -> f32 {
        self.depth_bias
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }

    fn specialize(
        _pipeline: &bevy::pbr::MaterialPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;

        let mut shader_defs = vec![];
        let mut vertex_attributes = vec![
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_UV_0.at_shader_location(1),
        ];

        if key.bind_group_data.use_vertex_color && layout.0.contains(Mesh::ATTRIBUTE_COLOR) {
            shader_defs.push(ShaderDefVal::from("VERTEX_COLORS"));
            vertex_attributes.push(Mesh::ATTRIBUTE_COLOR.at_shader_location(2));
        }

        let vertex_layout = layout.0.get_layout(&vertex_attributes)?;
        descriptor.vertex.buffers = vec![vertex_layout];
        descriptor.vertex.shader_defs.extend(shader_defs.clone());
        if let Some(fragment) = &mut descriptor.fragment {
            fragment.shader_defs.extend(shader_defs);
        }

        Ok(())
    }
}
