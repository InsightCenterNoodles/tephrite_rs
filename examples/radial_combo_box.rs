// use bevy::prelude::*;
// use tephrite_rs::prelude::*;
// use tephrite_rs::ui::prelude::*;

// struct MyPlugin;

// impl Plugin for MyPlugin {
//     fn build(&self, app: &mut App) {
//         app.add_plugins(RadialComboBoxPlugin {
//             trigger_button: JoystickButton::A,
//             ..Default::default()
//         });

//         app.add_systems(Startup, setup_scene);
//         app.add_systems(
//             Update,
//             (
//                 setup_interactor_visuals,
//                 handle_combo_populate_requests,
//                 log_combo_selection,
//             ),
//         );
//     }
// }

// #[derive(Component)]
// struct DemoInteractorReady;

// fn setup_scene(
//     mut commands: Commands,
//     mut meshes: ResMut<Assets<Mesh>>,
//     mut materials: ResMut<Assets<StandardMaterial>>,
// ) {
//     commands.spawn((
//         Mesh3d(meshes.add(Circle::new(3.5))),
//         MeshMaterial3d(materials.add(Color::srgb(0.12, 0.12, 0.14))),
//         Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
//         Replicated,
//     ));

//     commands.spawn((
//         DirectionalLight {
//             illuminance: 9000.0,
//             shadows_enabled: true,
//             ..Default::default()
//         },
//         Transform::from_xyz(2.0, 4.0, 2.0).looking_at(Vec3::ZERO, Vec3::Y),
//         Replicated,
//     ));

//     commands.spawn((
//         Camera3d::default(),
//         Transform::from_xyz(-2.0, 2.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
//     ));
// }

// fn setup_interactor_visuals(
//     interactors: Query<Entity, (With<Interactor>, Without<DemoInteractorReady>)>,
//     mut commands: Commands,
//     mut meshes: ResMut<Assets<Mesh>>,
//     mut materials: ResMut<Assets<StandardMaterial>>,
// ) {
//     for interactor in interactors {
//         commands.entity(interactor).insert((
//             DemoInteractorReady,
//             Replicated,
//             PropagateReplication::default(),
//             Name::new("DemoInteractor"),
//         ));

//         let mesh = meshes.add(Cuboid::new(0.03, 0.03, 0.18));
//         let material = materials.add(StandardMaterial {
//             base_color: Color::srgb(0.9, 0.9, 0.95),
//             emissive: LinearRgba::from(Color::srgb(0.08, 0.08, 0.1)),
//             ..Default::default()
//         });

//         commands.entity(interactor).with_child((
//             Mesh3d(mesh),
//             MeshMaterial3d(material),
//             Transform::from_xyz(0.0, 0.0, -0.09),
//         ));

//         info!("Interactor {interactor:?} ready. Press and hold button A to open radial combo.");
//     }
// }

// fn handle_combo_populate_requests(
//     mut reader: MessageReader<RadialComboBoxPopulateRequest>,
//     mut writer: MessageWriter<RadialComboBoxPopulateResponse>,
// ) {
//     for request in reader.read() {
//         let options = vec![
//             RadialComboBoxOption {
//                 name: "Move".to_string(),
//                 color: Color::srgb(0.3, 0.85, 1.0),
//                 payload: 10,
//             },
//             RadialComboBoxOption {
//                 name: "Scale".to_string(),
//                 color: Color::srgb(0.4, 1.0, 0.45),
//                 payload: 20,
//             },
//             RadialComboBoxOption {
//                 name: "Rotate".to_string(),
//                 color: Color::srgb(1.0, 0.75, 0.35),
//                 payload: 30,
//             },
//             RadialComboBoxOption {
//                 name: "Clone".to_string(),
//                 color: Color::srgb(1.0, 0.5, 0.5),
//                 payload: 40,
//             },
//             RadialComboBoxOption {
//                 name: "Delete".to_string(),
//                 color: Color::srgb(1.0, 0.25, 0.25),
//                 payload: 50,
//             },
//         ];

//         writer.write(RadialComboBoxPopulateResponse {
//             interactor: request.interactor,
//             request: Some(RadialComboBoxRequest {
//                 title: "Action".to_string(),
//                 options,
//             }),
//         });
//     }
// }

// fn log_combo_selection(mut reader: MessageReader<RadialComboBoxSelection>) {
//     for result in reader.read() {
//         match result.payload {
//             Some(payload) => info!(
//                 "Interactor {:?} selected payload {}",
//                 result.interactor, payload
//             ),
//             None => info!("Interactor {:?} cancelled radial combo", result.interactor),
//         }
//     }
// }

fn main() {
    // tephrite_rs::run(MyPlugin);
}
