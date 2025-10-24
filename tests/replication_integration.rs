// Integration test for replication: headless Bevy writer/reader over shared memory
// This models the shared-memory tests: one writer process and one reader process.

#![allow(clippy::needless_return)]
use std::time::Duration;

use bevy::app::ScheduleRunnerPlugin;
use bevy::pbr::{MaterialPlugin, MeshMaterial3d, StandardMaterial};
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::time::TimePlugin;

use tephrite_rs::multiprocess::{generate_session_id, install_ids, install_session_id};
use tephrite_rs::prelude::Replicated;

// Build a minimal headless Bevy app that supports Assets<Mesh> and Assets<StandardMaterial>
fn build_headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(ScheduleRunnerPlugin::run_loop(Duration::from_millis(2)))
        // Keep shader assets available (matches make_common_app behavior)
        .insert_resource(Assets::<Shader>::default())
        .add_plugins((
            TaskPoolPlugin::default(),
            TimePlugin,
            TransformPlugin,
            AssetPlugin::default(),
            bevy::render::mesh::MeshPlugin,
            bevy::render::texture::ImagePlugin::default(),
            MaterialPlugin::<StandardMaterial>::default(),
        ));
    app
}

// Create a tiny triangle mesh with positions + u16 indices
fn make_triangle_mesh() -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    let positions: Vec<[f32; 3]> = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_indices(Indices::U16(vec![0, 1, 2]));
    mesh
}

// Extract positions from a mesh as Float32x3 for comparison
fn mesh_positions(mesh: &Mesh) -> Option<Vec<[f32; 3]>> {
    mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        .and_then(|v| match v {
            VertexAttributeValues::Float32x3(v) => Some(v.clone()),
            _ => None,
        })
}

// Expected data to confirm replication
static EXPECTED_POSITIONS: [[f32; 3]; 3] = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
static EXPECTED_COLOR: Color = Color::srgba(0.3, 0.6, 1.0, 0.8);

fn replicates_mesh_and_material() {
    // Unique session; both apps will share it via env.
    let session = generate_session_id();
    install_session_id(&session);

    let mut app = build_headless_app();

    // One child (reader)
    app.add_plugins(tephrite_rs::replication::writer::ReplicationWriterPlugin::new(1));

    let mut child = {
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());

        command.env(PROCESS_KEY, "1");

        install_ids(&mut command, &session, 0);

        command.spawn().unwrap()
    };

    //std::thread::sleep(Duration::from_secs(1));

    // Create content in writer world
    let mesh_handle = {
        let mut meshes = app.world_mut().resource_mut::<Assets<Mesh>>();
        meshes.add(make_triangle_mesh())
    };
    let mat_handle = {
        let mut materials = app.world_mut().resource_mut::<Assets<StandardMaterial>>();
        materials.add(StandardMaterial {
            base_color: EXPECTED_COLOR,
            unlit: true,
            ..Default::default()
        })
    };

    // Spawn replicated entity
    app.world_mut().spawn((
        Replicated,
        Mesh3d(mesh_handle.clone()),
        MeshMaterial3d::<StandardMaterial>(mat_handle.clone()),
        Transform::from_xyz(1.0, 2.0, 3.0),
    ));

    // Run a few ticks to publish frames
    for _ in 0..6 {
        app.update();
    }

    // Ensure content is still what we expect locally
    let meshes = app.world().resource::<Assets<Mesh>>();
    let mesh = meshes.get(mesh_handle.id()).unwrap();
    let pos = mesh_positions(mesh).unwrap();
    assert_eq!(pos, EXPECTED_POSITIONS);

    assert!(child.wait().unwrap().success());
}

fn replicates_mesh_and_material_client() {
    // Spawn reader thread
    let mut app = build_headless_app();
    app.add_plugins(tephrite_rs::replication::reader::ReplicationReaderPlugin);

    // Step until at least some frames are consumed
    for _ in 0..6 {
        app.update();
    }

    // Find the replicated entity and extract its mesh/material data
    let mut q = app
        .world_mut()
        .query::<(&Mesh3d, &MeshMaterial3d<StandardMaterial>)>();
    let Some((mesh_handle, mat_handle)) = q.iter(&app.world()).next() else {
        panic!("expected a replicated entity with mesh+material");
    };

    let meshes = app.world().resource::<Assets<Mesh>>();
    let materials = app.world().resource::<Assets<StandardMaterial>>();

    // the way we order frames we should never ref an asset that has not been created

    let mesh = meshes
        .get(mesh_handle.id())
        .expect("replicated mesh should exist");
    let mat = materials
        .get(mat_handle.id())
        .expect("replicated material should exist");

    let positions = mesh_positions(mesh).expect("positions must exist");
    let color = mat.base_color;

    assert_eq!(EXPECTED_POSITIONS.as_slice(), positions.as_slice());
    assert_eq!(EXPECTED_COLOR, color);
}

const PROCESS_KEY: &str = "REPLICATION_CHILD";

fn main() {
    if std::env::var(PROCESS_KEY).is_ok() {
        replicates_mesh_and_material_client();
    } else {
        replicates_mesh_and_material();
    }
}
