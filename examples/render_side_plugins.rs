//! Spawn an entity with a custom logic component that is replicated
//! to the render side.
//!
//! Should show two counters printing at the same time

use bevy::prelude::*;
use tephrite_rs::prelude::*;

struct MyPlugin;

impl Plugin for MyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup);

        app.add_plugins(NavigationPlugin::new(NavigatorMode::ObjectCentric));
    }
}

struct DoubleSidedPlugin;

impl Plugin for DoubleSidedPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, print_check);
    }
}

#[derive(Component)]
struct DoubleSidedComponent {
    counter: u32,
}

impl FastWrite for DoubleSidedComponent {
    unsafe fn write_fast(&self, w: &mut impl ByteSink) {
        w.put_u32(self.counter);
    }
}

impl FastRead for DoubleSidedComponent {
    type Ret = Self;
    type Context = ();

    unsafe fn read_fast<'a, S: ByteSource<'a>>(_: &mut Self::Context, r: &mut S) -> Self::Ret {
        Self {
            counter: r.get_u32(),
        }
    }
}

fn print_check(q_check: Query<&mut DoubleSidedComponent>) {
    for mut c in q_check {
        c.counter += 1;
        println!("{}", c.counter);
    }
}

impl tephrite_rs::TephriteApp for MyPlugin {
    fn configure_tephrite(config: &mut TephriteAppConfig) {
        config.add_plugins(DoubleSidedPlugin);
        config.mirror_component::<DoubleSidedComponent>();
    }
}

/// set up a simple 3D scene
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let ground_mat = StandardMaterial {
        base_color: Color::WHITE,
        metallic: 1.0,
        perceptual_roughness: 1.0,
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

    // light
    commands.spawn((
        DirectionalLight {
            illuminance: 5000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(0.0, 5.0, 3.0).looking_at((0.0, 0.0, 0.0).into(), Dir3::Y),
    ));

    commands.spawn(DoubleSidedComponent { counter: 0 });
}

fn main() {
    tephrite_rs::run(MyPlugin);
}
