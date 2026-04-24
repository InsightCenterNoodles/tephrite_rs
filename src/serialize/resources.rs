use crate::{
    common::{EnvironmentLighting, OrderIndependantTransparency},
    serialize::*,
};

impl_fast_serialize!(
    EnvironmentLighting,
    keep: {
        intensity, diffuse, specular, skybox_color
    },
    skip: {

    }
);

impl_fast_raw_item!(OrderIndependantTransparency);
