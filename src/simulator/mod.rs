use bevy::prelude::*;

mod head;
mod interactor;

/// The main Bevy plugin for the simulator.
pub(crate) struct SimulatorPlugin;

impl Plugin for SimulatorPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(interactor::InteractorSimulatorPlugin);
        app.add_plugins(head::HeadSimulatorPlugin);
    }
}
