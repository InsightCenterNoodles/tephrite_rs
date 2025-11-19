use bevy::{asset::RenderAssetUsages, image::CompressedImageFormats, prelude::*};
use tephrite_rs::prelude::*;

struct MyPlugin;

impl Plugin for MyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup);
    }
}

// For the moment, the normal way of using
// server: Res<AssetServer>
// is busted. Workarounds ahoy

fn image_from_file(path: &std::path::Path, format: ImageFormat, is_srgb: bool) -> Option<Image> {
    let file = std::fs::read(path).ok()?;

    Image::from_buffer(
        &file,
        bevy::image::ImageType::Format(format),
        CompressedImageFormats::all(),
        is_srgb,
        bevy::image::ImageSampler::Default,
        RenderAssetUsages::all(),
    )
    .ok()
}

/// set up a simple 3D scene
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let ground_color = image_from_file(
        std::path::Path::new("assets/tex/MetalPlates006_1K-JPG_Color.jpg"),
        ImageFormat::Jpeg,
        true,
    )
    .expect("missing ground color");
    let ground_normal = image_from_file(
        std::path::Path::new("assets/tex/MetalPlates006_1K-JPG_NormalGL.jpg"),
        ImageFormat::Jpeg,
        true,
    )
    .expect("missing ground normal");
    let ground_rm = image_from_file(
        std::path::Path::new("assets/tex/MetalPlates006_1K-JPG_RM.png"),
        ImageFormat::Png,
        true,
    )
    .expect("missing ground roughness/metallic");

    let ground_mat = StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(images.add(ground_color)),
        metallic: 1.0,
        perceptual_roughness: 1.0,
        metallic_roughness_texture: Some(images.add(ground_rm)),
        normal_map_texture: Some(images.add(ground_normal)),
        ..Default::default()
    };

    // circular base
    commands.spawn((
        Mesh3d(meshes.add(Circle::new(4.0))),
        MeshMaterial3d(materials.add(ground_mat)),
        Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
        Replicated,
    ));
    // cubes

    let mesh = meshes.add(Cuboid::new(0.1, 0.1, 0.1));

    let e = commands
        .spawn((
            Transform::from_xyz(0.0, 0.0, -1.0),
            PropagateReplication::default(),
        ))
        .id();

    commands.spawn((
        Mesh3d(mesh.clone()),
        MeshMaterial3d(materials.add(Color::srgb_u8(255, 0, 0))),
        Transform::from_xyz(0.2, 1.0, 0.0),
        ChildOf(e),
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
        Transform::from_xyz(0.0, 1.2, 0.0),
        ChildOf(e),
    ));

    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(materials.add(Color::srgb_u8(0, 0, 255))),
        Transform::from_xyz(0.0, 1.0, 0.2),
        ChildOf(e),
    ));

    // light
    commands.spawn((
        DirectionalLight {
            illuminance: 1000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(0.0, 5.0, 3.0).looking_at((0.0, 0.0, 0.0).into(), Dir3::Y),
        Replicated,
    ));

    // Hack to get around busted asset loading

    let env_map = image_from_file(
        std::path::Path::new("assets/ibl/workshop_4k_small.exr"),
        ImageFormat::OpenExr,
        false,
    )
    .expect("missing IBL image");

    let env_map = images.add(env_map);

    commands.insert_resource(EnvironmentLighting {
        intensity: 10000.0,
        equirect: env_map,
    });

    // camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(-2.5, 4.5, 9.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn main() {
    tephrite_rs::run(MyPlugin);
}
