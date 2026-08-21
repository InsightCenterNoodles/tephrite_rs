use crate::{
    common::{
        DeferredRendering, EnvironmentLighting, OffAxisProjectionSettings,
        OrderIndependantTransparency, ScreenSpaceAmbientOcclusionSettings,
        ScreenSpaceReflectionsSettings,
    },
    serialize::*,
};

use bevy::{
    asset::Assets, image::Image, light::DirectionalLightShadowMap,
    pbr::ScreenSpaceAmbientOcclusionQualityLevel,
};

impl crate::serialize::fast_ser::FastWrite for EnvironmentLighting {
    #[inline(always)]
    #[allow(unused)]
    unsafe fn write_fast(&self, w: &mut impl crate::serialize::fast_io::ByteSink) {
        unsafe { self.intensity.write_fast(w) };
        unsafe { self.diffuse.write_fast(w) };
        unsafe { self.specular.write_fast(w) };
        unsafe { self.skybox_color.write_fast(w) };
    }
}
impl crate::serialize::fast_ser::FastRead for EnvironmentLighting {
    type Ret = EnvironmentLighting;
    type Context = Assets<Image>;
    #[inline(always)]
    #[allow(unused)]
    unsafe fn read_fast<'z, S: crate::serialize::fast_io::ByteSource<'z>>(
        c: &mut Self::Context,
        r: &mut S,
    ) -> Self {
        #[allow(unused)]
        use crate::serialize::fast_ser::read_fast;
        let nc = &mut ();
        Self {
            intensity: read_fast(nc, r),
            diffuse: read_fast(c, r),
            specular: read_fast(c, r),
            skybox_color: read_fast(nc, r),
        }
    }
}

impl_fast_raw_item!(OrderIndependantTransparency);
impl_fast_raw_item!(DeferredRendering);
impl_fast_raw_item!(OffAxisProjectionSettings);

impl_fast_raw_item!(ScreenSpaceAmbientOcclusionQualityLevel);
impl_fast_raw_item!(ScreenSpaceAmbientOcclusionSettings);
impl_fast_raw_item!(ScreenSpaceReflectionsSettings);

impl_fast_raw_item!(DirectionalLightShadowMap);
