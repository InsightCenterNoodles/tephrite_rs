use bevy::{
    asset::RenderAssetUsages,
    image::ImageSampler,
    mesh::{Indices, PrimitiveTopology},
    pbr::StandardMaterial,
    prelude::{AlphaMode, Color, Image, Mesh},
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use tephrite_rs::serialize::{ByteWriter, FastWrite};

const BUFFER_SIZE: usize = 64 * 1024 * 1024;

fn write_asset<T: FastWrite>(asset: &T, buffer: &mut [u8]) -> usize {
    let mut writer = ByteWriter::new(buffer);
    unsafe { asset.write_fast(&mut writer) };
    writer.position()
}

fn bench_fast_write<T: FastWrite>(c: &mut Criterion, name: &str, asset: T) {
    c.bench_function(name, |b| {
        b.iter_batched_ref(
            || vec![0u8; BUFFER_SIZE],
            |buffer| black_box(write_asset(&asset, buffer)),
            BatchSize::SmallInput,
        )
    });
}

fn make_mesh() -> Mesh {
    let width = 256usize;
    let height = 256usize;
    let vertex_count = width * height;

    let mut positions = Vec::with_capacity(vertex_count);
    let mut normals = Vec::with_capacity(vertex_count);
    let mut uvs = Vec::with_capacity(vertex_count);
    let mut colors = Vec::with_capacity(vertex_count);

    for y in 0..height {
        for x in 0..width {
            let xf = x as f32 / (width - 1) as f32;
            let yf = y as f32 / (height - 1) as f32;
            let z = (xf * std::f32::consts::TAU).sin() * (yf * std::f32::consts::TAU).cos();

            positions.push([xf * 32.0, yf * 32.0, z]);
            normals.push([0.0, 0.0, 1.0]);
            uvs.push([xf, yf]);
            colors.push([xf, yf, 1.0 - xf, 1.0]);
        }
    }

    let mut indices = Vec::with_capacity((width - 1) * (height - 1) * 6);
    for y in 0..height - 1 {
        for x in 0..width - 1 {
            let i = (y * width + x) as u32;
            indices.extend_from_slice(&[
                i,
                i + 1,
                i + width as u32,
                i + 1,
                i + width as u32 + 1,
                i + width as u32,
            ]);
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

fn make_image() -> Image {
    let width = 2048u32;
    let height = 2048u32;
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);

    for y in 0..height {
        for x in 0..width {
            pixels.push((x & 0xff) as u8);
            pixels.push((y & 0xff) as u8);
            pixels.push(((x ^ y) & 0xff) as u8);
            pixels.push(255);
        }
    }

    let mut image = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::Default;
    image
}

fn make_standard_material() -> StandardMaterial {
    StandardMaterial {
        base_color: Color::srgba(0.2, 0.45, 0.8, 0.9),
        emissive: Color::srgb(0.01, 0.02, 0.03).into(),
        perceptual_roughness: 0.72,
        metallic: 0.35,
        reflectance: 0.6,
        diffuse_transmission: 0.15,
        specular_transmission: 0.08,
        thickness: 0.4,
        ior: 1.45,
        attenuation_color: Color::srgb(0.75, 0.85, 1.0),
        attenuation_distance: 18.0,
        clearcoat: 0.25,
        clearcoat_perceptual_roughness: 0.55,
        anisotropy_strength: 0.5,
        anisotropy_rotation: 0.2,
        flip_normal_map_y: true,
        double_sided: true,
        unlit: false,
        fog_enabled: true,
        alpha_mode: AlphaMode::Blend,
        depth_bias: 3.0,
        parallax_depth_scale: 0.08,
        max_parallax_layer_count: 24.0,
        lightmap_exposure: 1.2,
        ..Default::default()
    }
}

fn asset_serialization(c: &mut Criterion) {
    bench_fast_write(c, "serialize_mesh", make_mesh());
    bench_fast_write(c, "serialize_image", make_image());
    bench_fast_write(c, "serialize_standard_material", make_standard_material());
}

criterion_group!(benches, asset_serialization);
criterion_main!(benches);
