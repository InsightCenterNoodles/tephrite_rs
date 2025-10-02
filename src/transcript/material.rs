use bevy::pbr::{OpaqueRendererMethod, UvChannel};
use bevy::prelude::*;
use wgpu_types::Face;

use crate::serialize::*;

impl_fast_serialize!(
    StandardMaterial,
    keep: {
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
    }, skip: {
        specular_tint
    }
);

impl_fast_raw_item!(Color);
impl_fast_raw_item!(UvChannel);
impl_fast_raw_item!(LinearRgba);
impl_fast_raw_item!(Face);
impl_fast_raw_item!(AlphaMode);
impl_fast_raw_item!(ParallaxMappingMethod);
impl_fast_raw_item!(OpaqueRendererMethod);
