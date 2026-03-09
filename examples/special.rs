use bevy::{mesh::SphereMeshBuilder, prelude::*};
use tephrite_rs::prelude::*;

struct MyPlugin;

impl Plugin for MyPlugin {
    fn build(&self, app: &mut App) {
        //app.insert_resource(KnownScenes::default());
        app.add_systems(Startup, setup);

        app.add_systems(Update, advect_particles);

        app.add_systems(Update, (joy_spawn_particles, spawn_from_emitter));

        app.add_plugins(NavigationPlugin::new(NavigatorMode::ObjectCentric));
    }
}

fn setup(
    mut commands: Commands,
    server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    interactors: Query<Entity, With<Interactor>>,
) {
    // light
    commands.spawn((
        DirectionalLight {
            color: Color::srgb_u8(255, 235, 198),
            shadows_enabled: true,
            illuminance: 130000.0,
            ..default()
        },
        Transform::from_xyz(4.0, 4.0, 3.0).looking_at((0.0, 0.0, 0.0).into(), Dir3::Y),
        Replicated,
    ));

    commands.insert_resource(EnvironmentLighting {
        diffuse: server.load("ibl/workshop_diffuse.ktx2"),
        specular: server.load("ibl/workshop_specular.ktx2"),
        intensity: 5000.0,
        skybox_color: None,
    });

    // Interactor light

    for interactor_ent in interactors {
        commands.entity(interactor_ent).insert(PointLight {
            color: Color::WHITE,
            intensity: 4000.0,
            ..Default::default()
        });
    }

    let root = commands
        .spawn((Transform::default(), Replicated, NavigatorMarker))
        .id();

    let iter = std::env::args();

    let Some(arg) = iter.last() else {
        error!("No data dir");
        return;
    };

    info!("Loading data from {arg}");

    // Load field (synchronous; done once at startup)
    let field = Field::load(
        FIELD_NAME,
        &(arg.clone() + MASK_PATH),
        &(arg.clone() + FLOW_PATH),
    );
    let physical_min = field.physical_min;
    let physical_max = field.physical_max;

    // Resource
    commands.insert_resource(FlowField(field));

    // Spawn boundary surface from GLTF
    commands.spawn((
        SceneRoot(server.load_override(GltfAssetLabel::Scene(0).from_asset(arg + BOUNDARY_GLTF))),
        Replicated,
        PropagateReplication::default(),
        ChildOf(root),
        Transform {
            translation: vec3(1.16667, -1.16667, -2.7), //  (0,0,-0.2) + (1.16667,-1.16667,-2.5)
            rotation: Quat::from_rotation_y(-90.0_f32.to_radians()),
            scale: Vec3::splat(1.16667 / 6.3 * 2.0), // ≈ 0.37037
        },
    ));

    // Particle mesh & material
    let particle_mesh = meshes.add(SphereMeshBuilder::new(
        0.1,
        bevy::mesh::SphereKind::Ico { subdivisions: 1 },
    ));

    let particle_material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        ..Default::default()
    });

    // Static emitter position: center of physical domain
    let emitter = (physical_min + physical_max) * 0.5;

    let mut all_particles = AllParticles {
        list: vec![],
        cursor: 0,
    };

    for _ in 0..NUM_POINTS {
        all_particles.list.push(
            commands
                .spawn((
                    Mesh3d(particle_mesh.clone()),
                    MeshMaterial3d(particle_material.clone()),
                    Transform {
                        translation: emitter,
                        scale: Vec3::splat(0.0),
                        ..Default::default()
                    },
                    Particle { age: 1.0 },
                    Replicated,
                    ChildOf(root),
                ))
                .id(),
        )
    }

    commands.insert_resource(all_particles);

    // spawn an emitter somewhere

    /*
    let emitter_mesh = meshes.add(SphereMeshBuilder::new(
        0.05,
        bevy::mesh::SphereKind::Ico { subdivisions: 1 },
    ));

    let emitter_material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        ..Default::default()
    });

    commands.spawn((
        Emitter,
        Mesh3d(emitter_mesh),
        MeshMaterial3d(emitter_material),
        Transform::from_translation(vec3(-0.8, 0.15, 2.0)),
        Replicated,
    ));
     */
}

fn main() {
    tephrite_rs::run(MyPlugin);
}

