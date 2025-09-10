use std::{marker::PhantomData, ptr::NonNull};

#[allow(non_camel_case_types, non_upper_case_globals, non_snake_case, unused)]
pub mod ffi {
    include!(concat!(env!("OUT_DIR"), "/backfill_bindings.rs"));
}

// Safety: These are designed to be PODs from the C side of things
unsafe impl bytemuck::Zeroable for ffi::FPackedVertex {}
unsafe impl bytemuck::Pod for ffi::FPackedVertex {}

#[derive(thiserror::Error, Debug)]
pub enum BackfillError {
    #[error("construction failed")]
    EmptyPointer,
}

pub type BackfillResult<T> = Result<T, BackfillError>;

pub trait FFIHandle {
    type Raw;
}

/// Types that know how to release themselves via the C API.
/// # Safety
/// This trait is only to be implemented for reference counted pointer types
pub unsafe trait Releasable: FFIHandle {
    fn release(ptr: *mut Self::Raw);
}

/// Types that know how to retain (copy) themselves by the C API
/// # Safety
/// This trait is only to be implemented for reference counted pointer types
pub unsafe trait Retainable: Releasable {
    fn retain(ptr: *mut Self::Raw);
}

/// Owning handle for ref-counted C objects (RC starts at 1 after init()).
/// Not `Clone` because the C API does not expose add_ref.
pub struct Handle<T: Releasable> {
    ptr: NonNull<T::Raw>,
    _pd: PhantomData<T>,
}

impl<T: Releasable> Handle<T> {
    /// # Safety
    /// `ptr` must be non-null and come from the matching C `init` for `T`.
    pub unsafe fn from_nonnull(ptr: NonNull<T::Raw>) -> Self {
        Self {
            ptr,
            _pd: PhantomData,
        }
    }

    /// Expose the raw pointer for FFI calls.
    pub fn as_ptr(&self) -> *mut T::Raw {
        self.ptr.as_ptr()
    }
}

impl<T> Drop for Handle<T>
where
    T: Releasable,
{
    fn drop(&mut self) {
        T::release(self.ptr.as_ptr())
    }
}

impl<T: Retainable> Clone for Handle<T> {
    fn clone(&self) -> Self {
        T::retain(self.ptr.as_ptr());
        Self {
            ptr: self.ptr,
            _pd: self._pd,
        }
    }
}

impl<T: Releasable> std::fmt::Debug for Handle<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Handle<{}>({:p})", std::any::type_name::<T>(), self.ptr)
    }
}

/* ------------------------------------------------------------------------- */
/* Opaque handle marker types + Releasable impls                             */
/* ------------------------------------------------------------------------- */

macro_rules! opaque {
    ($name:ident, $handle:ident, $release:tt) => {
        pub enum $name {}
        impl FFIHandle for $name {
            type Raw = ffi::$name;
        }
        unsafe impl Releasable for $name {
            fn release(ptr: *mut Self::Raw) {
                unsafe { ffi::$release(ptr) }
            }
        }
        pub type $handle = Handle<$name>;
    };
}

macro_rules! opaque_cloneable {
    ($name:ident, $handle:ident, $retain:tt, $release:tt) => {
        pub enum $name {}
        impl FFIHandle for $name {
            type Raw = ffi::$name;
        }
        unsafe impl Releasable for $name {
            fn release(ptr: *mut Self::Raw) {
                unsafe { ffi::$release(ptr) }
            }
        }
        unsafe impl Retainable for $name {
            fn retain(ptr: *mut Self::Raw) {
                unsafe { ffi::$retain(ptr) }
            }
        }
        pub type $handle = Handle<$name>;
    };
}

opaque_cloneable!(FBlob, FBlobHandle, fblob_acquire, fblob_release);
opaque_cloneable!(FImage, FImageHandle, fimg_acquire, fimg_release);
opaque_cloneable!(FTexture, FTextureHandle, ftex_acquire, ftex_release);
opaque!(FTextureConfig, FTextureConfigHandle, ftex_config_destroy);
opaque!(
    FEnvironmentLight,
    FEnvironmentLightHandle,
    fenv_light_release
);
opaque_cloneable!(FMesh, FMeshHandle, fmesh_acquire, fmesh_release);
opaque_cloneable!(
    FMaterial,
    FMaterialHandle,
    fmaterial_acquire,
    fmaterial_release
);
opaque!(FLightConfig, FLightConfigHandle, flightconfig_destroy);
opaque!(FConfig, FConfigHandle, fconfig_destroy);
opaque!(FSession, FSessionHandle, fs_destroy);

/* ------------------------------------------------------------------------- */
/* Convenience constructors and helpers                                      */
/* ------------------------------------------------------------------------- */

