use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU32, Ordering},
};

use bevy::math::{Quat, Vec3};
use crossbeam_queue::SegQueue;

/// The last known state of a VRPN item.
///
/// Values are stored as Bevy double-precision types (`DVec3`/`DQuat`).
#[derive(Debug, Clone, Default)]
pub(crate) struct SharedItemState {
    pub pose: Arc<Mutex<Pose>>,
    pub latest_analog: Arc<Vec<AtomicF32>>,
    pub previous_analog: Arc<Vec<AtomicF32>>,
    pub button_changes: Arc<SegQueue<(u8, u8)>>,
}

impl SharedItemState {
    pub(crate) fn new() -> Self {
        Self {
            pose: Arc::new(Mutex::new(Pose::default())),
            latest_analog: Arc::new(vec![AtomicF32::default(); 256]),
            previous_analog: Arc::new(vec![AtomicF32::default(); 256]),
            button_changes: Arc::new(Default::default()),
        }
    }
}

#[derive(Debug, Default, Copy, Clone)]
pub struct Pose {
    pub position: Vec3,
    pub rotation: Quat,
}

#[derive(Debug, Default)]
pub struct AtomicF32 {
    storage: AtomicU32,
}
impl AtomicF32 {
    #[allow(unused)]
    pub fn new(value: f32) -> Self {
        let as_u32 = value.to_bits();
        Self {
            storage: AtomicU32::new(as_u32),
        }
    }
    pub fn store(&self, value: f32, ordering: Ordering) {
        let as_u32 = value.to_bits();
        self.storage.store(as_u32, ordering)
    }
    pub fn load(&self, ordering: Ordering) -> f32 {
        let as_u32 = self.storage.load(ordering);
        f32::from_bits(as_u32)
    }
}

impl Clone for AtomicF32 {
    fn clone(&self) -> Self {
        Self {
            storage: AtomicU32::new(self.storage.load(Ordering::Relaxed)),
        }
    }
}
