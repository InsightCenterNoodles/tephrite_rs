use std::sync::{LazyLock, RwLock};

use bevy::{platform::collections::HashMap, prelude::*};

use crate::serialize::*;

static GIZMO_ASSET_MAP: LazyLock<RwLock<HashMap<AssetId<GizmoAsset>, Handle<GizmoAsset>>>> =
    LazyLock::new(|| Default::default());

impl RemappableAsset for GizmoAsset {
    #[inline]
    fn with_remapper<F: FnOnce(&HashMap<AssetId<Self>, Handle<Self>>)>(func: F) {
        func(&GIZMO_ASSET_MAP.read().unwrap());
    }

    #[inline]
    fn with_remapper_mut<F: FnOnce(&mut HashMap<AssetId<Self>, Handle<Self>>)>(func: F) {
        func(&mut GIZMO_ASSET_MAP.write().unwrap());
    }
}

impl FastWrite for GizmoLineJoint {
    unsafe fn write_fast(&self, w: &mut impl ByteSink) {
        unsafe {
            match self {
                GizmoLineJoint::None => 0u8.write_fast(w),
                GizmoLineJoint::Miter => 1u8.write_fast(w),
                GizmoLineJoint::Round(resolution) => {
                    2u8.write_fast(w);
                    resolution.write_fast(w);
                }
                GizmoLineJoint::Bevel => 3u8.write_fast(w),
            }
        }
    }
}

impl FastRead for GizmoLineJoint {
    type Ret = Self;
    type Context = ();

    unsafe fn read_fast<'a, S: ByteSource<'a>>(_: &mut Self::Context, r: &mut S) -> Self::Ret {
        match unsafe { u8::easy_read_fast(r) } {
            0 => GizmoLineJoint::None,
            1 => GizmoLineJoint::Miter,
            2 => GizmoLineJoint::Round(unsafe { u32::easy_read_fast(r) }),
            3 => GizmoLineJoint::Bevel,
            _ => GizmoLineJoint::None,
        }
    }
}

impl FastWrite for GizmoLineStyle {
    unsafe fn write_fast(&self, w: &mut impl ByteSink) {
        unsafe {
            match self {
                GizmoLineStyle::Solid => 0u8.write_fast(w),
                GizmoLineStyle::Dotted => 1u8.write_fast(w),
                GizmoLineStyle::Dashed {
                    gap_scale,
                    line_scale,
                } => {
                    2u8.write_fast(w);
                    gap_scale.write_fast(w);
                    line_scale.write_fast(w);
                }
                _ => 0u8.write_fast(w),
            }
        }
    }
}

impl FastRead for GizmoLineStyle {
    type Ret = Self;
    type Context = ();

    unsafe fn read_fast<'a, S: ByteSource<'a>>(_: &mut Self::Context, r: &mut S) -> Self::Ret {
        match unsafe { u8::easy_read_fast(r) } {
            0 => GizmoLineStyle::Solid,
            1 => GizmoLineStyle::Dotted,
            2 => GizmoLineStyle::Dashed {
                gap_scale: unsafe { f32::easy_read_fast(r) },
                line_scale: unsafe { f32::easy_read_fast(r) },
            },
            _ => GizmoLineStyle::Solid,
        }
    }
}

impl_fast_serialize!(
    GizmoLineConfig,
    (),
    keep: {
        width,
        perspective,
        style,
        joints
    }, skip: {
    }
);

impl FastWrite for GizmoAsset {
    unsafe fn write_fast(&self, w: &mut impl ByteSink) {
        let buffer = self.buffer();
        unsafe {
            write_vec(w, &buffer.list_positions);
            write_vec(w, &buffer.list_colors);
            write_vec(w, &buffer.strip_positions);
            write_vec(w, &buffer.strip_colors);
        }
    }
}

impl FastRead for GizmoAsset {
    type Ret = Self;
    type Context = ();

