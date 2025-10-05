//! Serialization for `bevy::image::Image` and related GPU descriptors.
//!
//! Skips fields that are compile-time or label-only metadata (e.g. `label`,
//! `view_formats`) and focuses on the data and descriptor fields required to
//! faithfully recreate images on the receiving side.
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

// `Image` is serialized as raw pixel data plus the essential descriptors.
impl_fast_serialize!(Image, keep: {
    data, texture_descriptor, sampler, texture_view_descriptor, asset_usage
}, skip: {
});

// =============================================================================

type TD<'a> = wgpu_types::TextureDescriptor<Option<&'a str>, &'a [TextureFormat]>;

// `TextureDescriptor` skips `label` and `view_formats`.
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

/// Compact tag-based encoding for `ImageSampler` variants.
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

// Serialize `ImageSamplerDescriptor` fully; all fields are needed to restore
// sampler state.
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

// Keep only fields used for view configuration; skip labels/usages.
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

// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serialize::fast_io::{ByteReader, ByteWriter};
    use crate::serialize::fast_ser::{FastRead, FastWrite};

    fn roundtrip<T: FastWrite + FastRead<Ret = T>>(x: &T) -> T {
        let mut buf = [0u8; 512];
        let mut w = ByteWriter::new(&mut buf);
        unsafe { x.write_fast(&mut w) };
        let mut r = ByteReader::new(&buf);
        unsafe { T::read_fast(&mut r) }
    }

    #[test]
    fn extent3d_roundtrip() {
        let e = Extent3d {
            width: 16,
            height: 8,
            depth_or_array_layers: 1,
        };
        let out = roundtrip(&e);
        assert_eq!(out, e);
    }

    #[test]
    fn image_sampler_default_roundtrip() {
        let s = ImageSampler::Default;
        let out = roundtrip(&s);
        assert!(matches!(out, ImageSampler::Default));
    }

    #[test]
    fn image_sampler_descriptor_roundtrip() {
        let desc = ImageSamplerDescriptor {
            address_mode_u: ImageAddressMode::Repeat,
            address_mode_v: ImageAddressMode::ClampToEdge,
            address_mode_w: ImageAddressMode::MirrorRepeat,
            mag_filter: ImageFilterMode::Linear,
            min_filter: ImageFilterMode::Nearest,
            mipmap_filter: ImageFilterMode::Nearest,
            lod_min_clamp: 0.0,
            lod_max_clamp: 1.0,
            compare: Some(ImageCompareFunction::LessEqual),
            anisotropy_clamp: 1,
            border_color: Some(ImageSamplerBorderColor::TransparentBlack),
            label: None,
        };
        let s = ImageSampler::Descriptor(desc);
        let out = roundtrip(&s);
        match out {
            ImageSampler::Descriptor(d) => {
                matches!(d.address_mode_u, ImageAddressMode::Repeat);
                matches!(d.address_mode_v, ImageAddressMode::ClampToEdge);
                matches!(d.address_mode_w, ImageAddressMode::MirrorRepeat);
                matches!(d.mag_filter, ImageFilterMode::Linear);
                matches!(d.min_filter, ImageFilterMode::Nearest);
                matches!(d.mipmap_filter, ImageFilterMode::Nearest);
                matches!(d.compare, Some(ImageCompareFunction::LessEqual));
                matches!(d.anisotropy_clamp, 1);
            }
            _ => panic!("unexpected variant"),
        }
    }

    #[test]
    fn texture_view_descriptor_roundtrip() {
        let d = TextureViewDescriptor {
            format: Some(TextureFormat::Rgba8Unorm),
            dimension: Some(TextureViewDimension::D2),
            aspect: TextureAspect::All,
            base_mip_level: 0,
            mip_level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
            label: None,
            usage: None,
        };
        let out = roundtrip(&d);
        assert_eq!(out.format, d.format);
        assert_eq!(out.dimension, d.dimension);
        assert_eq!(out.aspect, d.aspect);
        assert_eq!(out.base_mip_level, d.base_mip_level);
        assert_eq!(out.mip_level_count, d.mip_level_count);
        assert_eq!(out.base_array_layer, d.base_array_layer);
        assert_eq!(out.array_layer_count, d.array_layer_count);
    }
}
