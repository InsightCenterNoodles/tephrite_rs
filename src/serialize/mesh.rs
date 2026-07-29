//! Serialization for `bevy::render::mesh::Mesh` and related types.
//!
//! - `Indices` and `VertexAttributeValues` use compact tagged encodings.
//! - Only a fixed set of well-known vertex attributes are supported when
//!   decoding; others will cause a panic. This matches the needs of the
//!   replication pipeline and avoids string lookups.
//! - Morph targets are not yet supported.
use std::sync::{LazyLock, RwLock};

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{
    Indices, MeshVertexAttribute, MeshVertexAttributeId, PrimitiveTopology, VertexAttributeValues,
};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;

use crate::serialize::*;

/// Compact encoding for index buffers: tag + raw vector.
impl FastWrite for Indices {
    unsafe fn write_fast(&self, w: &mut impl ByteSink) {
        match self {
            Indices::U16(vec) => unsafe {
                0i8.write_fast(w);
                vec.as_slice().write_fast(w);
            },
            Indices::U32(vec) => unsafe {
                1i8.write_fast(w);
                vec.as_slice().write_fast(w);
            },
        }
    }
}
impl FastRead for Indices {
    type Ret = Self;
    unsafe fn read_fast<'a, S: ByteSource<'a>>(r: &mut S) -> Self::Ret {
        let index = unsafe { i8::read_fast(r) };

        match index {
            0 => Indices::U16(unsafe { Vec::<u16>::read_fast(r) }),
            1 => Indices::U32(unsafe { Vec::<u32>::read_fast(r) }),
            _ => panic!("Unknown index type"),
        }
    }
}

// =============================================================================

impl_fast_raw_item!(MeshVertexAttributeId);

