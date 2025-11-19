use crate::backfill;

/// Non-send resource to the session
pub struct Session(pub(crate) backfill::FSessionHandle);
