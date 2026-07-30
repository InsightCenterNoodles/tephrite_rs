use bevy::{mesh::SphereMeshBuilder, prelude::*};
use rand::RngExt;

const SPHERE_COUNT: usize = 100_000;
const ARM_COUNT: usize = 2;
const GALAXY_RADIUS: f32 = 4.0;
const CORE_RADIUS: f32 = 0.45;
const INITIAL_SCATTER_RADIUS: f32 = 2.5;
const RADIAL_SETTLE_RATE: f32 = 0.28;
const HEIGHT_SETTLE_RATE: f32 = 0.45;
const MIN_ORBIT_SPEED: f32 = 0.08;
const MAX_ORBIT_SPEED: f32 = 0.34;

struct MyPlugin;

impl Plugin for MyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup);
        app.add_systems(Update, update);
    }
}

impl tephrite_rs::TephriteApp for MyPlugin {}

#[derive(Debug, Component)]
struct OrbitingSphere {
    angular_speed: f32,
    target_radius: f32,
    target_height: f32,
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let star_materials = [
        materials.add(Color::srgb(1.0, 0.96, 0.82)),
        materials.add(Color::srgb(0.72, 0.86, 1.0)),
        materials.add(Color::srgb(1.0, 0.82, 0.58)),
        materials.add(Color::srgb(0.9, 0.72, 1.0)),
    ];

    let mesh = meshes.add(SphereMeshBuilder {
        sphere: Sphere::new(0.035),
        kind: bevy::mesh::SphereKind::Ico { subdivisions: 1 },
    });

    let mut rng = rand::rng();

    let spheres = (0..SPHERE_COUNT).map(|_| {
        let is_core = rng.random_bool(0.18);
        let target_radius = if is_core {
            rng.random::<f32>().sqrt() * CORE_RADIUS
        } else {
            CORE_RADIUS + rng.random::<f32>().sqrt() * (GALAXY_RADIUS - CORE_RADIUS)
        };
        let target_height = rng.random_range(-0.045..0.045) * (1.0 + target_radius * 0.35);

        let arm = rng.random_range(0..ARM_COUNT) as f32;
        let arm_angle = arm / ARM_COUNT as f32 * std::f32::consts::TAU;
        let winding = target_radius * 1.55;
        let angle = if is_core {
            rng.random_range(0.0..std::f32::consts::TAU)
        } else {
            arm_angle + winding + rng.random_range(-0.34..0.34)
        };

        let target = Vec3::new(
            angle.cos() * target_radius,
            target_height,
            angle.sin() * target_radius,
        );
        let scatter = Sphere::new(INITIAL_SCATTER_RADIUS).sample_interior(&mut rng);
        let translation = target + scatter;

        let angular_speed =
            MIN_ORBIT_SPEED + (MAX_ORBIT_SPEED - MIN_ORBIT_SPEED) / (1.0 + target_radius.max(0.2));

        let material_index = if is_core {
            rng.random_range(0..2)
        } else {
            rng.random_range(0..star_materials.len())
        };

        (
            MeshMaterial3d(star_materials[material_index].clone()),
            Mesh3d(mesh.clone()),
            OrbitingSphere {
                angular_speed,
                target_radius,
                target_height,
            },
            Transform::from_translation(translation),
        )
    });

    commands.spawn_batch(spheres.collect::<Vec<_>>());

    // light
    commands.spawn((
        DirectionalLight {
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(0.0, 5.0, 3.0).looking_at((0.0, 0.0, 0.0).into(), Dir3::Y),
    ));
}

fn update(sphere_q: Query<(&mut Transform, &OrbitingSphere)>, time: Res<Time>) {
    let dt = time.delta_secs();

    for (mut tf, sphere) in sphere_q {
        let orbit = Quat::from_rotation_y(sphere.angular_speed * dt);
        let mut pos = orbit * tf.translation;

        let disk_pos = Vec2::new(pos.x, pos.z);
        let radius = disk_pos.length();
        if radius > f32::EPSILON {
            let target = disk_pos.normalize() * sphere.target_radius;
            let disk_pos = disk_pos.lerp(target, 1.0 - (-RADIAL_SETTLE_RATE * dt).exp());
            pos.x = disk_pos.x;
            pos.z = disk_pos.y;
        } else {
            pos.x = sphere.target_radius;
        }
        pos.y = pos
            .y
            .lerp(sphere.target_height, 1.0 - (-HEIGHT_SETTLE_RATE * dt).exp());

        tf.translation = pos;
    }
}

fn main() {
    tephrite_rs::run(MyPlugin);
}