// =============================================================================
// Encode `VertexAttributeValues` by variant tag then vector payload.
impl FastWrite for VertexAttributeValues {
    unsafe fn write_fast(&self, w: &mut impl ByteSink) {
        match self {
            VertexAttributeValues::Float32(values) => unsafe {
                0i8.write_fast(w);
                values.write_fast(w);
            },
            VertexAttributeValues::Sint32(values) => unsafe {
                1i8.write_fast(w);
                values.write_fast(w);
            },
            VertexAttributeValues::Uint32(values) => unsafe {
                2i8.write_fast(w);
                values.write_fast(w);
            },
            VertexAttributeValues::Float32x2(values) => unsafe {
                3i8.write_fast(w);
                values.write_fast(w);
            },
            VertexAttributeValues::Sint32x2(values) => unsafe {
                4i8.write_fast(w);
                values.write_fast(w);
            },
            VertexAttributeValues::Uint32x2(values) => unsafe {
                5i8.write_fast(w);
                values.write_fast(w);
            },
            VertexAttributeValues::Float32x3(values) => unsafe {
                6i8.write_fast(w);
                values.write_fast(w);
            },
            VertexAttributeValues::Sint32x3(values) => unsafe {
                7i8.write_fast(w);
                values.write_fast(w);
            },
            VertexAttributeValues::Uint32x3(values) => unsafe {
                8i8.write_fast(w);
                values.write_fast(w);
            },
            VertexAttributeValues::Float32x4(values) => unsafe {
                9i8.write_fast(w);
                values.write_fast(w);
            },
            VertexAttributeValues::Sint32x4(values) => unsafe {
                10i8.write_fast(w);
                values.write_fast(w);
            },
            VertexAttributeValues::Uint32x4(values) => unsafe {
                11i8.write_fast(w);
                values.write_fast(w);
            },
            VertexAttributeValues::Sint16x2(values) => unsafe {
                12i8.write_fast(w);
                values.write_fast(w);
            },
            VertexAttributeValues::Snorm16x2(values) => unsafe {
                13i8.write_fast(w);
                values.write_fast(w);
            },
            VertexAttributeValues::Uint16x2(values) => unsafe {
                14i8.write_fast(w);
                values.write_fast(w);
            },
            VertexAttributeValues::Unorm16x2(values) => unsafe {
                15i8.write_fast(w);
                values.write_fast(w);
            },
            VertexAttributeValues::Sint16x4(values) => unsafe {
                16i8.write_fast(w);
                values.write_fast(w);
            },
            VertexAttributeValues::Snorm16x4(values) => unsafe {
                17i8.write_fast(w);
                values.write_fast(w);
            },
            VertexAttributeValues::Uint16x4(values) => unsafe {
                18i8.write_fast(w);
                values.write_fast(w);
            },
            VertexAttributeValues::Unorm16x4(values) => unsafe {
                19i8.write_fast(w);
                values.write_fast(w);
            },
            VertexAttributeValues::Sint8x2(values) => unsafe {
                20i8.write_fast(w);
                values.write_fast(w);
            },
            VertexAttributeValues::Snorm8x2(values) => unsafe {
                21i8.write_fast(w);
                values.write_fast(w);
            },
            VertexAttributeValues::Uint8x2(values) => unsafe {
                22i8.write_fast(w);
                values.write_fast(w);
            },
            VertexAttributeValues::Unorm8x2(values) => unsafe {
                23i8.write_fast(w);
                values.write_fast(w);
            },
            VertexAttributeValues::Sint8x4(values) => unsafe {
                24i8.write_fast(w);
                values.write_fast(w);
            },
            VertexAttributeValues::Snorm8x4(values) => unsafe {
                25i8.write_fast(w);
                values.write_fast(w);
            },
            VertexAttributeValues::Uint8x4(values) => unsafe {
                26i8.write_fast(w);
                values.write_fast(w);
            },
            VertexAttributeValues::Unorm8x4(values) => unsafe {
                27i8.write_fast(w);
                values.write_fast(w);
            },
            VertexAttributeValues::Uint8(items) => unsafe {
                28i8.write_fast(w);
                items.write_fast(w);
            },
            VertexAttributeValues::Sint8(items) => unsafe {
                29i8.write_fast(w);
                items.write_fast(w);
            },
            VertexAttributeValues::Unorm8(items) => unsafe {
                30i8.write_fast(w);
                items.write_fast(w);
            },
            VertexAttributeValues::Snorm8(items) => unsafe {
                31i8.write_fast(w);
                items.write_fast(w);
            },
            VertexAttributeValues::Uint16(items) => unsafe {
                32i8.write_fast(w);
                items.write_fast(w);
            },
            VertexAttributeValues::Sint16(items) => unsafe {
                33i8.write_fast(w);
                items.write_fast(w);
            },
            VertexAttributeValues::Unorm16(items) => unsafe {
                34i8.write_fast(w);
                items.write_fast(w);
            },
            VertexAttributeValues::Snorm16(items) => unsafe {
                35i8.write_fast(w);
                items.write_fast(w);
            },
            VertexAttributeValues::Float16(f16s) => unsafe {
                36i8.write_fast(w);
                f16s.write_fast(w);
            },
            VertexAttributeValues::Float16x2(items) => unsafe {
                37i8.write_fast(w);
                items.write_fast(w);
            },
            VertexAttributeValues::Float16x4(items) => unsafe {
                38i8.write_fast(w);
                items.write_fast(w);
            },
            VertexAttributeValues::Float64(items) => unsafe {
                39i8.write_fast(w);
                items.write_fast(w);
            },
            VertexAttributeValues::Float64x2(items) => unsafe {
                40i8.write_fast(w);
                items.write_fast(w);
            },
            VertexAttributeValues::Float64x3(items) => unsafe {
                41i8.write_fast(w);
                items.write_fast(w);
            },
            VertexAttributeValues::Float64x4(items) => unsafe {
                42i8.write_fast(w);
                items.write_fast(w);
            },
            VertexAttributeValues::Unorm10_10_10_2(items) => unsafe {
                43i8.write_fast(w);
                items.write_fast(w);
            },
            VertexAttributeValues::Unorm8x4Bgra(items) => unsafe {
                44i8.write_fast(w);
                items.write_fast(w);
            },
        }
    }
}

