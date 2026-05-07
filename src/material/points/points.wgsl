#import bevy_pbr::mesh_view_bindings::view
#import bevy_pbr::mesh_bindings::mesh
#import bevy_pbr::mesh_functions::get_world_from_local

struct PointMaterial {
    point_size: f32,
    color: vec4<f32>,
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> material: PointMaterial;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
#ifdef VERTEX_COLORS
    @location(2) color: vec4<f32>,
#endif
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
#ifdef VERTEX_COLORS
    @location(0) color: vec4<f32>,
#endif
};

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let world_from_local = get_world_from_local(vertex.instance_index);
    let center_world = world_from_local * vec4<f32>(vertex.position, 1.0);

    let scale_x = length(world_from_local[0].xyz);
    let scale_y = length(world_from_local[1].xyz);
    let scale_z = length(world_from_local[2].xyz);
    let scale = max((scale_x + scale_y + scale_z) / 3.0, 1e-5);

    let delta = material.point_size * scale * vertex.uv;
    var view_position = view.view_from_world * center_world;
    view_position = vec4<f32>(view_position.xy + delta, view_position.zw);
    out.clip_position = view.clip_from_view * view_position;
#ifdef VERTEX_COLORS
    out.color = vertex.color;
#endif
    return out;
}

@fragment
fn fragment(input: VertexOutput) -> @location(0) vec4<f32> {
#ifdef VERTEX_COLORS
    return material.color * input.color;
#else
    return material.color;
#endif
}
