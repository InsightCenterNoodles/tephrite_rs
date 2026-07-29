//! Serialization for `bevy::pbr::StandardMaterial` and related enums.
//!
//! The serializer keeps the vast majority of render‑affecting fields and skips
//! a few fields that are either redundant, unstable across versions, or not
//! required on the receiving side (e.g. `specular_tint`).
use std::sync::{LazyLock, RwLock};

use bevy::material::OpaqueRendererMethod;
use bevy::mesh::UvChannel;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::render::render_resource::Face;

use crate::prelude::PointsMaterial;
use crate::prelude::points::PointsShaderSettings;
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

// =============================================================================

impl_fast_raw_item!(PointsShaderSettings);

impl_fast_serialize!(
    PointsMaterial,
    keep: {
        settings,
        depth_bias,
        use_vertex_color
    }, skip: {
    }
);

// =============================================================================

static MAP: LazyLock<RwLock<HashMap<AssetId<StandardMaterial>, Handle<StandardMaterial>>>> =
    LazyLock::new(|| Default::default());

impl RemappableAsset for StandardMaterial {
    #[inline]
    fn with_remapper<F: FnOnce(&HashMap<AssetId<Self>, Handle<Self>>)>(func: F) {
        func(&MAP.read().unwrap());
    }
    #[inline]
    fn with_remapper_mut<F: FnOnce(&mut HashMap<AssetId<Self>, Handle<Self>>)>(func: F) {
        func(&mut MAP.write().unwrap());
    }
}

// =============================================================================

static P_MAP: LazyLock<RwLock<HashMap<AssetId<PointsMaterial>, Handle<PointsMaterial>>>> =
    LazyLock::new(|| Default::default());

impl RemappableAsset for PointsMaterial {
    #[inline]
    fn with_remapper<F: FnOnce(&HashMap<AssetId<Self>, Handle<Self>>)>(func: F) {
        func(&P_MAP.read().unwrap());
    }
    #[inline]
    fn with_remapper_mut<F: FnOnce(&mut HashMap<AssetId<Self>, Handle<Self>>)>(func: F) {
        func(&mut P_MAP.write().unwrap());
    }
}

// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serialize::fast_io::{ByteReader, ByteWriter};
    use crate::serialize::fast_ser::{FastRead, FastWrite};

    fn roundtrip<T: FastWrite + FastRead<Ret = T>>(x: &T) -> T {
        let mut buf = [0u8; 1024];
        let mut w = ByteWriter::new(&mut buf);
        unsafe { x.write_fast(&mut w) };
        let mut r = ByteReader::new(&buf);
        unsafe { T::read_fast(&mut r) }
    }

    #[test]
    fn standard_material_core_fields_roundtrip() {
        let mut m = StandardMaterial::default();
        m.base_color = Color::srgba(0.1, 0.2, 0.3, 0.4);
        m.perceptual_roughness = 0.75;
        m.metallic = 0.25;
        m.reflectance = 0.3;
        m.flip_normal_map_y = true;
        m.double_sided = true;
        m.cull_mode = Some(Face::Back);
        m.unlit = true;
        m.fog_enabled = false;
        m.alpha_mode = AlphaMode::Mask(0.5);

        let out = roundtrip(&m);

        assert_eq!(out.base_color, m.base_color);
        assert_eq!(out.perceptual_roughness, m.perceptual_roughness);
        assert_eq!(out.metallic, m.metallic);
        assert_eq!(out.reflectance, m.reflectance);
        assert_eq!(out.flip_normal_map_y, m.flip_normal_map_y);
        assert_eq!(out.double_sided, m.double_sided);
        assert_eq!(out.cull_mode, m.cull_mode);
        assert_eq!(out.unlit, m.unlit);
        assert_eq!(out.fog_enabled, m.fog_enabled);
        assert_eq!(out.alpha_mode, m.alpha_mode);
    }
}
