//! Fast, allocation-free serialization traits and helpers.
//!
//! The core of this module is two traits:
//! - `FastWrite`: encode a value to a `ByteSink`.
//! - `FastRead`: decode a value from a `ByteSource`.
//!
//! The design favors performance over generality and uses `unsafe` in hot
//! paths. Implementations must be symmetric: values produced by `write_fast`
//! must be consumable by the corresponding `read_fast`. Many encoders are
//! defined for Bevy types and common Rust primitives.
//!
//! Safety and invariants
//! - Most methods are `unsafe` because misuse can cause UB or logic errors
//!   (e.g., reading with the wrong type, violating POD assumptions).
//! - POD-based encodings use native endianness and raw memcpy of in-memory
//!   layouts; both ends must be compatible.
//! - Length-prefixed collections use `usize` for counts.
use super::fast_io::*;

use half::f16;

pub trait FastWrite {
    /// Encode `self` into `w`.
    ///
    /// Safety
    /// - Must write a valid encoding for the corresponding `FastRead` impl.
    /// - Must advance the writer exactly by the number of bytes written.
    unsafe fn write_fast(&self, w: &mut impl ByteSink);
}

impl<T: FastWrite> FastWrite for &T {
    unsafe fn write_fast(&self, w: &mut impl ByteSink) {
        unsafe { (*self).write_fast(w) };
    }
}

pub trait FastRead: Sized {
    type Ret;
    /// Decode a value from `r`.
    ///
    /// Safety
    /// - Must read a valid encoding produced by the matching `write_fast`.
    /// - Must not read beyond the needed number of bytes.
    unsafe fn read_fast<'a, S: ByteSource<'a>>(r: &mut S) -> Self::Ret;
}

/// Convenience wrapper to read a type implementing `FastRead<Ret = T>`.
pub fn read_fast<'a, T: FastRead<Ret = T>, S: ByteSource<'a>>(r: &mut S) -> T {
    unsafe { T::read_fast(r) }
}

// MARK: Primitives & PODs
macro_rules! impl_fast_prim {
    ($t:ty, $put:ident, $get:ident) => {
        impl FastWrite for $t {
            #[inline(always)]
            unsafe fn write_fast(&self, w: &mut impl ByteSink) {
                w.$put(*self);
            }
        }
        impl FastRead for $t {
            type Ret = $t;
            #[inline(always)]
            unsafe fn read_fast<'a, S: ByteSource<'a>>(r: &mut S) -> Self {
                r.$get()
            }
        }
    };
}
impl_fast_prim!(u8, put_u8, get_u8);
impl_fast_prim!(i8, put_i8, get_i8);
impl_fast_prim!(u16, put_u16, get_u16);
impl_fast_prim!(i16, put_i16, get_i16);
impl_fast_prim!(u32, put_u32, get_u32);
impl_fast_prim!(i32, put_i32, get_i32);
impl_fast_prim!(u64, put_u64, get_u64);
impl_fast_prim!(i64, put_i64, get_i64);
impl_fast_prim!(usize, put_usize, get_usize);
impl_fast_prim!(isize, put_isize, get_isize);
impl_fast_prim!(half::f16, put_f16, get_f16);
impl_fast_prim!(f32, put_f32, get_f32);
impl_fast_prim!(f64, put_f64, get_f64);

impl FastWrite for bool {
    #[inline(always)]
    unsafe fn write_fast(&self, w: &mut impl ByteSink) {
        w.put_bool(*self);
    }
}
impl FastRead for bool {
    type Ret = bool;
    #[inline(always)]
    unsafe fn read_fast<'a, S: ByteSource<'a>>(r: &mut S) -> Self {
        r.get_bool()
    }
}

// MARK: Strings
impl FastWrite for String {
    #[inline(always)]
    unsafe fn write_fast(&self, w: &mut impl ByteSink) {
        w.put_str(self.as_str())
    }
}
impl FastWrite for &str {
    #[inline(always)]
    unsafe fn write_fast(&self, w: &mut impl ByteSink) {
        w.put_str(self)
    }
}
impl FastRead for String {
    type Ret = String;
    #[inline(always)]
    unsafe fn read_fast<'a, S: ByteSource<'a>>(r: &mut S) -> Self {
        r.get_string()
    }
}

// MARK: Slice + Vectors

// we have to split this up because rusts specialization is still super immature, and hacks dont work

