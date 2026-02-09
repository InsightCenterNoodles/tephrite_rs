use std::ptr::NonNull;

use bevy::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;

use crate::backfill;
use crate::backfill::ffi::{
    self as bffi, FColorSpace_CS_LINEAR, FColorSpace_CS_SRGB, FPixelType_PIXEL_FLOAT32,
    FPixelType_PIXEL_UBYTE,
};

fn iter_or_value<'a, T>(
    slice: Option<&'a [T]>,
    fallback: &'a T,
) -> itertools::Either<std::slice::Iter<'a, T>, std::iter::Repeat<&'a T>> {
    match slice {
        Some(s) => itertools::Either::Left(s.iter()),
        None => itertools::Either::Right(std::iter::repeat(fallback)),
    }
}

/// Compute the axis-aligned bounding box of a mesh.
/// Returns (min, max) as Vec3, where min is the bottom-left-back corner,
/// and max is the top-right-front corner.
fn mesh_aabb(mesh: &Mesh) -> Option<(Vec3, Vec3)> {
    // Get the vertex positions
    if let Some(VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    {
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);

        for pos in positions {
            let v = Vec3::from(*pos);
            min = min.min(v);
            max = max.max(v);
        }

        Some((min, max))
    } else {
        None
    }
}

pub fn pack_with_index<T: bytemuck::Pod>(
    session: &backfill::FSessionHandle,
    mesh: &Mesh,
    unpacked: Vec<bffi::FVertexPNU>,
    index_slice: &[T],
    index_type: bffi::FMeshIndexType,
) -> Option<backfill::FMeshHandle> {
    let mut out_verts = Vec::<bffi::FPackedVertex>::new();

    // Safety: Vertex information is POD and will be overwritten
    out_verts.resize(mesh.count_vertices(), unsafe { std::mem::zeroed() });

    assert!(
        index_slice.len() % 3 == 0,
        "TriangleList must be multiple of 3 indices"
    );
    let tri_count: u32 = (index_slice.len() / 3).try_into().unwrap();

    match index_type {
        bffi::FMeshIndexType_U16 => {
            unsafe {
                backfill::DYN_LIBRARY.pack_vertex_u16(
                    unpacked.as_ptr(),
                    unpacked.len().try_into().unwrap(),
                    index_slice.as_ptr() as *const bffi::ushort3,
                    tri_count,
                    out_verts.as_mut_ptr(),
                )
            };
        }

        bffi::FMeshIndexType_U32 => unsafe {
            backfill::DYN_LIBRARY.pack_vertex_u32(
                unpacked.as_ptr(),
                unpacked.len().try_into().unwrap(),
                index_slice.as_ptr() as *const bffi::uint3,
                tri_count,
                out_verts.as_mut_ptr(),
            );
        },

        _ => {
            panic!("Unsupported!");
        }
    }

    let (vert_blob, vcount) = {
        let bytes: &[u8] = bytemuck::cast_slice(&out_verts);
        (
            backfill::blob_from_slice(bytes).unwrap(),
            mesh.count_vertices(),
        )
    };

    let index_blob = {
        let bytes: &[u8] = bytemuck::cast_slice(index_slice);
        backfill::blob_from_slice(bytes).unwrap()
    };

    let bounding = mesh_aabb(mesh).unwrap_or((Vec3::splat(-1.0), Vec3::splat(1.0)));

    debug_assert_eq!(unpacked.len(), mesh.count_vertices());
    debug_assert!(index_slice.len() % 3 == 0);
    //debug_assert!(index_slice.iter().all(|&i| (i as usize) < unpacked.len()));
    debug_assert_eq!(std::mem::size_of::<bffi::ushort3>(), 6);

    backfill::mesh_from_refs(
        session,
        backfill::BlobReference::whole(&vert_blob),
        vcount as u32,
        backfill::BlobReference::whole(&index_blob),
        index_slice.len() as u32,
        index_type,
        bffi::aabb {
            minimum: bounding.0.into(),
            maximum: bounding.1.into(),
        },
    )
    .ok()
}

