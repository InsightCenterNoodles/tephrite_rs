use bevy::{image::ImageLoaderSettings, prelude::*};
use tephrite_rs::prelude::*;

struct MyPlugin;

impl Plugin for MyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup);
        app.add_systems(Update, blink);

        app.add_plugins(NavigationPlugin::new(NavigatorMode::ObjectCentric));
    }
}

impl tephrite_rs::TephriteApp for MyPlugin {}

/// set up a simple 3D scene
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    server: Res<AssetServer>,
) {
    let color_settings = |x: &mut ImageLoaderSettings| {
        x.sampler = bevy::image::ImageSampler::linear();
    };
    let linear_settings = |x: &mut ImageLoaderSettings| {
        x.sampler = bevy::image::ImageSampler::linear();
        // PBR data textures (normal, roughness/metallic, AO, etc.) must be sampled in linear space.
        x.is_srgb = false;
    };

    let ground_color =
        server.load_with_settings("tex/MetalPlates006_1K-JPG_Color.jpg", color_settings);
    let ground_normal =
        server.load_with_settings("tex/MetalPlates006_1K-JPG_NormalGL.jpg", linear_settings);
    let ground_roughmet =
        server.load_with_settings("tex/MetalPlates006_1K-JPG_RM.png", linear_settings);

    let ground_mat = StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(ground_color),
        metallic: 1.0,
        perceptual_roughness: 1.0,

        metallic_roughness_texture: Some(ground_roughmet),

        normal_map_texture: Some(ground_normal),
        ..Default::default()
    };

    let mut ground_mesh = Mesh::from(Circle::new(4.0));
    if let Err(err) = ground_mesh.generate_tangents() {
        warn!("Failed to generate tangents for ground mesh: {err}");
    }

    // circular base
    commands.spawn((
        Mesh3d(meshes.add(ground_mesh)),
        MeshMaterial3d(materials.add(ground_mat)),
        Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
    ));
    // cubes

    let mesh = meshes.add(Cuboid::new(0.1, 0.1, 0.1));

    let root = commands
        .spawn((Transform::from_xyz(0.0, 0.0, -1.0), NavigatorMarker))
        .id();

    commands.spawn((
        Mesh3d(mesh.clone()),
        MeshMaterial3d(materials.add(Color::srgb_u8(255, 0, 0))),
        Transform::from_xyz(0.2, 1.0, 0.0),
        ChildOf(root),
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
        ChildOf(root),
        Blinker,
        Visibility::Visible,
    ));

    let translucent = materials.add(StandardMaterial {
        base_color: Color::srgb_u8(0, 0, 255),
        metallic: 0.0,
        perceptual_roughness: 0.1,
        diffuse_transmission: 1.0,
        ..Default::default()
    });

    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(translucent),
        Transform::from_xyz(0.0, 1.0, 0.2),
        ChildOf(root),
    ));

    // light
    commands.spawn((
        DirectionalLight {
            illuminance: 5000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(0.0, 5.0, 3.0).looking_at((0.0, 0.0, 0.0).into(), Dir3::Y),
    ));

    commands.insert_resource(EnvironmentLighting {
        diffuse: server.load("ibl/workshop_diffuse.ktx2"),
        specular: server.load("ibl/workshop_specular.ktx2"),
        intensity: 5000.0,
        skybox_color: Color::srgb(0.5, 0.5, 1.0).into(),
    });
}

#[derive(Debug, Component)]
struct Blinker;

// Show visibility
fn blink(time: Res<Time>, mut local: Local<f32>, query: Query<&mut Visibility, With<Blinker>>) {
    *local -= time.delta_secs();

    if *local > 0.0 {
        return;
    }

    *local = 2.5;

    for mut q in query {
        debug!("Blink!");
        q.toggle_visible_hidden();
    }
}

fn main() {
    tephrite_rs::run(MyPlugin);
}