use field::{Field, inside_bounds_strict, lerp_box};
use grid::sample_vec3;

const NUM_POINTS: usize = 1000;
const PARTICLE_LIFETIME: f32 = 20.0; // seconds
const SMOKE_VELOCITY: f32 = 0.1; // arbitrary units / sec
const NUM_SPAWN: usize = 7;
const FIELD_NAME: &str = "Electrolyte Flux Ce";

// Paths: adjust to suit your data layout
const MASK_PATH: &str = "/Vectorfield_Electrolyte_Flux_Ce000000.pvtu_mask_float64_135_63_63.bin";
const FLOW_PATH: &str = "/Vectorfield_Electrolyte_Flux_Ce000000.pvtu_data_float64_135_63_63_3.bin";
const BOUNDARY_GLTF: &str = "/microsetup.glb";

#[derive(Resource)]
struct FlowField(Field);

#[derive(Component)]
struct Particle {
    age: f32, // 0..1 normalized age
}

#[derive(Resource)]
struct AllParticles {
    list: Vec<Entity>,
    cursor: usize,
}

fn advect_particles(
    time: Res<Time>,
    q_transform: Query<(&GlobalTransform), With<NavigatorMarker>>,
    mut q_particles: Query<(&mut Transform, &mut Particle)>,
    field_res: Res<FlowField>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }

    let field = &field_res.0;

    let v_min = field.v_min;
    let v_max = field.v_max;
    let physical_min = field.physical_min;
    let physical_max = field.physical_max;

    let smoke_advection_dist = SMOKE_VELOCITY * dt;
    let particle_time_delta = dt / PARTICLE_LIFETIME;

    let to_grid = q_transform
        .single()
        .map(|x| x.affine().inverse())
        .unwrap_or_default();

    q_particles
        .par_iter_mut()
        .for_each(|(mut transform, mut particle)| {
            particle.age += particle_time_delta;

            //let world_pos = to_grid.transform_point3(transform.translation);
            let world_pos = transform.translation;

            // world → volume coordinates
            let vol_pos = lerp_box(world_pos, physical_min, physical_max, v_min, v_max);

            let mut vector_sample = Vec3::ZERO;

            if inside_bounds_strict(vol_pos, v_min, v_max) {
                // sample vector field, scaled like original (*30)
                vector_sample = sample_vec3(&field.flow, vol_pos) * 30.0;
            } else {
                // out of range, kill it
                particle.age = 1.0;
            }

            let vector_mag = vector_sample.length();
            let actual_dist = smoke_advection_dist * vector_mag;

            transform.translation += vector_sample * actual_dist;

            // size & color mapping
            let size = (1.0 - particle.age).max(0.0).sqrt();
            transform.scale = Vec3::splat(0.02 + 0.08 * size);

            // info!("x {} {}", transform.scale, vector_mag);

            // If you want color mapping, you can insert a separate system that
            // writes into StandardMaterial based on vector_mag.
            // For now we keep them white; you can upgrade to a material-handle
            // per particle or a small palette.
        });
}

// TODO: having to do a global query on interactors is stupid. move to a per-interactor state

fn joy_spawn_particles(
    q_root: Query<&GlobalTransform, With<NavigatorMarker>>,
    q_emitter: Query<(&GlobalTransform, &InteractorState), With<Interactor>>,
    mut q_particle: Query<(&mut Transform, &mut Particle), Without<Interactor>>,
    mut all_particles: ResMut<AllParticles>,
) {
    let root_inv = q_root
        .single()
        .map(|x| x.affine().inverse())
        .unwrap_or_default();

    for (tf, state) in q_emitter {
        // is button down?

        if !state.buttons.pressed(JoystickButton::A) {
            continue;
        };

        // spawn where?

        let joy_world_point = tf.translation();

        let root_local = root_inv.transform_point3(joy_world_point);

        spawn_from_point(root_local, &mut all_particles, &mut q_particle);
    }
}

#[derive(Debug, Component)]
#[require(Transform)]
struct Emitter;

fn spawn_from_emitter(
    q_emitter: Query<&GlobalTransform, With<Emitter>>,
    mut q_particle: Query<(&mut Transform, &mut Particle), Without<Interactor>>,
    mut all_particles: ResMut<AllParticles>,
) {
    for tf in q_emitter {
        // spawn where?

        let world_point = tf.transform_point(Vec3::ZERO);

        spawn_from_point(world_point, &mut all_particles, &mut q_particle);
    }
}

