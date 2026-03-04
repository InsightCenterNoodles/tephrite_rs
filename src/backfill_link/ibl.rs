use bevy::prelude::*;

use crate::{
    backfill,
    backfill_link::{assets::AssetCache, resources::Session, sets::RenderableSet},
    common::EnvironmentLighting,
};

pub(crate) struct EnvironmentLightPlugin;

impl Plugin for EnvironmentLightPlugin {
    fn build(&self, app: &mut App) {
        app.init_non_send_resource::<IBLResource>();
        app.add_systems(
            PostUpdate,
            (watch_for_ibl_updates, watch_for_ibl_remove)
                .chain()
                .in_set(RenderableSet::Refresh),
        );
    }
}

fn watch_for_ibl_updates(
    query: Option<Res<EnvironmentLighting>>,
    cache: NonSend<AssetCache>,
    session: NonSend<Session>,
    ftex_assets: Res<Assets<Image>>,
    mut ibl: NonSendMut<IBLResource>,
) {
    let Some(query) = query else {
        return;
    };

    if !query.is_changed() {
        return;
    }

    if !ftex_assets.contains(&query.equirect) {
        error!("Environment map is not available");
        return;
    }

    let Some(asset) = cache.textures.get(&query.equirect.id()) else {
        error!("Environment map is not mapped");
        return;
    };

    let handle = backfill::env_light_from_equirect(&session.0, &asset.0).ok();

    if let Some(h) = &handle {
        backfill::set_environment_light(&session.0, h);
        h.set_intensity(query.intensity);
    }

    ibl.fenv = handle;
}

fn watch_for_ibl_remove(
    query: Option<Res<EnvironmentLighting>>,
    mut res_existed: Local<bool>,
    mut ibl: NonSendMut<IBLResource>,
) {
    if query.is_some() {
        // the resource exists
        *res_existed = true;
    } else if *res_existed {
        // the resource was removed

        *res_existed = false;

        // clear out IBL map
        ibl.fenv = None;
    }
}

#[derive(Default)]
pub struct IBLResource {
    pub(crate) fenv: Option<backfill::FEnvironmentLightHandle>,
}
