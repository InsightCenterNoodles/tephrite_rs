// use std::f32::consts::{FRAC_PI_2, PI};

// use bevy::{ecs::entity::EntityHashMap, platform::collections::HashSet, prelude::*};

// use crate::{
//     input::{ButtonEventKind, ButtonMessage, Interactor, InteractorState, JoystickButton},
//     prelude::{PropagateReplication, Replicated},
//     ui::{
//         label::make_label,
//         rounded_rect::{RoundedRectOptions, rounded_rect_mesh},
//         text_bake::{CpuTextBaker, TextStyle},
//     },
// };

// // MARK: Options

// /// One selectable radial combo-box option.
// #[derive(Debug, Clone)]
// pub struct RadialComboBoxOption {
//     pub name: String,
//     pub color: Color,
//     pub payload: i32,
// }

// /// Full UI request payload for one radial combo session.
// #[derive(Debug, Clone)]
// pub struct RadialComboBoxRequest {
//     pub title: String,
//     pub options: Vec<RadialComboBoxOption>,
// }

// // MARK: Request response

// /// Emitted when the configured trigger button goes down.
// ///
// /// User systems should answer with a message to [`RadialComboBoxPopulateResponse`].
// #[derive(Debug, Event)]
// pub struct RadialComboBoxPopulateRequest {
//     pub interactor: Entity,
// }

// /// Response to [`RadialComboBoxPopulateRequest`].
// /// Note that only the first non-empty title will be used. The options will be concatenated.
// #[derive(Debug, Message)]
// pub struct RadialComboBoxPopulateResponse {
//     pub interactor: Entity,
//     pub request: RadialComboBoxRequest,
// }

// // MARK: Completion

// /// Final selection result emitted when trigger button is released.
// ///
// /// `payload: None` means cancel.
// #[derive(Debug, Message)]
// pub struct RadialComboBoxSelection {
//     pub interactor: Entity,
//     pub payload: Option<i32>,
// }

// // MARK: Plugin

// /// Plugin implementing a radial combo-box UI affordance.
// pub struct RadialComboBoxPlugin {
//     pub trigger_button: JoystickButton,
//     pub radius: f32,
//     pub forward_offset: f32,
//     pub up_offset: f32,
//     pub title_style: TextStyle,
//     pub option_style: TextStyle,
//     pub cancel_style: TextStyle,
//     pub cancel_label: String,
// }

// impl Default for RadialComboBoxPlugin {
//     fn default() -> Self {
//         Self {
//             trigger_button: JoystickButton::Y,
//             radius: 0.22,
//             forward_offset: -0.33,
//             up_offset: 0.12,
//             title_style: TextStyle::new(46.0).with_background_color([0, 0, 0, 180]),
//             option_style: TextStyle::new(36.0).with_background_color([0, 0, 0, 180]),
//             cancel_style: TextStyle::new(30.0).with_background_color([0, 0, 0, 180]),
//             cancel_label: "Cancel".to_string(),
//         }
//     }
// }

// #[derive(Debug, Resource, Clone)]
// struct RadialComboBoxConfig {
//     trigger_button: JoystickButton,
//     radius: f32,
//     forward_offset: f32,
//     up_offset: f32,
//     title_style: TextStyle,
//     option_style: TextStyle,
//     cancel_style: TextStyle,
//     cancel_label: String,
// }

// /// Cached text baking resources for the radial combo box.
// #[derive(Resource)]
// struct RadialComboTextBaker(CpuTextBaker);

// impl Default for RadialComboTextBaker {
//     fn default() -> Self {
//         Self(CpuTextBaker::new())
//     }
// }