fn spawn_from_point(
    world_point: Vec3,
    all_particles: &mut AllParticles,
    q_particle: &mut Query<(&mut Transform, &mut Particle), Without<Interactor>>,
) {
    // Get an RNG:
    let mut rng = rand::rng();

    // spawn X number of particles
    for _ in 0..NUM_SPAWN {
        all_particles.cursor = (all_particles.cursor + 1) % all_particles.list.len();

        let particle_e = all_particles.list[all_particles.cursor];

        let (mut particle_tf, mut particle_info) = q_particle.get_mut(particle_e).unwrap();

        let new_pos = Sphere::new(0.3).sample_interior(&mut rng) + world_point;

        particle_tf.translation = new_pos;
        particle_info.age = 0.0;
    }
}

mod grid {
    use std::ops::{Index, IndexMut};

    use bevy::math::{DVec3, Vec3};

    /// A 3D grid with integral indices, stored as a flat Vec<T>.
    ///
    /// Memory layout matches the C++ version:
    /// index(x, y, z) = x + size_x * (y + size_y * z)
    #[derive(Clone, Debug)]
    pub struct Grid3D<T> {
        data: Vec<T>,
        dims: [usize; 3], // [size_x, size_y, size_z]
    }

    impl<T: Clone + Default> Grid3D<T> {
        /// Construct an empty 3D grid (all dimensions 0).
        pub fn new_empty() -> Self {
            Self {
                data: Vec::new(),
                dims: [0, 0, 0],
            }
        }

        /// Construct a 3D grid with the specified dimensions, filled with T::default().
        pub fn new(x: usize, y: usize, z: usize) -> Self {
            let len = x
                .checked_mul(y)
                .and_then(|v| v.checked_mul(z))
                .expect("Grid3D dimensions overflowed usize");

            Self {
                data: vec![T::default(); len],
                dims: [x, y, z],
            }
        }

        /// Construct from `[size_x, size_y, size_z]`.
        pub fn from_dims(dims: [usize; 3]) -> Self {
            Self::new(dims[0], dims[1], dims[2])
        }

        /// Fill the entire grid with the given value.
        pub fn fill(&mut self, value: T) {
            self.data.fill(value);
        }
    }

    impl<T> Grid3D<T> {
        /// Number of elements in the grid.
        pub fn len(&self) -> usize {
            self.data.len()
        }

        pub fn is_empty(&self) -> bool {
            self.data.is_empty()
        }

        /// Dimensions as `[size_x, size_y, size_z]`.
        pub fn dims(&self) -> [usize; 3] {
            self.dims
        }

        pub fn size_x(&self) -> usize {
            self.dims[0]
        }

        pub fn size_y(&self) -> usize {
            self.dims[1]
        }

        pub fn size_z(&self) -> usize {
            self.dims[2]
        }

        /// Compute the linear index. Panics if coordinates are out of range
        /// (same spirit as C++ `operator()` using unchecked access).
        #[inline]
        pub fn index(&self, x: usize, y: usize, z: usize) -> usize {
            debug_assert!(x < self.size_x(), "x out of range");
            debug_assert!(y < self.size_y(), "y out of range");
            debug_assert!(z < self.size_z(), "z out of range");

            // Matches C++: x + size_x * (y + size_y * z)
            x + self.size_x() * (y + self.size_y() * z)
        }

        /// Safe access with bounds check, returns `Option<&T>`.
        pub fn get(&self, x: usize, y: usize, z: usize) -> Option<&T> {
            if x < self.size_x() && y < self.size_y() && z < self.size_z() {
                let idx = self.index(x, y, z);
                Some(&self.data[idx])
            } else {
                None
            }
        }

        /// Safe mutable access with bounds check, returns `Option<&mut T>`.
        pub fn get_mut(&mut self, x: usize, y: usize, z: usize) -> Option<&mut T> {
            if x < self.size_x() && y < self.size_y() && z < self.size_z() {
                let idx = self.index(x, y, z);
                Some(&mut self.data[idx])
            } else {
                None
            }
        }

        /// "at" access that returns Result, similar to C++ throwing on OOB.
        pub fn at(&self, x: usize, y: usize, z: usize) -> Result<&T, &'static str> {
            self.get(x, y, z).ok_or("Grid3D index out of range")
        }

