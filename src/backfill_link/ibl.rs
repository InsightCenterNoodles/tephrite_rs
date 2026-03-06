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

    // Retry until env-light creation succeeds; resource changes may happen before
    // replicated/streamed image assets are fully ready.
    let should_refresh = query.is_changed() || ibl.fenv.is_none();
    if !should_refresh {
        return;
    }

    debug!(
        "IBL refresh attempt: equirect_id={:?}, intensity={}, resource_changed={}, has_existing_env={}",
        query.equirect.id(),
        query.intensity,
        query.is_changed(),
        ibl.fenv.is_some(),
    );

    if !ftex_assets.contains(&query.equirect) {
        debug!(
            "IBL blocked: image asset not present yet for equirect_id={:?}",
            query.equirect.id()
        );
        return;
    }

    if let Some(img) = ftex_assets.get(&query.equirect) {
        debug!(
            "IBL source image ready: id={:?}, size={}x{}, format={:?}, bytes={}",
            query.equirect.id(),
            img.width(),
            img.height(),
            img.texture_descriptor.format,
            img.data.as_ref().map(|x| x.len()).unwrap_or(0)
        );
    }

    let Some(asset) = cache.textures.get(&query.equirect.id()) else {
        debug!(
            "IBL blocked: backfill texture not cached yet for equirect_id={:?}",
            query.equirect.id()
        );
        return;
    };

    debug!(
        "IBL init call: equirect_id={:?}, backfill_tex_ptr={:p}",
        query.equirect.id(),
        asset.0.as_ptr()
    );

    let handle = backfill::env_light_from_equirect(&session.0, &asset.0).ok();

    if let Some(h) = &handle {
        info!(
            "IBL init success: equirect_id={:?}, env_light_ptr={:p}",
            query.equirect.id(),
            h.as_ptr()
        );
        backfill::set_environment_light(&session.0, h);
        h.set_intensity(query.intensity);
    } else {
        warn!(
            "IBL init failed (null env handle): equirect_id={:?}, backfill_tex_ptr={:p}",
            query.equirect.id(),
            asset.0.as_ptr()
        );
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
