use bevy::prelude::*;
use tephrite_rs::prelude::*;

struct MyPlugin;

impl Plugin for MyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup);
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(2.0, 0.1, 2.0))),
        MeshMaterial3d(materials.add(Color::srgb_u8(220, 220, 220))),
        Transform::from_xyz(0.0, -0.05, 0.0),
        Replicated,
    ));

    let cube_mesh = meshes.add(Cuboid::from_length(0.15));
    let cube_material = materials.add(Color::srgb_u8(80, 150, 240));

    let grid_root = commands
        .spawn((
            Transform::from_xyz(0.0, 1.0, 0.0),
            PropagateReplication::default(),
        ))
        .id();

    for x in -2..=2 {
        for z in -2..=2 {
            commands.spawn((
                Mesh3d(cube_mesh.clone()),
                MeshMaterial3d(cube_material.clone()),
                Transform::from_xyz(x as f32 * 0.35, 0.0, z as f32 * 0.35),
                ChildOf(grid_root),
            ));
        }
    }

    commands.spawn((
        DirectionalLight {
            shadows_enabled: true,
            illuminance: 20_000.0,
            ..default()
        },
        Transform::from_xyz(0.0, 4.0, 0.0).looking_at(Vec3::ZERO, Dir3::Z),
        Replicated,
    ));
}

fn main() {
    tephrite_rs::run(MyPlugin);
}
