//! Minimal in-process remote control webserver.
//!
//! This module is intentionally barebones for internal tooling:
//! - no authentication
//! - static control list configured at startup
//! - updates are forwarded over an `mpsc` channel
//!
//! The caller supplies a list of properties keyed by `Entity` handles.
//! A control web page is served, and user interactions produce
//! [`RemoteControlEvent`] values.

pub(crate) mod common;
pub(crate) mod content;
pub mod events;
pub mod property;

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::time::Duration;

use bevy::prelude::*;

use crate::remote_control::property::parse_property_value;

#[derive(Debug, Default, Resource)]
pub struct RemoteControlDefinitions(pub Vec<property::PropertyDefinition>);

#[derive(Debug, Default, Resource)]
pub struct RemoteControlOpts(String);

pub struct RemoteControlPlugin {
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
        app.add_message::<events::RemoteControlMessage>();
        app.insert_resource(RemoteControlOpts(self.bind_addr.clone()));
        app.add_systems(PostStartup, post_start);
        app.add_systems(Update, server_poll);
    }
}

fn post_start(
    mut commands: Commands,
    opts: Res<RemoteControlOpts>,
    defs: Option<Res<RemoteControlDefinitions>>,
) {
    let Some(defs) = defs else {
        return;
    };

    let Ok(server) = RemoteControlServer::new(&opts.0, defs.0.clone()) else {
        return;
    };

    commands.insert_resource(server);
}

/// Handle to the running remote control server thread.
#[derive(Debug, Resource)]
pub struct RemoteControlServer {
    server: TcpListener,

    index_page: String,
    property_lookup: HashMap<String, property::PropertyDefinition>,
}

impl RemoteControlServer {
    fn new(bind_addr: &str, properties: Vec<property::PropertyDefinition>) -> Result<Self> {
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
}

fn server_poll(
    server: ResMut<RemoteControlServer>,
    mut writer: MessageWriter<events::RemoteControlMessage>,
) {
    match server.server.accept() {
        Ok((mut stream, _addr)) => {
            // This intentionally handles one request per accepted connection.
            handle_connection(
                &mut stream,
                &server.index_page,
                &server.property_lookup,
                &mut writer,
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

/// Parse and serve a single HTTP request.
fn handle_connection(
    stream: &mut TcpStream,
    index_page: &str,
    property_lookup: &HashMap<String, property::PropertyDefinition>,
    writer: &mut MessageWriter<events::RemoteControlMessage>,
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
            // Form body is expected as `id=<entity_bits>&value=<encoded>`.
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
                writer.write(events::RemoteControlMessage::QuitRequested);
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
                    writer.write(events::RemoteControlMessage::PropertyChanged {
                        property: property.id,
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

/// Maps URL IDs (`Entity::to_bits`) back to caller property definitions.
fn build_property_lookup(
    properties: &[property::PropertyDefinition],
) -> HashMap<String, property::PropertyDefinition> {
    let mut map = HashMap::with_capacity(properties.len());
    for property in properties {
        map.insert(property.id.to_bits().to_string(), property.clone());
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
