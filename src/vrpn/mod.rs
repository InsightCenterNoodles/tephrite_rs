//! VRPN client integration for Bevy.
//!
//! It exposes a small Bevy plugin (`VRPNPlugin`) and a component (`VRPNLink`)
//! to bind an entity's `Transform` to a specific VRPN sender.

mod common;
mod protocol;

use std::time::Duration;

use bevy::{platform::collections::HashMap, prelude::*};

use crate::{
    config::{VRPNAddress, VRPNCoordinateTransform, get_configuration},
    input::{AxisMessage, ButtonEventKind, ButtonMessage, JoystickAxis, JoystickButton},
    vrpn::common::SharedItemState,
};

/// Worker thread entry point that services a single VRPN client.
fn vrpn_spinner(
    to_watch: HashMap<String, SharedItemState>,
    host_string: String,
    coordinate_transform: VRPNCoordinateTransform,
) {
    // try to connect, retrying if things go south

    const MAX_RETRY: usize = 5;

    for try_num in 0..MAX_RETRY {
        if try_num > 0 {
            // sleep if this is a retry
            std::thread::sleep(Duration::from_secs(try_num as u64));
        }

        let Ok(mut state) =
            protocol::VRPNClient::new(to_watch.clone(), &host_string, coordinate_transform)
        else {
            error!("Unable to connect to {host_string}, attempts: {try_num}/{MAX_RETRY}");
            continue;
        };

        // run the client service
        match state.run() {
            Ok(_) => {
                debug!("VRPN client for {host_string} exited");
                return;
            }
            Err(err) => {
                error!("VRPN client for {host_string} exited with error: {err}");
                error!("Attempting reconnect, attempts: {try_num}/{MAX_RETRY}");
                continue;
            }
        }
    }
    // If we get here, that means we've failed to connect after MAX_RETRY attempts
    error!("Unable to connect to {host_string}, your VRPN devices will not function properly!");
}

/// Start a VRPN client thread for `host_string` (`name_or_ip:port`).
fn start_vrpn_client(
    to_watch: HashMap<String, SharedItemState>,
    host_string: &str,
    coordinate_transform: VRPNCoordinateTransform,
    res: &mut VRPNResource,
) {
    let host_string = host_string.to_owned();

    let handle = std::thread::spawn(move || {
        vrpn_spinner(to_watch, host_string, coordinate_transform);
    });

    res.vrpn_threads.push(handle);
}

/// Resource holding application-level VRPN state.
#[derive(Resource)]
pub struct VRPNResource {
    vrpn_threads: Vec<std::thread::JoinHandle<()>>,
}

impl VRPNResource {
    // TODO: proper shutdown
    // pub fn wait_for_shutdown(self) {
    //     for t in self.vrpn_threads {
    //         t.join().unwrap();
    //     }
    // }
}

/// Connect this entity to a VRPN sender(s).
///
/// When attached to an entity, a network thread is spawned (per endpoint) and
/// the entity's `Transform` is updated in `FixedUpdate`.
#[derive(Component)]
#[component(immutable)]
pub struct VRPNObject(pub Vec<VRPNAddress>);

/// Represents the connected VRPN state
#[derive(Component)]
struct VRPNLinkConnected {
    reader: SharedItemState,
    sensor: usize,
}

/// Observer that establishes network connections for newly added `VRPNLink`s.
///
/// Spawns one client thread per endpoint and associates a shared state reader
/// with the entity via `VRPNLinkConnected`.
fn check_for_new_vrpn(
    trigger: On<Add, VRPNObject>,
    mut commands: Commands,
    query: Query<&VRPNObject, Without<VRPNLinkConnected>>,
    mut res: ResMut<VRPNResource>,
) {
    let event = trigger.event();
    let entity = event.entity;

    let Ok(link) = query.get(entity) else {
        // Somehow this got added to something WITH a link already? we dont handle this yet.
        return;
    };

    // Note that if more of these come along, we do NOT reuse existing connections. thats a WIP.
    // TODO Support late attachment

    // this should be in the resource...
    // index by [endpoint][sender]
    let mut map: HashMap<String, HashMap<String, SharedItemState>> = HashMap::default();

    let state = SharedItemState::new();

    // TODO: clean up
    let mut sensor = 0usize;

    for ep in &link.0 {
        // Not very fast...
        let endpoint = format!("{}:{}", ep.host, ep.port);

        if let Some(requested_sensor) = ep.sensor {
            sensor = requested_sensor as usize
        }

        map.entry(endpoint)
            .and_modify(|x| {
                x.insert(ep.sender.clone(), state.clone());
            })
            .or_insert_with(|| {
                let mut ret = HashMap::default();
                ret.insert(ep.sender.clone(), state.clone());
                ret
            });
    }

    commands.entity(entity).insert(VRPNLinkConnected {
        reader: state,
        sensor,
    });

    for (k, v) in map {
        start_vrpn_client(
            v,
            &k,
            get_configuration().vrpn.coordinate_transform,
            &mut res,
        );
    }
}

