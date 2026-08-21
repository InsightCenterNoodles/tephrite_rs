use bevy::{light::CascadeShadowConfigBuilder, prelude::*};
use tephrite_rs::prelude::*;

struct MyPlugin;

impl Plugin for MyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup);
    }
}

impl TephriteApp for MyPlugin {}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(3.0, 0.05, 3.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb_u8(210, 210, 210),
            perceptual_roughness: 0.75,
            ..default()
        })),
        Transform::from_xyz(0.0, -0.025, 0.0),
    ));

    let cube_mesh = meshes.add(Cuboid::from_length(1.0));
    let cube_material = materials.add(StandardMaterial {
        base_color: Color::srgb_u8(240, 110, 70),
        perceptual_roughness: 0.55,
        ..default()
    });
    let control_material = materials.add(StandardMaterial {
        base_color: Color::srgb_u8(90, 170, 240),
        perceptual_roughness: 0.55,
        ..default()
    });

    let light_position = Vec3::new(2.0, 4.0, 2.0);
    let light_direction = (Vec3::ZERO - light_position).normalize();
    let shadow_line = Vec3::new(light_direction.x, 0.0, light_direction.z).normalize();
    let self_shadow_start = Vec3::new(0.7, 1.2, 0.7);
    let self_shadow_spacing = 0.4;

    commands.spawn((
        Mesh3d(cube_mesh.clone()),
        MeshMaterial3d(control_material),
        Transform::from_translation(Vec3::new(-0.95, 0.3, 0.65)).with_scale(Vec3::splat(0.28)),
    ));

    commands.spawn((
        Mesh3d(cube_mesh),
        InstanceMeshMaterial3d(cube_material),
        Instances::new([
            Instance::new(
                self_shadow_start,
                Quat::IDENTITY,
                Vec3::splat(0.28),
                LinearRgba::new(1.0, 0.88, 0.72, 1.0),
            ),
            Instance::new(
                self_shadow_start + light_direction * self_shadow_spacing,
                Quat::IDENTITY,
                Vec3::splat(0.28),
                LinearRgba::new(0.75, 0.9, 1.0, 1.0),
            ),
            Instance::new(
                self_shadow_start + light_direction * self_shadow_spacing * 2.0,
                Quat::IDENTITY,
                Vec3::splat(0.28),
                LinearRgba::new(0.72, 1.0, 0.78, 1.0),
            ),
            Instance::new(
                self_shadow_start + light_direction * self_shadow_spacing * 3.0,
                Quat::IDENTITY,
                Vec3::splat(0.28),
                LinearRgba::new(1.0, 0.72, 0.92, 1.0),
            ),
            Instance::new(
                -shadow_line * 0.75 + Vec3::Y * 0.3,
                Quat::IDENTITY,
                Vec3::splat(0.28),
                LinearRgba::WHITE,
            ),
            Instance::new(
                shadow_line * 0.85 + Vec3::Y * 0.44,
                Quat::IDENTITY,
                Vec3::splat(0.28),
                LinearRgba::new(0.78, 0.82, 1.0, 1.0),
            ),
        ]),
        Transform::default(),
        Visibility::Visible,
    ));

    commands.spawn((
        DirectionalLight {
            shadow_maps_enabled: true,
            illuminance: 25_000.0,
            ..default()
        },
        Transform::from_translation(light_position).looking_at(Vec3::ZERO, Dir3::Y),
        CascadeShadowConfigBuilder {
            num_cascades: 1,
            minimum_distance: 0.05,
            maximum_distance: 8.0,
            ..default()
        }
        .build(),
    ));
}

fn main() {
    tephrite_rs::run(MyPlugin);
}
