//! Functionality to build a transcript, or serialized types

pub mod asset;
pub mod common;
pub mod components;
mod image;
pub mod material;
pub mod math;
pub mod mesh;
mod transcript_deserializer;
pub(crate) mod transcript_reader;
mod transcript_serializer;
pub(crate) mod transcript_writer;
mod zerocopy;

pub use transcript_deserializer::*;
pub use transcript_serializer::*;

#[allow(unused)]
fn test_serialization<T, F>(v: T, equals: F)
where
    T: TSerialize + TDeserialize,
    F: FnOnce(T, T) -> bool,
{
    use std::io::Cursor;

    let mut bytes = Vec::<u8>::new();
    v.serialize(&mut bytes);
    const SCRIBBLE_GUARD: u32 = 0xDEADBEEFu32;
    SCRIBBLE_GUARD.serialize(&mut bytes);

    let mut cursor = Cursor::new(bytes);
    let local_v = deserialize(&mut cursor);
    let check = u32::deserialize(&mut cursor);
    assert_eq!(SCRIBBLE_GUARD, check, "size guard check");
    assert!(equals(v, local_v));
}

pub mod macros {
    /// A macro to help serialize foreign structs.
    /// Takes the type to write serialization impls for, and then a list
    /// of members to serialize.
    macro_rules! struct_serde_helper {
        ($T:ty, $($idents:ident),+ ) => {
            impl TSerialize for $T {
                fn serialize(&self, w: &mut impl std::io::Write) {
                    $(
                        self.$idents.serialize(w);
                    )*
                }
            }

            impl TDeserialize for $T {
                #[allow(clippy::needless_update)]
                fn deserialize(r: &mut impl std::io::Read) -> Self {
                    Self {
                        $(
                            $idents : deserialize(r),
                        )*
                        ..Default::default()
                    }
                }
            }
        };
    }

    /// Implement serialization for a foreign POD type.
    /// ONLY FOR POD TYPES!!
    macro_rules! raw_item_helper {
        ($T:ty) => {
            impl TSerialize for $T {
                fn serialize(&self, w: &mut impl std::io::Write) {
                    unsafe { byte_serialize(self, w) }
                }
            }

            impl TDeserialize for $T {
                fn deserialize(r: &mut impl std::io::Read) -> Self {
                    unsafe { byte_deserialize(r) }
                }
            }
        };
    }

    pub(crate) use raw_item_helper;
    pub(crate) use struct_serde_helper;
}

#[cfg(test)]
mod tests {
    use core::f32;
    use std::io::Cursor;

    use teph_macro::TSerialize;

    use super::*;

    #[derive(Debug, TSerialize, Clone, PartialEq)]
    struct MyStruct {
        a: u8,
        b: String,
        c: f32,
    }

    #[derive(Debug, TSerialize, PartialEq)]
    enum MyVariant {
        First(i32),
        Second,
        Third(MyStruct),
    }

    #[test]
    fn basic_serialize() {
        let a = 10i8;
        let b = 26i16;
        let c = -45612i32;
        let d = 25623422234i64;

        let mut bytes = Vec::<u8>::new();

        a.serialize(&mut bytes);
        b.serialize(&mut bytes);
        c.serialize(&mut bytes);
        d.serialize(&mut bytes);

        assert_eq!(bytes.len(), 1 + 2 + 4 + 8);

        let mut cur = Cursor::new(bytes);

        assert_eq!(a, i8::deserialize(&mut cur));
        assert_eq!(b, i16::deserialize(&mut cur));
        assert_eq!(c, i32::deserialize(&mut cur));
        assert_eq!(d, i64::deserialize(&mut cur));
    }

    #[test]
    fn serialize_struct() {
        let s = MyStruct {
            a: 10,
            b: "This is a test".into(),
            c: f32::consts::PI,
        };

        let mut bytes = Vec::<u8>::new();

        s.serialize(&mut bytes);

        let mut cur = Cursor::new(bytes);

        assert_eq!(s, MyStruct::deserialize(&mut cur));
    }

    #[test]
    fn serialize_enum() {
        let s = MyStruct {
            a: 10,
            b: "This is a test".into(),
            c: f32::consts::PI,
        };

        let this_enum_a = MyVariant::First(25);

        let this_enum_b = MyVariant::Second;

        let this_enum_c = MyVariant::Third(s.clone());

        let mut bytes = Vec::<u8>::new();

        this_enum_a.serialize(&mut bytes);
        this_enum_b.serialize(&mut bytes);
        this_enum_c.serialize(&mut bytes);

        let mut cur = Cursor::new(bytes);

        assert_eq!(this_enum_a, MyVariant::deserialize(&mut cur));
        assert_eq!(this_enum_b, MyVariant::deserialize(&mut cur));
        assert_eq!(this_enum_c, MyVariant::deserialize(&mut cur));
    }
}
