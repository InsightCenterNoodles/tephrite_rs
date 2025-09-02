use bytemuck::{bytes_of, Pod};
use std::io::Write;

/// Trait to serialize an item into bytes
pub trait TSerialize {
    fn serialize(&self, w: &mut impl Write);
}

/// Serialize an item into bytes
pub fn serialize<T: TSerialize>(lt: &T, w: &mut impl Write) {
    lt.serialize(w);
}

macro_rules! impl_ser {
    ($T:ty) => {
        impl TSerialize for $T {
            fn serialize(&self, w: &mut impl Write) {
                w.write_all(bytes_of(self)).expect("deserialize primitive")
            }
        }
    };
}

impl<T: TSerialize> TSerialize for &T {
    fn serialize(&self, w: &mut impl Write) {
        T::serialize(self, w);
    }
}

impl_ser!(bool);

impl_ser!(u8);
impl_ser!(u16);
impl_ser!(u32);
impl_ser!(u64);
impl_ser!(usize);

impl_ser!(i8);
impl_ser!(i16);
impl_ser!(i32);
impl_ser!(i64);
impl_ser!(isize);

impl_ser!(f32);
impl_ser!(f64);

impl TSerialize for String {
    fn serialize(&self, w: &mut impl Write) {
        self.len().serialize(w);
        w.write_all(self.as_bytes()).expect("deserialize primitive")
    }
}

// slices doesnt really make sense...since on the deserialization side, we cant resize them
impl<T: TSerialize> TSerialize for Vec<T> {
    fn serialize(&self, w: &mut impl Write) {
        self.len().serialize(w);
        for i in self {
            i.serialize(w);
        }
    }
}

/// Serialize a slice of POD data. ONLY FOR POD TYPES!!!
pub fn serialize_pod_slice<T: TSerialize + Pod>(slice: &[T], w: &mut impl Write) {
    slice.len().serialize(w);

    let bytes: &[u8] = bytemuck::cast_slice(slice);

    w.write_all(bytes).unwrap();
}

macro_rules! impl_ser_tuple {
    ( $( $idx:tt $type:ident ),* ) => {
        impl< $($type : TSerialize),* > TSerialize for ($($type,)*) {
            fn serialize(&self, w: &mut impl Write) {
                $(
                    self.$idx.serialize(w);
                )*
            }
        }
    };
}

impl_ser_tuple!(0 T0, 1 T1);
impl_ser_tuple!(0 T0, 1 T1, 2 T2);
impl_ser_tuple!(0 T0, 1 T1, 2 T2, 3 T3);

impl<T: TSerialize> TSerialize for Option<T> {
    fn serialize(&self, w: &mut impl Write) {
        if let Some(value) = &self {
            true.serialize(w);
            value.serialize(w);
        } else {
            false.serialize(w);
        }
    }
}

impl<T: TSerialize, const N: usize> TSerialize for [T; N] {
    fn serialize(&self, w: &mut impl Write) {
        for i in self {
            i.serialize(w);
        }
    }
}
