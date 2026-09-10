//! Laser pointer selection for interactor entities.
//!
//! Add [`LaserPointer`] to an [`Interactor`](crate::input::Interactor) entity
//! and [`LaserSelectable`] to bounded targets. When the pointer's primary
//! action is pressed, the nearest selectable [`InteractionBounds`] hit by the
//! pointer ray receives a [`LaserSelected`] event.

use std::f32::consts::FRAC_PI_2;

use bevy::{
    ecs::entity::EntityHashSet,
    math::{
        Dir3,
        bounding::{Aabb3d, RayCast3d},
    },
    prelude::*,
};

use crate::input::{InteractionBounds, Interactor, InteractorAction, InteractorState};

/// Marks an interactor as emitting a laser pointer used for selection.
#[derive(Debug, Clone, Copy, PartialEq, Component)]
pub struct LaserPointer {
    /// Maximum ray distance, in meters.
    pub length: f32,
    /// Visual cylinder radius, in meters.
    pub radius: f32,
    /// Visual laser color.
    pub color: Color,
    /// Whether the generated laser visual is visible.
    pub visible: bool,
}

impl Default for LaserPointer {
    fn default() -> Self {
        Self {
            length: 2.0,
            radius: 0.005,
            color: Color::linear_rgb(1.0, 0.0, 0.0),
            visible: true,
        }
    }
}

/// Marks an entity as eligible for laser selection.
#[derive(Debug, Default, Clone, Copy, PartialEq, Component)]
#[require(InteractionBounds)]
pub struct LaserSelectable;

/// Current target hit by a [`LaserPointer`].
#[derive(Debug, Clone, Copy, PartialEq, Component)]
pub struct LaserPointerHit {
    /// Target entity currently intersected by the laser ray.
    pub entity: Entity,
    /// World-space distance from the interactor origin to the hit point.
    pub distance: f32,
}

/// Event delivered to the nearest hit [`LaserSelectable`] target.
#[derive(Debug, Clone, Copy, PartialEq, EntityEvent)]
pub struct LaserSelected {
    /// Target entity that was selected.
    pub entity: Entity,
    /// Interactor entity that produced the laser selection.
    pub interactor: Entity,
}

#[derive(Debug, Component)]
struct LaserPointerVisual {
    child: Entity,
}

#[derive(Debug, Component)]
struct LaserPointerVisualChild;

#[derive(Debug, Component)]
struct LaserHighlightVisual {
    root: Entity,
    material: Handle<StandardMaterial>,
}

#[derive(Debug, Component)]
struct LaserHighlightVisualChild;

#[derive(Default)]
struct LaserHighlightFrame {
    hit: EntityHashSet,
    selected: EntityHashSet,
}

/// Adds laser pointer visuals and routes primary presses to laser targets.
pub struct LaserSelectionPlugin;

impl Plugin for LaserSelectionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (attach_laser_visuals, update_lasers).chain());
        app.add_observer(on_laser_pointer_removal);
    }
}

fn attach_laser_visuals(
    pointers: Query<(Entity, &LaserPointer), (With<Interactor>, Without<LaserPointerVisual>)>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (entity, pointer) in &pointers {
        let mesh = meshes.add(Cylinder::new(pointer.radius, 1.0));
        let material = materials.add(StandardMaterial {
            base_color: pointer.color,
            unlit: true,
            ..Default::default()
        });

        let child = commands
            .spawn((
                LaserPointerVisualChild,
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::from_xyz(0.0, 0.0, -pointer.length * 0.5)
                    .with_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
                if pointer.visible {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                },
                ChildOf(entity),
            ))
            .id();

        commands.entity(entity).insert(LaserPointerVisual { child });
    }
}

fn on_laser_pointer_removal(
    trigger: On<Remove, LaserPointer>,
    visuals: Query<&LaserPointerVisual>,
    children: Query<(), With<LaserPointerVisualChild>>,
    mut commands: Commands,
) {
    if let Ok(visual) = visuals.get(trigger.entity)
        && children.contains(visual.child)
    {
        commands.entity(visual.child).despawn();
    }

    commands
        .entity(trigger.entity)
        .remove::<LaserPointerVisual>();
}

