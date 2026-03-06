use bevy::prelude::*;
use tephrite_rs::prelude::*;
use tephrite_rs::ui::prelude::*;

struct MyPlugin;

impl Plugin for MyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup);
    }
}

/// set up a simple 3D scene
fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) -> Result<()> {
    // circular base
    commands.spawn((
        Mesh3d(meshes.add(Circle::new(4.0))),
        MeshMaterial3d(materials.add(Color::WHITE)),
        Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
        Replicated,
    ));

    // label

    let mut baker = CpuTextBaker::new();

    commands.spawn((
        make_label(
            &mut baker,
            "Hello",
            TextStyle {
                font_size: 64.0,
                ..Default::default()
            },
            &mut images,
            &mut meshes,
            &mut materials,
        )?,
        Replicated,
        Transform::from_xyz(0.0, 1.0, 0.0), //.with_rotation(Quat::from_rotation_y(f32::consts::PI)),
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

    Ok(())
}

fn main() {
    tephrite_rs::run(MyPlugin);
}
