use bevy::{
    asset::RenderAssetUsages,
    color::palettes::css::*,
    math::dvec3,
    mesh::{PrimitiveTopology, VertexAttributeValues},
    prelude::*,
};
use tephrite_rs::prelude::*;

struct MyPlugin;

impl Plugin for MyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup);
    }
}

impl tephrite_rs::TephriteApp for MyPlugin {}

/// set up a simple 3D scene
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut point_materials: ResMut<Assets<PointsMaterial>>,
) {
    // circular base
    commands.spawn((
        Mesh3d(meshes.add(Circle::new(4.0))),
        MeshMaterial3d(materials.add(Color::WHITE)),
        Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
    ));

    let points = vec![
        Point {
            x: 0.1,
            y: 0.1,
            z: 0.1,
            color: Some(GREEN.into()),
        },
        Point {
            x: 0.2,
            y: 0.2,
            z: 0.2,
            color: Some(BLUE.into()),
        },
        Point {
            x: 0.3,
            y: 0.3,
            z: 0.3,
            color: Some(RED.into()),
        },
    ];

    let mesh = meshes.add(points_to_mesh(&points, Vec3::default(), 0.1).unwrap());

    commands.spawn((
        Mesh3d(mesh.clone()),
        MeshMaterial3d(point_materials.add(PointsMaterial {
            use_vertex_color: true,
            ..Default::default()
        })),
        Transform::from_xyz(0.2, 1.0, 0.0),
    ));

    // light
    commands.spawn((
        DirectionalLight {
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(0.0, 5.0, 3.0).looking_at((0.0, 0.0, 0.0).into(), Dir3::Y),
    ));
}

/// A three dimensional point.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Point {
    /// The x coordinate, as a float.
    pub x: f64,

    /// The y coordinate, as a float.
    pub y: f64,

    /// The z coordinate, as a float.
    pub z: f64,

    /// This point's color.
    pub color: Option<Color>,
}

pub fn points_to_mesh(pts: &[Point], origin: Vec3, spacing: f32) -> Option<Mesh> {
    if pts.is_empty() {
        return None;
    }

    let origin = origin.as_dvec3();
    let point_positions: Vec<Vec3> = pts
        .iter()
        .map(|p| (dvec3(p.x, p.y, p.z) - origin).as_vec3())
        .collect();

    let billboard_shape = billboard_shape_offsets((0.75 * spacing).max(1e-4));
    let vertices_per_point = billboard_shape.len();

    let mut positions = Vec::with_capacity(point_positions.len() * vertices_per_point);
    let mut billboard_offsets = Vec::with_capacity(point_positions.len() * vertices_per_point);
    let mut colors = Vec::with_capacity(point_positions.len() * vertices_per_point);

    for (&center, point) in point_positions.iter().zip(pts.iter()) {
        for offset in &billboard_shape {
            positions.push(center.to_array());
            billboard_offsets.push(offset.to_array());
        }

        let color = point
            .color
            .unwrap_or(Color::WHITE)
            .to_srgba()
            .to_f32_array();

        colors.extend(std::iter::repeat_n(color, vertices_per_point));
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::all());
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        VertexAttributeValues::Float32x3(positions),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_UV_0,
        VertexAttributeValues::Float32x2(billboard_offsets),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_COLOR,
        VertexAttributeValues::Float32x4(colors),
    );

    Some(mesh)
}

fn billboard_shape_offsets(radius: f32) -> Vec<Vec2> {
    vec![
        Vec2::new(-radius, -radius),
        Vec2::new(radius, -radius),
        Vec2::new(radius, radius),
        Vec2::new(-radius, -radius),
        Vec2::new(radius, radius),
        Vec2::new(-radius, radius),
    ]
}

fn main() {
    tephrite_rs::run(MyPlugin);
}
