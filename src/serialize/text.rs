use crate::serialize::*;
use bevy::{platform::collections::HashMap, prelude::*, text::FontSmoothing};
use std::sync::{LazyLock, RwLock};

impl_fast_newtype!(TextColor);

static P_MAP: LazyLock<RwLock<HashMap<AssetId<Font>, Handle<Font>>>> =
    LazyLock::new(|| Default::default());

impl RemappableAsset for Font {
    #[inline]
    fn with_remapper<F: FnOnce(&HashMap<AssetId<Self>, Handle<Self>>)>(func: F) {
        func(&P_MAP.read().unwrap());
    }
    #[inline]
    fn with_remapper_mut<F: FnOnce(&mut HashMap<AssetId<Self>, Handle<Self>>)>(func: F) {
        func(&mut P_MAP.write().unwrap());
    }

    fn remap_to_local_or_reserve(id: AssetId<Self>) -> Handle<Self>
    where
        Self: bevy::prelude::Asset,
        Self: Sized,
    {
        if id == AssetId::default() {
            dbg!("DEFAULT ASSET FONT");
            return Handle::<Font>::default();
        }

        if let Some(handle) = Self::remap_to_local(id) {
            return handle;
        }

        let local = Handle::Uuid(bevy::asset::uuid::Uuid::new_v4(), std::marker::PhantomData);

        warn!(
            "Missing asset mapping for {} id {id}; reserving client-local placeholder {}",
            std::any::type_name::<Self>(),
            local.id()
        );

        Self::with_remapper_mut(|map| {
            map.insert(id, local.clone());
        });

        local
    }
}

impl_fast_newtype!(FontWeight);
impl_fast_raw_item!(FontSmoothing);

impl_fast_serialize!(
    TextFont,
    keep: {
        font,
        font_size,
        weight,
        font_smoothing
    }, skip: {
        font_features // does not allow us to inspect it yet
    }
);

impl_fast_raw_item!(Justify);
impl_fast_raw_item!(LineBreak);

impl_fast_serialize!(
    TextLayout,
    keep: {
        justify,
        linebreak
    }, skip: {
    }
);

impl_fast_newtype!(TextSpan);

impl FastWrite for Font {
    unsafe fn write_fast(&self, w: &mut impl ByteSink) {
        unsafe { self.data.write_fast(w) };
    }
}

impl FastRead for Font {
    type Ret = Self;

    unsafe fn read_fast<'a, S: ByteSource<'a>>(r: &mut S) -> Self::Ret {
        Self {
            data: std::sync::Arc::new(unsafe { Vec::<u8>::read_fast(r) }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_font_id_remaps_to_default_font_handle() {
        let handle = Font::remap_to_local_or_reserve(AssetId::default());

        assert_eq!(handle.id(), Handle::<Font>::default().id());
    }
}