        pub fn at_mut(&mut self, x: usize, y: usize, z: usize) -> Result<&mut T, &'static str> {
            self.get_mut(x, y, z).ok_or("Grid3D index out of range")
        }

        /// Clamp coordinates to be inside grid bounds, in-place.
        ///
        /// Matches the C++ `clamp_bounds` semantics.
        pub fn clamp_coords(&self, x: &mut usize, y: &mut usize, z: &mut usize) {
            *x = (*x).clamp(0, self.size_x() - 1);
            *y = (*y).clamp(0, self.size_y() - 1);
            *z = (*z).clamp(0, self.size_z() - 1);
        }

        /// Linear access, like C++ `operator[](size_t)`.
        pub fn get_linear(&self, idx: usize) -> Option<&T> {
            self.data.get(idx)
        }

        pub fn get_linear_mut(&mut self, idx: usize) -> Option<&mut T> {
            self.data.get_mut(idx)
        }

        /// Direct access to the underlying storage.
        pub fn as_slice(&self) -> &[T] {
            &self.data
        }

        pub fn as_mut_slice(&mut self) -> &mut [T] {
            &mut self.data
        }

        pub fn into_vec(self) -> Vec<T> {
            self.data
        }

        pub fn iter(&self) -> impl Iterator<Item = &T> {
            self.data.iter()
        }

        pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
            self.data.iter_mut()
        }
    }

    /// Indexing by linear index: grid[i]
    impl<T> Index<usize> for Grid3D<T> {
        type Output = T;

        fn index(&self, index: usize) -> &Self::Output {
            &self.data[index]
        }
    }

    impl<T> IndexMut<usize> for Grid3D<T> {
        fn index_mut(&mut self, index: usize) -> &mut Self::Output {
            &mut self.data[index]
        }
    }

    /// Indexing by (x, y, z) tuple: grid[(x, y, z)]
    impl<T> Index<(usize, usize, usize)> for Grid3D<T> {
        type Output = T;

        fn index(&self, index: (usize, usize, usize)) -> &Self::Output {
            let (x, y, z) = index;
            let idx = self.index(x, y, z);
            &self.data[idx]
        }
    }

    impl<T> IndexMut<(usize, usize, usize)> for Grid3D<T> {
        fn index_mut(&mut self, index: (usize, usize, usize)) -> &mut Self::Output {
            let (x, y, z) = index;
            let idx = self.index(x, y, z);
            &mut self.data[idx]
        }
    }

    // -----------------------------
    // Type aliases for your use case
    // -----------------------------

    /// Scalar field (single precision).
    pub type Grid3Df = Grid3D<f32>;

    /// Scalar field (double precision).
    pub type Grid3Dd = Grid3D<f64>;

    /// 3-component vector field, packed as 3x f32 using Bevy's Vec3.
    pub type Grid3DVec3 = Grid3D<Vec3>;

    /// Trilinear sample of a scalar f32 field at position `p` in grid coordinates.
    /// Assumes:
    /// - 0 <= p.x <= (size_x-1), etc
    /// - and that x1 = floor(p.x) + 1 < size_x, etc.
    pub fn sample_f32(grid: &Grid3Df, p: Vec3) -> f32 {
        // floor per component
        let fl = p.floor();

        let x0 = fl.x as usize;
        let y0 = fl.y as usize;
        let z0 = fl.z as usize;

        let x1 = x0 + 1;
        let y1 = y0 + 1;
        let z1 = z0 + 1;

        let xd = p.x - x0 as f32;
        let yd = p.y - y0 as f32;
        let zd = p.z - z0 as f32;

        // c00, c10, c01, c11 like in C++ code
        let c00 = grid[(x0, y0, z0)] * (1.0 - xd) + grid[(x1, y0, z0)] * xd;
        let c10 = grid[(x0, y1, z0)] * (1.0 - xd) + grid[(x1, y1, z0)] * xd;
        let c01 = grid[(x0, y0, z1)] * (1.0 - xd) + grid[(x1, y0, z1)] * xd;
        let c11 = grid[(x0, y1, z1)] * (1.0 - xd) + grid[(x1, y1, z1)] * xd;

        let c0 = c00 * (1.0 - yd) + c10 * yd;
        let c1 = c01 * (1.0 - yd) + c11 * yd;

        c0 * (1.0 - zd) + c1 * zd
    }

    /// Trilinear sample of a scalar f64 field at position `p` in grid coordinates.
    pub fn sample_f64(grid: &Grid3Dd, p: DVec3) -> f64 {
        let fl = p.floor();

        let x0 = fl.x as usize;
        let y0 = fl.y as usize;
        let z0 = fl.z as usize;

        let x1 = x0 + 1;
        let y1 = y0 + 1;
        let z1 = z0 + 1;

        let xd = p.x - x0 as f64;
        let yd = p.y - y0 as f64;
        let zd = p.z - z0 as f64;

        let c00 = grid[(x0, y0, z0)] * (1.0 - xd) + grid[(x1, y0, z0)] * xd;
        let c10 = grid[(x0, y1, z0)] * (1.0 - xd) + grid[(x1, y1, z0)] * xd;
        let c01 = grid[(x0, y0, z1)] * (1.0 - xd) + grid[(x1, y0, z1)] * xd;
        let c11 = grid[(x0, y1, z1)] * (1.0 - xd) + grid[(x1, y1, z1)] * xd;

        let c0 = c00 * (1.0 - yd) + c10 * yd;
        let c1 = c01 * (1.0 - yd) + c11 * yd;

        c0 * (1.0 - zd) + c1 * zd
    }

    /// Trilinear sample of a Vec3 vector field (packed 3xf32) at position `p`.
    pub fn sample_vec3(grid: &Grid3DVec3, p: Vec3) -> Vec3 {
        let fl = p.floor();

        let x0 = fl.x as usize;
        let y0 = fl.y as usize;
        let z0 = fl.z as usize;

        let x1 = x0 + 1;
        let y1 = y0 + 1;
        let z1 = z0 + 1;

        let xd = p.x - x0 as f32;
        let yd = p.y - y0 as f32;
        let zd = p.z - z0 as f32;

        // Bevy Vec3 supports `* f32` and `+ Vec3` so this is identical structurally.
        let c00 = grid[(x0, y0, z0)] * (1.0 - xd) + grid[(x1, y0, z0)] * xd;
        let c10 = grid[(x0, y1, z0)] * (1.0 - xd) + grid[(x1, y1, z0)] * xd;
        let c01 = grid[(x0, y0, z1)] * (1.0 - xd) + grid[(x1, y0, z1)] * xd;
        let c11 = grid[(x0, y1, z1)] * (1.0 - xd) + grid[(x1, y1, z1)] * xd;

        let c0 = c00 * (1.0 - yd) + c10 * yd;
        let c1 = c01 * (1.0 - yd) + c11 * yd;

        c0 * (1.0 - zd) + c1 * zd
    }
}