// macro_rules! slow_vec {
//     ($T:ty) => {
//         impl FastWrite for [$T] {
//             #[inline(always)]
//             unsafe fn write_fast(&self, w: &mut impl ByteSink) {
//                 w.put_usize(self.len());
//                 for v in **self {
//                     unsafe { v.write_fast(w) };
//                 }
//             }
//         }

//         impl FastRead for Vec<$T> {
//             type Ret = Vec<$T>;
//             #[inline(always)]
//             unsafe fn read_fast<'a, S: ByteSource<'a>>(r: &mut S) -> Self::Ret {
//                 let n = unsafe { usize::read_fast(r) };
//                 let mut v = Vec::with_capacity(n);
//                 for _ in 0..n {
//                     v.push(unsafe { T::read_fast(r) });
//                 }
//                 v
//             }
//         }
//     };
// }

macro_rules! fast_vec {
    ($T:ty) => {
        impl FastWrite for [$T] {
            #[inline(always)]
            unsafe fn write_fast(&self, w: &mut impl ByteSink) {
                // SAFETY: if pod_collectible says yes, we can cast.
                w.put_pod_slice(self);
            }
        }

        impl FastWrite for Vec<$T> {
            #[inline(always)]
            unsafe fn write_fast(&self, w: &mut impl ByteSink) {
                // SAFETY: if pod_collectible says yes, we can cast.
                w.put_pod_slice(self);
            }
        }

        impl FastRead for Vec<$T> {
            type Ret = Vec<$T>;
            #[inline(always)]
            unsafe fn read_fast<'a, S: ByteSource<'a>>(r: &mut S) -> Self::Ret {
                r.get_pod_vec::<$T>()
            }
        }
    };
}

fast_vec!(u8);
fast_vec!([u8; 2]);
fast_vec!([u8; 3]);
fast_vec!([u8; 4]);

fast_vec!(u16);
fast_vec!([u16; 2]);
fast_vec!([u16; 3]);
fast_vec!([u16; 4]);

fast_vec!(u32);
fast_vec!([u32; 2]);
fast_vec!([u32; 3]);
fast_vec!([u32; 4]);

fast_vec!(i8);
fast_vec!([i8; 2]);
fast_vec!([i8; 3]);
fast_vec!([i8; 4]);

fast_vec!(i16);
fast_vec!([i16; 2]);
fast_vec!([i16; 3]);
fast_vec!([i16; 4]);

fast_vec!(i32);
fast_vec!([i32; 2]);
fast_vec!([i32; 3]);
fast_vec!([i32; 4]);

fast_vec!(f16);
fast_vec!([f16; 2]);
fast_vec!([f16; 3]);
fast_vec!([f16; 4]);

fast_vec!(f32);
fast_vec!([f32; 2]);
fast_vec!([f32; 3]);
fast_vec!([f32; 4]);

fast_vec!(f64);
fast_vec!([f64; 2]);
fast_vec!([f64; 3]);
fast_vec!([f64; 4]);

// MARK: Arrays
impl<T: FastWrite, const N: usize> FastWrite for [T; N] {
    #[inline(always)]
    unsafe fn write_fast(&self, w: &mut impl ByteSink) {
        for x in self {
            unsafe { x.write_fast(w) };
        }
    }
}
impl<T, const N: usize> FastRead for [T; N]
where
    T: FastRead<Ret = T> + Default,
{
    type Ret = [T; N];
    #[inline(always)]
    unsafe fn read_fast<'a, S: ByteSource<'a>>(r: &mut S) -> Self::Ret {
        core::array::from_fn(|_| unsafe { T::read_fast(r) })
    }
}

// MARK: Box<T>
impl<T: FastWrite> FastWrite for Box<T> {
    #[inline(always)]
    unsafe fn write_fast(&self, w: &mut impl ByteSink) {
        let t: &T = &self;
        unsafe { t.write_fast(w) };
    }
}
impl<T: FastRead<Ret = T>> FastRead for Box<T> {
    type Ret = Self;
    #[inline(always)]
    unsafe fn read_fast<'a, S: ByteSource<'a>>(r: &mut S) -> Self::Ret {
        Self::new(unsafe { T::read_fast(r) })
    }
}

