/// Lethal function to turn any thing into an array of bytes.
/// Should ONLY be used on POD data.
/// BE VERY CAREFUL you are not passing in a &&T. Otherwise you write a pointer!
/// Mut counterpart is [`any_as_u8_slice_mut`].
pub unsafe fn any_as_u8_slice<T: Sized>(p: &T) -> &[u8] {
    unsafe { ::core::slice::from_raw_parts((p as *const T) as *const u8, ::core::mem::size_of::<T>()) }
}

/// Lethal function to turn any thing into a mutable slice of bytes.
/// Non-mut counterpart is [`any_as_u8_slice`]
pub unsafe fn any_as_u8_slice_mut<T: Sized>(p: &mut T) -> &mut [u8] {
    unsafe { ::core::slice::from_raw_parts_mut((p as *mut T) as *mut u8, ::core::mem::size_of::<T>()) }
}

/// Dangerous function to serialize the underlying bytes of any item.
/// MUST ONLY BE USED ON POD TYPES.
/// BE VERY CAREFUL you are not passing in a &&T. Otherwise you write a pointer!
/// ONLY USE [`byte_deserialize`] to recover the type!
pub unsafe fn byte_serialize<T: Sized>(v: &T, w: &mut impl std::io::Write) {
    w.write_all(unsafe { any_as_u8_slice(v) }).unwrap()
}

/// Dangerous function to deserialize a type from bytes.
/// MUST ONLY BE USED ON POD TYPES.
/// ONLY USED WITH [`byte_serialize`]
pub unsafe fn byte_deserialize<T: Sized>(r: &mut impl std::io::Read) -> T {
    let mut val = std::mem::MaybeUninit::<T>::uninit();
    unsafe {
        let val_ref: &mut T = &mut *(val.as_mut_ptr());
        r.read_exact(any_as_u8_slice_mut(val_ref)).unwrap();
        val.assume_init()
    }
}

#[cfg(test)]
mod tests {
    use crate::transcript::{TDeserialize, TSerialize};

    use super::*;

    #[derive(Debug, Default, PartialEq)]
    struct PODType {
        a: f32,
        b: i8,
        c: [i16; 3],
    }

    #[test]
    fn check_byte_serialize() {
        use std::io::Cursor;

        let v = PODType {
            a: 4321.0,
            b: 24,
            c: [4, 5, 1134],
        };

        let mut bytes = Vec::<u8>::new();
        unsafe { byte_serialize(&v, &mut bytes) };
        const SCRIBBLE_GUARD: u32 = 0xDEADBEEFu32;
        SCRIBBLE_GUARD.serialize(&mut bytes);

        let mut cursor = Cursor::new(bytes);
        let local_v = unsafe { byte_deserialize(&mut cursor) };
        let check = u32::deserialize(&mut cursor);
        assert_eq!(SCRIBBLE_GUARD, check, "size guard check");
        assert_eq!(v, local_v);
    }
}
