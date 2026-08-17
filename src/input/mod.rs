pub mod common;
mod events;
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
pub use interactor::*;
pub use interactor_types::*;
pub use navigator::*;

/// Can be Activated (clicked)
#[derive(Debug, Clone, PartialEq, Component, Default)]
pub struct CanActivate {
    button_down_map: EntityHashMap<HashSet<InputButton>>,
}

/// The bounding box of an interactor, events inside this box will be channeled to the host entity
#[derive(Debug, Component)]
pub struct InteractionBounds {
    pub aabb: Aabb3d,
}

pub(crate) struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ButtonMessage>();
        app.add_message::<AxisMessage>();

        app.add_plugins(interactor::InteractorPlugin);
    }
}
