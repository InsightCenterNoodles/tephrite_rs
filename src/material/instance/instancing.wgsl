#import bevy_pbr::{
    mesh_view_bindings::view,
    pbr_bindings,
    pbr_functions::{
        alpha_discard,
        apply_pbr_lighting,
        calculate_view,
        main_pass_post_lighting_processing,
        prepare_world_normal,
    },
    pbr_types,
    view_transformations::position_world_to_clip,
}

struct Vertex {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,

    @location(3) i_pos: vec4<f32>,
    @location(4) i_rot: vec4<f32>,
    @location(5) i_sca: vec4<f32>,
    @location(6) i_tex: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec4<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) uv: vec2<f32>,
};

struct FragmentOutput {
    @location(0) color: vec4<f32>,
};

fn rotate_vertex_position(v: vec3<f32>, q: vec4<f32>) -> vec3<f32> {
    return v + 2.0 * cross(q.xyz, cross(q.xyz, v) + q.w * v);
}

fn unpack_rgba8(v: f32) -> vec4<u32> {
    let x = bitcast<u32>(v);

    return vec4<u32>(
        (x >> 0u) & 0xffu,
        (x >> 8u) & 0xffu,
        (x >> 16u) & 0xffu,
        (x >> 24u) & 0xffu,
    );
}

fn unpack_rgba8_norm(v: f32) -> vec4<f32> {
    return vec4<f32>(unpack_rgba8(v)) / 255.0;
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    let position =
        rotate_vertex_position(vertex.position * vertex.i_sca.xyz, vertex.i_rot) + vertex.i_pos.xyz;
    let normal = normalize(rotate_vertex_position(vertex.normal, vertex.i_rot));

    var out: VertexOutput;
    out.clip_position = position_world_to_clip(position);
    out.world_position = vec4<f32>(position, 1.0);
    out.world_normal = normal;
    out.color = unpack_rgba8_norm(vertex.i_pos.w);
    out.uv = vertex.i_tex.xy + vertex.uv * vertex.i_tex.zw;
    return out;
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input: pbr_types::PbrInput = pbr_types::pbr_input_new();

    pbr_input.is_orthographic = view.clip_from_view[3].w == 1.0;
    pbr_input.frag_coord = in.clip_position;
    pbr_input.world_position = in.world_position;
    pbr_input.V = calculate_view(in.world_position, pbr_input.is_orthographic);
    pbr_input.world_normal = prepare_world_normal(in.world_normal, false, is_front);
    pbr_input.N = normalize(pbr_input.world_normal);
    pbr_input.clearcoat_N = pbr_input.N;

    pbr_input.material = pbr_bindings::material;
    pbr_input.material.base_color *= in.color;

    if (pbr_input.material.flags & pbr_types::STANDARD_MATERIAL_FLAGS_BASE_COLOR_TEXTURE_BIT) != 0u {
        pbr_input.material.base_color *= textureSample(
            pbr_bindings::base_color_texture,
            pbr_bindings::base_color_sampler,
            in.uv,
        );
    }
    pbr_input.material.base_color =
        alpha_discard(pbr_input.material, pbr_input.material.base_color);

    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
