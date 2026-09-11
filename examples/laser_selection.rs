//! Laser selection example.
//!
//! Automatically add a laser pointer to Tephrite's built-in interactor and
//! spawn several selectable shapes. Press the primary interactor button to
//! print the currently highlighted shape.

use bevy::{math::bounding::Aabb3d, prelude::*};
use tephrite_rs::prelude::*;

struct MyPlugin;

impl Plugin for MyPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(LaserSelectionPlugin);
        app.insert_resource(AutoLaserPointer(LaserPointer {
            length: 2.5,
            ..Default::default()
        }));
        app.add_systems(Startup, setup);
        app.add_systems(Update, expire_laser_hit_lights);
        app.add_observer(print_laser_selection);
    }
}

impl tephrite_rs::TephriteApp for MyPlugin {}

#[derive(Component)]
struct LaserHitLight {
    timer: Timer,
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 1.35, 3.0).looking_at(Vec3::new(0.0, 0.35, 0.0), Dir3::Y),
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 4000.0,
            shadows_enabled: true,
            ..Default::default()
        },
        Transform::from_xyz(0.0, 4.0, 2.0).looking_at(Vec3::ZERO, Dir3::Y),
    ));

    commands.spawn((
        Mesh3d(meshes.add(Circle::new(1.5))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.18, 0.18, 0.18),
            perceptual_roughness: 0.9,
            ..Default::default()
        })),
        Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
    ));

    spawn_selectable(
        &mut commands,
        meshes.add(Sphere::new(0.16)),
        materials.add(Color::srgb(0.95, 0.15, 0.12)),
        "Sphere",
        Vec3::new(-0.55, 0.16, -0.45),
        Vec3::splat(0.16),
    );

    spawn_selectable(
        &mut commands,
        meshes.add(Cuboid::from_length(0.28)),
        materials.add(Color::srgb(0.1, 0.55, 0.95)),
        "Cube",
        Vec3::new(0.0, 0.14, -0.65),
        Vec3::splat(0.14),
    );

    spawn_selectable(
        &mut commands,
        meshes.add(Capsule3d::new(0.09, 0.24)),
        materials.add(Color::srgb(0.2, 0.85, 0.35)),
        "Capsule",
        Vec3::new(0.55, 0.21, -0.35),
        Vec3::new(0.09, 0.21, 0.09),
    );

    spawn_selectable(
        &mut commands,
        meshes.add(Cone::new(0.15, 0.32)),
        materials.add(Color::srgb(1.0, 0.75, 0.18)),
        "Cone",
        Vec3::new(-0.35, 0.16, 0.25),
        Vec3::new(0.15, 0.16, 0.15),
    );

    spawn_selectable(
        &mut commands,
        meshes.add(Torus::new(0.09, 0.24)),
        materials.add(Color::srgb(0.7, 0.35, 0.95)),
        "Torus",
        Vec3::new(0.45, 0.075, 0.25),
        Vec3::new(0.24, 0.075, 0.24),
    );
}

fn spawn_selectable(
    commands: &mut Commands,
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    name: &'static str,
    translation: Vec3,
    half_extents: Vec3,
) {
    commands.spawn((
        Name::new(name),
        LaserSelectable,
        InteractionBounds::aabb(Aabb3d::new(Vec3A::ZERO, half_extents)),
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::from_translation(translation),
    ));
}

fn print_laser_selection(
    trigger: On<LaserSelected>,
    names: Query<&Name>,
    transforms: Query<&GlobalTransform>,
    mut commands: Commands,
) {
    let event = trigger.event();
    if let Ok(name) = names.get(event.entity) {
        println!("Laser hit: {}", name.as_str());
    } else {
        println!("Laser hit: {:?}", event.entity);
    }

    let Ok(transform) = transforms.get(event.entity) else {
        return;
    };

    commands.spawn((
        Name::new("Laser Hit Light"),
        LaserHitLight {
            timer: Timer::from_seconds(3.0, TimerMode::Once),
        },
        PointLight {
            color: Color::WHITE,
            intensity: 10000.0,
            range: 1.0,
            shadows_enabled: false,
            ..Default::default()
        },
        Transform::from_translation(transform.translation() + Vec3::Y * 0.5),
    ));
}

fn expire_laser_hit_lights(
    mut commands: Commands,
    time: Res<Time>,
    mut lights: Query<(Entity, &mut LaserHitLight)>,
) {
    for (entity, mut light) in &mut lights {
        light.timer.tick(time.delta());
        if light.timer.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

fn main() {
    tephrite_rs::run(MyPlugin);
}