pub fn convert_mesh(
    session: &backfill::FSessionHandle,
    mesh: &Mesh,
) -> Option<backfill::FMeshHandle> {
    use bevy::mesh::VertexAttributeValues::*;

    if !matches!(mesh.primitive_topology(), PrimitiveTopology::TriangleList) {
        warn!("Mesh is not triangles");
        return None;
    }

    let Some(Float32x3(positions)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else {
        warn!("Mesh is missing position information");
        return None;
    };

    let normal_iter = iter_or_value(
        mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
            .and_then(|x| match x {
                Float32x3(n) => Some(n.as_slice()),
                _ => None,
            }),
        &[0.0, 0.0, 0.0],
    );

    let uv_iter = iter_or_value(
        mesh.attribute(Mesh::ATTRIBUTE_UV_0).and_then(|x| match x {
            Float32x2(n) => Some(n.as_slice()),
            _ => None,
        }),
        &[0.0, 0.0],
    );

    let unpacked: Vec<bffi::FVertexPNU> = positions
        .iter()
        .zip(normal_iter.zip(uv_iter))
        .map(|(&p, (&n, &u))| bffi::FVertexPNU {
            position: p.into(),
            normal: n.into(),
            uv: u.into(),
        })
        .collect();

    let Some(index) = mesh.indices() else {
        warn!("Mesh has no index information");
        return None;
    };

    match index {
        Indices::U16(items) => {
            pack_with_index::<u16>(session, mesh, unpacked, &items, bffi::FMeshIndexType_U16)
        }
        Indices::U32(items) => {
            pack_with_index::<u32>(session, mesh, unpacked, &items, bffi::FMeshIndexType_U32)
        }
    }
}

pub fn is_image_float(texture: &Image) -> Option<bool> {
    use bevy::render::render_resource::TextureFormat;
    match texture.texture_descriptor.format {
        TextureFormat::R16Float
        | TextureFormat::R32Float
        | TextureFormat::Rg16Float
        | TextureFormat::Rgb9e5Ufloat
        | TextureFormat::Rg11b10Ufloat
        | TextureFormat::Rg32Float
        | TextureFormat::Rgba16Float
        | TextureFormat::Rgba32Float => Some(true),

        TextureFormat::R8Unorm
        | TextureFormat::R8Snorm
        | TextureFormat::R8Uint
        | TextureFormat::R8Sint
        | TextureFormat::R16Uint
        | TextureFormat::R16Sint
        | TextureFormat::R16Unorm
        | TextureFormat::R16Snorm
        | TextureFormat::Rg8Unorm
        | TextureFormat::Rg8Snorm
        | TextureFormat::Rg8Uint
        | TextureFormat::Rg8Sint
        | TextureFormat::R32Uint
        | TextureFormat::R32Sint
        | TextureFormat::Rg16Uint
        | TextureFormat::Rg16Sint
        | TextureFormat::Rg16Unorm
        | TextureFormat::Rg16Snorm
        | TextureFormat::Rgba8Unorm
        | TextureFormat::Rgba8UnormSrgb
        | TextureFormat::Rgba8Snorm
        | TextureFormat::Rgba8Uint
        | TextureFormat::Rgba8Sint
        | TextureFormat::Bgra8Unorm
        | TextureFormat::Bgra8UnormSrgb
        | TextureFormat::Rgb10a2Uint
        | TextureFormat::Rgb10a2Unorm
        | TextureFormat::R64Uint
        | TextureFormat::Rg32Uint
        | TextureFormat::Rg32Sint
        | TextureFormat::Rgba16Uint
        | TextureFormat::Rgba16Sint
        | TextureFormat::Rgba16Unorm
        | TextureFormat::Rgba16Snorm
        | TextureFormat::Rgba32Uint
        | TextureFormat::Rgba32Sint => Some(false),

        TextureFormat::Stencil8
        | TextureFormat::Depth16Unorm
        | TextureFormat::Depth24Plus
        | TextureFormat::Depth24PlusStencil8
        | TextureFormat::Depth32Float
        | TextureFormat::Depth32FloatStencil8
        | TextureFormat::NV12
        | TextureFormat::Bc1RgbaUnorm
        | TextureFormat::Bc1RgbaUnormSrgb
        | TextureFormat::Bc2RgbaUnorm
        | TextureFormat::Bc2RgbaUnormSrgb
        | TextureFormat::Bc3RgbaUnorm
        | TextureFormat::Bc3RgbaUnormSrgb
        | TextureFormat::Bc4RUnorm
        | TextureFormat::Bc4RSnorm
        | TextureFormat::Bc5RgUnorm
        | TextureFormat::Bc5RgSnorm
        | TextureFormat::Bc6hRgbUfloat
        | TextureFormat::Bc6hRgbFloat
        | TextureFormat::Bc7RgbaUnorm
        | TextureFormat::Bc7RgbaUnormSrgb
        | TextureFormat::Etc2Rgb8Unorm
        | TextureFormat::Etc2Rgb8UnormSrgb
        | TextureFormat::Etc2Rgb8A1Unorm
        | TextureFormat::Etc2Rgb8A1UnormSrgb
        | TextureFormat::Etc2Rgba8Unorm
        | TextureFormat::Etc2Rgba8UnormSrgb
        | TextureFormat::EacR11Unorm
        | TextureFormat::EacR11Snorm
        | TextureFormat::EacRg11Unorm
        | TextureFormat::EacRg11Snorm
        | TextureFormat::Astc {
            block: _,
            channel: _,
        } => None,
    }
}

struct RawBackfillImage {
    image: backfill::FImageHandle,
    _blob: backfill::FBlobHandle,
}

impl RawBackfillImage {
    fn handle(&self) -> &backfill::FImageHandle {
        &self.image
    }
}

fn convert_image(texture: &Image) -> Option<RawBackfillImage> {
    debug!("Converting image to bffi image...");

    let desc = bffi::FImageRawDesc {
        width: texture.width(),
        height: texture.height(),
        n_channels: texture.texture_descriptor.format.components(),
        byte_size: texture.data.as_deref()?.len().try_into().ok()?,
        type_: if is_image_float(texture).unwrap_or_default() {
            FPixelType_PIXEL_FLOAT32
        } else {
            FPixelType_PIXEL_UBYTE
        },
        colorspace: if texture.texture_descriptor.format.is_srgb() {
            FColorSpace_CS_SRGB
        } else {
            FColorSpace_CS_LINEAR
        },
    };

    debug!("Built description {:?}", desc);

    let blob = backfill::blob_from_slice(texture.data.as_deref()?).ok()?;

    let reference = backfill::blobref_whole(&blob);

    unsafe {
        NonNull::new(backfill::DYN_LIBRARY.fimg_init_raw(reference, &raw const desc)).map(|x| {
            RawBackfillImage {
                image: backfill::FImageHandle::from_nonnull(x),
                _blob: blob,
            }
        })
    }
}

pub fn convert_texture(
    session: &backfill::FSessionHandle,
    texture: &Image,
) -> Option<backfill::FTextureHandle> {
    debug!("Converting image to bffi texture...");
    let image = convert_image(texture)?;

    debug!("Image ready, creating texture...");

    use bevy::render::render_resource::TextureFormat;

    use bffi::*;

    // TODO, convert if the backend cant support a texture
    let bffmt = match texture.texture_descriptor.format.remove_srgb_suffix() {
        TextureFormat::R8Unorm => FTextureFormat_FMT_R8,
        TextureFormat::Rg8Unorm => FTextureFormat_FMT_RG8,
        TextureFormat::Rgba8Unorm => FTextureFormat_FMT_RGBA8,
        TextureFormat::R16Float => FTextureFormat_FMT_R16F,
        TextureFormat::Rg16Float => FTextureFormat_FMT_RG16F,
        TextureFormat::Rgba16Float => FTextureFormat_FMT_RGBA16F,
        TextureFormat::Rgba32Float => FTextureFormat_FMT_RGBA32F,
        TextureFormat::Rg11b10Ufloat => FTextureFormat_FMT_R11F_G11F_B10F,
        _ => {
            debug!(
                "Unable to match texture format {:?}",
                texture.texture_descriptor.format
            );
            return None;
        }
    };

    let config = backfill::tex_config_from_image(image.handle(), bffmt).ok()?;

    unsafe {
        NonNull::new(backfill::DYN_LIBRARY.ftex_init(session.as_ptr(), config.as_ptr()))
            .map(|x| backfill::FTextureHandle::from_nonnull(x))
    }
}

pub fn convert_material(
    session: &backfill::FSessionHandle,
    material: &StandardMaterial,
    map: &super::assets::TextureMap,
) -> Option<backfill::FMaterialHandle> {
    let mut config = backfill::material_config().ok()?;

    let needs_clearcoat = material.clearcoat > 0.0;
    let needs_transmission = material.specular_transmission > 0.0;

    if needs_clearcoat {
        config.set_option(backfill::ffi::FMatOption_CLEARCOAT, true);
    }

    config.set_option(backfill::ffi::FMatOption_IOR, true);

    if needs_transmission {
        config.set_option(backfill::ffi::FMatOption_TRANSMISSION, true);
    }

    match material.alpha_mode {
        AlphaMode::Opaque => config.set_blend(backfill::ffi::FMatBlendType_OPAQUE),
        AlphaMode::Mask(_) => config.set_blend(backfill::ffi::FMatBlendType_MASK),
        AlphaMode::Blend => config.set_blend(backfill::ffi::FMatBlendType_BLEND),
        _ => {}
    }

    set_texture(
        &mut config,
        bffi::FMatTexSemantic_BASE_COLOR_TEX,
        &material.base_color_texture,
        &material.base_color_channel,
        map,
    );

    set_texture(
        &mut config,
        bffi::FMatTexSemantic_NORMAL_TEX,
        &material.normal_map_texture,
        &material.normal_map_channel,
        map,
    );

    set_texture(
        &mut config,
        bffi::FMatTexSemantic_METAL_ROUGH_TEX,
        &material.metallic_roughness_texture,
        &material.metallic_roughness_channel,
        map,
    );

    let bmat = backfill::material(session, &config).unwrap();

    let color = material.base_color.to_linear();

    backfill::material_set_base_color(
        &bmat,
        bffi::FColor {
            r: color.red,
            g: color.green,
            b: color.blue,
            a: color.alpha,
        },
    );

    unsafe {
        pub fn split_color_max(c: LinearRgba) -> (Vec3, f32) {
            let rgb = Vec3::new(c.red, c.green, c.blue);

            let strength = rgb.max_element();

            if strength > 0.0 {
                (rgb / strength, strength)
            } else {
                (Vec3::ZERO, 0.0)
            }
        }

        if material.emissive != LinearRgba::BLACK {
            let (factor, strength) = split_color_max(material.emissive);

            backfill::DYN_LIBRARY.fmaterial_set_emissive(bmat.as_ptr(), strength, factor.into());
        }
    }

    backfill::material_set_rm(&bmat, material.perceptual_roughness, material.metallic);

    unsafe {
        backfill::DYN_LIBRARY.fmaterial_set_ior(bmat.as_ptr(), material.ior);

        if needs_clearcoat {
            backfill::DYN_LIBRARY.fmaterial_set_clearcoat(bmat.as_ptr(), material.clearcoat);
        }

        if needs_transmission {
            backfill::DYN_LIBRARY
                .fmaterial_set_transmission(bmat.as_ptr(), material.specular_transmission);
        }
    }

    Some(bmat)
}

fn set_wrap(
    s: &mut backfill::BSampler,
    src_mode: bevy::image::ImageAddressMode,
    dst_mode: bffi::FTexAxis,
) {
    match src_mode {
        bevy::image::ImageAddressMode::ClampToEdge => {
            s.set_wrap(bffi::FWrapMode_WRAP_CLAMP, dst_mode)
        }
        bevy::image::ImageAddressMode::Repeat => s.set_wrap(bffi::FWrapMode_WRAP_REPEAT, dst_mode),
        bevy::image::ImageAddressMode::MirrorRepeat => {
            s.set_wrap(bffi::FWrapMode_WRAP_MIRROR_REPEAT, dst_mode)
        }
        bevy::image::ImageAddressMode::ClampToBorder => {
            s.set_wrap(bffi::FWrapMode_WRAP_CLAMP, dst_mode)
        }
    }
}

fn set_texture(
    config: &mut backfill::FMaterialConfigHandle,
    semantic: bffi::FMatTexSemantic,
    handle: &Option<Handle<Image>>,
    channel: &bevy::pbr::UvChannel,
    map: &super::assets::TextureMap,
) {
    debug!("set texture for material semantic {}", semantic);

    let Some(handle) = handle else {
        debug!("handle is none");
        return;
    };

    debug!("Handle is {}", handle.id());

    let Some((tex, sampler)) = map.get(&handle.id()) else {
        warn!("handle {} referencing unknown image", handle.id());
        return;
    };

    let slot = match channel {
        bevy::pbr::UvChannel::Uv0 => backfill::ffi::FMatTexUVSlot_UV0,
        bevy::pbr::UvChannel::Uv1 => backfill::ffi::FMatTexUVSlot_UV1,
    };

    let sampler = {
        let mut s = backfill::BSampler::new();

        let sampler = match sampler {
            bevy::image::ImageSampler::Default => &Default::default(),
            bevy::image::ImageSampler::Descriptor(image_sampler_descriptor) => {
                image_sampler_descriptor
            }
        };

        s.set_aniso(sampler.anisotropy_clamp.clamp(0, u8::MAX as u16) as u8);

        match (sampler.min_filter, sampler.mipmap_filter) {
            (bevy::image::ImageFilterMode::Nearest, bevy::image::ImageFilterMode::Nearest) => {
                s.set_min(bffi::FMinFilter_MIN_FILTER_NEAREST);
            }
            (bevy::image::ImageFilterMode::Linear, bevy::image::ImageFilterMode::Nearest) => {
                s.set_min(bffi::FMinFilter_MIN_FILTER_LINEAR);
            }
            _ => {
                s.set_min(bffi::FMinFilter_MIN_FILTER_LINEAR_MIPMAP_LINEAR);
            }
        }

        s.set_mag(match sampler.mag_filter {
            bevy::image::ImageFilterMode::Nearest => bffi::FMagFilter_MAG_FILTER_NEAREST,
            bevy::image::ImageFilterMode::Linear => bffi::FMagFilter_MAG_FILTER_LINEAR,
        });

        set_wrap(&mut s, sampler.address_mode_u, bffi::FTexAxis_AXIS_U);
        set_wrap(&mut s, sampler.address_mode_v, bffi::FTexAxis_AXIS_V);
        set_wrap(&mut s, sampler.address_mode_w, bffi::FTexAxis_AXIS_W);

        s
    };

    debug!("installing texture to ffi mat config");

    config.set_texture(semantic, slot, tex, sampler);
}
