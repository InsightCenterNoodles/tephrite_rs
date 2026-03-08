//! Simulator-side bindings for interactive devices and remote control.
//!
//! This plugin wires simulator entities (head + interactors) into shared helper
//! use-cases that expose transform controls on the remote-control webpage.

use bevy::prelude::*;

mod head;
mod interactor;

/// Main simulator plugin.
///
/// Registers per-device setup systems for head and interactor entities.
pub(crate) struct SimulatorPlugin;

impl Plugin for SimulatorPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(interactor::InteractorSimulatorPlugin);
        app.add_plugins(head::HeadSimulatorPlugin);
    }
}