fn update_lasers(
    pointers: Query<
        (
            Entity,
            &GlobalTransform,
            &InteractorState,
            &LaserPointer,
            Option<&LaserPointerVisual>,
        ),
        With<Interactor>,
    >,
    targets: Query<(Entity, &GlobalTransform, &InteractionBounds), With<LaserSelectable>>,
    highlights: Query<(Entity, &LaserHighlightVisual)>,
    mut visual_children: Query<(&mut Transform, &mut Visibility), With<LaserPointerVisualChild>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut frame: Local<LaserHighlightFrame>,
    mut local_hit_cache: Local<Vec<(Entity, f32)>>,
) {
    frame.hit.clear();
    frame.selected.clear();

    for (interactor, interactor_tf, state, pointer, visual) in &pointers {
        let hit = nearest_laser_hit(
            interactor_tf,
            pointer.length,
            &targets,
            &mut local_hit_cache,
        );

        update_laser_hit_component(&mut commands, interactor, hit);

        update_laser_visual(
            pointer,
            hit.map_or(pointer.length, |(_, distance)| distance),
            visual,
            &mut visual_children,
        );

        let Some((entity, _distance)) = hit else {
            continue;
        };

        frame.hit.insert(entity);

        if state.just_pressed(InteractorAction::Primary) {
            frame.selected.insert(entity);
            commands.trigger(LaserSelected { entity, interactor });
        }
    }

    remove_stale_highlights(&mut commands, &frame.hit, &highlights);

    update_highlights(
        &mut commands,
        &mut meshes,
        &mut materials,
        &frame,
        &targets,
        &highlights,
    );
}

fn update_laser_hit_component(
    commands: &mut Commands,
    interactor: Entity,
    hit: Option<(Entity, f32)>,
) {
    if let Some((entity, distance)) = hit {
        commands
            .entity(interactor)
            .insert(LaserPointerHit { entity, distance });
    } else {
        commands.entity(interactor).remove::<LaserPointerHit>();
    }
}

