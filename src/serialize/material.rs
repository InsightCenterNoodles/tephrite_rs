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

impl crate::serialize::fast_ser::FastWrite for StandardMaterial {
    #[inline(always)]
    #[allow(unused)]
    unsafe fn write_fast(&self, w: &mut impl crate::serialize::fast_io::ByteSink) {
        unsafe { self.base_color.write_fast(w) };
        unsafe { self.base_color_channel.write_fast(w) };
        unsafe { self.base_color_texture.write_fast(w) };
        unsafe { self.emissive.write_fast(w) };
        unsafe { self.emissive_exposure_weight.write_fast(w) };
        unsafe { self.emissive_channel.write_fast(w) };
        unsafe { self.emissive_texture.write_fast(w) };
        unsafe { self.perceptual_roughness.write_fast(w) };
        unsafe { self.metallic.write_fast(w) };
        unsafe { self.metallic_roughness_channel.write_fast(w) };
        unsafe { self.metallic_roughness_texture.write_fast(w) };
        unsafe { self.reflectance.write_fast(w) };
        unsafe { self.diffuse_transmission.write_fast(w) };
        unsafe { self.specular_transmission.write_fast(w) };
        unsafe { self.thickness.write_fast(w) };
        unsafe { self.ior.write_fast(w) };
        unsafe { self.attenuation_color.write_fast(w) };
        unsafe { self.attenuation_distance.write_fast(w) };
        unsafe { self.occlusion_channel.write_fast(w) };
        unsafe { self.occlusion_texture.write_fast(w) };
        unsafe { self.normal_map_channel.write_fast(w) };
        unsafe { self.normal_map_texture.write_fast(w) };
        unsafe { self.clearcoat.write_fast(w) };
        unsafe { self.clearcoat_perceptual_roughness.write_fast(w) };
        unsafe { self.anisotropy_strength.write_fast(w) };
        unsafe { self.anisotropy_rotation.write_fast(w) };
        unsafe { self.flip_normal_map_y.write_fast(w) };
        unsafe { self.double_sided.write_fast(w) };
        unsafe { self.cull_mode.write_fast(w) };
        unsafe { self.unlit.write_fast(w) };
        unsafe { self.fog_enabled.write_fast(w) };
        unsafe { self.alpha_mode.write_fast(w) };
        unsafe { self.depth_bias.write_fast(w) };
        unsafe { self.depth_map.write_fast(w) };
        unsafe { self.parallax_depth_scale.write_fast(w) };
        unsafe { self.max_parallax_layer_count.write_fast(w) };
        unsafe { self.lightmap_exposure.write_fast(w) };
        unsafe { self.parallax_mapping_method.write_fast(w) };
        unsafe { self.opaque_render_method.write_fast(w) };
        unsafe { self.deferred_lighting_pass_id.write_fast(w) };
        unsafe { self.uv_transform.write_fast(w) };
    }
}

impl crate::serialize::fast_ser::FastRead for StandardMaterial {
    type Ret = StandardMaterial;
    type Context = Assets<Image>;
    #[inline(always)]
    #[allow(unused)]
    unsafe fn read_fast<'z, S: crate::serialize::fast_io::ByteSource<'z>>(
        c: &mut Self::Context,
        r: &mut S,
    ) -> Self {
        use crate::serialize::fast_ser::read_fast;
        let nc = &mut ();
        Self {
            base_color: read_fast(nc, r),
            base_color_channel: read_fast(nc, r),
            base_color_texture: read_fast(c, r),
            emissive: read_fast(nc, r),
            emissive_exposure_weight: read_fast(nc, r),
            emissive_channel: read_fast(nc, r),
            emissive_texture: read_fast(c, r),
            perceptual_roughness: read_fast(nc, r),
            metallic: read_fast(nc, r),
            metallic_roughness_channel: read_fast(nc, r),
            metallic_roughness_texture: read_fast(c, r),
            reflectance: read_fast(nc, r),
            diffuse_transmission: read_fast(nc, r),
            specular_transmission: read_fast(nc, r),
            thickness: read_fast(nc, r),
            ior: read_fast(nc, r),
            attenuation_color: read_fast(nc, r),
            attenuation_distance: read_fast(nc, r),
            occlusion_channel: read_fast(nc, r),
            occlusion_texture: read_fast(c, r),
            normal_map_channel: read_fast(nc, r),
            normal_map_texture: read_fast(c, r),
            clearcoat: read_fast(nc, r),
            clearcoat_perceptual_roughness: read_fast(nc, r),
            anisotropy_strength: read_fast(nc, r),
            anisotropy_rotation: read_fast(nc, r),
            flip_normal_map_y: read_fast(nc, r),
            double_sided: read_fast(nc, r),
            cull_mode: read_fast(nc, r),
            unlit: read_fast(nc, r),
            fog_enabled: read_fast(nc, r),
            alpha_mode: read_fast(nc, r),
            depth_bias: read_fast(nc, r),
            depth_map: read_fast(c, r),
            parallax_depth_scale: read_fast(nc, r),
            max_parallax_layer_count: read_fast(nc, r),
            lightmap_exposure: read_fast(nc, r),
            parallax_mapping_method: read_fast(nc, r),
            opaque_render_method: read_fast(nc, r),
            deferred_lighting_pass_id: read_fast(nc, r),
            uv_transform: read_fast(nc, r),
            specular_tint: Default::default(),
        }
    }
}

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
    (),
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

    fn roundtrip<T: FastWrite + FastRead<Ret = T, Context = Assets<Image>>>(x: &T) -> T {
        let mut assets = Assets::<Image>::default();
        let mut buf = [0u8; 1024];
        let mut w = ByteWriter::new(&mut buf);
        unsafe { x.write_fast(&mut w) };
        let mut r = ByteReader::new(&buf);
        unsafe { T::read_fast(&mut assets, &mut r) }
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
