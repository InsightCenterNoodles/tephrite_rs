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

#[derive(Debug)]
struct AlertPlane {
    point: Vec3,
    normal: Vec3,
    distance: f32,
    image: AlertImage,
}

#[derive(Debug, Component)]
struct AlertDisplay;

fn setup_alerts(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
    head: Query<Entity, With<Head>>,
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

    let Ok(head_entity) = head.single() else {
        warn!("Skipping alert setup because no single Head entity exists");
        return;
    };

    let forward = alert_material(AlertImage::Forward, &asset_server, &mut materials);
    let left = alert_material(AlertImage::Left, &asset_server, &mut materials);
    let rear = alert_material(AlertImage::Rear, &asset_server, &mut materials);
    let right = alert_material(AlertImage::Right, &asset_server, &mut materials);

    let mesh = meshes.add(Rectangle::new(1.0, 1.0));

    commands.spawn((
        Replicated,
        AlertDisplay,
        ChildOf(head_entity),
        Mesh3d(mesh),
        MeshMaterial3d(forward.clone()),
        Transform::from_translation(config.alert_offset.into())
            .with_scale(Vec3::splat(config.alert_scale)),
        Visibility::Hidden,
    ));

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
    head: Query<&GlobalTransform, With<Head>>,
    mut display: Query<
        (&mut Visibility, &mut MeshMaterial3d<StandardMaterial>),
        With<AlertDisplay>,
    >,
) {
    let Some(environment) = environment else {
        return;
    };

    let Some(head_position) = head.iter().next().map(GlobalTransform::translation) else {
        return;
    };

    let Ok((mut visibility, mut material)) = display.single_mut() else {
        return;
    };

    let Some(alert) = environment.zones.iter().find(|alert| {
        let distance = (head_position - alert.point).dot(alert.normal).abs();
        distance <= alert.distance
    }) else {
        *visibility = Visibility::Hidden;
        return;
    };

    *visibility = Visibility::Visible;
    *material = MeshMaterial3d(environment.material(alert.image).clone());
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
