use std::f32;

use bevy::asset::RenderAssetUsages;
use bevy::mesh::Indices;
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;

pub struct RoundedRectOptions {
    pub radius: f32,
    pub corner_segments: usize,
    pub double_sided: bool,
}

impl RoundedRectOptions {}

impl Default for RoundedRectOptions {
    fn default() -> Self {
        Self {
            radius: 0.05,
            corner_segments: 6,
            double_sided: false,
        }
    }
}

/// Create a rounded rectangle mesh on the XY plane at z = 0.
///
/// Conventions:
/// - physical size = `width` x `height`
/// - up = +Y
/// - front face = +Z
/// - UVs span the full rectangle [0,1]x[0,1]
///
/// `corner_segments` controls arc tessellation per corner.
/// If `radius <= 0.0`, this returns a plain quad.
/// If `radius` is too large, it is clamped to `min(width, height) * 0.5`.
pub fn rounded_rect_mesh(width: f32, height: f32, options: RoundedRectOptions) -> Result<Mesh> {
    if width <= 0.0 || height <= 0.0 {
        return Err("width and height must be > 0".into());
    }

    let RoundedRectOptions {
        radius,
        corner_segments,
        double_sided,
    } = options;

    let half_w = width * 0.5;
    let half_h = height * 0.5;

    let radius = radius.clamp(0.0, half_w.min(half_h));

    let mut mesh = if radius <= 0.0 {
        quad_mesh(width, height)
    } else {
        make_rounded(width, height, corner_segments, half_w, half_h, radius)
    };

    if double_sided {
        let indexes = mesh.indices_mut().unwrap();

        match indexes {
            Indices::U16(items) => {
                let mut back = items.clone();
                for tri in back.chunks_exact_mut(3) {
                    tri.swap(1, 2);
                }
                items.extend(back);
            }
            Indices::U32(items) => {
                let mut back = items.clone();
                for tri in back.chunks_exact_mut(3) {
                    tri.swap(1, 2);
                }
                items.extend(back);
            }
        }
    }

    Ok(mesh)
}

fn make_rounded(
    width: f32,
    height: f32,
    corner_segments: usize,
    half_w: f32,
    half_h: f32,
    radius: f32,
) -> Mesh {
    let corner_segments = corner_segments.max(1);

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    // Center vertex for triangle fan.
    positions.push([0.0, 0.0, 0.0]);
    normals.push([0.0, 0.0, 1.0]);
    uvs.push([0.5, 0.5]);

    // Build perimeter in CLOCKWISE order as seen from +Z,
    // so the resulting triangles face -Z.
    let mut perimeter: Vec<Vec2> = Vec::new();

    let tr = Vec2::new(half_w - radius, half_h - radius);
    let br = Vec2::new(half_w - radius, -half_h + radius);
    let bl = Vec2::new(-half_w + radius, -half_h + radius);
    let tl = Vec2::new(-half_w + radius, half_h - radius);

    // Helper: append an arc, inclusive of end only when requested.
    let mut push_arc = |center: Vec2, start: f32, end: f32, include_start: bool| {
        for i in 0..=corner_segments {
            if i == 0 && !include_start {
                continue;
            }
            let t = i as f32 / corner_segments as f32;
            let a = start + (end - start) * t;
            let p = center + Vec2::new(a.cos() * radius, a.sin() * radius);
            perimeter.push(p);
        }
    };

    // Clockwise perimeter:
    // top-right:   90°  ->   0°
    // bottom-right: 0°  -> -90°
    // bottom-left: -90° -> -180°
    // top-left:   180°  ->  90°
    push_arc(tr, std::f32::consts::FRAC_PI_2, 0.0, true);
    push_arc(br, 0.0, -std::f32::consts::FRAC_PI_2, false);
    push_arc(
        bl,
        -std::f32::consts::FRAC_PI_2,
        -std::f32::consts::PI,
        false,
    );
    push_arc(tl, std::f32::consts::PI, std::f32::consts::FRAC_PI_2, false);

    // Add perimeter vertices.
    for p in &perimeter {
        positions.push([p.x, p.y, 0.0]);
        normals.push([0.0, 0.0, 1.0]);

        // Map XY directly into full-rect UV space.
        let u = (p.x + half_w) / width;
        let v = 1.0 - ((p.y + half_h) / height); // top-left-style UV orientation
        uvs.push([u, v]);
    }

    // Triangle fan from center.
    // Center is vertex 0, perimeter starts at 1.
    let ring_start = 1u32;
    let ring_len = perimeter.len() as u32;

    for i in 0..ring_len {
        let a = ring_start + i;
        let b = ring_start + ((i + 1) % ring_len);
        indices.extend_from_slice(&[0, b, a]);
    }

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
}

/// Fast path plain quad on the XY plane
fn quad_mesh(width: f32, height: f32) -> Mesh {
    assert!(width > 0.0, "width must be > 0");
    assert!(height > 0.0, "height must be > 0");

    let hw = width * 0.5;
    let hh = height * 0.5;

    let positions = vec![
        [-hw, -hh, 0.0], // 0 bottom-left
        [-hw, hh, 0.0],  // 1 top-left
        [hw, hh, 0.0],   // 2 top-right
        [hw, -hh, 0.0],  // 3 bottom-right
    ];

    let normals = vec![
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
    ];

    let uvs = vec![[0.0, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0]];

    let indices = vec![0, 2, 1, 0, 3, 2];

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
}
