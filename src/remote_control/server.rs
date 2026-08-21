use std::collections::HashMap;

use bevy::prelude::*;
use bytes::Bytes;
use http::{Request, Response, StatusCode};

use crate::http::{HTTPNodeHandler, HTTPResources};
use crate::remote_control::common::*;
use crate::remote_control::content;
use crate::remote_control::events;
use crate::remote_control::property::{PropertyDefinition, parse_property_value};
use crate::remote_control::scene_api::{
    parse_vec3_fields, render_entities_json, render_transform_json, resolve_entity,
};
use crate::remote_control::{RemoteControlDefinitions, RemoteControlOpts};

pub(super) fn register_http_handlers(resources: &mut HTTPResources) {
    resources.insert(INDEX_PATH.into(), IndexHandler);
    resources.insert(EVENT_PATH.into(), EventHandler);
    resources.insert(
        content::JQUERY_JS_PATH.into(),
        StaticAssetHandler {
            content_type: "application/javascript; charset=utf-8",
            body: content::JQUERY_JS,
        },
    );
    resources.insert(
        content::JQUERY_TERMINAL_JS_PATH.into(),
        StaticAssetHandler {
            content_type: "application/javascript; charset=utf-8",
            body: content::JQUERY_TERMINAL_JS,
        },
    );
    resources.insert(
        content::JQUERY_TERMINAL_CSS_PATH.into(),
        StaticAssetHandler {
            content_type: "text/css; charset=utf-8",
            body: content::JQUERY_TERMINAL_CSS,
        },
    );
    resources.insert(API_ENTITIES_PATH.into(), EntitiesHandler);
    resources.insert(API_TRANSFORM_PATH.into(), TransformHandler);
    resources.insert(API_TRANSFORM_POSITION_PATH.into(), TransformPositionHandler);
    resources.insert(API_TRANSFORM_LOOK_AT_PATH.into(), TransformLookAtHandler);
    resources.insert(API_DEBUG_ENABLE_PATH.into(), DebugEnableHandler);
}

/// Rebuild server-rendered control state when definitions change.
pub(super) fn check_updates(
    mut commands: Commands,
    opts: Res<RemoteControlOpts>,
    defs: Res<RemoteControlDefinitions>,
    server: Option<ResMut<RemoteControlState>>,
) {
    if !defs.is_changed() {
        return;
    }

    info!("Remote control definitions changed, updating HTTP handlers...");

    let state = RemoteControlState::new(opts.brp_port, defs.0.clone());
    if let Some(mut server) = server {
        *server = state;
    } else {
        commands.insert_resource(state);
    }
}

/// Shared state used by generic HTTP handlers.
#[derive(Debug, Resource)]
pub struct RemoteControlState {
    brp_port: Option<u16>,
    index_page: String,
    property_lookup: HashMap<String, PropertyDefinition>,
}

impl RemoteControlState {
    fn new(brp_port: Option<u16>, mut properties: Vec<PropertyDefinition>) -> Self {
        properties.sort_by_key(|x| x.id);

        let rendered_controls = content::render_controls(&properties);
        let index_page = content::render_index_page(&rendered_controls, brp_port);
        let property_lookup = build_property_lookup(&properties);

        Self {
            brp_port,
            index_page,
            property_lookup,
        }
    }
}

struct IndexHandler;
struct EventHandler;
struct EntitiesHandler;
struct TransformHandler;
struct TransformPositionHandler;
struct TransformLookAtHandler;
struct DebugEnableHandler;

struct StaticAssetHandler {
    content_type: &'static str,
    body: &'static [u8],
}

impl HTTPNodeHandler for IndexHandler {
    fn on_get(&self, world: &mut World, _request: &Request<Bytes>) -> Option<Response<Bytes>> {
        let Some(state) = world.get_resource::<RemoteControlState>() else {
            return Some(text_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "remote control not ready",
            ));
        };