// Small helpers for by-value PODs coming from bindgen.
#[inline]
pub fn float3(x: f32, y: f32, z: f32) -> ffi::float3 {
    ffi::float3 { x, y, z }
}
#[inline]
pub fn float4(x: f32, y: f32, z: f32, w: f32) -> ffi::float4 {
    ffi::float4 { x, y, z, w }
}
#[inline]
pub fn color(r: f32, g: f32, b: f32, a: f32) -> ffi::FColor {
    ffi::FColor { r, g, b, a }
}

// MARK: Blob

pub fn blob_from_slice(bytes: &[u8]) -> BackfillResult<FBlobHandle> {
    let ptr =
        unsafe { ffi::fblob_init_copy(bytes.as_ptr() as *const i8, bytes.len() as ffi::u64_) };
    NonNull::new(ptr)
        .map(|p| unsafe { Handle::from_nonnull(p) })
        .ok_or(BackfillError::EmptyPointer)
}

pub fn blobref_whole(blob: &FBlobHandle) -> ffi::FBlobRef {
    unsafe { ffi::fblobref_whole(blob.as_ptr()) }
}

// MARK: Image

pub fn image_from_exr(blobref: BlobReference) -> BackfillResult<FImageHandle> {
    let ptr = unsafe { ffi::fimg_init_exr(blobref.internal()) };
    NonNull::new(ptr)
        .map(|p| unsafe { Handle::from_nonnull(p) })
        .ok_or(BackfillError::EmptyPointer)
}

// MARK: Texture

pub fn tex_config_from_image(
    img: &FImageHandle,
    fmt: ffi::TextureFormat,
) -> BackfillResult<FTextureConfigHandle> {
    let ptr = unsafe { ffi::ftex_config_init(img.as_ptr(), fmt) };
    NonNull::new(ptr)
        .map(|p| unsafe { Handle::from_nonnull(p) })
        .ok_or(BackfillError::EmptyPointer)
}

pub fn texture_from_config(
    sess: &FSessionHandle,
    cfg: &FTextureConfigHandle,
) -> BackfillResult<FTextureHandle> {
    let ptr = unsafe { ffi::ftex_init(sess.as_ptr(), cfg.as_ptr()) };
    NonNull::new(ptr)
        .map(|p| unsafe { Handle::from_nonnull(p) })
        .ok_or(BackfillError::EmptyPointer)
}

// MARK: Env light

pub fn env_light_from_equirect(
    sess: &FSessionHandle,
    tex: &FTextureHandle,
) -> BackfillResult<FEnvironmentLightHandle> {
    let ptr = unsafe { ffi::fenv_light_init_equirect(sess.as_ptr(), tex.as_ptr()) };
    NonNull::new(ptr)
        .map(|p| unsafe { Handle::from_nonnull(p) })
        .ok_or(BackfillError::EmptyPointer)
}

// MARK: Mesh

#[derive(Clone)]
pub struct BlobReference {
    pub id: Handle<FBlob>,
    pub start: u64,
    pub length: u64,
}

impl BlobReference {
    pub fn whole(h: &Handle<FBlob>) -> Self {
        unsafe {
            let r = ffi::fblobref_whole(h.as_ptr());
            Self {
                id: h.clone(),
                start: r.start,
                length: r.length,
            }
        }
    }

    fn internal(&self) -> ffi::FBlobRef {
        ffi::FBlobRef {
            id: self.id.as_ptr(),
            start: self.start,
            length: self.length,
        }
    }
}

pub fn mesh_from_refs(
    sess: &FSessionHandle,
    vref: BlobReference,
    vcount: u32,
    iref: BlobReference,
    icount: u32,
    itype: ffi::FMeshIndexType,
    bounds: ffi::aabb,
) -> BackfillResult<FMeshHandle> {
    let ptr = unsafe {
        ffi::fmesh_init(
            sess.as_ptr(),
            vref.internal(),
            vcount,
            iref.internal(),
            icount,
            itype,
            bounds,
        )
    };
    NonNull::new(ptr)
        .map(|p| unsafe { Handle::from_nonnull(p) })
        .ok_or(BackfillError::EmptyPointer)
}

//

bitflags::bitflags! {
    pub struct MatConfigFlags: u32 {
        const UNLIT = ffi::MatConfigFlags_MC_UNLIT;
    }
}

pub fn material(
    sess: &FSessionHandle,
    mask: MatConfigFlags,
    instance_count: u32,
) -> Result<FMaterialHandle, ()> {
    let mut cfg = ffi::FMaterialConfig {
        mask: mask.bits(),
        instance_count,
    };
    let ptr = unsafe { ffi::fmaterial_init(sess.as_ptr(), &mut cfg as *mut _) };
    NonNull::new(ptr)
        .map(|p| unsafe { Handle::from_nonnull(p) })
        .ok_or(())
}

