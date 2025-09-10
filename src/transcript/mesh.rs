use bevy::render::mesh::{Indices, MeshVertexAttribute, PrimitiveTopology, VertexAttributeValues};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::{prelude::*, render::mesh::MeshVertexAttributeId};

use crate::transcript::deserialize;

use super::deserialize_pod_slice;
use super::{
    TDeserialize, TSerialize,
    common::{byte_deserialize, byte_serialize},
    serialize_pod_slice,
};

impl TSerialize for Indices {
    fn serialize(&self, w: &mut impl std::io::Write) {
        match self {
            Indices::U16(vec) => {
                0i8.serialize(w);
                serialize_pod_slice(vec, w);
            }
            Indices::U32(vec) => {
                1i8.serialize(w);
                serialize_pod_slice(vec, w);
            }
        }
    }
}
impl TDeserialize for Indices {
    fn deserialize(r: &mut impl std::io::Read) -> Self {
        let index = i8::deserialize(r);

        match index {
            0 => Indices::U16(deserialize_pod_slice(r)),
            1 => Indices::U32(deserialize_pod_slice(r)),
            _ => panic!("Unknown index type"),
        }
    }
}

// =============================================================================

impl TSerialize for MeshVertexAttributeId {
    fn serialize(&self, w: &mut impl std::io::Write) {
        // this should just be a u64
        unsafe { byte_serialize(self, w) };
    }
}
impl TDeserialize for MeshVertexAttributeId {
    fn deserialize(r: &mut impl std::io::Read) -> Self {
        unsafe { byte_deserialize(r) }
    }
}

// =============================================================================
// ugh
impl TSerialize for VertexAttributeValues {
    fn serialize(&self, w: &mut impl std::io::Write) {
        match self {
            VertexAttributeValues::Float32(values) => {
                0i8.serialize(w);
                serialize_pod_slice(values, w)
            }
            VertexAttributeValues::Sint32(values) => {
                1i8.serialize(w);
                serialize_pod_slice(values, w)
            }
            VertexAttributeValues::Uint32(values) => {
                2i8.serialize(w);
                serialize_pod_slice(values, w)
            }
            VertexAttributeValues::Float32x2(values) => {
                3i8.serialize(w);
                serialize_pod_slice(values, w)
            }
            VertexAttributeValues::Sint32x2(values) => {
                4i8.serialize(w);
                serialize_pod_slice(values, w)
            }
            VertexAttributeValues::Uint32x2(values) => {
                5i8.serialize(w);
                serialize_pod_slice(values, w)
            }
            VertexAttributeValues::Float32x3(values) => {
                6i8.serialize(w);
                serialize_pod_slice(values, w)
            }
            VertexAttributeValues::Sint32x3(values) => {
                7i8.serialize(w);
                serialize_pod_slice(values, w)
            }
            VertexAttributeValues::Uint32x3(values) => {
                8i8.serialize(w);
                serialize_pod_slice(values, w)
            }
            VertexAttributeValues::Float32x4(values) => {
                9i8.serialize(w);
                serialize_pod_slice(values, w)
            }
            VertexAttributeValues::Sint32x4(values) => {
                10i8.serialize(w);
                serialize_pod_slice(values, w)
            }
            VertexAttributeValues::Uint32x4(values) => {
                11i8.serialize(w);
                serialize_pod_slice(values, w)
            }
            VertexAttributeValues::Sint16x2(values) => {
                12i8.serialize(w);
                serialize_pod_slice(values, w)
            }
            VertexAttributeValues::Snorm16x2(values) => {
                13i8.serialize(w);
                serialize_pod_slice(values, w)
            }
            VertexAttributeValues::Uint16x2(values) => {
                14i8.serialize(w);
                serialize_pod_slice(values, w)
            }
            VertexAttributeValues::Unorm16x2(values) => {
                15i8.serialize(w);
                serialize_pod_slice(values, w)
            }
            VertexAttributeValues::Sint16x4(values) => {
                16i8.serialize(w);
                serialize_pod_slice(values, w)
            }
            VertexAttributeValues::Snorm16x4(values) => {
                17i8.serialize(w);
                serialize_pod_slice(values, w)
            }
            VertexAttributeValues::Uint16x4(values) => {
                18i8.serialize(w);
                serialize_pod_slice(values, w)
            }
            VertexAttributeValues::Unorm16x4(values) => {
                19i8.serialize(w);
                serialize_pod_slice(values, w)
            }
            VertexAttributeValues::Sint8x2(values) => {
                20i8.serialize(w);
                serialize_pod_slice(values, w)
            }
            VertexAttributeValues::Snorm8x2(values) => {
                21i8.serialize(w);
                serialize_pod_slice(values, w)
            }
            VertexAttributeValues::Uint8x2(values) => {
                22i8.serialize(w);
                serialize_pod_slice(values, w)
            }
            VertexAttributeValues::Unorm8x2(values) => {
                23i8.serialize(w);
                serialize_pod_slice(values, w)
            }
            VertexAttributeValues::Sint8x4(values) => {
                24i8.serialize(w);
                serialize_pod_slice(values, w)
            }
            VertexAttributeValues::Snorm8x4(values) => {
                25i8.serialize(w);
                serialize_pod_slice(values, w)
            }
            VertexAttributeValues::Uint8x4(values) => {
                26i8.serialize(w);
                serialize_pod_slice(values, w)
            }
            VertexAttributeValues::Unorm8x4(values) => {
                27i8.serialize(w);
                serialize_pod_slice(values, w)
            }
        }
    }
}