// MARK: Option<T>
impl<T: FastWrite> FastWrite for Option<T> {
    #[inline(always)]
    unsafe fn write_fast(&self, w: &mut impl ByteSink) {
        match self {
            Some(v) => {
                w.put_u8(1);
                unsafe { v.write_fast(w) };
            }
            None => {
                w.put_u8(0);
            }
        }
    }
}
impl<T: FastRead<Ret = T>> FastRead for Option<T> {
    type Ret = Option<T>;
    #[inline(always)]
    unsafe fn read_fast<'a, S: ByteSource<'a>>(r: &mut S) -> Self::Ret {
        if r.get_u8() != 0 {
            Some(unsafe { T::read_fast(r) })
        } else {
            None
        }
    }
}

// MARK: Result<U,V>

impl<U: FastWrite, V: FastWrite> FastWrite for Result<U, V> {
    #[inline(always)]
    unsafe fn write_fast(&self, w: &mut impl ByteSink) {
        match self {
            Ok(v) => {
                w.put_u8(1);
                unsafe { v.write_fast(w) };
            }
            Err(x) => {
                w.put_u8(0);
                unsafe { x.write_fast(w) };
            }
        }
    }
}
impl<U, V> FastRead for Result<U, V>
where
    U: FastRead<Ret = U>,
    V: FastRead<Ret = V>,
{
    type Ret = Result<U, V>;
    #[inline(always)]
    unsafe fn read_fast<'a, S: ByteSource<'a>>(r: &mut S) -> Self {
        if r.get_u8() != 0 {
            Ok(unsafe { U::read_fast(r) })
        } else {
            Err(unsafe { V::read_fast(r) })
        }
    }
}

// MARK: Small tuples
macro_rules! impl_tuple {
    ($($name:ident),+) => {
        impl<$($name: FastWrite),+> FastWrite for ($($name,)+) {
            #[inline(always)]
            unsafe fn write_fast(&self, w: &mut impl ByteSink) {
                #[allow(non_snake_case)]
                let ($($name,)+) = self;
                $( unsafe {$name.write_fast(w) }; )+
            }
        }
        impl<$($name: FastRead<Ret = $name>),+> FastRead for ($($name,)+) {
            type Ret = ($($name),+);
            #[inline(always)]
            unsafe fn read_fast<'a, S: ByteSource<'a>>(r: &mut S)  -> Self {
                ( $( unsafe {$name::read_fast(r)}, )+ )
            }
        }
    };
}
impl_tuple!(A, B);
impl_tuple!(A, B, C);
impl_tuple!(A, B, C, D);

// MARK: Danger!

/// Lethal function to turn any thing into an array of bytes.
/// Should ONLY be used on POD data.
/// BE VERY CAREFUL you are not passing in a &&T. Otherwise you write a pointer!
/// Mut counterpart is [`any_as_u8_slice_mut`].
unsafe fn any_as_u8_slice<T: Sized>(p: &T) -> &[u8] {
    unsafe {
        ::core::slice::from_raw_parts((p as *const T) as *const u8, ::core::mem::size_of::<T>())
    }
}

/// Lethal function to turn any thing into a mutable slice of bytes.
/// Non-mut counterpart is [`any_as_u8_slice`]
unsafe fn any_as_u8_slice_mut<T: Sized>(p: &mut T) -> &mut [u8] {
    unsafe {
        ::core::slice::from_raw_parts_mut((p as *mut T) as *mut u8, ::core::mem::size_of::<T>())
    }
}

/// Dangerous function to serialize the underlying bytes of any item.
///
/// MUST ONLY BE USED ON POD TYPES with identical memory layout at read time.
/// Passing a reference-to-reference like `&&T` will serialize a pointer!
/// Only use [`byte_deserialize`] to recover the value.
pub unsafe fn byte_serialize<T: Sized>(v: &T, w: &mut impl ByteSink) {
    w.put_bytes(unsafe { any_as_u8_slice(v) })
}

/// Dangerous function to deserialize a type from bytes.
///
/// MUST ONLY BE USED ON POD TYPES and only for bytes previously written by
/// [`byte_serialize`]. The resulting value is created without running any
/// constructors or validation.
pub unsafe fn byte_deserialize<'a, T: Sized>(r: &mut impl ByteSource<'a>) -> T {
    let mut val = std::mem::MaybeUninit::<T>::uninit();
    unsafe {
        let val_ref: &mut T = &mut *(val.as_mut_ptr());
        r.take_bytes_to(any_as_u8_slice_mut(val_ref));
        val.assume_init()
    }
}

