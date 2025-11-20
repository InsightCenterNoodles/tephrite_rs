//! VRPN client integration for Bevy.
//!
//! It exposes a small Bevy plugin (`VRPNPlugin`) and a component (`VRPNLink`)
//! to bind an entity's `Transform` to a specific VRPN sender.

mod comm;
mod common;

use bevy::{platform::collections::HashMap, prelude::*};

use crate::{
    input::{AxisEvent, ButtonEvent, ButtonEventKind, InteractorButton},
    vrpn::common::SharedItemState,
};

/// Worker thread entry point that services a single VRPN client.
fn vrpn_spinner(
    to_watch: HashMap<String, SharedItemState>,
    host_string: String,
    shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    let Ok(mut state) = comm::VRPNClient::new(to_watch, &host_string) else {
        error!("Unable to connect to {host_string}");
        return;
    };

    state.run(shutdown);
}

/// Start a VRPN client thread for `host_string` (`name_or_ip:port`).
fn start_vrpn_client(
    to_watch: HashMap<String, SharedItemState>,
    host_string: &str,
    res: &mut VRPNResource,
) {
    let host_string = host_string.to_owned();

    let sd = res.shutdown.clone();

    let handle = std::thread::spawn(move || {
        vrpn_spinner(to_watch, host_string, sd);
    });

    res.vrpn_threads.push(handle);
}

/// Resource holding application-level VRPN state.
#[derive(Resource)]
pub struct VRPNResource {
    shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>,
    vrpn_threads: Vec<std::thread::JoinHandle<()>>,
}

impl VRPNResource {
    #[allow(unused)]
    /// Signal network threads to stop and join them.
    pub fn wait_for_shutdown(self) {
        self.shutdown
            .store(false, std::sync::atomic::Ordering::Release);
        for t in self.vrpn_threads {
            t.join().unwrap();
        }
    }
}

/// Connect this entity to a VRPN sender(s).
///
/// When attached to an entity, a network thread is spawned (per endpoint) and
/// the entity's `Transform` is updated in `FixedUpdate`.
#[derive(Component)]
#[component(immutable)]
pub struct VRPNObject(pub Vec<crate::config::VRPNAddress>);

/// Represents the connected VRPN state
#[derive(Component)]
struct VRPNLinkConnected {
    reader: SharedItemState,
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

    let state = common::new_shared_item_state();

    for ep in &link.0 {
        // Not very fast...
        let endpoint = format!("{}:{}", ep.host, ep.port);

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

    commands
        .entity(entity)
        .insert(VRPNLinkConnected { reader: state });

    for (k, v) in map {
        start_vrpn_client(v, &k, &mut res);
    }
}

/// System that applies the latest VRPN-derived transform to entities.
fn service_vrpn(
    mut query: Query<(Entity, &VRPNLinkConnected, &mut Transform)>,
    mut writer: MessageWriter<ButtonEvent>,
    mut axis_writer: MessageWriter<AxisEvent>,
) {
    for (e, c, mut tf) in query.iter_mut() {
        // some funky optimization here. we dont want to always hold a write lock

        let need_write = {
            let new_pos = c.reader.read().unwrap();

            tf.translation = new_pos.position.as_vec3();
            tf.rotation = new_pos.rotation.as_quat().normalize();

            axis_writer.write_batch(new_pos.analog_state.iter().enumerate().filter_map(|x| {
                // We restrict analog IDs to <= u8

                // TODO: sensitivity config
                if x.1.abs() > 0.00001 {
                    //debug!("Send axis event: {x:?}");
                    Some(AxisEvent {
                        from: e,
                        axis: x.0 as u8,
                        value: (*x.1) as f32,
                    })
                } else {
                    None
                }
            }));

            new_pos.button_changes.len() > 0
        };

        if need_write {
            let mut lock = c.reader.write().unwrap();

            writer.write_batch(lock.button_changes.drain(..).map(|x| {
                //debug!("Send button event {x:?}");
                let kind = if x.1 > 0 {
                    ButtonEventKind::ButtonPressed(InteractorButton(x.0))
                } else {
                    ButtonEventKind::ButtonReleased(InteractorButton(x.0))
                };

                ButtonEvent { from: e, kind }
            }));
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
        app.add_message::<ButtonEvent>();
        app.insert_resource(VRPNResource {
            shutdown: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
            vrpn_threads: vec![],
        });
        app.add_observer(check_for_new_vrpn);
        app.add_systems(FixedPreUpdate, service_vrpn);
    }
}