impl FastRead for VertexAttributeValues {
    type Ret = Self;

    unsafe fn read_fast<'a, S: ByteSource<'a>>(r: &mut S) -> Self::Ret {
        let index = unsafe { i8::read_fast(r) };

        match index {
            0 => VertexAttributeValues::Float32(read_fast(r)),
            1 => VertexAttributeValues::Sint32(read_fast(r)),
            2 => VertexAttributeValues::Uint32(read_fast(r)),
            3 => VertexAttributeValues::Float32x2(read_fast(r)),
            4 => VertexAttributeValues::Sint32x2(read_fast(r)),
            5 => VertexAttributeValues::Uint32x2(read_fast(r)),
            6 => VertexAttributeValues::Float32x3(read_fast(r)),
            7 => VertexAttributeValues::Sint32x3(read_fast(r)),
            8 => VertexAttributeValues::Uint32x3(read_fast(r)),
            9 => VertexAttributeValues::Float32x4(read_fast(r)),
            10 => VertexAttributeValues::Sint32x4(read_fast(r)),
            11 => VertexAttributeValues::Uint32x4(read_fast(r)),
            12 => VertexAttributeValues::Sint16x2(read_fast(r)),
            13 => VertexAttributeValues::Snorm16x2(read_fast(r)),
            14 => VertexAttributeValues::Uint16x2(read_fast(r)),
            15 => VertexAttributeValues::Unorm16x2(read_fast(r)),
            16 => VertexAttributeValues::Sint16x4(read_fast(r)),
            17 => VertexAttributeValues::Snorm16x4(read_fast(r)),
            18 => VertexAttributeValues::Uint16x4(read_fast(r)),
            19 => VertexAttributeValues::Unorm16x4(read_fast(r)),
            20 => VertexAttributeValues::Sint8x2(read_fast(r)),
            21 => VertexAttributeValues::Snorm8x2(read_fast(r)),
            22 => VertexAttributeValues::Uint8x2(read_fast(r)),
            23 => VertexAttributeValues::Unorm8x2(read_fast(r)),
            24 => VertexAttributeValues::Sint8x4(read_fast(r)),
            25 => VertexAttributeValues::Snorm8x4(read_fast(r)),
            26 => VertexAttributeValues::Uint8x4(read_fast(r)),
            27 => VertexAttributeValues::Unorm8x4(read_fast(r)),
            28 => VertexAttributeValues::Uint8(read_fast(r)),
            29 => VertexAttributeValues::Sint8(read_fast(r)),
            30 => VertexAttributeValues::Unorm8(read_fast(r)),
            31 => VertexAttributeValues::Snorm8(read_fast(r)),
            32 => VertexAttributeValues::Uint16(read_fast(r)),
            33 => VertexAttributeValues::Sint16(read_fast(r)),
            34 => VertexAttributeValues::Unorm16(read_fast(r)),
            35 => VertexAttributeValues::Snorm16(read_fast(r)),
            36 => VertexAttributeValues::Float16(read_fast(r)),
            37 => VertexAttributeValues::Float16x2(read_fast(r)),
            38 => VertexAttributeValues::Float16x4(read_fast(r)),
            39 => VertexAttributeValues::Float64(read_fast(r)),
            40 => VertexAttributeValues::Float64x2(read_fast(r)),
            41 => VertexAttributeValues::Float64x3(read_fast(r)),
            42 => VertexAttributeValues::Float64x4(read_fast(r)),
            43 => VertexAttributeValues::Unorm10_10_10_2(read_fast(r)),
            44 => VertexAttributeValues::Unorm8x4Bgra(read_fast(r)),
            _ => panic!("Unknown vertex attribute type. This should not happen."),
        }
    }
}

// =============================================================================

impl_fast_raw_item!(PrimitiveTopology);

