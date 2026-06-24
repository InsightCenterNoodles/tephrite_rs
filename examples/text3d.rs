use bevy::color::palettes::tailwind::RED_900;
use bevy::prelude::*;
use bevy_rich_text3d::{Text3d, Text3dPlugin, Text3dStyling, TextAtlas};
use tephrite_rs::prelude::*;

struct MyPlugin;

impl Plugin for MyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup);
        app.add_plugins(Text3dPlugin {
            default_atlas_dimension: (2048, 2048),
            load_system_fonts: true,
            ..Default::default()
        });
        //app.add_plugins(BillboardPlugin);
    }
}

/// set up a simple 3D scene
fn setup(
    mut commands: Commands,
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

    let mat = materials.add(StandardMaterial {
        base_color_texture: Some(TextAtlas::DEFAULT_IMAGE.clone()),
        //alpha_mode: AlphaMode::Mask(0.5),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        cull_mode: None,
        ..Default::default()
    });

    // Spawn a text item. Note that you can add a stroke to help visibility.
    commands.spawn((
        Text3d::new("Hello!"),
        Text3dStyling {
            size: 128.,
            //stroke: NonZero::new(3),
            color: RED_900,
            //stroke_color: BLACK,
            world_scale: Some(Vec2::splat(0.15)),
            //layer_offset: 0.001,
            ..Default::default()
        },
        Mesh3d::default(),
        MeshMaterial3d(mat),
        Transform::from_xyz(0.0, 0.25, 0.0),
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

    Ok(())
}

fn main() {
    tephrite_rs::run(MyPlugin);
}
