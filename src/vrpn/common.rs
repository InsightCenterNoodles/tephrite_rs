use bevy::math::{DQuat, DVec3};

pub(crate) type SharedItemState = std::sync::Arc<seqlock::SeqLock<ItemState>>;

pub(crate) fn new_shared_item_state() -> SharedItemState {
    std::sync::Arc::new(seqlock::SeqLock::new(ItemState::default()))
}

/// The last known state of a VRPN item.
///
/// Values are stored as Bevy double-precision types (`DVec3`/`DQuat`).
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct ItemState {
    pub(crate) position: DVec3,
    pub(crate) rotation: DQuat,
}
