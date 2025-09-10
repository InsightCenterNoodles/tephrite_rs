use std::io::Read;
use std::mem::MaybeUninit;

use bytemuck::Pod;

/// Trait to recover items from byte streams. See [`TSerialize`]
pub trait TDeserialize {
    fn deserialize(r: &mut impl Read) -> Self;
}

/// Deserialize an item from a byte stream
pub fn deserialize<T: TDeserialize>(r: &mut impl Read) -> T {
    T::deserialize(r)
}

macro_rules! impl_deser {
    ($T:ty) => {
        impl TDeserialize for $T {
            fn deserialize(r: &mut impl Read) -> Self {
                let mut x = MaybeUninit::<$T>::uninit();
                r.read_exact(bytemuck::bytes_of_mut(unsafe { &mut *x.as_mut_ptr() }))
                    .unwrap();
                unsafe { x.assume_init() }
            }
        }
    };
}

impl TDeserialize for bool {
    fn deserialize(r: &mut impl Read) -> Self {
        u8::deserialize(r) != 0
    }
}

impl_deser!(u8);
impl_deser!(u16);
impl_deser!(u32);
impl_deser!(u64);
impl_deser!(usize);

impl_deser!(i8);
impl_deser!(i16);
impl_deser!(i32);
impl_deser!(i64);
impl_deser!(isize);

impl_deser!(f32);
impl_deser!(f64);

impl TDeserialize for String {
    fn deserialize(r: &mut impl Read) -> Self {
        let count = usize::deserialize(r);
        let mut buffer = vec![0u8; count];

        r.read_exact(&mut buffer).unwrap();

        unsafe { String::from_utf8_unchecked(buffer) }
    }
}

impl<T: TDeserialize> TDeserialize for Vec<T> {
    fn deserialize(r: &mut impl Read) -> Self {
        let count: usize = deserialize(r);
        (0..count).map(|_| deserialize(r)).collect()
    }
}

pub fn deserialize_pod_slice<T: TDeserialize + Pod>(r: &mut impl Read) -> Vec<T> {
    let count: usize = deserialize(r);
    let mut ret = bytemuck::zeroed_vec(count);

    let bytes = bytemuck::cast_slice_mut(&mut ret);

    r.read_exact(bytes).unwrap();

    ret
}

macro_rules! impl_ser_tuple {
    ( $( $type:ident ),* ) => {
        impl< $($type : TDeserialize),* > TDeserialize for ($($type,)*) {
            fn deserialize(r: &mut impl Read) -> Self {
                ( $(
                    $type::deserialize(r),
                )* )
            }
        }
    };
}

impl_ser_tuple!(T0);
impl_ser_tuple!(T0, T1);
impl_ser_tuple!(T0, T1, T2);

impl<T: TDeserialize> TDeserialize for Option<T> {
    fn deserialize(r: &mut impl Read) -> Self {
        let exists = bool::deserialize(r);

        if exists {
            Some(T::deserialize(r))
        } else {
            None
        }
    }
}

impl<T: TDeserialize + Default, const N: usize> TDeserialize for [T; N] {
    fn deserialize(r: &mut impl Read) -> Self {
        core::array::from_fn(|_| T::deserialize(r))
    }
}
