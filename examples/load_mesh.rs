use bevy::prelude::*;
use tephrite_rs::prelude::*;

struct MyPlugin;

impl Plugin for MyPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(KnownScenes::default());
        app.add_systems(Startup, setup);

        app.add_observer(on_button);

        app.add_plugins(NavigationPlugin::new(NavigatorMode::ObjectCentric));
    }
}

fn setup(mut commands: Commands, server: Res<AssetServer>, mut known: ResMut<KnownScenes>) {
    // light
    commands.spawn((
        DirectionalLight {
            color: Color::srgb_u8(255, 224, 141),
            shadows_enabled: true,
            illuminance: 130000.0,
            ..default()
        },
        Transform::from_xyz(4.0, 4.0, 3.0).looking_at((0.0, 0.0, 0.0).into(), Dir3::Y),
        Replicated,
    ));

    let env_map = server.load("ibl/workshop_4k_small.exr");

    commands.insert_resource(EnvironmentLighting {
        //intensity: 15000.0,
        intensity: 5000.0,
        equirect: env_map,
    });

    let root = commands
        .spawn((Transform::default(), Replicated, NavigatorMarker))
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
                    SceneRoot(server.load_override(GltfAssetLabel::Scene(0).from_asset(val))),
                    Replicated,
                    PropagateReplication::default(),
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

    match trigger.button {
        JoystickButton::TL => {
            new = (new + current_len - 1) % current_len;
        }
        JoystickButton::TR => {
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
