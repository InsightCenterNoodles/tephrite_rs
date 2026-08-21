//! Components that mark entities as input targets.
//!
//! Tephrite interaction is bounds based. Add [`CanActivate`] to an entity to
//! receive targeted activation and semantic interactor action events. The
//! required [`InteractionBounds`] component defines the hit volume in the
//! entity's local space.

use bevy::{
    ecs::entity::EntityHashMap,
    math::bounding::{Aabb3d, IntersectsVolume},
    platform::collections::HashSet,
    prelude::*,
};

use super::interactor::InputButton;

/// Marks an entity as an activation target.
///
/// When an interactor button press begins inside this entity's
/// [`InteractionBounds`], the matching release is routed back to this entity as
/// an [`Activate`](super::Activate) event. Semantic button mappings are also
/// routed as [`InteractorActionEvent`](super::InteractorActionEvent).
#[derive(Debug, Clone, PartialEq, Component)]
#[require(InteractionBounds)]
pub struct CanActivate {
    pub(crate) button_down_map: EntityHashMap<HashSet<InputButton>>,
    /// Set to `false` to temporarily exclude this entity from activation
    /// routing without removing its bounds or observers.
    pub enable: bool,
}

impl Default for CanActivate {
    fn default() -> Self {
        Self {
            button_down_map: Default::default(),
            enable: true,
        }
    }
}

/// Local-space axis-aligned bounds used for input targeting.
///
/// The bounds are evaluated in the target entity's local space. Interactor
/// points are transformed into that space before containment checks are made.
#[derive(Debug, Clone, Copy, PartialEq, Component)]
pub struct InteractionBounds {
    pub aabb: Aabb3d,
}

impl Default for InteractionBounds {
    fn default() -> Self {
        Self {
            aabb: Aabb3d::new(Vec3A::ZERO, Vec3A::splat(0.001)),
        }
    }
}

impl InteractionBounds {
    /// Create local-space axis-aligned interaction bounds.
    pub const fn aabb(aabb: Aabb3d) -> Self {
        Self { aabb }
    }

    /// Returns true if these bounds intersect the provided local-space AABB.
    pub fn intersects_aabb(&self, aabb: &Aabb3d) -> bool {
        self.aabb.intersects(aabb)
    }

    /// Returns true if these bounds contain the provided local-space point.
    pub fn contains_point(&self, point: impl Into<Vec3A>) -> bool {
        let point = point.into();
        point.cmpge(self.aabb.min).all() && point.cmple(self.aabb.max).all()
    }
}