mod field {
    use bevy::log::info;
    // src/field.rs
    use bevy::math::Vec3;
    use std::fs::File;
    use std::io::{BufReader, Read};

    use super::grid::{Grid3D, Grid3DVec3, Grid3Df};

    const NX: usize = 63;
    const NY: usize = 63;
    const NZ: usize = 135;
    const GRID_SIZE: usize = NX * NY * NZ;

    pub struct Field {
        pub name: String,
        pub mask: Grid3Df,
        pub flow: Grid3DVec3,
        pub v_min: Vec3,
        pub v_max: Vec3,
        pub physical_min: Vec3,
        pub physical_max: Vec3,
    }

    fn swap_pos_vec3(grid: &mut Grid3D<Vec3>) {
        let sx = grid.size_x();
        let sy = grid.size_y();
        let sz = grid.size_z();

        for x in 0..(sx / 2) {
            let ix = sx - x - 1;
            for y in 0..sy {
                for z in 0..sz {
                    let a = grid.index(x, y, z);
                    let b = grid.index(ix, y, z);
                    grid.as_mut_slice().swap(a, b);
                }
            }
        }
    }

    fn swap_pos_f32(grid: &mut Grid3Df) {
        let sx = grid.size_x();
        let sy = grid.size_y();
        let sz = grid.size_z();

        for x in 0..(sx / 2) {
            let ix = sx - x - 1;
            for y in 0..sy {
                for z in 0..sz {
                    let a = grid.index(x, y, z);
                    let b = grid.index(ix, y, z);
                    grid.as_mut_slice().swap(a, b);
                }
            }
        }
    }