        let _ = state.brp_port;
        Some(response(
            StatusCode::OK,
            "text/html; charset=utf-8",
            state.index_page.clone(),
        ))
    }
}

impl HTTPNodeHandler for StaticAssetHandler {
    fn on_get(&self, _world: &mut World, _request: &Request<Bytes>) -> Option<Response<Bytes>> {
        Some(response(
            StatusCode::OK,
            self.content_type,
            Bytes::from_static(self.body),
        ))
    }
}

impl HTTPNodeHandler for EntitiesHandler {
    fn on_get(&self, world: &mut World, request: &Request<Bytes>) -> Option<Response<Bytes>> {
        let query = parse_form_urlencoded(request.uri().query().unwrap_or_default().as_bytes());
        let name = query.get("name").map(String::as_str);
        Some(response(
            StatusCode::OK,
            "application/json; charset=utf-8",
            render_entities_json(world, name),
        ))
    }
}

impl HTTPNodeHandler for TransformHandler {
    fn on_get(&self, world: &mut World, request: &Request<Bytes>) -> Option<Response<Bytes>> {
        let query = parse_form_urlencoded(request.uri().query().unwrap_or_default().as_bytes());
        let Some(target) = query
            .get("target")
            .or_else(|| query.get("id"))
            .or_else(|| query.get("name"))
        else {
            return Some(text_response(StatusCode::BAD_REQUEST, "missing target"));
        };
        let Some(entity) = resolve_entity(world, target) else {
            return Some(text_response(StatusCode::NOT_FOUND, "unknown entity"));
        };
        let Some(transform) = world.get::<Transform>(entity) else {
            return Some(text_response(
                StatusCode::NOT_FOUND,
                "entity has no Transform",
            ));
        };

        Some(response(
            StatusCode::OK,
            "application/json; charset=utf-8",
            render_transform_json(entity, transform),
        ))
    }
}

impl HTTPNodeHandler for EventHandler {
    fn on_post(&self, world: &mut World, request: &Request<Bytes>) -> Option<Response<Bytes>> {
        let pairs = parse_form_urlencoded(request.body());
        let Some(id) = pairs.get("id") else {
            return Some(text_response(StatusCode::BAD_REQUEST, "missing id"));
        };

        if id == QUIT_ID {
            world
                .commands()
                .trigger(events::RemoteControlEventInternal::QuitRequested);
            return Some(text_response(StatusCode::OK, "ok"));
        }

        let property = {
            let Some(state) = world.get_resource::<RemoteControlState>() else {
                return Some(text_response(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "remote control not ready",
                ));
            };
            let Some(property) = state.property_lookup.get(id) else {
                return Some(text_response(StatusCode::NOT_FOUND, "unknown property"));
            };
            property.clone()
        };

        match parse_property_value(&property.control, pairs.get("value")) {
            Ok(value) => {
                world
                    .commands()
                    .trigger(events::RemoteControlEventInternal::PropertyChanged {
                        property: property.id,
                        aspect_id: property.aspect_id,
                        value,
                    });
                Some(text_response(StatusCode::OK, "ok"))
            }
            Err(message) => Some(text_response(StatusCode::BAD_REQUEST, message)),
        }
    }
}

impl HTTPNodeHandler for DebugEnableHandler {
    fn on_post(&self, world: &mut World, _request: &Request<Bytes>) -> Option<Response<Bytes>> {
        let brp_port = world
            .get_resource::<RemoteControlState>()
            .and_then(|state| state.brp_port);

        world
            .commands()
            .trigger(events::RemoteControlEventInternal::DebuggingRequested);

        let body = match brp_port {
            Some(port) => format!("{{\"ok\":true,\"brp_port\":{port}}}"),
            None => "{\"ok\":true,\"brp_port\":null}".to_string(),
        };

        Some(response(
            StatusCode::OK,
            "application/json; charset=utf-8",
            body,
        ))
    }
}

