use std::collections::VecDeque;

use bevy::math::{DQuat, DVec3};

pub(crate) type SharedItemState = std::sync::Arc<std::sync::RwLock<ItemState>>;

pub(crate) fn new_shared_item_state() -> SharedItemState {
    std::sync::Arc::new(std::sync::RwLock::new(ItemState::default()))
}

/// The last known state of a VRPN item.
///
/// Values are stored as Bevy double-precision types (`DVec3`/`DQuat`).
#[derive(Debug, Default)]
pub(crate) struct ItemState {
    pub(crate) position: DVec3,
    pub(crate) rotation: DQuat,

    // state per channel
    pub(crate) analog_state: Vec<f64>,

    pub(crate) button_changes: VecDeque<(u8, u8)>,
}
