use bevy::{mesh::SphereMeshBuilder, prelude::*};
use rand::RngExt;
use tephrite_rs::prelude::*;

const SPHERE_COUNT: usize = 100_000;
const ARM_COUNT: usize = 2;
const GALAXY_RADIUS: f32 = 4.0;
const CORE_RADIUS: f32 = 0.45;
const INITIAL_SCATTER_RADIUS: f32 = 2.5;
const RADIAL_SETTLE_RATE: f32 = 0.28;
const HEIGHT_SETTLE_RATE: f32 = 0.45;
const MIN_ORBIT_SPEED: f32 = 0.08;
const MAX_ORBIT_SPEED: f32 = 0.34;
const STAR_RADIUS: f32 = 0.035;

struct MyPlugin;

impl Plugin for MyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup);
        app.add_systems(Update, update);
    }
}

impl TephriteApp for MyPlugin {}

#[derive(Debug, Component)]
struct OrbitingInstances {
    spheres: Vec<OrbitingSphere>,
}

#[derive(Debug)]
struct OrbitingSphere {
    angular_speed: f32,
    target_radius: f32,
    target_height: f32,
}

const STAR_COLORS: [bevy::prelude::LinearRgba; 4] = [
    LinearRgba::new(1.0, 1.0, 1.0, 1.0),
    LinearRgba::new(0.0, 0.86, 0.0, 1.0),
    LinearRgba::new(0.0, 0.0, 0.98, 1.0),
    LinearRgba::new(0.9, 0.72, 1.0, 1.0),
];
const QUADRANT_COLORS: [bevy::prelude::LinearRgba; 4] = [
    LinearRgba::new(1.0, 0.95, 0.72, 1.0),
    LinearRgba::new(0.45, 0.95, 1.0, 1.0),
    LinearRgba::new(1.0, 0.5, 0.78, 1.0),
    LinearRgba::new(0.55, 1.0, 0.5, 1.0),
];

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mesh = meshes.add(SphereMeshBuilder {
        sphere: Sphere::new(1.0),
        kind: bevy::mesh::SphereKind::Ico { subdivisions: 1 },
    });
    let material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        ..default()
    });

    let mut rng = rand::rng();
    let mut instances = Vec::with_capacity(SPHERE_COUNT);
    let mut spheres = Vec::with_capacity(SPHERE_COUNT);

    for _ in 0..SPHERE_COUNT {
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

        let color_index = if is_core {
            rng.random_range(0..2)
        } else {
            rng.random_range(0..STAR_COLORS.len())
        };

        instances.push(Instance::new(
            translation,
            Quat::IDENTITY,
            Vec3::splat(STAR_RADIUS) * rng.random_range(0.5..1.0),
            STAR_COLORS[color_index],
        ));
        spheres.push(OrbitingSphere {
            angular_speed,
            target_radius,
            target_height,
        });
    }

    commands.spawn((
        Mesh3d(mesh),
        InstanceMeshMaterial3d(material),
        Instances::new(instances),
        OrbitingInstances { spheres },
        Transform::default(),
        Visibility::Visible,
    ));

    commands.spawn((
        DirectionalLight {
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(0.0, 5.0, 3.0).looking_at((0.0, 0.0, 0.0).into(), Dir3::Y),
    ));
}

fn update(mut query: Query<(&mut Instances, &OrbitingInstances)>, time: Res<Time>) {
    let dt = time.delta_secs();

    for (mut instances, orbits) in &mut query {
        for (instance, sphere) in instances.instances_mut().iter_mut().zip(&orbits.spheres) {
            let orbit = Quat::from_rotation_y(sphere.angular_speed * dt);
            let mut pos = orbit * instance.pos.xyz();

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

            instance.pos.x = pos.x;
            instance.pos.y = pos.y;
            instance.pos.z = pos.z;

            instance.set_color(color_for_orbit_quadrant(pos));
        }
    }
}

fn color_for_orbit_quadrant(pos: Vec3) -> LinearRgba {
    let angle = pos.z.atan2(pos.x).rem_euclid(std::f32::consts::TAU);
    let quadrant = (angle / std::f32::consts::FRAC_PI_2).floor() as usize;
    QUADRANT_COLORS[quadrant.min(QUADRANT_COLORS.len() - 1)]
}

fn main() {
    tephrite_rs::run(MyPlugin);
}
