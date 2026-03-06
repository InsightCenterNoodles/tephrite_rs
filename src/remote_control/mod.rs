//! Minimal in-process remote control webserver for Bevy apps.
//!
//! 1. Add [`RemoteControlPlugin`]. This is usually done automatically by Tephrite.
//! 2. Initialize or fetch [`RemoteControlDefinitions`].
//! 3. Push one [`PropertyDefinition`] per controllable property.
//! 4. Observe [`RemoteControlEvent`] on those property entities (or use
//!    [`use_cases::RemoteControlTransform`] for common transform controls).
//! 5. In the observer, mutate your world state/components based on `event.value`.
//!
//! Property routing uses `(entity, aspect_id)` as a composite identifier. This
//! allows multiple controls to target one entity without allocating helper
//! entities just to disambiguate callbacks.
//!
//! # Example
//! ```ignore
//! use bevy::prelude::*;
//! use tephrite_rs::remote_control::prelude::*;
//!
//! fn setup(mut commands: Commands, mut defs: ResMut<RemoteControlDefinitions>) {
//!     let speed_property = commands.spawn_empty().id();
//!     defs.push(PropertyDefinition {
//!         id: speed_property,
//!         aspect_id: 0, // Multiple definitions can refer to the same entity; use this to discriminate between them.
//!         label: "Speed".into(),
//!         control: PropertyControl::Slider {
//!             min: 0.0,
//!             max: 20.0,
//!             step: 0.1,
//!             initial: 5.0,
//!         },
//!     });
//!
//!     commands
//!         .entity(speed_property)
//!         .observe(|trigger: On<RemoteControlEvent>, mut query: Query<&mut Transform>| {
//!             if let Ok(mut tf) = query.get_mut(trigger.entity()) {
//!                 if let PropertyValue::Float(v) = trigger.event().value {
//!                     tf.translation.x = v;
//!                 }
//!             }
//!         });
//! }
//!
//! App::new()
//!     .add_plugins(DefaultPlugins)
//!     .init_resource::<RemoteControlDefinitions>()
//!     .add_plugins(RemoteControlPlugin::default())
//!     .add_systems(Startup, setup);
//! ```

pub(crate) mod common;
pub(crate) mod content;
pub mod events;
pub mod property;
pub mod use_cases;

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::time::Duration;

use bevy::prelude::*;

use crate::remote_control::events::{RemoteControlEvent, RemoteControlEventInternal};
use crate::remote_control::property::{PropertyDefinition, parse_property_value};

/// Startup definitions consumed by [`RemoteControlPlugin`] to build the control page.
///
/// Typical setup:
/// - call `app.init_resource::<RemoteControlDefinitions>()`
/// - in startup systems, push [`PropertyDefinition`] entries into this resource
/// - add observers on each property entity for [`events::RemoteControlEvent`]
#[derive(Debug, Default, Resource)]
pub struct RemoteControlDefinitions(pub Vec<PropertyDefinition>);

impl RemoteControlDefinitions {
    /// Add one property definition to be exposed on the remote control page.
    pub fn push(&mut self, definition: PropertyDefinition) {
        self.0.push(definition);
    }

    /// Extend the exposed property list.
    pub fn extend(&mut self, definitions: impl IntoIterator<Item = PropertyDefinition>) {
        self.0.extend(definitions);
    }
}

#[derive(Debug, Default, Resource)]
pub struct RemoteControlOpts(String);

/// Bevy plugin that hosts the local remote-control HTTP endpoint.
///
/// The plugin snapshots [`RemoteControlDefinitions`] during `PostStartup`.
/// Definitions added after startup are not reflected until next app launch.
pub struct RemoteControlPlugin {
    /// HTTP bind address for the control page (for example `127.0.0.1:8081`).
    pub bind_addr: String,
}

impl Default for RemoteControlPlugin {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:8081".into(),
        }
    }
}

impl Plugin for RemoteControlPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.insert_resource(RemoteControlOpts(self.bind_addr.clone()));

        app.world_mut()
            .get_resource_or_init::<RemoteControlDefinitions>();
        app.add_systems(Update, (check_updates, server_poll).chain());
        app.add_observer(bounce);

        app.add_plugins(use_cases::UseCasesPlugin);
    }
}