// MARK: Macros

mod macros {
    //! Macro helpers to implement `FastWrite`/`FastRead` for structs, enums,
    //! newtypes, and POD wrappers.
    macro_rules! impl_fast_serialize {
        (
            $T:ty,
            $(lifetime: $lifetime:tt,)?
            keep: {$($fld:ident),* },
            skip: {$($sfld:ident),*}
        ) => {
            impl $(<$lifetime>)? crate::serialize::fast_ser::FastWrite for $T {
                #[inline(always)]
                #[allow(unused)]
                unsafe fn write_fast(&self, w: &mut impl crate::serialize::fast_io::ByteSink) {
                    $( unsafe {self. $fld.write_fast(w) }; )*
                }
            }

            impl $(<$lifetime>)? crate::serialize::fast_ser::FastRead for $T {
                type Ret = $T ;
                #[inline(always)]
                #[allow(unused)]
                unsafe fn read_fast<'z, S: crate::serialize::fast_io::ByteSource<'z>>(r: &mut S)  -> Self {
                    #[allow(unused)]
                    use crate::serialize::fast_ser::read_fast;
                    Self {
                        //$( $fld : unsafe {<$fld_type>::read_fast(r) }, )*
                        $( $fld : read_fast(r), )*
                        $( $sfld : Default::default(), )*
                    }
                }
            }
        };
    }

    pub(crate) use impl_fast_serialize;

    macro_rules! impl_fast_serialize_write_only {
        (
            $T:ty,
            $(lifetime: $lifetime:tt,)?
            keep: {$($fld:ident),* },
            skip: {$($sfld:ident),*}
        ) => {
            impl $(<$lifetime>)? crate::serialize::fast_ser::FastWrite for $T {
                #[inline(always)]
                #[allow(unused)]
                unsafe fn write_fast(&self, w: &mut impl crate::serialize::fast_io::ByteSink) {
                    $( unsafe {self. $fld.write_fast(w) }; )*
                }
            }
        };
    }

    pub(crate) use impl_fast_serialize_write_only;

    /// Implement serialization for a foreign type that is a POD but might not be marked as such.
    /// This is incredibly dangerous. But for things like payload-less enums, we can just byte cast.
    macro_rules! impl_fast_raw_item {
        ($T:ty) => {
            impl crate::serialize::fast_ser::FastWrite for $T {
                #[inline(always)]
                unsafe fn write_fast(&self, w: &mut impl crate::serialize::fast_io::ByteSink) {
                    unsafe { crate::serialize::fast_ser::byte_serialize(self, w) }
                }
            }

            impl crate::serialize::fast_ser::FastRead for $T {
                type Ret = $T;
                #[inline(always)]
                unsafe fn read_fast<'b, S: crate::serialize::fast_io::ByteSource<'b>>(
                    r: &mut S,
                ) -> Self {
                    unsafe { crate::serialize::fast_ser::byte_deserialize(r) }
                }
            }
        };
    }

    pub(crate) use impl_fast_raw_item;

    macro_rules! impl_fast_newtype {
        ($T:ty) => {
            impl crate::serialize::fast_ser::FastWrite for $T {
                #[inline(always)]
                unsafe fn write_fast(&self, w: &mut impl crate::serialize::fast_io::ByteSink) {
                    unsafe { self.0.write_fast(w) }
                }
            }

            impl crate::serialize::fast_ser::FastRead for $T {
                type Ret = $T;
                #[inline(always)]
                unsafe fn read_fast<'b, S: crate::serialize::fast_io::ByteSource<'b>>(
                    r: &mut S,
                ) -> Self {
                    Self(read_fast(r))
                }
            }
        };
    }

    pub(crate) use impl_fast_newtype;

