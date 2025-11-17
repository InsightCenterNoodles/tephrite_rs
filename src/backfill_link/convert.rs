use bevy::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;

use crate::backfill;
use crate::backfill::ffi as bffi;

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

pub fn convert_mesh(
    session: &backfill::FSessionHandle,
    mesh: &Mesh,
) -> Option<backfill::FMeshHandle> {
    use bevy::mesh::VertexAttributeValues::*;

    if !matches!(mesh.primitive_topology(), PrimitiveTopology::TriangleList) {
        debug!("Mesh is not triangles");
        return None;
    }

    let Some(Float32x3(positions)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else {
        debug!("Mesh is missing position information");
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
        debug!("Mesh has no index information");
        return None;
    };

    let index_tmp: Vec<_>;

    let index_slice = match index {
        Indices::U16(items) => items.as_slice(),
        Indices::U32(items) => {
            if mesh.count_vertices() >= u16::MAX.into() {
                debug!("Cannot convert!");
                return None;
            }
            index_tmp = items.iter().map(|x| *x as u16).collect();
            index_tmp.as_slice()
        }
    };

    let mut out_verts = Vec::<bffi::FPackedVertex>::new();

    // Safety: Vertex information is POD and will be overwritten
    out_verts.resize(mesh.count_vertices(), unsafe { std::mem::zeroed() });

    assert!(
        index_slice.len() % 3 == 0,
        "TriangleList must be multiple of 3 indices"
    );
    let tri_count: u32 = (index_slice.len() / 3).try_into().unwrap();

    unsafe {
        bffi::pack_vertex_u16(
            unpacked.as_ptr(),
            unpacked.len().try_into().unwrap(),
            index_slice.as_ptr() as *const bffi::ushort3,
            tri_count,
            out_verts.as_mut_ptr(),
        )
    };

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
    debug_assert!(index_slice.iter().all(|&i| (i as usize) < unpacked.len()));
    debug_assert_eq!(std::mem::size_of::<bffi::ushort3>(), 6);

    backfill::mesh_from_refs(
        session,
        backfill::BlobReference::whole(&vert_blob),
        vcount as u32,
        backfill::BlobReference::whole(&index_blob),
        index_slice.len() as u32,
        bffi::FMeshIndexType_U16,
        bffi::aabb {
            minimum: bounding.0.into(),
            maximum: bounding.1.into(),
        },
    )
    .ok()
}

pub fn convert_texture(
    session: &backfill::FSessionHandle,
    texture: &Image,
) -> Option<backfill::FTextureHandle> {
    todo!()
}

pub fn convert_material(
    session: &backfill::FSessionHandle,
    material: &StandardMaterial,
    map: &super::mesh_mat_bind::TextureMap,
) -> Option<backfill::FMaterialHandle> {
    let mut config = backfill::material_config().ok()?;

    config.set_option(backfill::ffi::FMatOption_CLEARCOAT, true);
    config.set_option(backfill::ffi::FMatOption_IOR, true);
    config.set_option(backfill::ffi::FMatOption_TRANSMISSION, true);

    match material.alpha_mode {
        AlphaMode::Opaque => config.set_blend(backfill::ffi::FMatBlendType_OPAQUE),
        AlphaMode::Mask(_) => config.set_blend(backfill::ffi::FMatBlendType_MASK),
        AlphaMode::Blend => config.set_blend(backfill::ffi::FMatBlendType_BLEND),
        _ => {}
    }

    set_texture(
        &mut config,
        &material.base_color_texture,
        &material.base_color_channel,
        map,
    );

    set_texture(
        &mut config,
        &material.normal_map_texture,
        &material.normal_map_channel,
        map,
    );

    set_texture(
        &mut config,
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

    backfill::material_set_rm(&bmat, material.perceptual_roughness, material.metallic);

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
    handle: &Option<Handle<Image>>,
    channel: &bevy::pbr::UvChannel,
    map: &super::mesh_mat_bind::TextureMap,
) {
    if let Some((tex, sampler)) = handle.as_ref().and_then(|x| map.get(&x.id())) {
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

        config.set_texture(
            backfill::ffi::FMatTexSemantic_BASE_COLOR_TEX,
            slot,
            tex,
            sampler,
        );
    }
}
