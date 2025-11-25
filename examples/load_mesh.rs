use bevy::prelude::*;
use tephrite_rs::prelude::*;

struct MyPlugin;

impl Plugin for MyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup);

        app.add_plugins(NavigationPlugin::new(NavigatorMode::ObjectCentric));
    }
}

fn setup(mut commands: Commands, server: Res<AssetServer>) {
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

    let env_map = server.load("ibl/workshop_4k_small.exr");

    commands.insert_resource(EnvironmentLighting {
        intensity: 15000.0,
        equirect: env_map,
    });

    let mut iter = std::env::args();
    while let Some(arg) = iter.next() {
        if arg != "-m" {
            continue;
        }

        if let Some(val) = iter.next() {
            commands.spawn((
                SceneRoot(server.load(GltfAssetLabel::Scene(0).from_asset(val))),
                Replicated,
                PropagateReplication::default(),
                NavigatorMarker,
            ));
        }
    }
}

fn main() {
    tephrite_rs::run(MyPlugin);
}