fn update_laser_visual(
    pointer: &LaserPointer,
    distance: f32,
    visual: Option<&LaserPointerVisual>,
    visual_children: &mut Query<(&mut Transform, &mut Visibility), With<LaserPointerVisualChild>>,
) {
    let Some(visual) = visual else {
        return;
    };

    let Ok((mut transform, mut visibility)) = visual_children.get_mut(visual.child) else {
        return;
    };

    let distance = distance.clamp(0.0, pointer.length);
    transform.translation = Vec3::new(0.0, 0.0, -distance * 0.5);
    transform.rotation = Quat::from_rotation_x(-FRAC_PI_2);
    transform.scale = Vec3::new(1.0, distance, 1.0);
    *visibility = if pointer.visible {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
}

fn remove_stale_highlights(
    commands: &mut Commands,
    current_hits: &EntityHashSet,
    highlights: &Query<(Entity, &LaserHighlightVisual)>,
) {
    for (entity, visual) in highlights {
        if !current_hits.contains(&entity) {
            commands.entity(visual.root).despawn();
            commands.entity(entity).remove::<LaserHighlightVisual>();
        }
    }
}

fn update_highlights(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    frame: &LaserHighlightFrame,
    targets: &Query<(Entity, &GlobalTransform, &InteractionBounds), With<LaserSelectable>>,
    highlights: &Query<(Entity, &LaserHighlightVisual)>,
) {
    for entity in frame.hit.iter().copied() {
        let color = if frame.selected.contains(&entity) {
            Color::WHITE
        } else {
            Color::linear_rgb(1.0, 0.0, 0.0)
        };

        if let Ok((_, visual)) = highlights.get(entity) {
            if let Some(material) = materials.get_mut(visual.material.id()) {
                material.base_color = color;
            }
            continue;
        }

        let Ok((_, _, bounds)) = targets.get(entity) else {
            continue;
        };

        let material = materials.add(StandardMaterial {
            base_color: color,
            unlit: true,
            ..Default::default()
        });
        let root = spawn_highlight(commands, meshes, entity, bounds, material.clone());

        commands
            .entity(entity)
            .insert(LaserHighlightVisual { root, material });
    }
}

fn spawn_highlight(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    target: Entity,
    bounds: &InteractionBounds,
    material: Handle<StandardMaterial>,
) -> Entity {
    let root = commands
        .spawn((
            LaserHighlightVisualChild,
            Name::new("LaserSelectionHighlight"),
            Transform::IDENTITY,
            Visibility::Visible,
            ChildOf(target),
        ))
        .id();

    commands.entity(root).with_children(|parent| {
        spawn_wireframe_edges(parent, meshes, material, bounds.aabb);
    });

    root
}

fn spawn_wireframe_edges(
    parent: &mut ChildSpawnerCommands,
    meshes: &mut Assets<Mesh>,
    material: Handle<StandardMaterial>,
    aabb: Aabb3d,
) {
    let min = Vec3::from(aabb.min);
    let max = Vec3::from(aabb.max);
    let center = (min + max) * 0.5;
    let size = max - min;
    let radius = 0.005;

    for y in [min.y, max.y] {
        for z in [min.z, max.z] {
            spawn_edge(
                parent,
                meshes,
                material.clone(),
                Vec3::new(center.x, y, z),
                size.x,
                Quat::from_rotation_z(FRAC_PI_2),
                radius,
            );
        }
    }

    for x in [min.x, max.x] {
        for z in [min.z, max.z] {
            spawn_edge(
                parent,
                meshes,
                material.clone(),
                Vec3::new(x, center.y, z),
                size.y,
                Quat::IDENTITY,
                radius,
            );
        }
    }

    for x in [min.x, max.x] {
        for y in [min.y, max.y] {
            spawn_edge(
                parent,
                meshes,
                material.clone(),
                Vec3::new(x, y, center.z),
                size.z,
                Quat::from_rotation_x(FRAC_PI_2),
                radius,
            );
        }
    }
}

fn spawn_edge(
    parent: &mut ChildSpawnerCommands,
    meshes: &mut Assets<Mesh>,
    material: Handle<StandardMaterial>,
    translation: Vec3,
    length: f32,
    rotation: Quat,
    radius: f32,
) {
    if length <= 0.0 {
        return;
    }

    parent.spawn((
        LaserHighlightVisualChild,
        Mesh3d(meshes.add(Cylinder::new(radius, length))),
        MeshMaterial3d(material),
        Transform::from_translation(translation).with_rotation(rotation),
    ));
}

fn nearest_laser_hit(
    interactor_tf: &GlobalTransform,
    max_distance: f32,
    targets: &Query<(Entity, &GlobalTransform, &InteractionBounds), With<LaserSelectable>>,
    cached: &mut Vec<(Entity, f32)>,
) -> Option<(Entity, f32)> {
    cached.clear();

    // we could use thread local here, but this is all silly. need a real query structure.
    // its unlikely that we will have many intersections

    let interactor = interactor_tf.compute_transform();
    let origin = interactor.translation;
    let direction = interactor.forward();

    {
        let guarded = std::sync::Mutex::<&mut Vec<(Entity, f32)>>::new(cached);

        targets.par_iter().for_each(|(entity, target_tf, bounds)| {
            if let Some(ev) =
                ray_intersection_at(origin, direction, max_distance, target_tf, bounds)
                    .map(|distance| (entity, distance))
            {
                guarded.lock().unwrap().push(ev);
            }
        });
    }

    cached
        .iter()
        .min_by(|(_, a), (_, b)| a.total_cmp(b))
        .cloned()
}

fn ray_intersection_at(
    origin: Vec3,
    direction: Dir3,
    max_distance: f32,
    target_tf: &GlobalTransform,
    bounds: &InteractionBounds,
) -> Option<f32> {
    let target_from_world = target_tf.affine().inverse();
    let local_origin = target_from_world.transform_point3(origin);
    let local_direction = Dir3::new(target_from_world.transform_vector3(*direction)).ok()?;

    let local_distance = RayCast3d::new(local_origin, local_direction, f32::MAX)
        .aabb_intersection_at(&bounds.aabb)?;
    let local_hit = local_origin + *local_direction * local_distance;
    let world_distance = target_tf
        .affine()
        .transform_point3(local_hit)
        .distance(origin);

    (world_distance <= max_distance).then_some(world_distance)
}

#[cfg(test)]
mod tests {
    use bevy::math::bounding::Aabb3d;

    use super::*;

    #[derive(Debug, Default, Resource)]
    struct LaserLog(Vec<LaserSelected>);

    fn selection_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<Assets<Mesh>>();
        app.init_resource::<Assets<StandardMaterial>>();
        app.init_resource::<LaserLog>();
        app.add_plugins(crate::input::InputPlugin);
        app.add_plugins(LaserSelectionPlugin);
        app.add_observer(|trigger: On<LaserSelected>, mut log: ResMut<LaserLog>| {
            log.0.push(*trigger.event());
        });
        app
    }

    fn selectable_at(app: &mut App, z: f32) -> Entity {
        app.world_mut()
            .spawn((
                LaserSelectable,
                InteractionBounds::aabb(Aabb3d::new(Vec3A::ZERO, Vec3A::splat(0.1))),
                GlobalTransform::from(Transform::from_xyz(0.0, 0.0, z)),
            ))
            .id()
    }

    fn press_primary(app: &mut App, interactor: Entity) {
        app.world_mut()
            .resource_mut::<Messages<crate::input::ButtonMessage>>()
            .write(crate::input::ButtonMessage {
                from: interactor,
                kind: crate::input::ButtonEventKind::ButtonPressed(
                    crate::input::InputButton::Button1,
                ),
            });
    }

    #[test]
    fn primary_press_selects_nearest_target_in_front() {
        let mut app = selection_app();
        let interactor = app
            .world_mut()
            .spawn((
                Interactor::Controller,
                InteractorState::new(Interactor::Controller),
                LaserPointer::default(),
                GlobalTransform::IDENTITY,
            ))
            .id();
        let near = selectable_at(&mut app, -1.0);
        let _far = selectable_at(&mut app, -1.5);

        press_primary(&mut app, interactor);

        app.update();

        assert_eq!(
            app.world().resource::<LaserLog>().0,
            vec![LaserSelected {
                entity: near,
                interactor,
            }]
        );
    }

    #[test]
    fn primary_press_ignores_targets_behind_the_interactor() {
        let mut app = selection_app();
        let interactor = app
            .world_mut()
            .spawn((
                Interactor::Controller,
                InteractorState::new(Interactor::Controller),
                LaserPointer::default(),
                GlobalTransform::IDENTITY,
            ))
            .id();
        selectable_at(&mut app, 1.0);

        press_primary(&mut app, interactor);

        app.update();

        assert!(app.world().resource::<LaserLog>().0.is_empty());
    }

    #[test]
    fn primary_press_ignores_targets_past_laser_length() {
        let mut app = selection_app();
        let interactor = app
            .world_mut()
            .spawn((
                Interactor::Controller,
                InteractorState::new(Interactor::Controller),
                LaserPointer {
                    length: 0.5,
                    ..Default::default()
                },
                GlobalTransform::IDENTITY,
            ))
            .id();
        selectable_at(&mut app, -1.0);

        press_primary(&mut app, interactor);

        app.update();

        assert!(app.world().resource::<LaserLog>().0.is_empty());
    }

    #[test]
    fn hit_test_runs_without_press_and_shortens_laser_visual() {
        let mut app = selection_app();
        let interactor = app
            .world_mut()
            .spawn((
                Interactor::Controller,
                InteractorState::new(Interactor::Controller),
                LaserPointer::default(),
                GlobalTransform::IDENTITY,
            ))
            .id();
        let target = selectable_at(&mut app, -1.0);

        app.update();

        let hit = app
            .world()
            .entity(interactor)
            .get::<LaserPointerHit>()
            .expect("laser should cache its current hit");
        assert_eq!(hit.entity, target);
        assert!((hit.distance - 0.9).abs() < 0.001);

        let visual = app
            .world()
            .entity(interactor)
            .get::<LaserPointerVisual>()
            .expect("laser pointer visual should be tracked");
        let transform = app
            .world()
            .entity(visual.child)
            .get::<Transform>()
            .expect("laser visual should have a transform");
        assert!((transform.translation.z + 0.45).abs() < 0.001);
        assert!((transform.scale.y - 0.9).abs() < 0.001);
    }

    #[test]
    fn current_hit_gets_red_wireframe_highlight() {
        let mut app = selection_app();
        app.world_mut().spawn((
            Interactor::Controller,
            InteractorState::new(Interactor::Controller),
            LaserPointer::default(),
            GlobalTransform::IDENTITY,
        ));
        let target = selectable_at(&mut app, -1.0);

        app.update();

        let highlight = app
            .world()
            .entity(target)
            .get::<LaserHighlightVisual>()
            .expect("hit target should get a highlight");
        let material = app
            .world()
            .resource::<Assets<StandardMaterial>>()
            .get(highlight.material.id())
            .expect("highlight material should exist");
        assert_eq!(material.base_color, Color::linear_rgb(1.0, 0.0, 0.0));

        let edge_count = app
            .world_mut()
            .query_filtered::<&Mesh3d, With<LaserHighlightVisualChild>>()
            .iter(app.world())
            .count();
        assert_eq!(edge_count, 12);
    }

    #[test]
    fn selected_highlight_turns_white_when_event_is_sent() {
        let mut app = selection_app();
        let interactor = app
            .world_mut()
            .spawn((
                Interactor::Controller,
                InteractorState::new(Interactor::Controller),
                LaserPointer::default(),
                GlobalTransform::IDENTITY,
            ))
            .id();
        let target = selectable_at(&mut app, -1.0);

        app.update();
        press_primary(&mut app, interactor);
        app.update();

        assert_eq!(
            app.world().resource::<LaserLog>().0,
            vec![LaserSelected {
                entity: target,
                interactor,
            }]
        );

        let highlight = app
            .world()
            .entity(target)
            .get::<LaserHighlightVisual>()
            .expect("selected target should still be highlighted");
        let material = app
            .world()
            .resource::<Assets<StandardMaterial>>()
            .get(highlight.material.id())
            .expect("highlight material should exist");
        assert_eq!(material.base_color, Color::WHITE);
    }

    #[test]
    fn highlight_is_removed_when_target_is_no_longer_hit() {
        let mut app = selection_app();
        let interactor = app
            .world_mut()
            .spawn((
                Interactor::Controller,
                InteractorState::new(Interactor::Controller),
                LaserPointer::default(),
                GlobalTransform::IDENTITY,
            ))
            .id();
        let target = selectable_at(&mut app, -1.0);

        app.update();
        assert!(
            app.world()
                .entity(target)
                .contains::<LaserHighlightVisual>()
        );

        app.world_mut()
            .entity_mut(interactor)
            .insert(GlobalTransform::from(Transform::from_xyz(5.0, 0.0, 0.0)));
        app.update();

        assert!(
            !app.world()
                .entity(target)
                .contains::<LaserHighlightVisual>()
        );
        assert!(!app.world().entity(interactor).contains::<LaserPointerHit>());
    }

    #[test]
    fn adding_laser_pointer_attaches_visual_child() {
        let mut app = selection_app();
        let interactor = app
            .world_mut()
            .spawn((
                Interactor::Controller,
                InteractorState::new(Interactor::Controller),
                LaserPointer::default(),
                GlobalTransform::IDENTITY,
            ))
            .id();

        app.update();

        let visual = app
            .world()
            .entity(interactor)
            .get::<LaserPointerVisual>()
            .expect("laser pointer visual should be tracked");
        assert!(
            app.world()
                .entity(visual.child)
                .contains::<LaserPointerVisualChild>()
        );
    }
}
