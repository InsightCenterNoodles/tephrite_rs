use super::*;
use crate::remote_control::common::{
    API_DEBUG_ENABLE_PATH, API_ENTITIES_PATH, API_TRANSFORM_LOOK_AT_PATH,
    API_TRANSFORM_POSITION_PATH, EVENT_PATH, PropertyValue,
};
use crate::remote_control::property::PropertyControl;
use crate::remote_control::server::{RemoteControlState, parse_form_urlencoded};
use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::time::{Duration, Instant};

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
    app.add_plugins(MinimalPlugins);
    app.add_plugins(RemoteControlPlugin {
        bind_addr: "127.0.0.1:0".into(),
        brp_port: None,
    });
    if let Some(definitions) = definitions {
        let mut defs = app.world_mut().resource_mut::<RemoteControlDefinitions>();
        defs.0 = definitions;
    }
    app.update();
    app.update();
    app
}

fn server_addr(app: &mut App) -> std::net::SocketAddr {
    let world = app.world_mut();
    let mut query = world.query::<&crate::http::HTTPServer>();
    query
        .iter(world)
        .next()
        .expect("HTTP server should be spawned")
        .local_addr()
        .expect("listener should be bound")
}

fn send_request(app: &mut App, request: &str) -> String {
    let addr = server_addr(app);
    send_request_to_addr(app, addr, request)
}

fn send_request_to_addr(app: &mut App, addr: SocketAddr, request: &str) -> String {
    let mut client = TcpStream::connect(addr).expect("connect");
    client
        .set_read_timeout(Some(Duration::from_millis(50)))
        .expect("set read timeout");
    client.write_all(request.as_bytes()).expect("write request");
    client.shutdown(Shutdown::Write).expect("shutdown write");

    let mut response = String::new();
    let deadline = Instant::now() + Duration::from_secs(1);
    while Instant::now() < deadline {
        app.update();

        match client.read_to_string(&mut response) {
            Ok(_) => return response,
            Err(err)
                if matches!(
                    err.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                continue;
            }
            Err(err)
                if err.kind() == std::io::ErrorKind::ConnectionReset && !response.is_empty() =>
            {
                return response;
            }
            Err(err) => panic!("read response body: {err}"),
        }
    }

    panic!("timed out waiting for remote-control response; partial response: {response:?}");
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
        property::parse_property_value(
            &PropertyControl::Analog {
                initial: Vec2::ZERO,
            },
            Some(&"0.5,-0.25".to_string()),
        ),
        Ok(PropertyValue::Vec2(Vec2::new(0.5, -0.25)))
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
fn parse_property_value_vec2_is_strict() {
    let control = PropertyControl::Analog {
        initial: Vec2::ZERO,
    };

    assert!(property::parse_property_value(&control, Some(&"1".to_string())).is_err());
    assert!(property::parse_property_value(&control, Some(&"1,y".to_string())).is_err());
}

#[test]
fn render_controls_contains_controls() {
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
        PropertyDefinition {
            id: make_entity(7),
            aspect_id: 0,
            label: "analog".into(),
            control: PropertyControl::Analog {
                initial: Vec2::new(0.2, -0.4),
            },
        },
    ];

    let html = content::render_controls(&defs);
    assert!(html.contains("type=\"range\""));
    assert!(html.contains("type=\"checkbox\""));
    assert!(html.contains("<select"));
    assert!(html.contains("type=\"text\""));
    assert!(html.contains("sendVec3"));
    assert!(html.contains("class=\"analog\""));
    assert!(!html.contains("Quit</button>"));
    assert!(html.contains(&format!("value-{}", defs[0].lookup_id())));
}

#[test]
fn render_index_page_contains_expected_js_wiring() {
    let html = content::render_index_page("<div>controls</div>", Some(15702));
    assert!(html.contains(EVENT_PATH));
    assert!(html.contains(API_ENTITIES_PATH));
    assert!(html.contains(API_TRANSFORM_LOOK_AT_PATH));
    assert!(html.contains("sendUpdate"));
    assert!(html.contains("window.teph"));
    assert!(html.contains("window.teph_quick"));
    assert!(html.contains("jquery.terminal.min.js"));
    assert!(html.contains("jquery.terminal.min.css"));
    assert!(html.contains("Scene Terminal"));
    assert!(html.contains("evaluateTerminalCommand"));
    assert!(html.contains("executeTerminalCommand"));
    assert!(html.contains("Unknown command or variable"));
    assert!(html.contains("help teph_quick"));
    assert!(html.contains("formatEntityList"));
    assert!(html.contains("formatComponentList"));
    assert!(html.contains("ls --all"));
    assert!(html.contains("ls <name or id>"));
    assert!(html.contains("listEntityComponents"));
    assert!(html.contains("Enable Debugging"));
    assert!(html.contains("enableDebugging"));
    assert!(html.contains(API_DEBUG_ENABLE_PATH));
    assert!(html.contains("text/plain;charset=UTF-8"));
    assert!(html.contains("rpc.discover"));
    assert!(html.contains("world.query"));
    assert!(html.contains("world.get_components"));
    assert!(html.contains("world.spawn_entity"));
    assert!(html.contains("world.mutate_components"));
    assert!(html.contains("tephVec3ArrayArgs"));
    assert!(html.contains("world.get_resources"));
    assert!(html.contains("schedule.graph"));
    assert!(html.contains(":15702"));
    assert!(html.contains("sendVec3"));
    assert!(html.contains("setupAnalog"));
    assert!(html.contains("quitApp"));
    assert!(html.contains("Quit</button>"));
}

