pub mod common;
mod events;
pub mod hover;
mod interactor;
pub mod interactor_types;
mod navigator;

use bevy::{
    ecs::entity::EntityHashMap,
    math::bounding::{Aabb3d, BoundingVolume},
    platform::collections::HashSet,
    prelude::*,
};

pub use common::*;
pub use events::*;
pub use hover::*;
pub use interactor::*;
pub use interactor_types::*;
pub use navigator::*;

/// Can be Activated (clicked)
#[derive(Debug, Clone, PartialEq, Component)]
#[require(InteractionBounds)]
pub struct CanActivate {
    button_down_map: EntityHashMap<HashSet<InputButton>>,
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

/// The bounding box of an interactable entity, events inside this box will be channeled to the host entity
#[derive(Debug, Component)]
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

pub(crate) struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ButtonMessage>();
        app.add_message::<AxisMessage>();

        app.add_plugins(interactor::InteractorPlugin);
    }
}
