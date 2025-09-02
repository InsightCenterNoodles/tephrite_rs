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

use super::macros::{raw_item_helper, struct_serde_helper};
use super::serialize;
use super::{
    TDeserialize, TSerialize,
    common::{byte_deserialize, byte_serialize},
};
use crate::transcript::deserialize;

impl TSerialize for Image {
    fn serialize(&self, w: &mut impl std::io::Write) {
        self.data.serialize(w);
        serialize(&self.texture_descriptor, w);
        self.sampler.serialize(w);
        self.texture_view_descriptor.serialize(w);
        self.asset_usage.serialize(w);
    }
}

impl TDeserialize for Image {
    fn deserialize(r: &mut impl std::io::Read) -> Self {
        Self {
            data: deserialize(r),
            texture_descriptor: deserialize(r),
            sampler: deserialize(r),
            texture_view_descriptor: deserialize(r),
            asset_usage: deserialize(r),
        }
    }
}

// =============================================================================

impl<'a> TSerialize for wgpu_types::TextureDescriptor<Option<&'a str>, &'a [TextureFormat]> {
    fn serialize(&self, w: &mut impl std::io::Write) {
        // skip label
        self.size.serialize(w);
        self.mip_level_count.serialize(w);
        self.sample_count.serialize(w);
        self.dimension.serialize(w);
        self.format.serialize(w);
        self.usage.serialize(w);
        // skip view_formats for now, can probably do a hashmap since this is a static label thing
    }
}

impl<'a> TDeserialize for wgpu_types::TextureDescriptor<Option<&'a str>, &'a [TextureFormat]> {
    fn deserialize(r: &mut impl std::io::Read) -> Self {
        Self {
            label: None,
            size: deserialize(r),
            mip_level_count: deserialize(r),
            sample_count: deserialize(r),
            dimension: deserialize(r),
            format: deserialize(r),
            usage: deserialize(r),
            view_formats: &[],
        }
    }
}

// =============================================================================

raw_item_helper!(Extent3d);

// =============================================================================

raw_item_helper!(TextureDimension);

// =============================================================================

raw_item_helper!(TextureFormat);

// =============================================================================

raw_item_helper!(TextureUsages);

// =============================================================================

impl TSerialize for ImageSampler {
    fn serialize(&self, w: &mut impl std::io::Write) {
        match self {
            ImageSampler::Default => 0i8.serialize(w),
            ImageSampler::Descriptor(image_sampler_descriptor) => {
                1i8.serialize(w);
                image_sampler_descriptor.serialize(w)
            }
        }
    }
}

impl TDeserialize for ImageSampler {
    fn deserialize(r: &mut impl std::io::Read) -> Self {
        let index = i8::deserialize(r);

        match index {
            0 => Self::Default,
            1 => Self::Descriptor(deserialize(r)),
            _ => unreachable!(),
        }
    }
}

// =============================================================================

struct_serde_helper!(
    ImageSamplerDescriptor,
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
);

// =============================================================================

struct_serde_helper!(
    TextureViewDescriptor<'static>,
    format,
    dimension,
    aspect,
    base_mip_level,
    mip_level_count,
    base_array_layer,
    array_layer_count
);

// =============================================================================

raw_item_helper!(ImageAddressMode);
raw_item_helper!(ImageFilterMode);
raw_item_helper!(ImageCompareFunction);
raw_item_helper!(ImageSamplerBorderColor);
raw_item_helper!(TextureViewDimension);
raw_item_helper!(TextureAspect);
