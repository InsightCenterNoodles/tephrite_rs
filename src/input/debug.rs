use bevy::{math::bounding::BoundingVolume, prelude::*};

use super::InteractionBounds;

/// Adds a replicated retained gizmo showing an entity's [`InteractionBounds`].
#[derive(Debug, Clone, Component)]
#[require(InteractionBounds, Transform)]
pub struct DebugInteractionBounds {
    pub color: Color,
    pub line_width: f32,
    pub depth_bias: f32,
}

impl Default for DebugInteractionBounds {
    fn default() -> Self {
        Self {
            color: Color::srgb(0.0, 0.85, 1.0),
            line_width: 2.0,
            depth_bias: -0.01,
        }
    }
}

#[derive(Component)]
struct DebugInteractionBoundsGizmo {
    child: Entity,
    handle: Handle<GizmoAsset>,
}

#[derive(Component)]
struct DebugInteractionBoundsGizmoChild {
    source: Entity,
    handle: Handle<GizmoAsset>,
}

pub(super) struct DebugInteractionBoundsPlugin;

impl Plugin for DebugInteractionBoundsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                spawn_debug_interaction_bounds_gizmos,
                update_debug_interaction_bounds_gizmos,
                cleanup_debug_interaction_bounds_gizmos,
            ),
        );
    }
}

fn spawn_debug_interaction_bounds_gizmos(
    mut commands: Commands,
    mut gizmo_assets: ResMut<Assets<GizmoAsset>>,
    query: Query<
        (Entity, &InteractionBounds, &DebugInteractionBounds),
        Without<DebugInteractionBoundsGizmo>,
    >,
) {
    for (source, bounds, debug) in &query {
        let mut asset = GizmoAsset::new();
        draw_bounds(&mut asset, bounds, debug.color);
        let handle = gizmo_assets.add(asset);

        let child = commands
            .spawn((
                Name::new("DebugInteractionBounds"),
                Gizmo {
                    handle: handle.clone(),
                    line_config: GizmoLineConfig {
                        width: debug.line_width,
                        ..default()
                    },
                    depth_bias: debug.depth_bias,
                },
                Transform::IDENTITY,
                DebugInteractionBoundsGizmoChild {
                    source,
                    handle: handle.clone(),
                },
            ))
            .id();

        commands.entity(source).add_child(child);
        commands
            .entity(source)
            .insert(DebugInteractionBoundsGizmo { child, handle });
    }
}

fn update_debug_interaction_bounds_gizmos(
    mut gizmo_assets: ResMut<Assets<GizmoAsset>>,
    sources: Query<
        (
            &InteractionBounds,
            &DebugInteractionBounds,
            &DebugInteractionBoundsGizmo,
        ),
        Or<(Changed<InteractionBounds>, Changed<DebugInteractionBounds>)>,
    >,
    mut gizmos: Query<&mut Gizmo>,
) {
    for (bounds, debug, state) in &sources {
        if let Some(mut asset) = gizmo_assets.get_mut(&state.handle) {
            asset.clear();
            draw_bounds(&mut asset, bounds, debug.color);
        }

        if let Ok(mut gizmo) = gizmos.get_mut(state.child) {
            gizmo.line_config.width = debug.line_width;
            gizmo.depth_bias = debug.depth_bias;
        }
    }
}

fn cleanup_debug_interaction_bounds_gizmos(
    mut commands: Commands,
    mut gizmo_assets: ResMut<Assets<GizmoAsset>>,
    missing_debug_sources: Query<
        (Entity, &DebugInteractionBoundsGizmo),
        Without<DebugInteractionBounds>,
    >,
    sources: Query<(), With<DebugInteractionBoundsGizmo>>,
    children: Query<(Entity, &DebugInteractionBoundsGizmoChild), With<Gizmo>>,
) {
    for (source, state) in &missing_debug_sources {
        gizmo_assets.remove(&state.handle);
        commands.entity(state.child).despawn();
        commands
            .entity(source)
            .remove::<DebugInteractionBoundsGizmo>();
    }

    for (child, state) in &children {
        if sources.get(state.source).is_err() {
            gizmo_assets.remove(&state.handle);
            commands.entity(child).despawn();
        }
    }
}

fn draw_bounds(asset: &mut GizmoAsset, bounds: &InteractionBounds, color: Color) {
    let center = bounds.aabb.center();
    let half_size = bounds.aabb.half_size();
    asset.primitive_3d(
        &Cuboid {
            half_size: half_size.into(),
        },
        Isometry3d::from_translation(center),
        color,
    );
}

#[cfg(test)]
mod tests {
    use bevy::math::bounding::Aabb3d;

    use super::*;

    #[test]
    fn debug_interaction_bounds_spawns_retained_gizmo_child() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(AssetPlugin::default());
        app.init_asset::<GizmoAsset>();
        app.add_plugins(DebugInteractionBoundsPlugin);

        let source = app
            .world_mut()
            .spawn((
                Transform::IDENTITY,
                InteractionBounds {
                    aabb: Aabb3d::new(Vec3A::ZERO, Vec3A::ONE),
                },
                DebugInteractionBounds::default(),
            ))
            .id();

        app.update();

        let mut query =
            app.world_mut()
                .query::<(Entity, &ChildOf, &Gizmo, &DebugInteractionBoundsGizmoChild)>();
        let children = query.iter(app.world()).collect::<Vec<_>>();

        assert_eq!(children.len(), 1);
        assert_eq!(children[0].1.0, source);
        assert_eq!(children[0].3.source, source);
    }
}