// =============================================================================

impl_fast_raw_item!(RenderAssetUsages);

// =============================================================================

impl FastWrite for MeshVertexAttribute {
    unsafe fn write_fast(&self, w: &mut impl ByteSink) {
        // we write_fast under the assumption that we know what we are doing.
        // That is, that this attrib is well known
        unsafe { self.id.write_fast(w) };
    }
}

impl FastRead for MeshVertexAttribute {
    type Ret = Self;

    unsafe fn read_fast<'a, S: ByteSource<'a>>(r: &mut S) -> Self::Ret {
        let id: MeshVertexAttributeId = read_fast(r);

        const POS_ID: MeshVertexAttributeId = Mesh::ATTRIBUTE_POSITION.id;
        const NOR_ID: MeshVertexAttributeId = Mesh::ATTRIBUTE_NORMAL.id;

        const UV0_ID: MeshVertexAttributeId = Mesh::ATTRIBUTE_UV_0.id;
        const UV1_ID: MeshVertexAttributeId = Mesh::ATTRIBUTE_UV_1.id;

        const TAN_ID: MeshVertexAttributeId = Mesh::ATTRIBUTE_TANGENT.id;
        const COL_ID: MeshVertexAttributeId = Mesh::ATTRIBUTE_COLOR.id;

        const JW_ID: MeshVertexAttributeId = Mesh::ATTRIBUTE_JOINT_WEIGHT.id;
        const JI_ID: MeshVertexAttributeId = Mesh::ATTRIBUTE_JOINT_INDEX.id;

        match id {
            POS_ID => Mesh::ATTRIBUTE_POSITION,
            NOR_ID => Mesh::ATTRIBUTE_NORMAL,

            UV0_ID => Mesh::ATTRIBUTE_UV_0,
            UV1_ID => Mesh::ATTRIBUTE_UV_1,

            TAN_ID => Mesh::ATTRIBUTE_TANGENT,
            COL_ID => Mesh::ATTRIBUTE_COLOR,

            JW_ID => Mesh::ATTRIBUTE_JOINT_WEIGHT,
            JI_ID => Mesh::ATTRIBUTE_JOINT_INDEX,
            _ => panic!("Unknown vertex attribute!"),
        }
    }
}

// =============================================================================

/// Serialize core mesh state: topology, usage, attributes, and optional indices.
impl FastWrite for Mesh {
    unsafe fn write_fast(&self, w: &mut impl ByteSink) {
        unsafe {
            self.primitive_topology().write_fast(w);
            self.asset_usage.write_fast(w);

            // slow, but it works for now
            let count: u8 = self.attributes().count().try_into().unwrap();
            count.write_fast(w);

            for attrib in self.attributes() {
                attrib.0.write_fast(w);
                attrib.1.write_fast(w);
            }

            self.indices().write_fast(w);

            // MORPH IS NOT YET SUPPORTED!!
        }
    }
}

// Only a subset of attributes is supported by this decoder. The
// indices into this table must match `MeshVertexAttributeId` values
// for these attributes.
const ATTRIB_LOOKUP: [MeshVertexAttribute; 8] = [
    Mesh::ATTRIBUTE_POSITION,
    Mesh::ATTRIBUTE_NORMAL,
    Mesh::ATTRIBUTE_UV_0,
    Mesh::ATTRIBUTE_UV_1,
    Mesh::ATTRIBUTE_TANGENT,
    Mesh::ATTRIBUTE_COLOR,
    Mesh::ATTRIBUTE_JOINT_WEIGHT,
    Mesh::ATTRIBUTE_JOINT_INDEX,
];