#[test]
fn parse_form_urlencoded_decodes_plus_and_percent() {
    let map = parse_form_urlencoded(b"id=abc%2B123&value=hello+world%2C42");
    assert_eq!(map.get("id").map(String::as_str), Some("abc+123"));
    assert_eq!(map.get("value").map(String::as_str), Some("hello world,42"));
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
    assert!(root.contains("Scene Terminal"));

    let jquery = send_request(&mut app, &make_request("GET", content::JQUERY_JS_PATH, ""));
    assert!(jquery.starts_with("HTTP/1.1 200 OK"));
    assert!(jquery.contains("application/javascript"));

    let terminal_js = send_request(
        &mut app,
        &make_request("GET", content::JQUERY_TERMINAL_JS_PATH, ""),
    );
    assert!(terminal_js.starts_with("HTTP/1.1 200 OK"));
    assert!(terminal_js.contains("application/javascript"));

    let terminal_css = send_request(
        &mut app,
        &make_request("GET", content::JQUERY_TERMINAL_CSS_PATH, ""),
    );
    assert!(terminal_css.starts_with("HTTP/1.1 200 OK"));
    assert!(terminal_css.contains("text/css"));

    let enable_debug = send_request(&mut app, &make_request("POST", API_DEBUG_ENABLE_PATH, ""));
    assert!(enable_debug.starts_with("HTTP/1.1 200 OK"));
    assert!(enable_debug.contains("\"brp_port\":null"));
    app.update();
    assert!(app.world().contains_resource::<RemoteDebuggingEnabled>());

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
fn api_entities_finds_named_entities() {
    let mut app = app_with_server(Some(vec![]));

    let entity = app
        .world_mut()
        .spawn((Name::new("bob"), Transform::default()))
        .id();

    let response = send_request(
        &mut app,
        &make_request("GET", &format!("{API_ENTITIES_PATH}?name=bob"), ""),
    );
    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.contains(&format!("\"id\":\"{}\"", entity.to_bits())));
    assert!(response.contains("\"name\":\"bob\""));
    assert!(response.contains("\"has_transform\":true"));
}

#[test]
fn api_transform_sets_position_by_name() {
    let mut app = app_with_server(Some(vec![]));

    let entity = app
        .world_mut()
        .spawn((Name::new("bob"), Transform::default()))
        .id();

    let response = send_request(
        &mut app,
        &make_request(
            "POST",
            API_TRANSFORM_POSITION_PATH,
            "target=bob&x=4&y=5&z=2",
        ),
    );
    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert_eq!(
        app.world()
            .entity(entity)
            .get::<Transform>()
            .unwrap()
            .translation,
        Vec3::new(4.0, 5.0, 2.0)
    );
    assert!(response.contains("\"translation\":[4,5,2]"));
}

#[test]
fn api_transform_look_at_rotates_entity_by_name() {
    let mut app = app_with_server(Some(vec![]));

    let entity = app
        .world_mut()
        .spawn((Name::new("bob"), Transform::default()))
        .id();

    let response = send_request(
        &mut app,
        &make_request(
            "POST",
            API_TRANSFORM_LOOK_AT_PATH,
            "target=bob&x=0&y=0&z=-1",
        ),
    );
    assert!(response.starts_with("HTTP/1.1 200 OK"));

    let forward = app
        .world()
        .entity(entity)
        .get::<Transform>()
        .unwrap()
        .forward();
    assert!(forward.dot(Vec3::NEG_Z) > 0.999);
}

#[test]
fn valid_property_update_triggers_entity_observer_and_updates_world() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
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
        brp_port: None,
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
    app.add_plugins(MinimalPlugins);
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
        brp_port: None,
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

    assert!(app.world().contains_resource::<RemoteControlState>());
    let world = app.world_mut();
    let mut query = world.query::<&crate::http::HTTPServer>();
    assert!(query.iter(world).next().is_some());

    // Exercise quit path to ensure endpoint is reachable.
    let response = send_request(
        &mut app,
        &make_request("POST", EVENT_PATH, "id=__tephrite_quit"),
    );
    assert!(response.starts_with("HTTP/1.1 200 OK"));
}