// impl Plugin for RadialComboBoxPlugin {
//     fn build(&self, app: &mut App) {
//         app.insert_resource(RadialComboBoxConfig {
//             trigger_button: self.trigger_button,
//             radius: self.radius,
//             forward_offset: self.forward_offset,
//             up_offset: self.up_offset,
//             title_style: self.title_style,
//             option_style: self.option_style,
//             cancel_style: self.cancel_style,
//             cancel_label: self.cancel_label.clone(),
//         });
//         app.init_resource::<RadialComboTextBaker>();
//         app.init_resource::<RadialComboBoxRuntime>();
//         //app.add_message::<RadialComboBoxPopulateRequest>();
//         app.add_message::<RadialComboBoxPopulateResponse>();
//         app.add_message::<RadialComboBoxSelection>();
//         app.add_systems(
//             Update,
//             (
//                 drive_combo_trigger,
//                 apply_populate_responses,
//                 update_combo_selection,
//                 cleanup_orphaned_combos,
//             ),
//         );
//     }
// }

// #[derive(Debug, Default, Resource)]
// struct RadialComboBoxRuntime {
//     awaiting: HashSet<Entity>,
//     active: EntityHashMap<ActiveCombo>,
// }

// #[derive(Debug)]
// struct ActiveCombo {
//     root: Entity,
//     arrow: Entity,
//     option_angles: Vec<f32>,
//     payloads: Vec<i32>,
//     selection: Selection,
// }

// #[derive(Debug, Clone, Copy)]
// enum Selection {
//     Cancel,
//     Option(usize),
// }

// fn drive_combo_trigger(
//     opts: Res<RadialComboBoxConfig>,
//     mut runtime: ResMut<RadialComboBoxRuntime>,
//     mut button_reader: MessageReader<ButtonMessage>,
//     mut selection_writer: MessageWriter<RadialComboBoxSelection>,
//     mut commands: Commands,
// ) {
//     for msg in button_reader.read() {
//         let matches_button = match msg.kind {
//             ButtonEventKind::ButtonPressed(button) | ButtonEventKind::ButtonReleased(button) => {
//                 button == opts.trigger_button
//             }
//         };

//         if !matches_button {
//             continue;
//         }

//         match msg.kind {
//             ButtonEventKind::ButtonPressed(_) => {
//                 runtime.awaiting.insert(msg.from);
//                 commands.trigger(RadialComboBoxPopulateRequest {
//                     interactor: msg.from,
//                 });
//             }
//             ButtonEventKind::ButtonReleased(_) => {
//                 // no longer awaiting, so we can process the selection.
//                 runtime.awaiting.remove(&msg.from);

//                 // are we active awaiting selection?
//                 let Some(active) = runtime.active.remove(&msg.from) else {
//                     selection_writer.write(RadialComboBoxSelection {
//                         interactor: msg.from,
//                         payload: None,
//                     });
//                     continue;
//                 };

//                 let payload = match active.selection {
//                     Selection::Cancel => None,
//                     Selection::Option(i) => active.payloads.get(i).copied(),
//                 };

//                 commands.entity(active.root).despawn();
//                 selection_writer.write(RadialComboBoxSelection {
//                     interactor: msg.from,
//                     payload,
//                 });
//             }
//         }
//     }
// }

// fn apply_populate_responses(
//     opts: Res<RadialComboBoxConfig>,
//     mut runtime: ResMut<RadialComboBoxRuntime>,
//     mut responses: MessageReader<RadialComboBoxPopulateResponse>,
//     mut commands: Commands,
//     interactors: Query<(), With<Interactor>>,
//     interactor_states: Query<&InteractorState>,
//     mut baker: ResMut<RadialComboTextBaker>,
//     mut images: ResMut<Assets<Image>>,
//     mut meshes: ResMut<Assets<Mesh>>,
//     mut materials: ResMut<Assets<StandardMaterial>>,
// ) {
//     for response in responses.read() {
//         if !runtime.awaiting.remove(&response.interactor) {
//             continue;
//         }

//         let Some(request) = &response.request else {
//             continue;
//         };

//         if !interactors.contains(response.interactor) {
//             continue;
//         }

