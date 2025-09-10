use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};

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
    use bevy::render::mesh::VertexAttributeValues::*;

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

pub fn convert_material(
    session: &backfill::FSessionHandle,
    material: &StandardMaterial,
) -> Option<backfill::FMaterialHandle> {
    let bmat = backfill::material(session, backfill::MatConfigFlags::empty(), 0).unwrap();

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