    macro_rules! create_serialize_enum {
        (
            $name:ident,
            $repr:ty,
            $(lifetime: $lifetime:tt,)?
            {
                $( ( $tag:tt, $var_name:tt, $var_type:ty ), )+
            }
        ) => {

            #[derive(Debug)]
            pub(crate) enum $name $(<$lifetime>)? {
                $(
                    $var_name ( $var_type ),
                )+
            }

            impl$(<$lifetime>)? FastWrite for $name $(<$lifetime>)? {
                #[inline(always)]
                unsafe fn write_fast(&self, w: &mut impl ByteSink) {
                    match self {
                        $(
                            Self:: $var_name (x) => unsafe {
                                let d : $repr = $tag;
                                d.write_fast(w);
                                x.write_fast(w);
                            }
                        )+
                    }
                }
            }

            impl$(<$lifetime>)? FastRead for $name $(<$lifetime>)? {
                type Ret = Self;
                #[inline(always)]
                unsafe fn read_fast<'z, S: ByteSource<'z>>(r: &mut S)  -> Self {
                    let d : $repr = read_fast(r);

                    match d {
                        $(
                            $tag => Self:: $var_name(read_fast(r)),
                        )+
                        _ => unreachable!(),
                    }
                }
            }

        };
    }

    pub(crate) use create_serialize_enum;

    macro_rules! create_serialize_enum_write_only {
        (
            $name:ident,
            $repr:ty,
            $(lifetime: $lifetime:tt,)?
            {
                $( ( $tag:tt, $var_name:tt, $var_type:ty ), )+
            }
        ) => {

            #[derive(Debug)]
            pub(crate) enum $name $(<$lifetime>)? {
                $(
                    $var_name ( $var_type ),
                )+
            }

            impl$(<$lifetime>)? FastWrite for $name $(<$lifetime>)? {
                #[inline(always)]
                unsafe fn write_fast(&self, w: &mut impl ByteSink) {
                    match self {
                        $(
                            Self:: $var_name (x) => unsafe {
                                let d : $repr = $tag;
                                d.write_fast(w);
                                x.write_fast(w);
                            }
                        )+
                    }
                }
            }

        };
    }

    pub(crate) use create_serialize_enum_write_only;

    macro_rules! create_serialize_enum_simple {
        (
            $name:ident,
            $repr:ty,
            {
                $( ( $tag:tt, $var_name:tt), )+
            }
        ) => {

            #[derive(Debug)]
            pub(crate) enum $name {
                $(
                    $var_name,
                )+
            }

            impl FastWrite for $name {
                #[inline(always)]
                unsafe fn write_fast(&self, w: &mut impl ByteSink) {
                    match self {
                        $(
                            Self:: $var_name => unsafe {
                                let d : $repr = $tag;
                                d.write_fast(w);
                            }
                        )+
                    }
                }
            }

            impl FastRead for $name {
                type Ret = Self;
                #[inline(always)]
                unsafe fn read_fast<'a, S: ByteSource<'a>>(r: &mut S)  -> Self {
                    let d = unsafe { <$repr>::read_fast(r) };

                    match d {
                        $(
                            $tag => Self:: $var_name,
                        )+
                        _ => unreachable!(),
                    }
                }
            }

        };
    }

    pub(crate) use create_serialize_enum_simple;
}

pub(crate) use macros::create_serialize_enum;
pub(crate) use macros::create_serialize_enum_simple;
pub(crate) use macros::create_serialize_enum_write_only;
pub(crate) use macros::impl_fast_newtype;
pub(crate) use macros::impl_fast_raw_item;
pub(crate) use macros::impl_fast_serialize;
pub(crate) use macros::impl_fast_serialize_write_only;