/// Rebuild server-rendered control state when definitions change.
fn check_updates(
    mut commands: Commands,
    opts: Res<RemoteControlOpts>,
    defs: Res<RemoteControlDefinitions>,
    server: Option<ResMut<RemoteControlServer>>,
) {
    if !defs.is_changed() {
        return;
    }

    info!("Remote control definitions changed, restarting server...");
    //println!("Definitions: {:?}", defs);

    if let Some(mut server) = server {
        server.update(defs.0.clone());
    } else {
        match RemoteControlServer::new(&opts.0, defs.0.clone()) {
            Ok(server) => {
                commands.insert_resource(server);
            }
            Err(err) => {
                error!("Unable to start remote control server! Error: {err:?}");
            }
        }
    }
}

/// Handle to the running remote control server thread.
#[derive(Debug, Resource)]
pub struct RemoteControlServer {
    server: TcpListener,

    index_page: String,
    property_lookup: HashMap<String, PropertyDefinition>,
}

impl RemoteControlServer {
    fn new(bind_addr: &str, properties: Vec<PropertyDefinition>) -> Result<Self> {
        let listener = TcpListener::bind(&bind_addr)?;
        listener.set_nonblocking(true)?;

        let rendered_controls = content::render_controls(&properties);
        let index_page = content::render_index_page(&rendered_controls);
        let property_lookup = build_property_lookup(&properties);

        Ok(Self {
            server: listener,
            index_page,
            property_lookup,
        })
    }

    fn update(&mut self, properties: Vec<PropertyDefinition>) {
        let rendered_controls = content::render_controls(&properties);
        let index_page = content::render_index_page(&rendered_controls);
        let property_lookup = build_property_lookup(&properties);

        self.index_page = index_page;
        self.property_lookup = property_lookup;
    }
}

/// Poll for one HTTP request and handle it synchronously.
fn server_poll(server: Option<ResMut<RemoteControlServer>>, mut commands: Commands) {
    let Some(server) = server else {
        return;
    };

    //println!("Polling server {server:?}");

    match server.server.accept() {
        Ok((mut stream, _addr)) => {
            // This intentionally handles one request per accepted connection.
            handle_connection(
                &mut stream,
                &server.index_page,
                &server.property_lookup,
                &mut commands,
            );
        }
        Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
            // No incoming...
        }
        Err(_err) => {
            // There was an error...
        }
    }
}

/// Translate internal remote-control events into public Bevy entity events.
fn bounce(
    trigger: On<RemoteControlEventInternal>,
    mut commands: Commands,
    mut writer: MessageWriter<AppExit>,
) {
    info!("Handling remote control event {:?}", trigger.event());
    match trigger.event() {
        RemoteControlEventInternal::PropertyChanged {
            property,
            aspect_id,
            value,
        } => commands.trigger(RemoteControlEvent {
            entity: *property,
            aspect_id: *aspect_id,
            value: value.clone(),
        }),
        RemoteControlEventInternal::QuitRequested => {
            writer.write(AppExit::Success);
        }
    }
}

/// Parse and serve a single HTTP request.
fn handle_connection(
    stream: &mut TcpStream,
    index_page: &str,
    property_lookup: &HashMap<String, PropertyDefinition>,
    commands: &mut Commands,
) {
    let Ok(request) = read_http_request(stream) else {
        respond(
            stream,
            "400 Bad Request",
            "text/plain; charset=utf-8",
            "bad request",
        );
        return;
    };

    use common::*;

    //println!("Properties {property_lookup:?}");
    //println!("Request {:?}", request.path);

    match (request.method.as_str(), request.path.as_str()) {
        ("GET", INDEX_PATH) => {
            respond(
                stream,
                "200 OK",
                "text/html; charset=utf-8",
                index_page.as_bytes(),
            );
        }
        ("POST", EVENT_PATH) => {
            // Form body is expected as `id=<entity_bits>:<aspect_id>&value=<encoded>`.
            let pairs = parse_form_urlencoded(&request.body);
            let Some(id) = pairs.get("id") else {
                respond(
                    stream,
                    "400 Bad Request",
                    "text/plain; charset=utf-8",
                    "missing id",
                );
                return;
            };

            if id == QUIT_ID {
                commands.trigger(events::RemoteControlEventInternal::QuitRequested);
                respond(stream, "200 OK", "text/plain; charset=utf-8", "ok");
                return;
            }

            let Some(property) = property_lookup.get(id) else {
                respond(
                    stream,
                    "404 Not Found",
                    "text/plain; charset=utf-8",
                    "unknown property",
                );
                return;
            };

            match parse_property_value(&property.control, pairs.get("value")) {
                Ok(value) => {
                    commands.trigger(events::RemoteControlEventInternal::PropertyChanged {
                        property: property.id,
                        aspect_id: property.aspect_id,
                        value,
                    });
                    respond(stream, "200 OK", "text/plain; charset=utf-8", "ok");
                }
                Err(message) => {
                    respond(
                        stream,
                        "400 Bad Request",
                        "text/plain; charset=utf-8",
                        message,
                    );
                }
            }
        }
        _ => {
            respond(
                stream,
                "404 Not Found",
                "text/plain; charset=utf-8",
                "not found",
            );
        }
    }
}