pub fn material_set_base_color(mat: &FMaterialHandle, c: ffi::FColor) {
    unsafe { ffi::fmaterial_set_base_color(mat.as_ptr(), c) }
}

pub fn material_set_rm(mat: &FMaterialHandle, roughness: f32, metallic: f32) {
    unsafe { ffi::fmaterial_set_roughness_metallic(mat.as_ptr(), roughness, metallic) }
}

pub fn material_set_instances(mat: &FMaterialHandle, data: &[ffi::mat4]) {
    unsafe { ffi::fmaterial_set_instances(mat.as_ptr(), data.as_ptr(), data.len() as ffi::u64_) }
}

/* ------------------------------ Lights ------------------------------- */

pub fn light_config(ty: ffi::FLightType) -> BackfillResult<FLightConfigHandle> {
    let ptr = unsafe { ffi::flightconfig_init(ty) };
    NonNull::new(ptr)
        .map(|p| unsafe { Handle::from_nonnull(p) })
        .ok_or(BackfillError::EmptyPointer)
}

pub fn light_set_point_defaults(
    lc: &FLightConfigHandle,
    intensity: f32,
    rgb: ffi::FColor,
    falloff: f32,
) {
    unsafe {
        ffi::flc_set_intensity(lc.as_ptr(), intensity);
        ffi::flc_set_color(lc.as_ptr(), rgb);
        ffi::flc_set_falloff(lc.as_ptr(), falloff);
    }
}

/* ------------------------- Config / Session --------------------------- */

pub fn config() -> BackfillResult<FConfigHandle> {
    let ptr = unsafe { ffi::fconfig_init() };
    NonNull::new(ptr)
        .map(|p| unsafe { Handle::from_nonnull(p) })
        .ok_or(BackfillError::EmptyPointer)
}

pub fn config_title(cfg: &FConfigHandle, title: &str) {
    let c = std::ffi::CString::new(title).unwrap();
    unsafe { ffi::fconfig_set_title(cfg.as_ptr(), c.as_ptr()) }
}

pub fn config_screen(cfg: &FConfigHandle, w: i32, h: i32) {
    unsafe { ffi::fconfig_set_screen(cfg.as_ptr(), w, h) }
}

pub fn config_display(cfg: &FConfigHandle, display: &str) {
    let c = std::ffi::CString::new(display).unwrap();
    unsafe { ffi::fconfig_set_display(cfg.as_ptr(), c.as_ptr()) }
}

pub fn session(cfg: &FConfigHandle) -> BackfillResult<FSessionHandle> {
    let ptr = unsafe { ffi::fs_init(cfg.as_ptr()) };
    NonNull::new(ptr)
        .map(|p| unsafe { Handle::from_nonnull(p) })
        .ok_or(BackfillError::EmptyPointer)
}

/// Strongly-typed entity id wrapper
#[repr(transparent)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct EntityId(pub ffi::i32_);

impl From<EntityId> for i32 {
    fn from(value: EntityId) -> Self {
        value.0
    }
}

pub fn new_entity(sess: &FSessionHandle) -> EntityId {
    EntityId(unsafe { ffi::fs_new_entity(sess.as_ptr()) })
}

pub fn destroy_entity(sess: &FSessionHandle, id: EntityId) {
    unsafe { ffi::fs_destroy_entity(sess.as_ptr(), id.0) }
}

pub fn add_renderable(
    sess: &FSessionHandle,
    id: EntityId,
    mesh: &FMeshHandle,
    mat: &FMaterialHandle,
) {
    unsafe { ffi::fs_add_renderable(sess.as_ptr(), id.0, mesh.as_ptr(), mat.as_ptr()) }
}

pub fn set_transform(sess: &FSessionHandle, id: EntityId, m: &bevy::math::Mat4) {
    const _SIZE_OK: () = assert!(size_of::<ffi::mat4>() == size_of::<bevy::math::Mat4>());
    unsafe { ffi::fs_set_transform(sess.as_ptr(), id.0, (m as *const _) as *const ffi::mat4) }
}

pub fn set_parent(sess: &FSessionHandle, child: EntityId, parent: EntityId) {
    unsafe { ffi::fs_set_parent(sess.as_ptr(), child.0, parent.0) }
}

pub fn update_head(sess: &FSessionHandle, pos: bevy::math::Vec3, rot: bevy::math::Quat) {
    let pos = pos.into();
    let rot = rot.into();

    unsafe { ffi::fs_update_head(sess.as_ptr(), pos, rot) };
}

pub fn set_environment_light(sess: &FSessionHandle, handle: &FEnvironmentLightHandle) {
    unsafe { ffi::fs_set_environment_light(sess.as_ptr(), handle.as_ptr()) };
}

pub fn frame(sess: &FSessionHandle) -> bool {
    unsafe { ffi::fs_frame(sess.as_ptr()) != 0 }
}

