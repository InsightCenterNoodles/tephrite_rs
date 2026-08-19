use bevy::{asset::AssetLoadFailedEvent, prelude::*};
use tephrite_rs::prelude::*;

use bevy::camera_controller::free_camera::{FreeCamera, FreeCameraPlugin};

struct MyPlugin;

impl Plugin for MyPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(KnownScenes::default());
        app.add_systems(Startup, setup);

        app.add_observer(on_button);

        app.add_plugins(NavigationPlugin::new(NavigatorMode::ObjectCentric));

        app.add_systems(Update, world_load_errors);
    }
}

impl tephrite_rs::TephriteApp for MyPlugin {
    fn non_tephrite_plugin() -> impl Plugin {
        non_tephrite_content
    }
}

fn non_tephrite_content(app: &mut App) {
    app.add_plugins(FreeCameraPlugin);
    app.add_systems(Startup, setup_non_teph);
}

fn setup_non_teph(mut commands: Commands) {
    info!("Basic non-Teph scene setup");

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 2.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
        FreeCamera {
            sensitivity: 0.2,
            friction: 25.0,
            walk_speed: 3.0,
            run_speed: 9.0,
            ..default()
        },
    ));
}

fn setup(
    mut commands: Commands,
    server: Res<AssetServer>,
    mut known: ResMut<KnownScenes>,
    //mut gltf_scenes: ResMut<GltfSceneAssets>,
) {
    info!("Basic Teph scene setup");

    // light
    commands.spawn((
        DirectionalLight {
            color: Color::srgb_u8(255, 224, 141),
            shadow_maps_enabled: true,
            illuminance: 5000.0,
            ..default()
        },
        Transform::from_xyz(4.0, 4.0, 3.0).looking_at((0.0, 0.0, 0.0).into(), Dir3::Y),
    ));

    commands.insert_resource(EnvironmentLighting {
        diffuse: server.load("ibl/workshop_diffuse.ktx2"),
        specular: server.load("ibl/workshop_specular.ktx2"),
        intensity: 5000.0,
        skybox_color: Color::srgb(0.5, 0.5, 1.0).into(),
    });

    let root = commands
        .spawn((Transform::default(), NavigatorMarker, Visibility::Inherited))
        .id();

    let mut iter = std::env::args();
    while let Some(arg) = iter.next() {
        if arg != "-m" {
            continue;
        }

        if let Some(val) = iter.next() {
            let vis = if known.vec.is_empty() {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };

            info!("Loading from: {val}");

            let id = commands
                .spawn((
                    //WorldAssetRoot(gltf_scenes.load_scene(&server, val, 0)),
                    WorldAssetRoot(
                        server
                            .load_builder()
                            .override_unapproved()
                            .load(GltfAssetLabel::Scene(0).from_asset(val)),
                    ),
                    ChildOf(root),
                    vis,
                ))
                .id();
            known.vec.push(id);
        }
    }
}

#[derive(Debug, Default, Resource)]
struct KnownScenes {
    vec: Vec<Entity>,
    current: usize,
}

fn on_button(trigger: On<GlobalActivate>, mut known: ResMut<KnownScenes>, mut commands: Commands) {
    if known.vec.is_empty() {
        return;
    }

    let mut new = known.current;

    let current_len = known.vec.len();

    let Some(translated) = Controller::reverse_translate_button(trigger.button) else {
        return;
    };

    match translated {
        ControllerButton::TL => {
            new = (new + current_len - 1) % current_len;
        }
        ControllerButton::TR => {
            new = (new + 1) % current_len;
        }
        _ => {}
    }

    if new == known.current {
        return;
    }

    if let Some(e) = known.vec.get(known.current) {
        commands.entity(*e).insert(Visibility::Hidden);
    }

    if let Some(e) = known.vec.get(new) {
        commands.entity(*e).insert(Visibility::Visible);
    }

    known.current = new;
}

fn main() {
    tephrite_rs::run(MyPlugin);
}

fn world_load_errors(mut failures: MessageReader<AssetLoadFailedEvent<WorldAsset>>) {
    for failure in failures.read() {
        error!(
            "Failed to load world '{}':\n{:#?}",
            failure.path, failure.error,
        );
    }
}