    unsafe fn read_fast<'a, S: ByteSource<'a>>(_: &mut Self::Context, r: &mut S) -> Self::Ret {
        let mut asset = GizmoAsset::new();
        asset.list_positions = unsafe { read_vec(r) };
        asset.list_colors = unsafe { read_vec(r) };
        asset.strip_positions = unsafe { read_vec(r) };
        asset.strip_colors = unsafe { read_vec(r) };
        asset
    }
}

impl FastWrite for Gizmo {
    unsafe fn write_fast(&self, w: &mut impl ByteSink) {
        unsafe {
            self.handle.write_fast(w);
            self.line_config.write_fast(w);
            self.depth_bias.write_fast(w);
        }
    }
}

impl FastRead for Gizmo {
    type Ret = Self;
    type Context = Assets<GizmoAsset>;

    unsafe fn read_fast<'a, S: ByteSource<'a>>(assets: &mut Self::Context, r: &mut S) -> Self::Ret {
        Gizmo {
            handle: read_fast(assets, r),
            line_config: read_fast(&mut (), r),
            depth_bias: unsafe { f32::easy_read_fast(r) },
        }
    }
}

unsafe fn write_vec<T: FastWrite>(w: &mut impl ByteSink, values: &[T]) {
    unsafe {
        values.len().write_fast(w);
        for value in values {
            value.write_fast(w);
        }
    }
}

unsafe fn read_vec<'a, T, S>(r: &mut S) -> Vec<T>
where
    T: FastRead<Ret = T, Context = ()>,
    S: ByteSource<'a>,
{
    let len = unsafe { usize::easy_read_fast(r) };
    let mut values = Vec::with_capacity(len);
    for _ in 0..len {
        values.push(unsafe { T::easy_read_fast(r) });
    }
    values
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gizmo_line_config_roundtrip() {
        let config = GizmoLineConfig {
            width: 7.0,
            perspective: true,
            style: GizmoLineStyle::Dashed {
                gap_scale: 2.0,
                line_scale: 3.0,
            },
            joints: GizmoLineJoint::Round(6),
        };

        test_serialization((), config, |a, b| {
            a.width == b.width
                && a.perspective == b.perspective
                && a.style == b.style
                && a.joints == b.joints
        });
    }

    #[test]
    fn gizmo_asset_roundtrip() {
        let mut asset = GizmoAsset::new();
        asset.line(Vec3::ZERO, Vec3::X, Color::WHITE);
        asset.linestrip([Vec3::ZERO, Vec3::Y, Vec3::Z], Color::srgb(1.0, 0.0, 0.0));

        test_serialization((), asset, |a, b| {
            let a = a.buffer();
            let b = b.buffer();
            vec3_slices_eq(&a.list_positions, &b.list_positions)
                && color_slices_eq(&a.list_colors, &b.list_colors)
                && vec3_slices_eq(&a.strip_positions, &b.strip_positions)
                && color_slices_eq(&a.strip_colors, &b.strip_colors)
        });
    }

    fn vec3_slices_eq(a: &[Vec3], b: &[Vec3]) -> bool {
        a.len() == b.len() && a.iter().zip(b).all(|(a, b)| vec3_eq(*a, *b))
    }

    fn color_slices_eq(a: &[LinearRgba], b: &[LinearRgba]) -> bool {
        a.len() == b.len() && a.iter().zip(b).all(|(a, b)| color_eq(*a, *b))
    }

    fn vec3_eq(a: Vec3, b: Vec3) -> bool {
        f32_eq(a.x, b.x) && f32_eq(a.y, b.y) && f32_eq(a.z, b.z)
    }

    fn color_eq(a: LinearRgba, b: LinearRgba) -> bool {
        f32_eq(a.red, b.red)
            && f32_eq(a.green, b.green)
            && f32_eq(a.blue, b.blue)
            && f32_eq(a.alpha, b.alpha)
    }

    fn f32_eq(a: f32, b: f32) -> bool {
        a == b || (a.is_nan() && b.is_nan())
    }
}
