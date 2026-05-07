pub mod points;

use bevy::{asset::embedded_asset, prelude::*};

pub use points::PointsMaterial;

pub(crate) fn builtin_materials_plugin(app: &mut App) {
    embedded_asset!(app, "points/points.wgsl");
    app.add_plugins(MaterialPlugin::<points::PointsMaterial>::default());
}
