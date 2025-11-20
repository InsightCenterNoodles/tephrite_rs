use bevy::{
    ecs::{
        entity::{EntityHashMap, EntityHashSet},
        system::entity_command::observe,
    },
    math::bounding::{Aabb3d, BoundingVolume},
    platform::collections::HashSet,
    prelude::*,
    render::render_resource::ShaderType,
};

/// Marker for the entity that represents a user's controller
#[derive(Component, Debug)]
pub struct Interactor;

/// A button on an interactor
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InteractorButton(pub(crate) u8);

#[derive(Debug, Default)]
pub struct InteractorState {
    buttons: ButtonInput<InteractorButton>,
    analogs: Vec<f32>,
}

/// The states of all interactors
#[derive(Debug, Resource, Default)]
pub struct AllInteractorState(EntityHashMap<InteractorState>);

/// A raw event for joystick button messages
#[derive(Message, Debug)]
pub struct ButtonEvent {
    /// The joystick this event came from
    pub from: Entity,
    pub kind: ButtonEventKind,
}

/// A raw event for joystick axis messages
#[derive(Message, Debug)]
pub struct AxisEvent {
    /// The joystick this event came from
    pub from: Entity,
    pub axis: u8,
    pub value: f32,
}

/// A button event from an Interactor
#[derive(Debug)]
pub enum ButtonEventKind {
    ButtonPressed(InteractorButton),
    ButtonReleased(InteractorButton),
}

/// Can be Activated (clicked)
#[derive(Debug, Clone, PartialEq, Component, Default)]
pub struct CanActivate {
    button_down_map: EntityHashMap<HashSet<u8>>,
}

/// The bounding box of an interactor, events inside this box will be channeled to the host entity
#[derive(Debug, Component)]
pub struct InteractionBounds {
    aabb: Aabb3d,
}

/*
/// Children of this entity will be elegible for interaction
#[derive(Debug, Component)]
pub struct InteractionRoot;
 */

/// Notification sent by input system that an Entity has been Activated (clicked)
#[derive(Debug, Clone, Copy, PartialEq, EntityEvent)]
pub struct Activate {
    pub entity: Entity, // kind for right/left?
}

pub(crate) struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PreUpdate,
            (reset_current_states, update_current_states).chain(),
            // TODO missing joy removal!
        );
    }
}

fn reset_current_states(mut state: ResMut<AllInteractorState>) {
    for x in state.bypass_change_detection().0.values_mut() {
        x.buttons.clear();
    }
}

fn update_current_states(
    mut button_reader: MessageReader<ButtonEvent>,
    mut axis_reader: MessageReader<AxisEvent>,
    mut state: ResMut<AllInteractorState>,
) {
    for event in button_reader.read() {
        let state = state.0.entry(event.from).or_default();

        match event.kind {
            ButtonEventKind::ButtonPressed(interactor_button) => {
                state.buttons.press(interactor_button)
            }
            ButtonEventKind::ButtonReleased(interactor_button) => {
                state.buttons.release(interactor_button)
            }
        }
    }

    for event in axis_reader.read() {
        let state = state.0.entry(event.from).or_default();

        if state.analogs.len() <= event.axis.into() {
            state.analogs.resize(state.analogs.len() * 2, 0.0);
        }
        state.analogs[event.axis as usize] = event.value;
    }
}

#[inline]
fn map_point(p_in_local_a: Vec3, a: &GlobalTransform, b: &GlobalTransform) -> Vec3 {
    let global = a.transform_point(p_in_local_a);

    b.affine().inverse().transform_point3(global)
}

// TODO: use something better than the global transform? itll be a frame out of date.
// TODO: we are just picking our first intersection
fn read_events(
    mut reader: MessageReader<ButtonEvent>,
    mut root_query: Query<(
        Entity,
        &InteractionBounds,
        &GlobalTransform,
        &mut CanActivate,
    )>,
    joy_query: Query<&GlobalTransform, With<Interactor>>,
) {
    //let mut handled = false;

    for event in reader.read() {
        let Ok(joy_tf) = joy_query.get(event.from) else {
            continue;
        };

        // for now, the origin of the interactor is our activation point

        let activation_point = Vec3::ZERO;

        for (e, bounds, tf, mut active) in root_query.iter_mut() {
            // map to our local

            let local = map_point(activation_point, joy_tf, tf);

            let local = Aabb3d::from_point_cloud(Isometry3d::default(), std::iter::once(local));

            // local in bounds?

            if !bounds.aabb.contains(&local) {
                continue;
            }

            // in bounds

            match event.kind {
                ButtonEventKind::ButtonPressed(interactor_button) => {
                    active
                        .button_down_map
                        .entry(event.from)
                        .or_default()
                        .insert(interactor_button.0);
                }
                ButtonEventKind::ButtonReleased(interactor_button) => {
                    let was_down = active
                        .button_down_map
                        .entry(event.from)
                        .or_default()
                        .remove(&interactor_button.0);

                    if was_down {
                        // emit action
                    }
                }
            }
        }
    }
}