//         if let Some(old) = runtime.active.remove(&response.interactor) {
//             commands.entity(old.root).despawn();
//         }

//         let root = commands
//             .spawn((
//                 Name::new("RadialComboRoot"),
//                 Transform::from_xyz(0.0, opts.up_offset, opts.forward_offset),
//                 Visibility::Visible,
//                 Replicated,
//                 PropagateReplication::default(),
//             ))
//             .id();

//         commands.entity(response.interactor).add_child(root);

//         let title = match make_label(
//             &mut baker.0,
//             &request.title,
//             opts.title_style,
//             &mut images,
//             &mut meshes,
//             &mut materials,
//         ) {
//             Ok(title) => title,
//             Err(err) => {
//                 error!("Unable to build radial combo title label: {err:?}");
//                 commands.entity(root).despawn();
//                 continue;
//             }
//         };

//         commands.entity(root).with_child((
//             Name::new("RadialComboTitle"),
//             title,
//             Transform::from_xyz(0.0, opts.radius + 0.12, 0.0).with_scale(Vec3::splat(0.24)),
//         ));

//         let arrow_mesh = match rounded_rect_mesh(
//             0.022,
//             opts.radius * 1.6,
//             RoundedRectOptions {
//                 radius: 0.007,
//                 ..Default::default()
//             },
//         ) {
//             Ok(mesh) => meshes.add(mesh),
//             Err(err) => {
//                 error!("Unable to build radial combo arrow mesh: {err:?}");
//                 commands.entity(root).despawn();
//                 continue;
//             }
//         };
//         let arrow_mat = materials.add(StandardMaterial {
//             base_color: Color::srgb(1.0, 0.95, 0.2),
//             emissive: LinearRgba::from(Color::srgb(0.4, 0.37, 0.04)),
//             unlit: true,
//             alpha_mode: AlphaMode::Blend,
//             ..Default::default()
//         });

//         let option_count = request.options.len();
//         let mut option_angles = Vec::with_capacity(option_count);
//         let mut payloads = Vec::with_capacity(option_count);
//         let mut combo_failed = false;

//         for (index, item) in request.options.iter().enumerate() {
//             let angle = option_angle(index, option_count);
//             option_angles.push(angle);
//             payloads.push(item.payload);

//             let label = match make_label(
//                 &mut baker.0,
//                 &item.name,
//                 opts.option_style,
//                 &mut images,
//                 &mut meshes,
//                 &mut materials,
//             ) {
//                 Ok(label) => label,
//                 Err(err) => {
//                     error!("Unable to build radial combo option label: {err:?}");
//                     combo_failed = true;
//                     break;
//                 }
//             };

//             let MeshMaterial3d(mat_handle) = &label.material;
//             if let Some(mat) = materials.get_mut(mat_handle) {
//                 mat.base_color = item.color;
//             }

//             let position = vec3(angle.cos() * opts.radius, angle.sin() * opts.radius, 0.0);

//             // Keep text upright while following the arc's tangent direction.
//             let mut tangent_rotation = angle - FRAC_PI_2;
//             if angle < FRAC_PI_2 {
//                 tangent_rotation += PI;
//             }

//             commands.entity(root).with_child((
//                 Name::new(format!("RadialComboOption{index}")),
//                 label,
//                 Transform::from_translation(position)
//                     .with_rotation(Quat::from_rotation_z(tangent_rotation))
//                     .with_scale(Vec3::splat(0.18)),
//             ));
//         }

//         if combo_failed {
//             commands.entity(root).despawn();
//             continue;
//         }