/// Maps URL IDs (`<entity_bits>:<aspect_id>`) back to caller property definitions.
fn build_property_lookup(properties: &[PropertyDefinition]) -> HashMap<String, PropertyDefinition> {
    let mut map = HashMap::with_capacity(properties.len());
    for property in properties {
        map.insert(property.lookup_id(), property.clone());
    }
    map
}

struct HttpRequest {
    method: String,
    path: String,
    body: Vec<u8>,
}

/// Very small HTTP/1.1 request parser (sufficient for the local control page).
fn read_http_request(stream: &mut TcpStream) -> std::io::Result<HttpRequest> {
    stream.set_read_timeout(Some(Duration::from_millis(200)))?;
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];

    loop {
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..read]);
        if find_header_end(&buf).is_some() {
            break;
        }
        if buf.len() > 1024 * 1024 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "request too large",
            ));
        }
    }

    let header_end = find_header_end(&buf).ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "missing header terminator")
    })?;

    let header_text = std::str::from_utf8(&buf[..header_end]).map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid header encoding")
    })?;

    let mut lines = header_text.lines();
    let request_line = lines.next().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "missing request line")
    })?;
    let mut request_parts = request_line.split_whitespace();

    let method = request_parts
        .next()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "missing method"))?
        .to_string();
    let path = request_parts
        .next()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "missing path"))?
        .to_string();

    let mut content_length = 0usize;
    for line in lines {
        if let Some(value) = line.strip_prefix("Content-Length:") {
            content_length = value.trim().parse::<usize>().unwrap_or(0);
        }
    }

    let body_start = header_end + 4;
    let mut body = buf[body_start..].to_vec();
    // Continue reading until we have the full declared body.
    while body.len() < content_length {
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..read]);
    }
    body.truncate(content_length);

    Ok(HttpRequest { method, path, body })
}

/// Return index where HTTP headers end (`\r\n\r\n`).
fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

/// Parse `application/x-www-form-urlencoded` request bodies.
fn parse_form_urlencoded(body: &[u8]) -> HashMap<String, String> {
    let text = String::from_utf8_lossy(body);
    text.split('&')
        .filter(|pair| !pair.is_empty())
        .filter_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            let key = parts.next()?;
            let value = parts.next().unwrap_or_default();
            Some((percent_decode(key), percent_decode(value)))
        })
        .collect()
}