impl FastRead for Mesh {
    type Ret = Self;
    unsafe fn read_fast<'a, S: ByteSource<'a>>(r: &mut S) -> Self::Ret {
        let pt = unsafe { PrimitiveTopology::read_fast(r) };
        let au = unsafe { RenderAssetUsages::read_fast(r) };

        let mut ret = Mesh::new(pt, au);

        let attrib_count = unsafe { u8::read_fast(r) };

        // Check tests for more intensive validation of our assertions here
        const {
            assert!(std::mem::size_of::<usize>() == std::mem::size_of::<MeshVertexAttributeId>());
        }

        for _ in 0..attrib_count {
            let aid = unsafe { MeshVertexAttributeId::read_fast(r) };
            let data = unsafe { VertexAttributeValues::read_fast(r) };

            // This should be safe as we are just getting access to the internal wrapped usize
            let unsafe_id: usize = unsafe { std::mem::transmute(aid) };

            // TODO: Custom mesh attributes will cause a crash
            let attrib = &ATTRIB_LOOKUP[unsafe_id];
            ret.insert_attribute(*attrib, data);
        }

        if let Some(index) = unsafe { Option::<Indices>::read_fast(r) } {
            ret.insert_indices(index);
        }

        ret
    }
}

static MAP: LazyLock<RwLock<HashMap<AssetId<Mesh>, Handle<Mesh>>>> =
    LazyLock::new(|| Default::default());

impl RemappableAsset for Mesh {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serialize::fast_io::{ByteReader, ByteWriter};
    use crate::serialize::fast_ser::{FastRead, FastWrite};

    fn roundtrip<T: FastWrite + FastRead<Ret = T>>(x: &T) -> T {
        let mut buf = [0u8; 2048];
        let mut w = ByteWriter::new(&mut buf);
        unsafe { x.write_fast(&mut w) };
        let mut r = ByteReader::new(&buf);
        unsafe { T::read_fast(&mut r) }
    }

    #[test]
    fn indices_roundtrip_u16() {
        let idx = Indices::U16(vec![0, 1, 2, 2, 3, 0]);
        let out = roundtrip(&idx);

        match (idx, out) {
            (Indices::U16(items), Indices::U16(items2)) => assert_eq!(items, items2),
            _ => panic!("Mismatch"),
        }
    }

    #[test]
    fn indices_roundtrip_u32() {
        let idx = Indices::U32(vec![0, 1, 2, 2, 3, 0]);
        let out = roundtrip(&idx);
        match (idx, out) {
            (Indices::U32(items), Indices::U32(items2)) => assert_eq!(items, items2),
            _ => panic!("Mismatch"),
        }
    }

    #[test]
    fn vertex_attribute_values_roundtrip() {
        let v = VertexAttributeValues::Float32x3(vec![[1.0, 2.0, 3.0], [4.5, 6.25, 8.125]]);
        let out = roundtrip(&v);

        match (v, out) {
            (VertexAttributeValues::Float32x3(x), VertexAttributeValues::Float32x3(y)) => {
                assert_eq!(x, y)
            }
            _ => panic!("Mismatch"),
        }
    }

    #[test]
    fn mesh_vertex_attribute_id_roundtrip() {
        let a = Mesh::ATTRIBUTE_POSITION; // known attribute
        let mut buf = [0u8; 64];
        let mut w = ByteWriter::new(&mut buf);
        unsafe { a.write_fast(&mut w) };
        let mut r = ByteReader::new(&buf);
        let out: MeshVertexAttribute = unsafe { MeshVertexAttribute::read_fast(&mut r) };
        assert_eq!(out.id, a.id);
    }

    #[test]
    fn mesh_attribute_assert() {
        for attrib in ATTRIB_LOOKUP {
            // Safety: the attribute ID is just a wrapper around a u64

            assert!(std::mem::size_of::<u64>() == std::mem::size_of::<MeshVertexAttributeId>());

            let unsafe_id: u64 = unsafe { std::mem::transmute(attrib.id) };

            let should_be = ATTRIB_LOOKUP.get(unsafe_id as usize).cloned().unwrap();

            assert_eq!(should_be.format, attrib.format);
            assert_eq!(should_be.id, attrib.id);
            assert_eq!(should_be.name, attrib.name);
        }
    }
}
