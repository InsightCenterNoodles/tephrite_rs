use bevy::{
    image::{ImageFilterMode, ImageSamplerDescriptor},
    prelude::*,
};

use crate::ui::{
    rounded_rect::{RoundedRectOptions, rounded_rect_mesh},
    text_bake::{CpuTextBaker, TextStyle},
};

#[derive(Debug, Bundle)]
pub struct Label {
    pub mesh: Mesh3d,
    pub material: MeshMaterial3d<StandardMaterial>,
}

pub fn make_label(
    baker: &mut CpuTextBaker,
    text: &str,
    style: TextStyle,
    images: &mut Assets<Image>,
    meshes: &mut Assets<Mesh>,
    mats: &mut Assets<StandardMaterial>,
) -> Result<Label> {
    let mut image = baker.bake_rgba8(text, style)?;

    image.sampler = bevy::image::ImageSampler::Descriptor(ImageSamplerDescriptor {
        mag_filter: ImageFilterMode::Nearest,
        min_filter: ImageFilterMode::Nearest,
        mipmap_filter: ImageFilterMode::Nearest,
        ..Default::default()
    });

    let aspect = image.aspect_ratio(); // w / h

    let image = images.add(image);

    let material = mats.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(image),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..Default::default()
    });

    let height = aspect.inverse().ratio();

    let mesh = rounded_rect_mesh(
        1.0,
        height,
        RoundedRectOptions {
            radius: height / 4.0,
            //double_sided: true,
            ..Default::default()
        },
    )?;

    let mesh = meshes.add(mesh);

    Ok(Label {
        mesh: bevy::prelude::Mesh3d(mesh),
        material: bevy::prelude::MeshMaterial3d(material),
    })
}
