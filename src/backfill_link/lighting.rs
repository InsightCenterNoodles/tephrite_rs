use std::ptr::NonNull;

use bevy::prelude::*;

use super::components::*;
use super::resources::*;

use crate::backfill;
use crate::backfill::ffi;
use crate::backfill::ffi::float3;

pub(crate) struct LightBindingPlugin;

impl Plugin for LightBindingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, check_add);
    }
}

fn install_light(l: &DirectionalLight, b: &BEntity, session: &Session) {
    let handle = unsafe {
        let ptr = ffi::flightconfig_init(ffi::FLightType_DIRECTIONAL);

        ffi::flc_set_color(ptr, l.color.into());
        ffi::flc_set_falloff(ptr, 10000.0);

        ffi::flc_set_direction(
            ptr,
            float3 {
                x: 0.0,
                y: 0.0,
                z: -1.0,
            },
        );

        ffi::flc_set_shadows(ptr, l.shadows_enabled.into());

        ffi::flc_set_intensity(ptr, l.illuminance);

        backfill::FLightConfigHandle::from_nonnull(NonNull::new(ptr).unwrap())
    };

    debug!("Adding light to {:?}", b.0);

    unsafe { ffi::fs_add_light(session.0.as_ptr(), b.0.into(), handle.as_ptr()) };
}

fn check_add(
    query: Query<(&DirectionalLight, &BEntity), Or<(Changed<DirectionalLight>, Changed<BEntity>)>>,
    session: NonSend<Session>,
) {
    for (l, b) in query {
        install_light(l, b, &session);
    }
}