impl TDeserialize for VertexAttributeValues {
    fn deserialize(r: &mut impl std::io::Read) -> Self {
        let index = i8::deserialize(r);

        match index {
            0 => VertexAttributeValues::Float32(deserialize_pod_slice(r)),
            1 => VertexAttributeValues::Sint32(deserialize_pod_slice(r)),
            2 => VertexAttributeValues::Uint32(deserialize_pod_slice(r)),
            3 => VertexAttributeValues::Float32x2(deserialize_pod_slice(r)),
            4 => VertexAttributeValues::Sint32x2(deserialize_pod_slice(r)),
            5 => VertexAttributeValues::Uint32x2(deserialize_pod_slice(r)),
            6 => VertexAttributeValues::Float32x3(deserialize_pod_slice(r)),
            7 => VertexAttributeValues::Sint32x3(deserialize_pod_slice(r)),
            8 => VertexAttributeValues::Uint32x3(deserialize_pod_slice(r)),
            9 => VertexAttributeValues::Float32x4(deserialize_pod_slice(r)),
            10 => VertexAttributeValues::Sint32x4(deserialize_pod_slice(r)),
            11 => VertexAttributeValues::Uint32x4(deserialize_pod_slice(r)),
            12 => VertexAttributeValues::Sint16x2(deserialize_pod_slice(r)),
            13 => VertexAttributeValues::Snorm16x2(deserialize_pod_slice(r)),
            14 => VertexAttributeValues::Uint16x2(deserialize_pod_slice(r)),
            15 => VertexAttributeValues::Unorm16x2(deserialize_pod_slice(r)),
            16 => VertexAttributeValues::Sint16x4(deserialize_pod_slice(r)),
            17 => VertexAttributeValues::Snorm16x4(deserialize_pod_slice(r)),
            18 => VertexAttributeValues::Uint16x4(deserialize_pod_slice(r)),
            19 => VertexAttributeValues::Unorm16x4(deserialize_pod_slice(r)),
            20 => VertexAttributeValues::Sint8x2(deserialize_pod_slice(r)),
            21 => VertexAttributeValues::Snorm8x2(deserialize_pod_slice(r)),
            22 => VertexAttributeValues::Uint8x2(deserialize_pod_slice(r)),
            23 => VertexAttributeValues::Unorm8x2(deserialize_pod_slice(r)),
            24 => VertexAttributeValues::Sint8x4(deserialize_pod_slice(r)),
            25 => VertexAttributeValues::Snorm8x4(deserialize_pod_slice(r)),
            26 => VertexAttributeValues::Uint8x4(deserialize_pod_slice(r)),
            27 => VertexAttributeValues::Unorm8x4(deserialize_pod_slice(r)),
            _ => panic!("Unknown attribute value type!"),
        }
    }
}

// =============================================================================

impl TSerialize for PrimitiveTopology {
    fn serialize(&self, w: &mut impl std::io::Write) {
        unsafe { byte_serialize(self, w) };
    }
}
impl TDeserialize for PrimitiveTopology {
    fn deserialize(r: &mut impl std::io::Read) -> Self {
        unsafe { byte_deserialize(r) }
    }
}

// =============================================================================

impl TSerialize for RenderAssetUsages {
    fn serialize(&self, w: &mut impl std::io::Write) {
        unsafe { byte_serialize(self, w) };
    }
}
impl TDeserialize for RenderAssetUsages {
    fn deserialize(r: &mut impl std::io::Read) -> Self {
        unsafe { byte_deserialize(r) }
    }
}

// =============================================================================

impl TSerialize for MeshVertexAttribute {
    fn serialize(&self, w: &mut impl std::io::Write) {
        // we serialize under the assumption that we know what we are doing.
        // That is, that this attrib is well known
        self.id.serialize(w);
    }
}

impl TDeserialize for MeshVertexAttribute {
    fn deserialize(r: &mut impl std::io::Read) -> Self {
        let id = deserialize(r);

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

impl TSerialize for Mesh {
    fn serialize(&self, w: &mut impl std::io::Write) {
        self.primitive_topology().serialize(w);
        self.asset_usage.serialize(w);

        // slow, but it works for now
        let count: u8 = self.attributes().count().try_into().unwrap();
        count.serialize(w);

        for attrib in self.attributes() {
            attrib.0.serialize(w);
            attrib.1.serialize(w);
        }

        self.indices().serialize(w);

        // MORPH IS NOT YET SUPPORTED!!
    }
}
impl TDeserialize for Mesh {
    fn deserialize(r: &mut impl std::io::Read) -> Self {
        let pt = deserialize(r);
        let au = deserialize(r);

        let mut ret = Mesh::new(pt, au);

        let attrib_count = u8::deserialize(r);

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

        const {
            assert!(std::mem::size_of::<usize>() == std::mem::size_of::<MeshVertexAttributeId>());
        }

        for _ in 0..attrib_count {
            let aid = MeshVertexAttributeId::deserialize(r);
            let data = VertexAttributeValues::deserialize(r);

            // This should be safe as we are just getting access to the internal wrapped usize
            let unsafe_id: usize = unsafe { std::mem::transmute(aid) };

            let attrib = &ATTRIB_LOOKUP[unsafe_id];
            ret.insert_attribute(*attrib, data);
        }

        if let Some(index) = Option::<Indices>::deserialize(r) {
            ret.insert_indices(index);
        }

        ret
    }
}
