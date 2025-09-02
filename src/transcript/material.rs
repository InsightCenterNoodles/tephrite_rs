use bevy::pbr::{OpaqueRendererMethod, UvChannel};
use bevy::prelude::*;
use wgpu_types::Face;

use super::macros::{raw_item_helper, struct_serde_helper};
use super::{
    TDeserialize, TSerialize,
    common::{byte_deserialize, byte_serialize},
};
use crate::transcript::deserialize;

struct_serde_helper!(
    StandardMaterial,
    base_color,
    base_color_channel,
    base_color_texture,
    emissive,
    emissive_exposure_weight,
    emissive_channel,
    emissive_texture,
    perceptual_roughness,
    metallic,
    metallic_roughness_channel,
    metallic_roughness_texture,
    reflectance,
    diffuse_transmission,
    specular_transmission,
    thickness,
    ior,
    attenuation_color,
    attenuation_distance,
    occlusion_channel,
    occlusion_texture,
    normal_map_channel,
    normal_map_texture,
    clearcoat,
    clearcoat_perceptual_roughness,
    anisotropy_strength,
    anisotropy_rotation,
    flip_normal_map_y,
    double_sided,
    cull_mode,
    unlit,
    fog_enabled,
    alpha_mode,
    depth_bias,
    depth_map,
    parallax_depth_scale,
    max_parallax_layer_count,
    lightmap_exposure,
    parallax_mapping_method,
    opaque_render_method,
    deferred_lighting_pass_id,
    uv_transform
);

raw_item_helper!(Color);
raw_item_helper!(UvChannel);
raw_item_helper!(LinearRgba);
raw_item_helper!(Face);
raw_item_helper!(AlphaMode);
raw_item_helper!(ParallaxMappingMethod);
raw_item_helper!(OpaqueRendererMethod);
