use bevy::{
    image::{
        ImageAddressMode, ImageFilterMode, ImageLoaderSettings, ImageSampler,
        ImageSamplerDescriptor,
    },
    mesh::VertexAttributeValues,
    prelude::*,
};
use tephrite_rs::prelude::*;

const PIPE_RADIUS: f32 = 0.06;
const CHEVRON_WORLD_PERIOD: f32 = 0.45;
const FLOW_SPEED: f32 = 0.55;

struct MyPlugin;

impl Plugin for MyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup);
        app.add_systems(Update, update_flow);
    }
}

impl TephriteApp for MyPlugin {}

#[derive(Component)]
struct FlowPath {
    segments: Vec<FlowSegment>,
}

struct FlowSegment {
    length: f32,
    repeats: f32,
    phase: f32,
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let chevron_texture: Handle<Image> = asset_server
        .load_builder()
        .with_settings(|settings: &mut ImageLoaderSettings| {
            settings.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                address_mode_u: ImageAddressMode::Repeat,
                address_mode_v: ImageAddressMode::Repeat,
                mag_filter: ImageFilterMode::Linear,
                min_filter: ImageFilterMode::Linear,
                mipmap_filter: ImageFilterMode::Linear,
                anisotropy_clamp: 8,
                ..default()
            });
        })
        .load("tex/chevron.png");

    let material = materials.add(StandardMaterial {
        base_color_texture: Some(chevron_texture),
        base_color: Color::WHITE,
        perceptual_roughness: 0.68,
        metallic: 0.0,
        ..default()
    });

    let mut cyl_mesh: Mesh = Cylinder::new(1.0, 1.0).mesh().resolution(32).into();

    if let Some(VertexAttributeValues::Float32x2(attrib)) =
        cyl_mesh.attribute_mut(Mesh::ATTRIBUTE_UV_0)
    {
        info!("Swapping UVs");
        for uv in attrib {
            uv.swap(0, 1);
        }
    }

    let cylinder = meshes.add(cyl_mesh);
    let points = path_points();
    let mut instances = Vec::with_capacity(points.len().saturating_sub(1));
    let mut segments = Vec::with_capacity(points.len().saturating_sub(1));

    for (index, pair) in points.windows(2).enumerate() {
        let start = pair[0];
        let end = pair[1];
        let delta = end - start;
        let length = delta.length();
        if length <= f32::EPSILON {
            continue;
        }

        let direction = delta / length;
        let midpoint = start + delta * 0.5;
        let rotation = Quat::from_rotation_arc(Vec3::Y, direction);
        let repeats = (length / CHEVRON_WORLD_PERIOD).max(1.0);
        let phase = index as f32 * 0.17;

        instances.push(
            Instance::new(
                midpoint,
                rotation,
                Vec3::new(PIPE_RADIUS, length, PIPE_RADIUS),
                LinearRgba::WHITE,
            )
            .with_texture_transform(Vec2::new(0.0, phase), Vec2::new(2.0, repeats)),
        );
        segments.push(FlowSegment {
            length,
            repeats,
            phase,
        });
    }

    commands.spawn((
        Mesh3d(cylinder),
        InstanceMeshMaterial3d(material.clone()),
        Instances::new(instances),
        FlowPath { segments },
        Transform::default(),
        Visibility::Visible,
    ));

    commands.spawn((
        Mesh3d(meshes.add(Cuboid::from_length(0.55))),
        MeshMaterial3d(material),
        Transform::from_xyz(-1.6, 0.25, 0.0),
    ));

    commands.spawn((
        PointLight {
            intensity: 900.0,
            range: 8.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(0.4, 2.0, 2.0),
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 9_000.0,
            ..default()
        },
        Transform::from_xyz(-1.5, 3.0, 2.0).looking_at(Vec3::ZERO, Dir3::Y),
    ));

    commands.insert_resource(EnvironmentLighting {
        diffuse: asset_server.load("ibl/workshop_diffuse.ktx2"),
        specular: asset_server.load("ibl/workshop_specular.ktx2"),
        intensity: 5000.0,
        skybox_color: Color::srgb(0.5, 0.5, 1.0).into(),
    });
}

fn update_flow(mut query: Query<(&mut Instances, &FlowPath)>, time: Res<Time>) {
    let elapsed = time.elapsed_secs();

    for (mut instances, flow_path) in &mut query {
        for (instance, segment) in instances
            .instances_mut()
            .iter_mut()
            .zip(flow_path.segments.iter())
        {
            let offset = elapsed * FLOW_SPEED * segment.repeats / segment.length;
            instance.tex.x = (segment.phase - offset).rem_euclid(1.0);
        }
    }
}

fn path_points() -> Vec<Vec3> {
    (0..18)
        .map(|i| {
            let t = i as f32 / 17.0;
            let angle = t * std::f32::consts::TAU * 1.35;
            let radius = 0.45 + t * 1.45;
            Vec3::new(
                angle.cos() * radius,
                (t - 0.5) * 0.75 + (angle * 1.7).sin() * 0.12,
                angle.sin() * radius,
            )
        })
        .collect()
}

fn main() {
    tephrite_rs::run(MyPlugin);
}