//         let cancel = match make_label(
//             &mut baker.0,
//             &opts.cancel_label,
//             opts.cancel_style,
//             &mut images,
//             &mut meshes,
//             &mut materials,
//         ) {
//             Ok(cancel) => cancel,
//             Err(err) => {
//                 error!("Unable to build radial combo cancel label: {err:?}");
//                 commands.entity(root).despawn();
//                 continue;
//             }
//         };
//         commands.entity(root).with_child((
//             Name::new("RadialComboCancel"),
//             cancel,
//             Transform::from_xyz(0.0, -opts.radius * 1.15, 0.0)
//                 .with_rotation(Quat::from_rotation_z(PI))
//                 .with_scale(Vec3::splat(0.14)),
//         ));

//         let mut selection = Selection::Cancel;

//         if let Ok(state) = interactor_states.get(response.interactor) {
//             if let Some(stick) = state.stick_state(opts.stick) {
//                 selection = select_from_stick(stick, &option_angles);
//             }
//         }

//         let selected_angle = match selection {
//             Selection::Cancel => -FRAC_PI_2,
//             Selection::Option(i) => option_angles[i],
//         };

//         let arrow = commands
//             .spawn((
//                 Name::new("RadialComboArrow"),
//                 Mesh3d(arrow_mesh),
//                 MeshMaterial3d(arrow_mat),
//                 Transform::from_xyz(0.0, opts.radius * 0.5, 0.008)
//                     .with_rotation(Quat::from_rotation_z(selected_angle - FRAC_PI_2)),
//             ))
//             .id();

//         commands.entity(root).add_child(arrow);

//         runtime.active.insert(
//             response.interactor,
//             ActiveCombo {
//                 root,
//                 arrow,
//                 option_angles,
//                 payloads,
//                 selection,
//             },
//         );
//     }
// }

// fn update_combo_selection(
//     opts: Res<RadialComboBoxConfig>,
//     mut runtime: ResMut<RadialComboBoxRuntime>,
//     states: Query<&InteractorState>,
//     mut transforms: Query<&mut Transform>,
// ) {
//     for (interactor, active) in runtime.active.iter_mut() {
//         let Ok(state) = states.get(*interactor) else {
//             continue;
//         };

//         let Some(stick) = state.stick_state(opts.stick) else {
//             continue;
//         };

//         active.selection = select_from_stick(stick, &active.option_angles);

//         let selected_angle = match active.selection {
//             Selection::Cancel => -FRAC_PI_2,
//             Selection::Option(i) => active.option_angles[i],
//         };

//         if let Ok(mut tf) = transforms.get_mut(active.arrow) {
//             tf.rotation = Quat::from_rotation_z(selected_angle - FRAC_PI_2);
//         }
//     }
// }

// fn cleanup_orphaned_combos(
//     mut runtime: ResMut<RadialComboBoxRuntime>,
//     interactors: Query<(), With<Interactor>>,
//     mut commands: Commands,
// ) {
//     runtime.awaiting.retain(|e| interactors.contains(*e));

//     runtime.active.retain(|interactor, active| {
//         if interactors.contains(*interactor) {
//             true
//         } else {
//             commands.entity(active.root).despawn();
//             false
//         }
//     });
// }

// #[inline]
// fn option_angle(index: usize, count: usize) -> f32 {
//     match count {
//         0 => 0.0,
//         1 => FRAC_PI_2,
//         n => PI - (index as f32) * (PI / ((n - 1) as f32)),
//     }
// }

// #[inline]
// fn select_from_stick(stick: Vec2, option_angles: &[f32]) -> Selection {
//     let target = stick.y.atan2(stick.x);

//     let mut best = Selection::Cancel;
//     let mut best_dist = angle_distance(target, -FRAC_PI_2);

//     for (idx, angle) in option_angles.iter().copied().enumerate() {
//         let dist = angle_distance(target, angle);
//         if dist < best_dist {
//             best = Selection::Option(idx);
//             best_dist = dist;
//         }
//     }

//     best
// }

// #[inline]
// fn angle_distance(a: f32, b: f32) -> f32 {
//     (a - b).sin().atan2((a - b).cos()).abs()
// }
