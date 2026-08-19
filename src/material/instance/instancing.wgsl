#import bevy_pbr::{
    view_transformations::position_world_to_clip,
}

#ifndef PREPASS_PIPELINE
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
}

#ifdef BINDLESS
#import bevy_render::bindless::{bindless_samplers_filtering, bindless_textures_2d}
#endif
#endif

struct Vertex {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,

    @location(3) i_pos: vec3<f32>,
    @location(4) i_color: u32,
    @location(5) i_rot: vec4<f32>,
    @location(6) i_sca: vec4<f32>,
    @location(7) i_tex: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec4<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) uv: vec2<f32>,
    @location(4) @interpolate(flat) material_slot: u32,
};

struct ShadowVertex {
    @location(0) position: vec3<f32>,

    @location(3) i_pos: vec3<f32>,
    @location(4) i_color: u32,
    @location(5) i_rot: vec4<f32>,
    @location(6) i_sca: vec4<f32>,
    @location(7) i_tex: vec4<f32>,
};

struct ShadowVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
#ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
    @location(0) unclipped_depth: f32,
#endif
};

#ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
struct ShadowFragmentOutput {
    @builtin(frag_depth) frag_depth: f32,
};
#endif

#ifndef PREPASS_PIPELINE
struct FragmentOutput {
    @location(0) color: vec4<f32>,
};
#endif

fn rotate_vertex_position(v: vec3<f32>, q: vec4<f32>) -> vec3<f32> {
    return v + 2.0 * cross(q.xyz, cross(q.xyz, v) + q.w * v);
}

fn unpack_rgba8(x: u32) -> vec4<u32> {
    return vec4<u32>(
        (x >> 0u) & 0xffu,
        (x >> 8u) & 0xffu,
        (x >> 16u) & 0xffu,
        (x >> 24u) & 0xffu,
    );
}

fn unpack_rgba8_norm(v: u32) -> vec4<f32> {
    return vec4<f32>(unpack_rgba8(v)) / 255.0;
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    let position =
        rotate_vertex_position(vertex.position * vertex.i_sca.xyz, vertex.i_rot) + vertex.i_pos;
    let normal = normalize(rotate_vertex_position(vertex.normal, vertex.i_rot));

    var out: VertexOutput;
    out.clip_position = position_world_to_clip(position);
    out.world_position = vec4<f32>(position, 1.0);
    out.world_normal = normal;
    out.color = unpack_rgba8_norm(vertex.i_color);
    //out.color = vec4<f32>(1.0);
    out.uv = vertex.i_tex.xy + vertex.uv * vertex.i_tex.zw;
    out.material_slot = u32(vertex.i_sca.w);
    return out;
}

@vertex
fn shadow_vertex(vertex: ShadowVertex) -> ShadowVertexOutput {
    let position =
        rotate_vertex_position(vertex.position * vertex.i_sca.xyz, vertex.i_rot) + vertex.i_pos;

    var out: ShadowVertexOutput;
    out.clip_position = position_world_to_clip(position);
#ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
    out.unclipped_depth = out.clip_position.z;
    out.clip_position.z = min(out.clip_position.z, 1.0);
#endif
    return out;
}

#ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
@fragment
fn shadow_fragment(in: ShadowVertexOutput) -> ShadowFragmentOutput {
    var out: ShadowFragmentOutput;
    out.frag_depth = in.unclipped_depth;
    return out;
}
#endif

#ifndef PREPASS_PIPELINE
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

    pbr_input.material.base_color = vec4<f32>(1.0);

#ifdef BINDLESS
    let material_indices = pbr_bindings::material_indices[in.material_slot];
    pbr_input.material = pbr_bindings::material_array[material_indices.material];
#else
    pbr_input.material = pbr_bindings::material;
#endif

    pbr_input.material.base_color *= in.color;

    if (pbr_input.material.flags & pbr_types::STANDARD_MATERIAL_FLAGS_BASE_COLOR_TEXTURE_BIT) != 0u {
#ifdef BINDLESS
        pbr_input.material.base_color *= textureSample(
            bindless_textures_2d[material_indices.base_color_texture],
            bindless_samplers_filtering[material_indices.base_color_sampler],
            in.uv,
        );
#else
        pbr_input.material.base_color *= textureSample(
            pbr_bindings::base_color_texture,
            pbr_bindings::base_color_sampler,
            in.uv,
        );
#endif
    }
    
    pbr_input.material.base_color =
        alpha_discard(pbr_input.material, pbr_input.material.base_color);


    var out: FragmentOutput;

    if (pbr_input.material.flags & pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        out.color = apply_pbr_lighting(pbr_input);
    } else {
        out.color = pbr_input.material.base_color;
    }

    out.color = main_pass_post_lighting_processing(pbr_input, out.color);

    return out;
}
#endif
