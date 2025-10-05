//! Fast serialization adapters for glam/Bevy math types.
//!
//! These types are plain-old-data in memory (for Bevy/glam’s current layouts),
//! so the serializers use raw byte copies for speed. Tests include layout guard
//! checks to flag upstream changes early.
use bevy::math::{Affine2, Mat2, Mat3, Vec2};
use bevy::math::{Affine3A, Mat3A, Quat, Vec3, Vec3A};

use crate::serialize::impl_fast_raw_item;

// =============================================================================

impl_fast_raw_item!(Vec2);

// =============================================================================

impl_fast_raw_item!(Vec3);
impl_fast_raw_item!(Vec3A);

// =============================================================================

impl_fast_raw_item!(Quat);

// =============================================================================

impl_fast_raw_item!(Mat2);
impl_fast_raw_item!(Mat3);
impl_fast_raw_item!(Mat3A);

// =============================================================================

impl_fast_raw_item!(Affine2);
impl_fast_raw_item!(Affine3A);

// MARK: Tests

#[cfg(test)]
mod tests {

    use crate::serialize::fast_io::{ByteReader, ByteWriter};
    use crate::serialize::fast_ser::{FastRead, FastWrite};
    use bevy::math::{Affine2, Affine3A, Mat2, Mat3, Mat3A, Quat, Vec2, Vec3, Vec3A};

    fn roundtrip<T: FastWrite + FastRead<Ret = T> + core::fmt::Debug + PartialEq>(x: &T) -> T {
        // generous buffer; these are small fixed-size types
        let mut buf = [0u8; 256];
        let mut w = ByteWriter::new(&mut buf);
        unsafe {
            x.write_fast(&mut w);
        }
        let written = w.position();

        let mut r = ByteReader::new(&buf[..written]);
        unsafe { T::read_fast(&mut r) }
    }

    #[test]
    fn vec2_roundtrip() {
        let v = Vec2::new(123.25, -0.125);
        let out = roundtrip(&v);
        assert_eq!(v, out);
    }

    #[test]
    fn vec3_roundtrip() {
        let v = Vec3::new(1.0, 2.5, -9.75);
        let out = roundtrip(&v);
        assert_eq!(v, out);
    }

    #[test]
    fn vec3a_roundtrip_and_alignment() {
        let v = Vec3A::new(3.0, -4.0, 5.0);
        let out = roundtrip(&v);
        assert_eq!(v, out);

        // Catch upstream glam/bevy layout/alignment changes early.
        assert_eq!(core::mem::size_of::<Vec3A>(), 16);
        assert!(core::mem::align_of::<Vec3A>() >= 16);
    }

    #[test]
    fn quat_roundtrip() {
        let q = Quat::from_xyzw(0.1, -0.2, 0.3, 0.9);
        let out = roundtrip(&q);
        assert_eq!(q, out);
    }

    #[test]
    fn mat2_roundtrip() {
        let m = Mat2::from_cols_array(&[1.0, 2.0, 3.0, 4.0]);
        let out = roundtrip(&m);
        assert_eq!(m, out);
    }

    #[test]
    fn mat3_roundtrip() {
        let m = Mat3::from_cols_array(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0]);
        let out = roundtrip(&m);
        assert_eq!(m, out);
    }

    #[test]
    fn mat3a_roundtrip_and_layout_guard() {
        let m = Mat3A::from_cols_array(&[1.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 3.0]);
        let out = roundtrip(&m);
        assert_eq!(m, out);

        // Layout guardrails (update if upstream changes):
        assert_eq!(core::mem::size_of::<Mat3A>(), 48); // 3x Vec3A (16 each)
        assert!(core::mem::align_of::<Mat3A>() >= 16);
    }

    #[test]
    fn affine2_roundtrip() {
        // 2x2 + translation, as columns
        let a = Affine2::from_mat2_translation(
            Mat2::from_cols_array(&[1.0, 2.0, 3.0, 4.0]),
            Vec2::new(10.0, -20.0),
        );
        let out = roundtrip(&a);
        assert_eq!(a, out);
    }

    #[test]
    fn affine3a_roundtrip_and_size_guard() {
        let r = Mat3A::from_scale(Vec2::new(2.0, 3.0));
        let t = Vec3A::new(-5.0, 6.0, -7.0);
        let a = Affine3A::from_mat3_translation(r.into(), t.into());
        let out = roundtrip(&a);
        assert_eq!(a, out);

        // Known today: Mat3A(48) + Vec3A(16) = 64 bytes; aligned >= 16
        assert_eq!(core::mem::size_of::<Affine3A>(), 64);
        assert!(core::mem::align_of::<Affine3A>() >= 16);
    }

    #[test]
    fn determinism_same_bytes_each_time() {
        let v = Vec3A::new(0.25, 0.5, 0.75);

        let mut buf1 = [0u8; 32];
        let mut w1 = ByteWriter::new(&mut buf1);
        unsafe {
            v.write_fast(&mut w1);
        }
        let len1 = w1.position();

        let mut buf2 = [0u8; 32];
        let mut w2 = ByteWriter::new(&mut buf2);
        unsafe {
            v.write_fast(&mut w2);
        }
        let len2 = w2.position();

        assert_eq!(len1, len2);
        assert_eq!(
            &buf1[..len1],
            &buf2[..len2],
            "non-deterministic byte pattern"
        );
    }

    #[test]
    fn fuzzish_values_no_nans() {
        // Light randomized check (no NaNs to keep ==)
        let mut seed: u64 = 0xC01DF00D;
        let mut next = || {
            // LCG-ish
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            // Keep magnitude reasonable and avoid NaN/Inf
            ((seed >> 12) as u32 % 100_000) as f32 * 1e-4 - 5.0
        };

        for _ in 0..256 {
            let v2 = Vec2::new(next(), next());
            assert_eq!(v2, roundtrip(&v2));

            let v3 = Vec3::new(next(), next(), next());
            assert_eq!(v3, roundtrip(&v3));

            let q = Quat::from_xyzw(next(), next(), next(), next());
            assert_eq!(q, roundtrip(&q));
        }
    }
}
