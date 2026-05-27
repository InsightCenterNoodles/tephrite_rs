use std::{path::Path, str::FromStr};

use bevy::math::UVec2;

use super::*;

#[test]
fn loads_example_asset_and_builds_configs() {
    // Point the config loader to the example asset
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let example_path = Path::new(manifest_dir)
        .join("assets")
        .join("config_example.toml");

    assert!(
        example_path.exists(),
        "Example config not found at {:?}",
        example_path
    );

    // Ensure the config file is discovered
    unsafe { std::env::set_var("TEPH_CONFIG_PATH", &example_path) };

    // Validate logic configuration derived from the example
    let config = get_configuration();
    let head = config.vrpn.head.as_ref().unwrap();
    assert_eq!(config.child_count(), 12);
    assert_eq!(head.sender, "Head0");
    assert_eq!(head.host, "10.79.144.3");
    assert_eq!(head.port, 3883);
    assert!(matches!(
        config.vrpn.coordinate_transform,
        VRPNCoordinateTransform::Vicon
    ));

    // Prepare render context for child 0 and validate render configuration
    unsafe { std::env::set_var("TEPHRITE_CHILD_PROCESS", "0") };
    let render = get_render_configuration();
    assert_eq!(render.process_rank, 0);
    assert_eq!(render.card_index, Some(4));
    assert_eq!(render.display_name.as_deref(), Some(":0.0"));
    assert_eq!(render.resolution, UVec2::new(1920, 1200));
}

#[test]
fn parses_vrpn_address_without_sensor() {
    let address: VRPNAddress = "Head0@127.0.0.1:3883".parse().unwrap();

    assert_eq!(address.sender, "Head0");
    assert_eq!(address.host, "127.0.0.1");
    assert_eq!(address.port, 3883);
    assert_eq!(address.sensor, None);
}

#[test]
fn parses_vrpn_address_with_sensor() {
    let address: VRPNAddress = "Head0/3@127.0.0.1:3883".parse().unwrap();

    assert_eq!(address.sender, "Head0");
    assert_eq!(address.host, "127.0.0.1");
    assert_eq!(address.port, 3883);
    assert_eq!(address.sensor, Some(3));
}

#[test]
fn rejects_malformed_vrpn_addresses() {
    for address in [
        "@127.0.0.1:3883",
        "/1@127.0.0.1:3883",
        "Head0@",
        "Head0@:3883",
        "Head0@127.0.0.1",
        "Head0@127.0.0.1:",
        "Head0@127.0.0.1:3883:extra",
        "Head0@127.0.0.1:3883@extra",
        "Head0/@127.0.0.1:3883",
        "Head0/1/2@127.0.0.1:3883",
        "Head0/not-a-number@127.0.0.1:3883",
    ] {
        assert!(
            VRPNAddress::from_str(address).is_err(),
            "{address} should not parse"
        );
    }
}
