use std::collections::VecDeque;

use bevy::math::{DQuat, DVec3};

pub(crate) type SharedItemState = std::sync::Arc<std::sync::Mutex<ItemState>>;

pub(crate) fn new_shared_item_state() -> SharedItemState {
    std::sync::Arc::new(std::sync::Mutex::new(ItemState::default()))
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

/*

pub(crate) struct SharedItemState {
    pub pose: Arc<RwLock<Pose>>,
    pub analog: Arc<Vec<atomic_float::AtomicF64>>,
    pub button_changes: Arc<SegQueue<(u8, u8)>>,
}

#[derive(Default, Copy, Clone)]
pub struct Pose {
    pub position: DVec3,
    pub rotation: DQuat,
}

pub struct AtomicF64 {
    storage: AtomicU64,
}
impl AtomicF64 {
    pub fn new(value: f64) -> Self {
        let as_u64 = value.to_bits();
        Self {
            storage: AtomicU64::new(as_u64),
        }
    }
    pub fn store(&self, value: f64, ordering: Ordering) {
        let as_u64 = value.to_bits();
        self.storage.store(as_u64, ordering)
    }
    pub fn load(&self, ordering: Ordering) -> f64 {
        let as_u64 = self.storage.load(ordering);
        f64::from_bits(as_u64)
    }
}

     */
