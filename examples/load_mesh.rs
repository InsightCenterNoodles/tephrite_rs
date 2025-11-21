use std::path::{Path, PathBuf};

use bevy::{
    asset::RenderAssetUsages,
    image::CompressedImageFormats,
    mesh::{Indices, PrimitiveTopology},
    platform::collections::HashMap,
    prelude::*,
};
use tephrite_rs::prelude::*;

struct MyPlugin;

impl Plugin for MyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup);

        app.add_plugins(NavigationPlugin::new(NavigatorMode::ObjectCentric));

        let (tx, rx) = std::sync::mpsc::channel();

        std::thread::spawn(move || {
            if let Some(x) = load_mesh() {
                let _ = tx.send(x).inspect_err(|err| warn!("Unable to send: {err}"));
            }
        });

        app.insert_non_send_resource(DeferredMesh { channel: rx });

        app.add_systems(Update, deferred_mesh_watcher);
    }
}

/// Try to resolve a texture path referenced from a Wavefront MTL file.
///
/// `mtl_path`     – path to the `.mtl` file on disk  
/// `tex_ref`      – the raw string value from the MTL (e.g. "diffuse.png", "textures/diffuse.png")  
/// `extra_roots`  – optional extra directories to search (e.g. the directory of the `.obj`)
///
/// Returns `Some(PathBuf)` with the first existing file found, or `None` if nothing matched.
pub fn resolve_mtl_texture_path<P: AsRef<Path>>(
    mtl_path: P,
    tex_ref: &str,
    extra_roots: &[P],
) -> Option<PathBuf> {
    let mtl_path = mtl_path.as_ref();

    // Directory containing the .mtl file
    let mtl_dir = mtl_path.parent().unwrap_or_else(|| Path::new("."));

    // Normalize separators in the reference
    let tex_ref_norm = tex_ref.replace('\\', "/");
    let tex_ref_path = Path::new(&tex_ref_norm);

    // 1. If it's an absolute path and exists, just use it.
    if tex_ref_path.is_absolute() && tex_ref_path.is_file() {
        return Some(tex_ref_path.to_path_buf());
    }

    // Helper to check a candidate and return it if it exists.
    fn existing(p: PathBuf) -> Option<PathBuf> {
        if p.is_file() { Some(p) } else { None }
    }

    // 2. Try relative to the MTL directory
    if let Some(p) = existing(mtl_dir.join(tex_ref_path)) {
        return Some(p);
    }

    // 3. Try just the file name inside the MTL directory (in case the path in MTL has stale subdirs)
    if let Some(file_name) = tex_ref_path.file_name() {
        if let Some(p) = existing(mtl_dir.join(file_name)) {
            return Some(p);
        }
    }

    // 4. Try extra roots (e.g. the OBJ directory, or a global "assets" directory)
    for root in extra_roots {
        let root = root.as_ref();

        // root / tex_ref
        if let Some(p) = existing(root.join(tex_ref_path)) {
            return Some(p);
        }

        // root / file_name
        if let Some(file_name) = tex_ref_path.file_name() {
            if let Some(p) = existing(root.join(file_name)) {
                return Some(p);
            }
        }
    }

    None
}

struct ObjMaterial {
    /// Diffuse color of the material.
    pub diffuse: Option<[f32; 3]>,
    pub diffuse_texture: Option<Image>,
    pub normal_texture: Option<Image>,

    pub roughness: f32,
    pub metallic: f32,

    pub unknown_param: HashMap<String, String>,
}

type AMesh = (Vec<tobj::Model>, Option<Vec<ObjMaterial>>);

fn load_mesh() -> Option<AMesh> {
    let mesh = std::env::args().find(|x| x.starts_with("-m"))?;

    let mesh: PathBuf = mesh.strip_prefix("-m")?.into();

    let mesh = std::fs::canonicalize(mesh).ok()?;

    info!("Loading mesh {}", mesh.display());

    let result = tobj::load_obj(&mesh, &tobj::GPU_LOAD_OPTIONS)
        .inspect_err(|x| error!("Unable to load mesh {}: {x}", mesh.display()))
        .ok()?;

    use std::str::FromStr;

    let materials = result.1.ok().map(|x| {
        x.into_iter()
            .inspect(|x| debug!("{:?}", x.unknown_param))
            .map(|x| ObjMaterial {
                diffuse: x.diffuse,
                diffuse_texture: x
                    .diffuse_texture
                    .and_then(|f| resolve_mtl_texture_path(&mesh, &f, &[]))
                    .inspect(|x| info!("Found image at {}", x.display()))
                    .and_then(|f| image_from_file(&f, ImageFormat::Png, true)),
                roughness: x
                    .unknown_param
                    .get("Pr")
                    .and_then(|x| f32::from_str(&x).ok())
                    .unwrap_or(1.0),
                metallic: x
                    .unknown_param
                    .get("Pm")
                    .and_then(|x| f32::from_str(&x).ok())
                    .unwrap_or(1.0),

                normal_texture: x
                    .normal_texture
                    .and_then(|f| resolve_mtl_texture_path(&mesh, &f, &[]))
                    .inspect(|x| info!("Found image at {}", x.display()))
                    .and_then(|f| image_from_file(&f, ImageFormat::Png, true)),
                unknown_param: x.unknown_param.into_iter().collect(),
            })
            .collect()
    });

    Some((result.0, materials))
}