#[cfg(test)]
pub(crate) fn test_serialization<A, F>(a: A, f: F)
where
    A: FastRead<Ret = A> + FastWrite,
    F: FnOnce(A, A) -> bool,
{
    let mut buffer = vec![0u8; 1024];
    let mut writer = ByteWriter::new(&mut buffer);

    unsafe {
        a.write_fast(&mut writer);
    }

    let mut reader = ByteReader::new(&buffer);

    let b = unsafe { A::read_fast(&mut reader) };

    assert!(f(a, b));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_buffer() -> Vec<u8> {
        vec![0u8; 1024]
    }

    #[test]
    fn test_fast_write_read_primitives() {
        let mut buffer = make_buffer();
        let mut writer = ByteWriter::new(&mut buffer);

        // Write primitives using FastWrite
        unsafe {
            42_u32.write_fast(&mut writer);
            (-42_i32).write_fast(&mut writer);
            true.write_fast(&mut writer);
            "Hello, world!".to_string().write_fast(&mut writer);
        }

        let mut reader = ByteReader::new(&buffer);

        // Read primitives using FastRead
        unsafe {
            assert_eq!(u32::read_fast(&mut reader), 42_u32);
            assert_eq!(i32::read_fast(&mut reader), -42_i32);
            assert_eq!(bool::read_fast(&mut reader), true);
            assert_eq!(String::read_fast(&mut reader), "Hello, world!".to_string());
        }
    }

    #[test]
    fn test_fast_write_read_vectors() {
        let mut buffer = make_buffer();
        let mut writer = ByteWriter::new(&mut buffer);

        // Write a vector of integers
        unsafe {
            vec![42, -42].as_slice().write_fast(&mut writer);
        }

        let mut reader = ByteReader::new(&buffer);

        // Read the vector back
        unsafe {
            assert_eq!(Vec::<i32>::read_fast(&mut reader), vec![42, -42]);
        }
    }

    #[test]
    fn test_fast_write_read_option() {
        let mut buffer = make_buffer();
        let mut writer = ByteWriter::new(&mut buffer);

        // Write an Option
        unsafe {
            Some(42).write_fast(&mut writer);
            None::<i32>.write_fast(&mut writer);
        }

        let mut reader = ByteReader::new(&buffer);

        // Read the Option back
        unsafe {
            assert_eq!(Option::<i32>::read_fast(&mut reader), Some(42));
            assert_eq!(Option::<i32>::read_fast(&mut reader), None);
        }
    }

    #[test]
    fn test_fast_write_read_result() {
        let mut buffer = make_buffer();
        let mut writer = ByteWriter::new(&mut buffer);

        // Write a Result
        unsafe {
            Result::<i32, i32>::Ok(42).write_fast(&mut writer);
            Result::<i32, i32>::Err(-42).write_fast(&mut writer);
        }

        let mut reader = ByteReader::new(&buffer);

        // Read the Result back
        unsafe {
            assert_eq!(Result::<i32, i32>::read_fast(&mut reader), Ok(42));
            assert_eq!(Result::<i32, i32>::read_fast(&mut reader), Err(-42));
        }
    }

    #[test]
    fn test_fast_write_read_tuples() {
        let mut buffer = make_buffer();
        let mut writer = ByteWriter::new(&mut buffer);

        type ThisTuple = (i32, i32);

        // Write a tuple
        unsafe {
            (42, -42).write_fast(&mut writer);
        }

        let mut reader = ByteReader::new(&buffer);

        // Read the tuple back
        unsafe {
            assert_eq!(ThisTuple::read_fast(&mut reader), (42, -42));
        }
    }

    #[test]
    fn test_structure() {
        let mut buffer = make_buffer();
        let mut writer = ByteWriter::new(&mut buffer);

        #[derive(Default)]
        struct TestStruct {
            thing_a: i32,
            thing_b: String,
            thing_c: String,
        }

        impl_fast_serialize!(TestStruct, keep: {
            thing_a, thing_c
        }, skip: {
            thing_b
        });

        {
            let s = TestStruct {
                thing_a: 421,
                thing_b: "Hello, world!".into(),
                thing_c: "Hello, user!".into(),
            };

            unsafe { s.write_fast(&mut writer) };
        }

        let mut reader = ByteReader::new(&buffer);

        // Read the tuple back
        unsafe {
            let res = TestStruct::read_fast(&mut reader);
            assert_eq!(res.thing_a, 421);
            assert_eq!(res.thing_b, String::default());
            assert_eq!(res.thing_c, "Hello, user!");
        }
    }

    #[test]
    fn check_byte_serialize() {
        let mut buffer = make_buffer();
        let mut writer = ByteWriter::new(&mut buffer);

        #[derive(Debug, Default, PartialEq)]
        struct PODType {
            a: f32,
            b: i8,
            c: [i16; 3],
        }

        let v = PODType {
            a: 4321.0,
            b: 24,
            c: [4, 5, 1134],
        };

        unsafe { byte_serialize(&v, &mut writer) };
        const SCRIBBLE_GUARD: u32 = 0xDEADBEEFu32;
        unsafe { SCRIBBLE_GUARD.write_fast(&mut writer) };

        let mut reader = ByteReader::new(&buffer);

        let local_v = unsafe { byte_deserialize(&mut reader) };
        let check = unsafe { u32::read_fast(&mut reader) };
        assert_eq!(SCRIBBLE_GUARD, check, "size guard check");
        assert_eq!(v, local_v);
    }
}