    /// Load scalar mask (float64) → Grid3Df (f32), including x-swap.
    fn load_mask(path: &str) -> Grid3Df {
        let buf = std::fs::read(path).expect("Failed to open mask file");

        assert_eq!(buf.len(), GRID_SIZE * 8);

        let mut grid = Grid3Df::new(NX, NY, NZ);
        let data = grid.as_mut_slice();

        for i in 0..GRID_SIZE {
            let bytes = &buf[i * 8..(i + 1) * 8];
            let val_f64 = f64::from_le_bytes(bytes.try_into().unwrap());
            data[i] = val_f64 as f32;
        }

        swap_pos_f32(&mut grid);
        grid
    }

    /// Load vec field (float64 x3) → Grid3DVec3, normalized by max |v| inside mask>=0.5, including x-swap.
    fn load_flow(path: &str, mask: &Grid3Df) -> Grid3DVec3 {
        let buf = std::fs::read(path).expect("Failed to open vector field file");

        assert_eq!(buf.len(), GRID_SIZE * 3 * 8);

        // First pass: read into temp double vec3 and find max velocity where mask >= 0.5
        let mut temp: Vec<[f64; 3]> = vec![[0.0; 3]; GRID_SIZE];

        let mut max_vel = 0.0f64;
        for i in 0..GRID_SIZE {
            let base = i * 3 * 8;
            let fx = f64::from_le_bytes(buf[base + 0 * 8..base + 1 * 8].try_into().unwrap());
            let fy = f64::from_le_bytes(buf[base + 1 * 8..base + 2 * 8].try_into().unwrap());
            let fz = f64::from_le_bytes(buf[base + 2 * 8..base + 3 * 8].try_into().unwrap());

            // C++ ordering:
            // grid[grid_i].x = f.at(i + 1);
            // grid[grid_i].y = f.at(i + 2);
            // grid[grid_i].z = f.at(i + 0);
            let vx = fy;
            let vy = fz;
            let vz = fx;

            temp[i] = [vx, vy, vz];

            if mask[i] >= 0.5 {
                let len = (vx * vx + vy * vy + vz * vz).sqrt();
                if len > max_vel {
                    max_vel = len;
                }
            }
        }

        let max_vel = if max_vel == 0.0 { 1.0 } else { max_vel };

        let mut grid = Grid3DVec3::new(NX, NY, NZ);
        let data = grid.as_mut_slice();

        for i in 0..GRID_SIZE {
            let [vx, vy, vz] = temp[i];
            let vx = (vx / max_vel) as f32;
            let vy = (vy / max_vel) as f32;
            let vz = (vz / max_vel) as f32;
            data[i] = Vec3::new(vx, vy, vz);
        }

        swap_pos_vec3(&mut grid);
        grid
    }

    impl Field {
        pub fn load(name: &str, mask_path: &str, flow_path: &str) -> Self {
            info!("Load field {name} mask {mask_path} flow {flow_path}");
            let v_min = Vec3::ZERO;
            let v_max = Vec3::new((NX - 1) as f32, (NY - 1) as f32, (NZ - 1) as f32);

            // physical_size = (63,63,135) / largest_axis * 5, centered
            let mut physical_size = Vec3::new(NX as f32, NY as f32, NZ as f32);
            let largest_axis = physical_size.max_element();
            physical_size /= largest_axis;
            physical_size *= 5.0;

            let physical_min = -physical_size / 2.0;
            let physical_max = physical_size / 2.0;

            let mask = load_mask(mask_path);
            let flow = load_flow(flow_path, &mask);

            info!("Mask and flow loaded");

            Field {
                name: name.to_string(),
                mask,
                flow,
                v_min,
                v_max,
                physical_min,
                physical_max,
            }
        }
    }

    // Map p from [src_min, src_max] -> [dst_min, dst_max], per-component.
    pub fn lerp_box(p: Vec3, src_min: Vec3, src_max: Vec3, dst_min: Vec3, dst_max: Vec3) -> Vec3 {
        let t = (p - src_min) / (src_max - src_min);
        dst_min + t * (dst_max - dst_min)
    }

    pub fn inside_bounds_strict(p: Vec3, min: Vec3, max: Vec3) -> bool {
        p.x > min.x && p.y > min.y && p.z > min.z && p.x < max.x && p.y < max.y && p.z < max.z
    }
}