#[derive(Component)]
struct AttachTo;

struct DeferredMesh {
    channel: std::sync::mpsc::Receiver<AMesh>,
}

fn convert_mat(
    mat: ObjMaterial,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
) -> Handle<StandardMaterial> {
    let m = StandardMaterial {
        base_color: mat
            .diffuse
            .map(|x| Color::srgb_from_array(x))
            .unwrap_or(Color::WHITE),

        base_color_texture: mat.diffuse_texture.map(|x| images.add(x)),

        perceptual_roughness: mat.roughness,
        metallic: mat.metallic,

        normal_map_texture: mat.normal_texture.map(|x| images.add(x)),

        // others?
        ..Default::default()
    };

    materials.add(m)
}

pub(crate) struct MeshConverter {
    meshes: Vec<tobj::Mesh>,
}

// Quick hack from bevy_obj
impl MeshConverter {
    pub fn convert(&self) -> Mesh {
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );

        mesh.insert_indices(Indices::U32(self.indices()));
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.position());

        if self.has_uv() {
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uv());
        }

        if self.has_normal() {
            mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.normal());
        } else {
            mesh.compute_normals();
        }

        mesh
    }

    fn indices(&self) -> Vec<u32> {
        let count = self.meshes.iter().map(|m| m.indices.len()).sum();
        let mut data = Vec::with_capacity(count);
        let mut offset = 0;

        for mesh in &self.meshes {
            data.extend(mesh.indices.iter().map(|i| i + offset));
            offset += (mesh.positions.len() / 3) as u32;
        }

        data
    }

    fn position(&self) -> Vec<[f32; 3]> {
        let count = self.meshes.iter().map(|m| m.positions.len() / 3).sum();
        let mut data = Vec::with_capacity(count);

        for mesh in &self.meshes {
            data.append(&mut convert_vec3(&mesh.positions));
        }

        data
    }

    fn has_normal(&self) -> bool {
        !self.meshes.iter().any(|m| m.normals.is_empty())
    }

    fn normal(&self) -> Vec<[f32; 3]> {
        let count = self.meshes.iter().map(|m| m.normals.len() / 3).sum();
        let mut data = Vec::with_capacity(count);

        for mesh in &self.meshes {
            data.append(&mut convert_vec3(&mesh.normals));
        }

        data
    }

    fn has_uv(&self) -> bool {
        !self.meshes.iter().any(|m| m.texcoords.is_empty())
    }

    fn uv(&self) -> Vec<[f32; 2]> {
        let count = self.meshes.iter().map(|m| m.texcoords.len() / 2).sum();
        let mut data = Vec::with_capacity(count);

        for mesh in &self.meshes {
            data.append(&mut convert_uv(&mesh.texcoords));
        }

        data
    }
}

fn convert_vec3(vec: &[f32]) -> Vec<[f32; 3]> {
    vec.chunks_exact(3).map(|v| [v[0], v[1], v[2]]).collect()
}

fn convert_uv(uv: &[f32]) -> Vec<[f32; 2]> {
    uv.chunks_exact(2).map(|t| [t[0], 1.0 - t[1]]).collect()
}

fn deferred_mesh_watcher(
    res: NonSendMut<DeferredMesh>,
    mut commands: Commands,
    query: Query<Entity, With<AttachTo>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let Ok(x) = res.channel.recv() else {
        return;
    };

    let root = query.single().unwrap();

    let converted_mats: Option<Vec<_>> = x.1.map(|x| {
        x.into_iter()
            .map(|f| convert_mat(f, &mut materials, &mut images))
            .collect()
    });

    for (model_i, model) in x.0.into_iter().enumerate() {
        let Some(material) = converted_mats.as_ref().and_then(|x| x.get(model_i)) else {
            continue;
        };

        let converter = MeshConverter {
            meshes: vec![model.mesh],
        };
        let mesh = converter.convert();

        commands.spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(material.clone()),
            ChildOf(root),
        ));
    }
}

// For the moment, the normal way of using
// server: Res<AssetServer>
// is busted. Workarounds ahoy

fn image_from_file(path: &std::path::Path, format: ImageFormat, is_srgb: bool) -> Option<Image> {
    let file = std::fs::read(path).ok()?;

    Image::from_buffer(
        &file,
        bevy::image::ImageType::Format(format),
        CompressedImageFormats::all(),
        is_srgb,
        bevy::image::ImageSampler::linear(),
        RenderAssetUsages::all(),
    )
    .ok()
}

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    commands.spawn((
        Transform::from_xyz(0.0, 0.0, -1.0),
        PropagateReplication::default(),
        NavigatorMarker,
        AttachTo,
    ));

    // light
    commands.spawn((
        DirectionalLight {
            illuminance: 1000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(0.0, 5.0, 3.0).looking_at((0.0, 0.0, 0.0).into(), Dir3::Y),
        Replicated,
    ));

    // Hack to get around busted asset loading

    let env_map = image_from_file(
        std::path::Path::new("assets/ibl/workshop_4k_small.exr"),
        ImageFormat::OpenExr,
        false,
    )
    .expect("missing IBL image");

    let env_map = images.add(env_map);

    commands.insert_resource(EnvironmentLighting {
        intensity: 15000.0,
        equirect: env_map,
    });

    // camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(-2.5, 4.5, 9.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn main() {
    tephrite_rs::run(MyPlugin);
}
