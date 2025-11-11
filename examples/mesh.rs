use bevy::prelude::*;
use tephrite_rs::prelude::{Head, Replicated};

struct MyPlugin;

impl Plugin for MyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup);
        //app.add_systems(Update, reset_head);
    }
}

/// set up a simple 3D scene
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // circular base
    commands.spawn((
        Mesh3d(meshes.add(Circle::new(4.0))),
        MeshMaterial3d(materials.add(Color::WHITE)),
        Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
        Replicated,
    ));
    // cubes

    let mesh = meshes.add(Cuboid::new(0.1, 0.1, 0.1));

    //let e = commands.spawn(Transform::from_xyz(0.0, 0.5, 0.0)).id();

    //let zcenter = -0.5f32;
    let zcenter = -1.0f32;

    commands.spawn((
        Mesh3d(mesh.clone()),
        MeshMaterial3d(materials.add(Color::srgb_u8(255, 0, 0))),
        Transform::from_xyz(0.2, 1.0, zcenter),
        //ChildOf(e),
        Replicated,
    ));

    let shiny = materials.add(StandardMaterial {
        base_color: Color::srgb_u8(0, 255, 0),
        metallic: 1.0,
        perceptual_roughness: 0.1,
        ..Default::default()
    });

    commands.spawn((
        Mesh3d(mesh.clone()),
        MeshMaterial3d(shiny),
        Transform::from_xyz(0.0, 1.2, zcenter),
        //ChildOf(e),
        Replicated,
    ));

    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(materials.add(Color::srgb_u8(0, 0, 255))),
        Transform::from_xyz(0.0, 1.0, zcenter + 0.2),
        //ChildOf(e),
        Replicated,
    ));

    // light
    commands.spawn((
        DirectionalLight {
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(0.0, 5.0, 3.0).looking_at((0.0, 0.0, 0.0).into(), Dir3::Y),
        Replicated,
    ));
    // camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(-2.5, 4.5, 9.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn reset_head(
    mut query: Query<&mut Transform, With<Head>>,
    time: Res<Time>,
    mut local: Local<f32>,
) {
    *local += 0.5 * time.delta_secs();

    let new_head_x = (local).sin() * 2.0 - 1.0;

    let head_pos = vec3(new_head_x, 1.5, 2.0);
    let head_rot = Quat::default();

    for mut q in query.iter_mut() {
        q.translation = head_pos;
    }
}

fn main() {
    tephrite_rs::run(MyPlugin);
}