impl HTTPNodeHandler for TransformPositionHandler {
    fn on_post(&self, world: &mut World, request: &Request<Bytes>) -> Option<Response<Bytes>> {
        let pairs = parse_form_urlencoded(request.body());
        let Some(target) = pairs
            .get("target")
            .or_else(|| pairs.get("id"))
            .or_else(|| pairs.get("name"))
        else {
            return Some(text_response(StatusCode::BAD_REQUEST, "missing target"));
        };
        let Some(position) = parse_vec3_fields(&pairs, "x", "y", "z") else {
            return Some(text_response(StatusCode::BAD_REQUEST, "invalid position"));
        };
        let Some(entity) = resolve_entity(world, target) else {
            return Some(text_response(StatusCode::NOT_FOUND, "unknown entity"));
        };
        let Ok(mut entity_mut) = world.get_entity_mut(entity) else {
            return Some(text_response(StatusCode::NOT_FOUND, "unknown entity"));
        };
        let Some(mut transform) = entity_mut.get_mut::<Transform>() else {
            return Some(text_response(
                StatusCode::NOT_FOUND,
                "entity has no Transform",
            ));
        };
        transform.translation = position;

        Some(response(
            StatusCode::OK,
            "application/json; charset=utf-8",
            render_transform_json(entity, &transform),
        ))
    }
}

impl HTTPNodeHandler for TransformLookAtHandler {
    fn on_post(&self, world: &mut World, request: &Request<Bytes>) -> Option<Response<Bytes>> {
        let pairs = parse_form_urlencoded(request.body());
        let Some(target) = pairs
            .get("target")
            .or_else(|| pairs.get("id"))
            .or_else(|| pairs.get("name"))
        else {
            return Some(text_response(StatusCode::BAD_REQUEST, "missing target"));
        };
        let Some(point) = parse_vec3_fields(&pairs, "x", "y", "z") else {
            return Some(text_response(
                StatusCode::BAD_REQUEST,
                "invalid look-at target",
            ));
        };
        let up = parse_vec3_fields(&pairs, "up_x", "up_y", "up_z").unwrap_or(Vec3::Y);
        let Some(entity) = resolve_entity(world, target) else {
            return Some(text_response(StatusCode::NOT_FOUND, "unknown entity"));
        };
        let Ok(mut entity_mut) = world.get_entity_mut(entity) else {
            return Some(text_response(StatusCode::NOT_FOUND, "unknown entity"));
        };
        let Some(mut transform) = entity_mut.get_mut::<Transform>() else {
            return Some(text_response(
                StatusCode::NOT_FOUND,
                "entity has no Transform",
            ));
        };
        if transform.translation.distance_squared(point) <= f32::EPSILON {
            return Some(text_response(
                StatusCode::BAD_REQUEST,
                "look-at target matches entity position",
            ));
        }
        transform.look_at(point, up);

        Some(response(
            StatusCode::OK,
            "application/json; charset=utf-8",
            render_transform_json(entity, &transform),
        ))
    }
}

fn response(
    status: StatusCode,
    content_type: &'static str,
    body: impl Into<Bytes>,
) -> Response<Bytes> {
    Response::builder()
        .status(status)
        .header("Content-Type", content_type)
        .header("Connection", "close")
        .body(body.into())
        .expect("static response builder should be valid")
}

fn text_response(status: StatusCode, body: impl Into<Bytes>) -> Response<Bytes> {
    response(status, "text/plain; charset=utf-8", body)
}

/// Maps URL IDs (`<entity_bits>:<aspect_id>`) back to caller property definitions.
pub(super) fn build_property_lookup(
    properties: &[PropertyDefinition],
) -> HashMap<String, PropertyDefinition> {
    let mut map = HashMap::with_capacity(properties.len());
    for property in properties {
        map.insert(property.lookup_id(), property.clone());
    }
    map
}

/// Parse `application/x-www-form-urlencoded` request bodies.
pub(super) fn parse_form_urlencoded(body: &[u8]) -> HashMap<String, String> {
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