/// Percent-decoder for URL-encoded form values.
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                if let (Some(h), Some(l)) = (from_hex(bytes[i + 1]), from_hex(bytes[i + 2])) {
                    out.push((h << 4) | l);
                    i += 3;
                } else {
                    out.push(bytes[i]);
                    i += 1;
                }
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn from_hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Write an HTTP response and close the connection.
fn respond(stream: &mut TcpStream, status: &str, content_type: &str, body: impl AsRef<[u8]>) {
    let body = body.as_ref();
    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
    let _ = stream.shutdown(Shutdown::Both);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::remote_control::common::{EVENT_PATH, PropertyValue};
    use crate::remote_control::property::PropertyControl;
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpListener};

    #[derive(Component, Debug, Clone, Copy)]
    struct AppliedFloat(f32);
    #[derive(Component, Debug, Clone, Copy)]
    struct AppliedAspect(u32);

    fn make_entity(id: u32) -> Entity {
        Entity::from_bits(id as u64)
    }

    fn make_request(method: &str, path: &str, body: &str) -> String {
        format!(
            "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        )
    }

    fn app_with_server(definitions: Option<Vec<PropertyDefinition>>) -> App {
        let mut app = App::new();
        app.add_plugins(RemoteControlPlugin {
            bind_addr: "127.0.0.1:0".into(),
        });
        if let Some(definitions) = definitions {
            let mut defs = app.world_mut().resource_mut::<RemoteControlDefinitions>();
            defs.0 = definitions;
        }
        app.update();
        app.update();
        app
    }

    fn server_addr(app: &App) -> SocketAddr {
        app.world()
            .resource::<RemoteControlServer>()
            .server
            .local_addr()
            .expect("listener should be bound")
    }

    fn send_request(app: &mut App, request: &str) -> String {
        let mut client = TcpStream::connect(server_addr(app)).expect("connect");
        client.write_all(request.as_bytes()).expect("write request");
        client.shutdown(Shutdown::Write).expect("shutdown write");
        app.update();

        let mut response = String::new();
        client
            .read_to_string(&mut response)
            .expect("read response body");
        response
    }

    fn with_stream_pair(f: impl FnOnce(&mut TcpStream, &mut TcpStream)) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("local_addr");
        let mut client = TcpStream::connect(addr).expect("connect");
        let (mut server, _) = listener.accept().expect("accept");
        f(&mut client, &mut server);
    }

    #[test]
    fn parse_property_value_accepts_all_control_variants() {
        assert_eq!(
            property::parse_property_value(
                &PropertyControl::Slider {
                    min: 0.0,
                    max: 10.0,
                    step: 0.1,
                    initial: 1.0,
                },
                Some(&"2.5".to_string()),
            ),
            Ok(PropertyValue::Float(2.5))
        );

        assert_eq!(
            property::parse_property_value(
                &PropertyControl::Toggle { initial: false },
                Some(&"true".to_string()),
            ),
            Ok(PropertyValue::Bool(true))
        );

        assert_eq!(
            property::parse_property_value(
                &PropertyControl::Select {
                    options: vec!["a".into(), "b".into()],
                    initial: 0,
                },
                Some(&"b".to_string()),
            ),
            Ok(PropertyValue::Choice("b".into()))
        );

        assert_eq!(
            property::parse_property_value(
                &PropertyControl::String {
                    initial: "hi".into(),
                },
                Some(&"hello".to_string()),
            ),
            Ok(PropertyValue::Text("hello".into()))
        );

        assert_eq!(
            property::parse_property_value(
                &PropertyControl::Vector3 {
                    initial: Vec3::ZERO,
                    step: 0.1,
                },
                Some(&"1.0,2.0,3.0".to_string()),
            ),
            Ok(PropertyValue::Vec3(Vec3::new(1.0, 2.0, 3.0)))
        );

        assert_eq!(
            property::parse_property_value(&PropertyControl::Button, None),
            Ok(PropertyValue::Triggered)
        );
    }

    #[test]
    fn parse_property_value_rejects_invalid_inputs() {
        assert!(
            property::parse_property_value(
                &PropertyControl::Slider {
                    min: 0.0,
                    max: 10.0,
                    step: 0.1,
                    initial: 1.0,
                },
                None,
            )
            .is_err()
        );
        assert!(
            property::parse_property_value(
                &PropertyControl::Slider {
                    min: 0.0,
                    max: 10.0,
                    step: 0.1,
                    initial: 1.0,
                },
                Some(&"abc".to_string()),
            )
            .is_err()
        );

        assert!(
            property::parse_property_value(
                &PropertyControl::Toggle { initial: false },
                Some(&"maybe".to_string()),
            )
            .is_err()
        );

        assert!(
            property::parse_property_value(
                &PropertyControl::Select {
                    options: vec!["a".into()],
                    initial: 0,
                },
                Some(&"z".to_string()),
            )
            .is_err()
        );
    }

    #[test]
    fn parse_property_value_vec3_is_strict() {
        let control = PropertyControl::Vector3 {
            initial: Vec3::ZERO,
            step: 0.1,
        };

        assert!(property::parse_property_value(&control, Some(&"1,2".to_string())).is_err());
        assert!(property::parse_property_value(&control, Some(&"1,x,3".to_string())).is_err());
    }

    #[test]
    fn render_controls_contains_controls_and_quit() {
        let defs = vec![
            PropertyDefinition {
                id: make_entity(1),
                aspect_id: 0,
                label: "slider".into(),
                control: PropertyControl::Slider {
                    min: 0.0,
                    max: 1.0,
                    step: 0.1,
                    initial: 0.5,
                },
            },
            PropertyDefinition {
                id: make_entity(2),
                aspect_id: 0,
                label: "toggle".into(),
                control: PropertyControl::Toggle { initial: true },
            },
            PropertyDefinition {
                id: make_entity(3),
                aspect_id: 0,
                label: "select".into(),
                control: PropertyControl::Select {
                    options: vec!["A".into(), "B".into()],
                    initial: 1,
                },
            },
            PropertyDefinition {
                id: make_entity(4),
                aspect_id: 0,
                label: "string".into(),
                control: PropertyControl::String {
                    initial: "hello".into(),
                },
            },
            PropertyDefinition {
                id: make_entity(5),
                aspect_id: 0,
                label: "vec3".into(),
                control: PropertyControl::Vector3 {
                    initial: Vec3::new(1.0, 2.0, 3.0),
                    step: 0.1,
                },
            },
            PropertyDefinition {
                id: make_entity(6),
                aspect_id: 0,
                label: "button".into(),
                control: PropertyControl::Button,
            },
        ];

        let html = content::render_controls(&defs);
        assert!(html.contains("type=\"range\""));
        assert!(html.contains("type=\"checkbox\""));
        assert!(html.contains("<select"));
        assert!(html.contains("type=\"text\""));
        assert!(html.contains("sendVec3"));
        assert!(html.contains("Quit</button>"));
        assert!(html.contains(&format!("value-{}", defs[0].lookup_id())));
    }

    #[test]
    fn render_index_page_contains_expected_js_wiring() {
        let html = content::render_index_page("<div>controls</div>");
        assert!(html.contains(EVENT_PATH));
        assert!(html.contains("sendUpdate"));
        assert!(html.contains("sendVec3"));
        assert!(html.contains("quitApp"));
    }

    #[test]
    fn parse_form_urlencoded_decodes_plus_and_percent() {
        let map = parse_form_urlencoded(b"id=abc%2B123&value=hello+world%2C42");
        assert_eq!(map.get("id").map(String::as_str), Some("abc+123"));
        assert_eq!(map.get("value").map(String::as_str), Some("hello world,42"));
    }

    #[test]
    fn read_http_request_parses_get() {
        with_stream_pair(|client, server| {
            client
                .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")
                .expect("write request");
            client.shutdown(Shutdown::Write).expect("shutdown");

            let req = read_http_request(server).expect("request should parse");
            assert_eq!(req.method, "GET");
            assert_eq!(req.path, "/");
            assert!(req.body.is_empty());
        });
    }

    #[test]
    fn read_http_request_parses_post_body() {
        with_stream_pair(|client, server| {
            let body = "id=123&value=4.5";
            let request = make_request("POST", EVENT_PATH, body);
            client
                .write_all(request.as_bytes())
                .expect("write request bytes");
            client.shutdown(Shutdown::Write).expect("shutdown");

            let req = read_http_request(server).expect("request should parse");
            assert_eq!(req.method, "POST");
            assert_eq!(req.path, EVENT_PATH);
            assert_eq!(req.body, body.as_bytes());
        });
    }

    #[test]
    fn server_returns_expected_status_codes() {
        let property = PropertyDefinition {
            id: make_entity(100),
            aspect_id: 0,
            label: "speed".into(),
            control: PropertyControl::Slider {
                min: 0.0,
                max: 10.0,
                step: 0.1,
                initial: 1.0,
            },
        };

        let mut app = app_with_server(Some(vec![property]));

        let root = send_request(&mut app, &make_request("GET", "/", ""));
        assert!(root.starts_with("HTTP/1.1 200 OK"));

        let missing = send_request(&mut app, &make_request("POST", EVENT_PATH, "value=1.0"));
        assert!(missing.starts_with("HTTP/1.1 400 Bad Request"));

        let unknown = send_request(
            &mut app,
            &make_request("POST", EVENT_PATH, "id=99999&value=1.0"),
        );
        assert!(unknown.starts_with("HTTP/1.1 404 Not Found"));

        let bad_path = send_request(&mut app, &make_request("GET", "/nope", ""));
        assert!(bad_path.starts_with("HTTP/1.1 404 Not Found"));
    }

    #[test]
    fn valid_property_update_triggers_entity_observer_and_updates_world() {
        let mut app = App::new();
        let property = app.world_mut().spawn_empty().id();

        app.insert_resource(RemoteControlDefinitions(vec![PropertyDefinition {
            id: property,
            aspect_id: 0,
            label: "speed".into(),
            control: PropertyControl::Slider {
                min: 0.0,
                max: 10.0,
                step: 0.1,
                initial: 1.0,
            },
        }]));

        app.world_mut().entity_mut(property).observe(
            |trigger: On<RemoteControlEvent>, mut commands: Commands| {
                if let PropertyValue::Float(v) = trigger.event().value {
                    commands.entity(trigger.entity).insert(AppliedFloat(v));
                }
            },
        );

        app.add_plugins(RemoteControlPlugin {
            bind_addr: "127.0.0.1:0".into(),
        });

        app.update();
        app.update();
        app.update();

        let body = format!("id={}:0&value=3.5", property.to_bits());
        let response = send_request(&mut app, &make_request("POST", EVENT_PATH, &body));
        //println!("Response: {response}");
        assert!(response.starts_with("HTTP/1.1 200 OK"));

        // Apply deferred observer commands.
        app.update();

        let applied = app
            .world()
            .entity(property)
            .get::<AppliedFloat>()
            .expect("observer should update entity");
        assert_eq!(applied.0, 3.5);
    }

    #[test]
    fn same_entity_multiple_aspects_dispatches_with_aspect_id() {
        let mut app = App::new();
        let property = app.world_mut().spawn_empty().id();

        app.insert_resource(RemoteControlDefinitions(vec![
            PropertyDefinition {
                id: property,
                aspect_id: 0,
                label: "speed".into(),
                control: PropertyControl::Slider {
                    min: 0.0,
                    max: 10.0,
                    step: 0.1,
                    initial: 1.0,
                },
            },
            PropertyDefinition {
                id: property,
                aspect_id: 1,
                label: "alt speed".into(),
                control: PropertyControl::Slider {
                    min: 0.0,
                    max: 10.0,
                    step: 0.1,
                    initial: 1.0,
                },
            },
        ]));

        app.world_mut().entity_mut(property).observe(
            |trigger: On<RemoteControlEvent>, mut commands: Commands| {
                if matches!(trigger.event().value, PropertyValue::Float(_)) {
                    commands
                        .entity(trigger.entity)
                        .insert(AppliedAspect(trigger.event().aspect_id));
                }
            },
        );

        app.add_plugins(RemoteControlPlugin {
            bind_addr: "127.0.0.1:0".into(),
        });

        app.update();
        app.update();
        app.update();

        let body = format!("id={}:1&value=3.5", property.to_bits());
        let response = send_request(&mut app, &make_request("POST", EVENT_PATH, &body));
        assert!(response.starts_with("HTTP/1.1 200 OK"));

        app.update();

        let applied = app
            .world()
            .entity(property)
            .get::<AppliedAspect>()
            .expect("observer should get aspect id");
        assert_eq!(applied.0, 1);
    }

    #[test]
    fn updating_definitions_inserts_server() {
        let mut app = app_with_server(Some(vec![PropertyDefinition {
            id: make_entity(200),
            aspect_id: 0,
            label: "button".into(),
            control: PropertyControl::Button,
        }]));

        assert!(app.world().contains_resource::<RemoteControlServer>());

        // Exercise quit path to ensure endpoint is reachable.
        let response = send_request(
            &mut app,
            &make_request("POST", EVENT_PATH, "id=__tephrite_quit"),
        );
        assert!(response.starts_with("HTTP/1.1 200 OK"));
    }

    #[test]
    fn updating_definitions_replaces_server_resource() {
        let mut app = app_with_server(Some(vec![PropertyDefinition {
            id: make_entity(300),
            aspect_id: 0,
            label: "first".into(),
            control: PropertyControl::Button,
        }]));
        let first_addr = server_addr(&app);

        {
            let mut defs = app.world_mut().resource_mut::<RemoteControlDefinitions>();
            defs.0 = vec![PropertyDefinition {
                id: make_entity(301),
                aspect_id: 0,
                label: "second".into(),
                control: PropertyControl::Button,
            }];
        }
        app.update();
        app.update();

        let second_addr = server_addr(&app);
        assert_ne!(first_addr, second_addr);
    }
}

pub mod prelude {
    pub use super::RemoteControlDefinitions;
    pub use super::RemoteControlPlugin;
    pub use super::events::RemoteControlEvent;
    pub use super::property::PropertyControl;
    pub use super::property::PropertyDefinition;

    pub use super::use_cases::*;
}