const AXIS_MAP: [JoystickAxis; 9] = [
    JoystickAxis::LeftX,   //0
    JoystickAxis::LeftY,   //1
    JoystickAxis::RightX,  //2
    JoystickAxis::Unknown, //3
    JoystickAxis::Unknown, //4
    JoystickAxis::RightY,  //5
    JoystickAxis::Unknown, //6
    JoystickAxis::Unknown, //7
    JoystickAxis::DPad,    //8
];

const BUTTON_MAP: [JoystickButton; 10] = [
    JoystickButton::Button0,
    JoystickButton::Button2,
    JoystickButton::Button3,
    JoystickButton::Button1,
    JoystickButton::BL,
    JoystickButton::BR,
    JoystickButton::TL,
    JoystickButton::TR,
    JoystickButton::Back,
    JoystickButton::Start,
];

fn map_button(button_index: u8) -> JoystickButton {
    *(BUTTON_MAP
        .get(button_index as usize)
        .unwrap_or(&JoystickButton::Unknown))
}

/// System that applies the latest VRPN-derived transform to entities.
fn service_vrpn(
    mut query: Query<(Entity, &VRPNLinkConnected, &mut Transform)>,
    mut writer: MessageWriter<ButtonMessage>,
    mut axis_writer: MessageWriter<AxisMessage>,
) {
    for (e, c, mut tf) in query.iter_mut() {
        // some funky optimization here. we dont want to always hold a write lock

        let sensor = c.sensor;

        let new_pos = if let Some(mtx) = c.reader.poses.get(sensor) {
            mtx.lock().unwrap().clone()
        } else {
            Default::default()
        };

        tf.translation = new_pos.position;
        tf.rotation = new_pos.rotation.normalize();

        axis_writer.write_batch(
            c.reader
                .previous_analog
                .iter()
                .zip(c.reader.latest_analog.iter())
                .enumerate()
                .filter_map(|(index, (prev, latest))| {
                    // We restrict analog IDs to <= u8

                    let p_value = prev.load(std::sync::atomic::Ordering::Relaxed);
                    let value = latest.load(std::sync::atomic::Ordering::Relaxed);

                    // TODO: sensitivity config
                    if p_value != value {
                        prev.store(value, std::sync::atomic::Ordering::Relaxed);
                        //debug!("Send axis event: {x:?}");
                        Some(AxisMessage {
                            from: e,
                            axis: (AXIS_MAP.get(index as usize)).cloned().unwrap_or_default(),
                            value,
                        })
                    } else {
                        None
                    }
                }),
        );

        while let Some(x) = c.reader.button_changes.pop() {
            writer.write({
                //debug!("Send button event {x:?}");
                let kind = if x.1 > 0 {
                    ButtonEventKind::ButtonPressed(map_button(x.0))
                } else {
                    ButtonEventKind::ButtonReleased(map_button(x.0))
                };

                ButtonMessage { from: e, kind }
            });
        }
    }
}

/// Bevy plugin that wires up VRPN connectivity.
///
/// - Adds a `VRPNResource` to manage network threads.
/// - Observes `VRPNLink` additions to spawn clients.
/// - Updates linked entities' `Transform` each `FixedUpdate`.
///
/// Example
/// ```ignore
/// use bevy::prelude::*;
/// use tephrite_rs::vrpn::{VRPNLink, VRPNPlugin};
///
/// fn main() {
///     App::new()
///         .add_plugins(DefaultPlugins)
///         .add_plugins(VRPNPlugin)
///         .add_systems(Startup, |mut commands: Commands| {
///             // Replace with your server and sender name
///             let addr = tephrite_rs::config::VRPNAddress {
///                 host: "127.0.0.1".into(),
///                 port: 3883,
///                 sender: "Head0".into(),
///             };
///             commands.spawn((Transform::default(), GlobalTransform::default(), VRPNLink::new(addr)));
///         })
///         .run();
/// }
/// ```
pub struct VRPNPlugin;

impl Plugin for VRPNPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ButtonMessage>();
        app.insert_resource(VRPNResource {
            vrpn_threads: vec![],
        });
        app.add_observer(check_for_new_vrpn);
        app.add_systems(PreUpdate, service_vrpn);
    }
}
