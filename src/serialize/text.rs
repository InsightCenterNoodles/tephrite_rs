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

    fn remap_to_local_or_reserve(id: AssetId<Self>, assets: &mut Assets<Self>) -> Handle<Self>
    where
        Self: bevy::prelude::Asset,
        Self: Sized,
    {
        if id == AssetId::default() {
            // dbg!("DEFAULT ASSET FONT");
            return Handle::<Font>::default();
        }

        if let Some(handle) = Self::remap_to_local(id) {
            return handle;
        }

        let local = assets.reserve_handle();

        // warn!(
        //     "Missing asset mapping for {} id {id}; reserving client-local placeholder {}",
        //     std::any::type_name::<Self>(),
        //     local.id()
        // );

        Self::with_remapper_mut(|map| {
            map.insert(id, local.clone());
        });

        local
    }
}

impl_fast_newtype!(FontWeight);
impl_fast_raw_item!(FontSmoothing);

impl crate::serialize::fast_ser::FastWrite for TextFont {
    #[inline(always)]
    #[allow(unused)]
    unsafe fn write_fast(&self, w: &mut impl crate::serialize::fast_io::ByteSink) {
        unsafe { self.font.write_fast(w) };
        unsafe { self.font_size.write_fast(w) };
        unsafe { self.weight.write_fast(w) };
        unsafe { self.font_smoothing.write_fast(w) };
    }
}
impl crate::serialize::fast_ser::FastRead for TextFont {
    type Ret = TextFont;
    type Context = Assets<Font>;

    unsafe fn read_fast<'z, S: crate::serialize::fast_io::ByteSource<'z>>(
        c: &mut Self::Context,
        r: &mut S,
    ) -> Self {
        #[allow(unused)]
        use crate::serialize::fast_ser::read_fast;
        let nc = &mut ();
        Self {
            font: read_fast(c, r),
            font_size: read_fast(nc, r),
            weight: read_fast(nc, r),
            font_smoothing: read_fast(nc, r),
            font_features: Default::default(),
        }
    }
}

impl_fast_raw_item!(Justify);
impl_fast_raw_item!(LineBreak);

impl_fast_serialize!(
    TextLayout,
    (),
    keep: {
        justify,
        linebreak
    }, skip: {
    }
);

impl_fast_newtype!(TextSpan);

impl FastWrite for Font {
    unsafe fn write_fast(&self, w: &mut impl ByteSink) {
        unsafe {
            self.data.write_fast(w);
        };
    }
}

impl FastRead for Font {
    type Ret = Self;
    type Context = ();

    unsafe fn read_fast<'a, S: ByteSource<'a>>(c: &mut Self::Context, r: &mut S) -> Self::Ret {
        Font::try_from_bytes(unsafe { Vec::<u8>::read_fast(c, r) }).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_font_id_remaps_to_default_font_handle() {
        let mut assets = Assets::<Font>::default();
        let handle = Font::remap_to_local_or_reserve(AssetId::default(), &mut assets);

        assert_eq!(handle.id(), Handle::<Font>::default().id());
    }
}
