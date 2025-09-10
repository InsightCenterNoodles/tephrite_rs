use crate::backfill;

/// Non-send resource to the session
pub struct Session(pub(crate) backfill::FSessionHandle);

pub struct IBLResource {
    pub(crate) blob: backfill::FBlobHandle,
    pub(crate) img: backfill::FImageHandle,
    pub(crate) tex: backfill::FTextureHandle,
    pub(crate) fenv: backfill::FEnvironmentLightHandle,
}
