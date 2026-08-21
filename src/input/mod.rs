//! Input, interactor, activation, hover, and navigation support.
//!
//! The input stack has three layers:
//!
//! - Raw input messages: [`ButtonMessage`] and [`AxisMessage`] are written by
//!   device backends.
//! - Interactors: [`Interactor`] and [`InteractorState`] translate raw buttons
//!   into semantic [`InteractorAction`] values.
//! - Targets and helpers: [`CanActivate`], [`InteractionBounds`],
//!   [`Hoverable`], and [`NavigationPlugin`] provide common scene behavior.

pub mod common;
mod debug;
mod events;
pub mod hover;
pub mod interaction;
mod interactor;
pub mod interactor_types;
mod navigator;
pub mod spatial;

use bevy::prelude::*;

pub use debug::*;
pub use events::*;
pub use hover::*;
pub use interaction::*;
pub use interactor::*;
pub use interactor_types::*;
pub use navigator::*;
pub use spatial::*;

pub(crate) struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ButtonMessage>();
        app.add_message::<AxisMessage>();

        app.add_plugins(interactor::InteractorPlugin);
        app.add_plugins(debug::DebugInteractionBoundsPlugin);
    }
}
