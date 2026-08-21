use bevy::prelude::*;
use tephrite_rs::prelude::*;

struct MyPlugin;

impl Plugin for MyPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(NavigationPlugin::new(NavigatorMode::ObjectCentric));
        app.add_systems(Startup, mesh_scene.spawn());
    }
}

impl tephrite_rs::TephriteApp for MyPlugin {}

fn mesh_scene() -> impl Scene {
    bsn! {
        #MeshExampleRoot
        Transform::default()
        Visibility::Inherited
        NavigatorMarker
        Children [
            (
                #CircularBase
                Mesh3d(asset_value(Circle::new(4.0)))
                MeshMaterial3d::<StandardMaterial>(asset_value(Color::WHITE))
                Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))
            ),
            (
                #CubeCluster
                Transform::from_xyz(0.0, 0.0, -1.0)
                Visibility::Inherited
                Children [
                    (
                        #RedCube
                        Mesh3d(asset_value(Cuboid::new(0.1, 0.1, 0.1)))
                        MeshMaterial3d::<StandardMaterial>(asset_value(Color::srgb_u8(255, 0, 0)))
                        Transform::from_xyz(0.2, 1.0, 0.0)
                    ),
                    (
                        #MetallicGreenCube
                        Mesh3d(asset_value(Cuboid::new(0.1, 0.1, 0.1)))
                        MeshMaterial3d::<StandardMaterial>(asset_value(StandardMaterial {
                            base_color: Color::srgb_u8(0, 255, 0),
                            metallic: 1.0,
                            perceptual_roughness: 0.1,
                            ..default()
                        }))
                        Transform::from_xyz(0.0, 1.2, 0.0)
                    ),
                    (
                        #BlueCube
                        Mesh3d(asset_value(Cuboid::new(0.1, 0.1, 0.1)))
                        MeshMaterial3d::<StandardMaterial>(asset_value(Color::srgb_u8(0, 0, 255)))
                        Transform::from_xyz(0.0, 1.0, 0.2)
                    ),
                ]
            ),
            (
                #KeyLight
                DirectionalLight {
                    shadow_maps_enabled: true,
                }
                template_value(Transform::from_xyz(0.0, 5.0, 3.0).looking_at(Vec3::ZERO, Dir3::Y))
            ),
        ]
    }
}

fn main() {
    tephrite_rs::run(MyPlugin);
}