// MARK: Conversions

/* ---------------------------- Vec2 -------------------------------- */
impl From<ffi::float2> for bevy::math::Vec2 {
    #[inline]
    fn from(v: ffi::float2) -> Self {
        bevy::math::Vec2::new(v.x, v.y)
    }
}
impl From<bevy::math::Vec2> for ffi::float2 {
    #[inline]
    fn from(v: bevy::math::Vec2) -> Self {
        ffi::float2 { x: v.x, y: v.y }
    }
}

impl From<[f32; 2]> for ffi::float2 {
    #[inline]
    fn from(v: [f32; 2]) -> Self {
        ffi::float2 { x: v[0], y: v[1] }
    }
}

/* ---------------------------- Vec3 -------------------------------- */
impl From<ffi::float3> for bevy::math::Vec3 {
    #[inline]
    fn from(v: ffi::float3) -> Self {
        bevy::math::Vec3::new(v.x, v.y, v.z)
    }
}
impl From<bevy::math::Vec3> for ffi::float3 {
    #[inline]
    fn from(v: bevy::math::Vec3) -> Self {
        ffi::float3 {
            x: v.x,
            y: v.y,
            z: v.z,
        }
    }
}

impl From<[f32; 3]> for ffi::float3 {
    #[inline]
    fn from(v: [f32; 3]) -> Self {
        ffi::float3 {
            x: v[0],
            y: v[1],
            z: v[2],
        }
    }
}

/* ---------------------------- Vec4 -------------------------------- */
impl From<ffi::float4> for bevy::math::Vec4 {
    #[inline]
    fn from(v: ffi::float4) -> Self {
        bevy::math::Vec4::new(v.x, v.y, v.z, v.w)
    }
}
impl From<bevy::math::Vec4> for ffi::float4 {
    #[inline]
    fn from(v: bevy::math::Vec4) -> Self {
        ffi::float4 {
            x: v.x,
            y: v.y,
            z: v.z,
            w: v.w,
        }
    }
}

/* ---------------------------- Color ------------------------------- */
impl From<ffi::FColor> for bevy::math::Vec4 {
    #[inline]
    fn from(c: ffi::FColor) -> Self {
        bevy::math::Vec4::new(c.r, c.g, c.b, c.a)
    }
}
impl From<bevy::math::Vec4> for ffi::FColor {
    #[inline]
    fn from(v: bevy::math::Vec4) -> Self {
        ffi::FColor {
            r: v.x,
            g: v.y,
            b: v.z,
            a: v.w,
        }
    }
}

/* ---------------------------- Mat4 -------------------------------- */
// Your C mat4 is column-major with fields a,b,c,d = columns.
impl From<ffi::mat4> for bevy::math::Mat4 {
    #[inline]
    fn from(m: ffi::mat4) -> Self {
        bevy::math::Mat4::from_cols(
            bevy::math::Vec4::new(m.a.x, m.a.y, m.a.z, m.a.w),
            bevy::math::Vec4::new(m.b.x, m.b.y, m.b.z, m.b.w),
            bevy::math::Vec4::new(m.c.x, m.c.y, m.c.z, m.c.w),
            bevy::math::Vec4::new(m.d.x, m.d.y, m.d.z, m.d.w),
        )
    }
}
impl From<bevy::math::Mat4> for ffi::mat4 {
    #[inline]
    fn from(m: bevy::math::Mat4) -> Self {
        // glam stores columns in x_axis, y_axis, z_axis, w_axis (column-major)
        let a = m.x_axis;
        let b = m.y_axis;
        let c = m.z_axis;
        let d = m.w_axis;
        ffi::mat4 {
            a: ffi::float4 {
                x: a.x,
                y: a.y,
                z: a.z,
                w: a.w,
            },
            b: ffi::float4 {
                x: b.x,
                y: b.y,
                z: b.z,
                w: b.w,
            },
            c: ffi::float4 {
                x: c.x,
                y: c.y,
                z: c.z,
                w: c.w,
            },
            d: ffi::float4 {
                x: d.x,
                y: d.y,
                z: d.z,
                w: d.w,
            },
        }
    }
}

/* ---------------------------- Quat -------------------------------- */
// Assumes float4 stores quaternion as (x, y, z, w).
impl From<ffi::float4> for bevy::math::Quat {
    #[inline]
    fn from(q: ffi::float4) -> Self {
        bevy::math::Quat::from_xyzw(q.x, q.y, q.z, q.w)
    }
}
impl From<bevy::math::Quat> for ffi::float4 {
    #[inline]
    fn from(q: bevy::math::Quat) -> Self {
        ffi::float4 {
            x: q.x,
            y: q.y,
            z: q.z,
            w: q.w,
        }
    }
}
