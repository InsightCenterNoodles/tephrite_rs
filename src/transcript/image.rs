use bevy::prelude::*;
use bevy::{
    image::{
        ImageAddressMode, ImageCompareFunction, ImageFilterMode, ImageSampler,
        ImageSamplerBorderColor, ImageSamplerDescriptor,
    },
    render::render_resource::TextureViewDescriptor,
};
use wgpu_types::{
    Extent3d, TextureAspect, TextureDimension, TextureFormat, TextureUsages, TextureViewDimension,
};

use crate::serialize::*;

impl_fast_serialize!(Image, keep: {
    data, texture_descriptor, sampler, texture_view_descriptor, asset_usage
}, skip: {
});

// =============================================================================

type TD<'a> = wgpu_types::TextureDescriptor<Option<&'a str>, &'a [TextureFormat]>;

impl_fast_serialize!(TD<'a>,
lifetime: 'a,
keep: {
    size, mip_level_count, sample_count, dimension, format, usage
}, skip: {
    label, view_formats
});

// impl<'a> TSerialize for wgpu_types::TextureDescriptor<Option<&'a str>, &'a [TextureFormat]> {
//     fn serialize(&self, w: &mut impl std::io::Write) {
//         // skip label
//         self.size.serialize(w);
//         self.mip_level_count.serialize(w);
//         self.sample_count.serialize(w);
//         self.dimension.serialize(w);
//         self.format.serialize(w);
//         self.usage.serialize(w);
//         // skip view_formats for now, can probably do a hashmap since this is a static label thing
//     }
// }

// impl<'a> TDeserialize for wgpu_types::TextureDescriptor<Option<&'a str>, &'a [TextureFormat]> {
//     fn deserialize(r: &mut impl std::io::Read) -> Self {
//         Self {
//             label: None,
//             size: deserialize(r),
//             mip_level_count: deserialize(r),
//             sample_count: deserialize(r),
//             dimension: deserialize(r),
//             format: deserialize(r),
//             usage: deserialize(r),
//             view_formats: &[],
//         }
//     }
// }

// =============================================================================

impl_fast_raw_item!(Extent3d);

// =============================================================================

impl_fast_raw_item!(TextureDimension);

// =============================================================================

impl_fast_raw_item!(TextureFormat);

// =============================================================================

impl_fast_raw_item!(TextureUsages);

// =============================================================================

impl FastWrite for ImageSampler {
    unsafe fn write_fast(&self, w: &mut impl ByteSink) {
        match self {
            ImageSampler::Default => unsafe { 0i8.write_fast(w) },
            ImageSampler::Descriptor(image_sampler_descriptor) => unsafe {
                1i8.write_fast(w);
                image_sampler_descriptor.write_fast(w)
            },
        }
    }
}

impl FastRead for ImageSampler {
    type Ret = Self;
    unsafe fn read_fast<'a, S: ByteSource<'a>>(r: &mut S) -> Self {
        let index = unsafe { i8::read_fast(r) };

        match index {
            0 => Self::Default,
            1 => Self::Descriptor(unsafe { ImageSamplerDescriptor::read_fast(r) }),
            _ => unreachable!(),
        }
    }
}

// =============================================================================

impl_fast_serialize!(
    ImageSamplerDescriptor,
    keep: {
        label,
        address_mode_u,
        address_mode_v,
        address_mode_w,
        mag_filter,
        min_filter,
        mipmap_filter,
        lod_min_clamp,
        lod_max_clamp,
        compare,
        anisotropy_clamp,
        border_color
    }, skip: {

    }
);

// =============================================================================

impl_fast_serialize!(
    TextureViewDescriptor<'static>,
    keep: {
        format,
        dimension,
        aspect,
        base_mip_level,
        mip_level_count,
        base_array_layer,
        array_layer_count
    }, skip: {
        label,
        usage
    }
);

// =============================================================================

impl_fast_raw_item!(ImageAddressMode);
impl_fast_raw_item!(ImageFilterMode);
impl_fast_raw_item!(ImageCompareFunction);
impl_fast_raw_item!(ImageSamplerBorderColor);
impl_fast_raw_item!(TextureViewDimension);
impl_fast_raw_item!(TextureAspect);
