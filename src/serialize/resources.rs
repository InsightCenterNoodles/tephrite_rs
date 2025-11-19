use crate::{common::EnvironmentLighting, serialize::*};

impl_fast_serialize!(
    EnvironmentLighting,
    keep: {
        intensity, equirect
    },
    skip: {

    }
);
