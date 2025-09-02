use bevy::prelude::Component;

/// A component indicating that the entity should be replicated.
///
/// When added to an entity, the entity and supported components will be
/// replicated to all children processes. At the moment, this must be manually
/// added to your entities:
///
/// ```
/// commands.spawn((
///     PbrBundle {
///         mesh: meshes.add(Circle::new(4.0)),
///         material: materials.add(Color::WHITE),
///         transform: Transform::from_rotation(Quat::from_rotation_x(
///             -std::f32::consts::FRAC_PI_2,
///         )),
///         ..default()
///     },
///     Replicated, // <-- Add this component!
/// ));
/// ```
#[derive(Component, Debug, Clone, Copy)]
pub struct Replicated;
