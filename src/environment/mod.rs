use bevy::{asset::embedded_asset, prelude::*};

use crate::{
    common::Head,
    config::{AlertImage, get_configuration},
    replication::components::Replicated,
};

pub(crate) fn environment_plugin(app: &mut App) {
    embedded_asset!(app, "alert_forward.png");
    embedded_asset!(app, "alert_left.png");
    embedded_asset!(app, "alert_rear.png");
    embedded_asset!(app, "alert_right.png");

    app.add_systems(PostStartup, setup_alerts);
    app.add_systems(Update, update_alert);
}

#[derive(Debug, Resource)]
struct AlertEnvironment {
    zones: Vec<AlertPlane>,
    forward: Handle<StandardMaterial>,
    left: Handle<StandardMaterial>,
    rear: Handle<StandardMaterial>,
    right: Handle<StandardMaterial>,
}

const ALERT_SMOOTHING_HALF_LIFE_SECS: f32 = 0.08;

#[derive(Debug)]
struct AlertPlane {
    point: Vec3,
    normal: Vec3,
    distance: f32,
    image: AlertImage,
}

#[derive(Debug, Component)]
struct AlertDisplay {
    offset: Vec3,
    scale: f32,
    was_visible: bool,
}

#[derive(Debug, Component)]
struct AlertCube;

fn setup_alerts(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
) {
    let config = &get_configuration().environment;

    if config.alerts.is_empty() {
        return;
    }

    let zones = config
        .alerts
        .iter()
        .filter_map(|alert| {
            let Some(normal) = Vec3::from(alert.plane_normal).try_normalize() else {
                warn!("Skipping alert zone with zero plane_normal");
                return None;
            };

            Some(AlertPlane {
                point: alert.plane_point.into(),
                normal,
                distance: alert.distance.max(0.0),
                image: alert.image,
            })
        })
        .collect::<Vec<_>>();

    if zones.is_empty() {
        return;
    }

    let forward = alert_material(AlertImage::Forward, &asset_server, &mut materials);
    let left = alert_material(AlertImage::Left, &asset_server, &mut materials);
    let rear = alert_material(AlertImage::Rear, &asset_server, &mut materials);
    let right = alert_material(AlertImage::Right, &asset_server, &mut materials);

    let mesh = meshes.add(Rectangle::new(1.0, 1.0));

    commands.spawn((
        Replicated,
        AlertDisplay {
            offset: config.alert_offset.into(),
            scale: config.alert_scale,
            was_visible: false,
        },
        Mesh3d(mesh),
        MeshMaterial3d(forward.clone()),
        Transform::from_scale(Vec3::splat(config.alert_scale)),
        Visibility::Hidden,
    ));

    let cube_material = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.0, 0.0),
        unlit: true,
        ..Default::default()
    });

    for cube in &config.alert_cubes {
        let half_extents = Vec3::from(cube.half_extents).abs();

        if half_extents.min_element() <= 0.0 {
            warn!("Skipping alert cube with non-positive half_extents");
            continue;
        }

        commands.spawn((
            Replicated,
            AlertCube,
            Mesh3d(meshes.add(Cuboid::new(
                half_extents.x * 2.0,
                half_extents.y * 2.0,
                half_extents.z * 2.0,
            ))),
            MeshMaterial3d(cube_material.clone()),
            Transform::from_translation(cube.center.into()),
            Visibility::Hidden,
        ));
    }

    commands.insert_resource(AlertEnvironment {
        zones,
        forward,
        left,
        rear,
        right,
    });
}

fn update_alert(
    environment: Option<Res<AlertEnvironment>>,
    time: Res<Time>,
    head: Query<&GlobalTransform, With<Head>>,
    mut display: Query<
        (
            &mut AlertDisplay,
            &mut Transform,
            &mut Visibility,
            &mut MeshMaterial3d<StandardMaterial>,
        ),
        With<AlertDisplay>,
    >,
    mut cubes: Query<&mut Visibility, (With<AlertCube>, Without<AlertDisplay>)>,
) {
    let Some(environment) = environment else {
        return;
    };

    let Some(head_transform) = head.iter().next() else {
        return;
    };

    let Ok((mut display, mut transform, mut visibility, mut material)) = display.single_mut()
    else {
        return;
    };

    let head_position = head_transform.translation();

    let Some(alert) = environment.zones.iter().find(|alert| {
        let distance = (head_position - alert.point).dot(alert.normal).abs();
        distance <= alert.distance
    }) else {
        *visibility = Visibility::Hidden;
        display.was_visible = false;
        set_cube_visibility(&mut cubes, Visibility::Hidden);
        return;
    };

    let target_transform = alert_target_transform(head_transform, &display);
    smooth_alert_transform(
        &mut transform,
        target_transform,
        time.delta_secs(),
        display.was_visible,
    );

    *visibility = Visibility::Visible;
    display.was_visible = true;
    *material = MeshMaterial3d(environment.material(alert.image).clone());
    set_cube_visibility(&mut cubes, Visibility::Visible);
}

fn alert_target_transform(head_transform: &GlobalTransform, display: &AlertDisplay) -> Transform {
    let (_, head_rotation, _) = head_transform.to_scale_rotation_translation();

    Transform {
        translation: head_transform.transform_point(display.offset),
        rotation: head_rotation,
        scale: Vec3::splat(display.scale),
    }
}

fn smooth_alert_transform(
    transform: &mut Transform,
    target: Transform,
    delta_seconds: f32,
    was_visible: bool,
) {
    if !was_visible {
        *transform = target;
        return;
    }

    let alpha = 1.0 - 2.0_f32.powf(-delta_seconds / ALERT_SMOOTHING_HALF_LIFE_SECS);
    let alpha = alpha.clamp(0.0, 1.0);

    transform.translation = transform.translation.lerp(target.translation, alpha);
    transform.rotation = transform.rotation.slerp(target.rotation, alpha);
    transform.scale = transform.scale.lerp(target.scale, alpha);
}

fn set_cube_visibility(
    cubes: &mut Query<&mut Visibility, (With<AlertCube>, Without<AlertDisplay>)>,
    target: Visibility,
) {
    for mut visibility in cubes {
        *visibility = target;
    }
}

impl AlertEnvironment {
    fn material(&self, image: AlertImage) -> &Handle<StandardMaterial> {
        match image {
            AlertImage::Forward => &self.forward,
            AlertImage::Left => &self.left,
            AlertImage::Rear => &self.rear,
            AlertImage::Right => &self.right,
        }
    }
}

fn alert_material(
    image: AlertImage,
    asset_server: &AssetServer,
    materials: &mut Assets<StandardMaterial>,
) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color_texture: Some(load_alert_image(image, asset_server)),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        double_sided: true,
        ..Default::default()
    })
}

fn load_alert_image(image: AlertImage, asset_server: &AssetServer) -> Handle<Image> {
    match image {
        AlertImage::Forward => {
            asset_server.load("embedded://tephrite_rs/environment/alert_forward.png")
        }
        AlertImage::Left => asset_server.load("embedded://tephrite_rs/environment/alert_left.png"),
        AlertImage::Rear => asset_server.load("embedded://tephrite_rs/environment/alert_rear.png"),
        AlertImage::Right => {
            asset_server.load("embedded://tephrite_rs/environment/alert_right.png")
        }
    }
}
