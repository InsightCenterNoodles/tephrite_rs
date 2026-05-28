use bevy::{asset::embedded_asset, image::ImageSampler, prelude::*};

use crate::{
    common::Head,
    config::{AlertImage, AlertZone, get_configuration},
    replication::components::Replicated,
};

pub(crate) fn environment_plugin(app: &mut App) {
    embedded_asset!(app, "alert_forward.png");
    embedded_asset!(app, "alert_left.png");
    embedded_asset!(app, "alert_rear.png");
    embedded_asset!(app, "alert_right.png");

    app.add_systems(Startup, setup_alerts);
    app.add_systems(Update, update_alerts);
}

#[derive(Debug, Component)]
struct AlertPlane {
    point: Vec3,
    normal: Vec3,
    distance: f32,
}

fn setup_alerts(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    asset_server: Res<AssetServer>,
) {
    let mesh = meshes.add(Rectangle::new(1.0, 1.0));

    for alert in &get_configuration().environment.alerts {
        let Some(normal) = Vec3::from(alert.plane_normal).try_normalize() else {
            warn!("Skipping alert zone with zero plane_normal");
            continue;
        };

        let Some(direction) = Vec3::from(alert.direction).try_normalize() else {
            warn!("Skipping alert zone with zero direction");
            continue;
        };

        let image = load_alert_image(alert.image, &asset_server);
        if let Some(image_asset) = images.get_mut(&image) {
            image_asset.sampler = ImageSampler::linear();
        }

        let material = materials.add(StandardMaterial {
            base_color_texture: Some(image),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            double_sided: true,
            ..Default::default()
        });

        commands.spawn((
            Replicated,
            AlertPlane {
                point: alert.plane_point.into(),
                normal,
                distance: alert.distance.max(0.0),
            },
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material),
            alert_transform(alert, direction),
            Visibility::Hidden,
        ));
    }
}

fn update_alerts(
    head: Query<&GlobalTransform, With<Head>>,
    mut alerts: Query<(&AlertPlane, &mut Visibility)>,
) {
    let Some(head_position) = head.iter().next().map(GlobalTransform::translation) else {
        return;
    };

    for (alert, mut visibility) in &mut alerts {
        let distance = (head_position - alert.point).dot(alert.normal).abs();
        *visibility = if distance <= alert.distance {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
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

fn alert_transform(alert: &AlertZone, direction: Vec3) -> Transform {
    Transform::from_translation(alert.location.into())
        .with_rotation(Quat::from_rotation_arc(Vec3::Z, direction))
        .with_scale(Vec3::splat(alert.scale))
}
