mod off_axis_projection;
pub(crate) mod projection;

use bevy::prelude::*;

use crate::common::{Head, OffAxisProjectionSettings};

pub(crate) use projection::OffAxisProjection;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum TephriteRenderSystems {
    LateLatchHead,
    UpdateCamera,
}

// transfer a head position to an off axis projection
fn update_off_axis_projection(
    head_q: Query<(&Transform, &Head), Without<Projection>>,
    mut proj_q: Query<(&mut Transform, &mut Projection), Without<Head>>,
    settings: Option<Res<OffAxisProjectionSettings>>,
) {
    let Some((head_tf, _)) = head_q.iter().next() else {
        return;
    };

    let settings = settings.as_deref().copied().unwrap_or_default();

    for (mut camera_xform, mut projection) in &mut proj_q {
        let Projection::Custom(custom) = &mut *projection else {
            continue;
        };

        let Some(custom) = custom.get_mut::<projection::OffAxisProjection>() else {
            continue;
        };

        custom.set_clip_distances(settings.near, settings.far);
        let tf = custom.update_proj(head_tf.translation, head_tf.rotation);

        *camera_xform = tf;
    }
}

pub(crate) struct OffAxisPlugin;

impl Plugin for OffAxisPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            update_off_axis_projection
                .in_set(TephriteRenderSystems::UpdateCamera)
                .after(TephriteRenderSystems::LateLatchHead),
        );
    }
}
