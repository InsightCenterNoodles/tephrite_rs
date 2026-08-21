//! Optional hover detection for bounded input targets.
//!
//! Hover is intentionally separate from activation. Add [`HoverPlugin`] and
//! mark entities with [`Hoverable`] when you want Tephrite to maintain
//! [`IsHovered`] based on the current interactor position and
//! [`InteractionBounds`].

use bevy::{ecs::entity::EntityHashSet, math::bounding::Aabb3d, prelude::*, utils::Parallel};

use crate::input::{InteractionBounds, Interactor, map_point};

/// Marks an entity as eligible for hover detection.
///
/// Hover checks run against all hoverable entities, so this should be enabled
/// selectively in large scenes.
#[derive(Debug, Default, Clone, PartialEq, Component)]
#[require(InteractionBounds)]
pub struct Hoverable;

/// Temporarily disables hover detection for an otherwise [`Hoverable`] entity.
#[derive(Debug, Default, Clone, PartialEq, Component)]
pub struct HoverDisabled;

/// Marker indicating the entity is currently hovered by an interactor.
#[derive(Debug, Clone, PartialEq, Component)]
pub struct IsHovered;

/// Maintains [`IsHovered`] on [`Hoverable`] entities.
pub struct HoverPlugin;

impl Plugin for HoverPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, hover_check);
        app.add_observer(on_joy_removal);
    }
}

// this is SUPER RARE
fn on_joy_removal(
    _trigger: On<Remove, Interactor>,
    all: Query<Entity, With<IsHovered>>,
    mut commands: Commands,
) {
    for e in all {
        commands.entity(e).remove::<IsHovered>();
    }
}

fn hover_check(
    joy_query: Query<&GlobalTransform, With<Interactor>>,
    hoverable_query: Query<
        (Entity, &GlobalTransform, &InteractionBounds),
        (With<Hoverable>, Without<HoverDisabled>),
    >,
    mut commands: Commands,
    mut last_hovered: Local<EntityHashSet>,
    mut hover_hits: Local<Parallel<Vec<Entity>>>,
) {
    // for now
    let Ok(joy_world) = joy_query.single() else {
        warn_once!("No interactor found, hovering disabled.");
        return;
    };

    // hardcoded till we have controller configuration
    let activation_point = Vec3::ZERO;
    let activation_half_size = Vec3::splat(0.06);

    hover_hits.clear();
    hoverable_query.par_iter().for_each_init(
        || hover_hits.borrow_local_mut(),
        |hits, (entity, global_transform, bounds)| {
            let activation_bounds = activation_bounds_in_target(
                activation_point,
                activation_half_size,
                joy_world,
                global_transform,
            );

            if bounds.intersects_aabb(&activation_bounds) {
                hits.push(entity);
            }
        },
    );

    let cache = hover_hits.drain().collect::<EntityHashSet>();

    for e in cache.iter() {
        if last_hovered.contains(e) {
            // selected one that is already selected. pass
        } else {
            // selected on that is not already selected
            commands.entity(*e).insert(IsHovered);
        }
    }

    for e in &last_hovered {
        if !cache.contains(e) {
            commands.entity(*e).remove::<IsHovered>();
        }
    }

    *last_hovered = cache;
}

fn activation_corners(center: Vec3, half_size: Vec3) -> impl Iterator<Item = Vec3> {
    [-1.0, 1.0].into_iter().flat_map(move |x| {
        [-1.0, 1.0].into_iter().flat_map(move |y| {
            [-1.0, 1.0]
                .into_iter()
                .map(move |z| center + Vec3::new(x * half_size.x, y * half_size.y, z * half_size.z))
        })
    })
}

fn activation_bounds_in_target(
    activation_point: Vec3,
    activation_half_size: Vec3,
    interactor: &GlobalTransform,
    target: &GlobalTransform,
) -> Aabb3d {
    Aabb3d::from_point_cloud(
        Isometry3d::default(),
        activation_corners(activation_point, activation_half_size)
            .map(|corner| map_point(corner, interactor, target)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activation_bounds_are_centered_in_target_local_space() {
        let interactor = GlobalTransform::from(Transform::from_xyz(5.0, 0.0, 0.0));
        let target = GlobalTransform::from(Transform::from_xyz(5.0, 0.0, 0.0));
        let target_bounds = Aabb3d::new(Vec3A::ZERO, Vec3A::splat(0.001));

        let activation_bounds =
            activation_bounds_in_target(Vec3::ZERO, Vec3::splat(0.06), &interactor, &target);

        assert!(InteractionBounds::aabb(target_bounds).intersects_aabb(&activation_bounds));
    }

    #[test]
    fn activation_bounds_track_target_local_offset() {
        let interactor = GlobalTransform::from(Transform::from_xyz(5.0, 0.0, 0.0));
        let target = GlobalTransform::from(Transform::from_xyz(6.0, 0.0, 0.0));
        let target_bounds = Aabb3d::new(Vec3A::ZERO, Vec3A::splat(0.001));

        let activation_bounds =
            activation_bounds_in_target(Vec3::ZERO, Vec3::splat(0.06), &interactor, &target);

        assert!(!InteractionBounds::aabb(target_bounds).intersects_aabb(&activation_bounds));
    }
}
